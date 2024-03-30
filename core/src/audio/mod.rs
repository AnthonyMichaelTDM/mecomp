pub mod queue;

use std::{
    cell::{RefCell, RefMut},
    fs::File,
    io::BufReader,
    sync::{
        mpsc::{Receiver, Sender},
        Arc,
    },
};

use lazy_static::lazy_static;
use log::error;
use rodio::{Decoder, Source};
use tracing::instrument;

use crate::errors::LibraryError;
use mecomp_storage::db::schemas::song::Song;

use self::queue::Queue;

lazy_static! {
    pub static ref AUDIO_KERNEL: Arc<AudioKernelSender> = {
        let (tx, rx) = std::sync::mpsc::channel();
        tokio::spawn(async move {
            let kernel = AudioKernel::new();
            kernel.spawn(rx);
        });
        Arc::new(AudioKernelSender { tx })
    };
}

#[derive(Debug, Clone, PartialEq)]
pub enum AudioCommand {
    Play,
    Pause,
    TogglePlayback,
    /// only clear the player (i.e. stop playback)
    ClearPlayer,
    Clear,
    Skip(usize),
    Previous(Option<usize>),
    // PlaySource(Box<dyn Source<Item = f32> + Send>),
    AddSongToQueue(Song),
    Exit,
}

pub struct AudioKernelSender {
    tx: Sender<AudioCommand>,
}

impl AudioKernelSender {
    #[instrument(skip(self))]
    pub fn send(&self, command: AudioCommand) {
        self.tx.send(command).unwrap();
    }
}

pub struct AudioKernel {
    /// this is not used, but is needed to keep the stream alive
    _stream: rodio::OutputStream,
    /// this is not used, but is needed to keep the stream alive
    _stream_handle: rodio::OutputStreamHandle,
    player: rodio::Sink,
    queue: RefCell<Queue>,
}

impl AudioKernel {
    /// this function initializes the audio kernel
    /// it is not meant to be called directly, use `AUDIO_KERNEL` instead to send commands
    pub(self) fn new() -> Self {
        let (_stream, stream_handle) = rodio::OutputStream::try_default().unwrap();

        let player = rodio::Sink::try_new(&stream_handle).unwrap();
        let queue = Queue::new();

        Self {
            _stream,
            _stream_handle: stream_handle,
            player,
            queue: RefCell::new(queue),
        }
    }

    /// Spawn the audio kernel, taking ownership of self
    ///
    /// this function should be called in a detached thread to keep the audio kernel running,
    /// this function will block until the `Exit` command is received
    pub fn spawn(self, rx: Receiver<AudioCommand>) {
        loop {
            let command = rx.recv().unwrap();
            match command {
                AudioCommand::Play => self.play(),
                AudioCommand::Pause => self.pause(),
                AudioCommand::TogglePlayback => self.toggle_playback(),
                AudioCommand::ClearPlayer => self.clear_player(),
                AudioCommand::Clear => self.clear(),
                AudioCommand::Skip(n) => self.skip(n),
                AudioCommand::Previous(_threshold) => todo!(),
                //AudioCommand::PlaySource(source) => self.append_to_player(source),
                AudioCommand::AddSongToQueue(song) => self.add_song_to_queue(song),
                AudioCommand::Exit => break,
            }
        }
    }

    fn play(&self) {
        self.player.play();
    }

    fn pause(&self) {
        self.player.pause();
    }

    fn toggle_playback(&self) {
        if !self.player.is_paused() {
            self.player.pause();
        } else {
            self.player.play();
        }
    }

    fn clear_player(&self) {
        self.player.clear();
    }

    fn clear(&self) {
        self.clear_player();
        self.queue.borrow_mut().clear();
    }

    fn skip(&self, n: usize) {
        self.clear_player();

        let mut binding = self.queue();
        let next_song = binding.skip_song(n);

        if let Some(song) = next_song {
            self.append_song_to_player(song).unwrap();
        }
    }

    fn add_song_to_queue(&self, song: Song) {
        {
            let mut binding = self.queue();
            binding.add_song(song);
        }
        // if the player is empty, start playback
        if self.player.is_empty() {
            if let Some(song) = self.get_next_song() {
                if let Err(e) = self.append_song_to_player(&song) {
                    error!("Failed to append song to player: {}", e);
                }
                self.play();
            }
        }
    }

    fn get_next_song(&self) -> Option<Song> {
        let mut binding = self.queue();
        binding.next_song().cloned()
    }

    fn append_to_player<T>(&self, source: T)
    where
        T: Source<Item = f32> + Send + 'static,
    {
        self.player.append(source);
    }

    fn append_song_to_player(&self, song: &Song) -> Result<(), LibraryError> {
        let source = Decoder::new(BufReader::new(File::open(&song.path)?))?.convert_samples();

        self.append_to_player(source);

        Ok(())
    }

    fn queue(&self) -> RefMut<Queue> {
        self.queue.borrow_mut()
    }
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};

    use super::*;
    use std::sync::mpsc;

    #[fixture]
    fn audio_kernel() -> AudioKernel {
        AudioKernel::new()
    }

    #[fixture]
    fn sound() -> impl Source<Item = f32> + Send + 'static {
        rodio::source::SineWave::new(440.0)
    }

    #[rstest]
    fn test_audio_kernel_play_pause(
        audio_kernel: AudioKernel,
        sound: impl Source<Item = f32> + Send + 'static,
    ) {
        audio_kernel.player.append(sound);
        audio_kernel.play();
        assert!(!audio_kernel.player.is_paused());
        audio_kernel.pause();
        assert!(audio_kernel.player.is_paused());
    }

    #[rstest]
    fn test_audio_kernel_toggle_playback(
        audio_kernel: AudioKernel,
        sound: impl Source<Item = f32> + Send + 'static,
    ) {
        audio_kernel.player.append(sound);
        audio_kernel.play();
        assert!(!audio_kernel.player.is_paused());
        audio_kernel.toggle_playback();
        assert!(audio_kernel.player.is_paused());
        audio_kernel.toggle_playback();
        assert!(!audio_kernel.player.is_paused());
    }

    #[test]
    fn test_audio_kernel_sender_send() {
        let (tx, rx) = mpsc::channel();
        let sender = AudioKernelSender { tx };
        sender.send(AudioCommand::Play);
        assert_eq!(rx.recv().unwrap(), AudioCommand::Play);
    }
}
