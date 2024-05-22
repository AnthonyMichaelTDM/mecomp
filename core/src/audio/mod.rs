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

use crate::{
    errors::LibraryError,
    state::{RepeatMode, StateAudio, StateRuntime},
};
use mecomp_storage::{db::schemas::song::Song, util::OneOrMany};

use self::queue::Queue;

lazy_static! {
    pub static ref AUDIO_KERNEL: Arc<AudioKernelSender> = {
        let (tx, rx) = std::sync::mpsc::channel();
        tokio::spawn(async {
            let kernel = AudioKernel::new();
            kernel.spawn(rx);
        });
        Arc::new(AudioKernelSender { tx })
    };
}

#[derive(Debug)]
pub enum AudioCommand {
    Play,
    Pause,
    TogglePlayback,
    RestartSong,
    /// only clear the player (i.e. stop playback)
    ClearPlayer,
    Clear,
    SkipForward(usize),
    SkipBackward(usize),
    ShuffleQueue,
    // PlaySource(Box<dyn Source<Item = f32> + Send>),
    AddToQueue(OneOrMany<Song>),
    SetRepeatMode(RepeatMode),
    Exit,
    /// used to report information about the state of the audio kernel
    ReportStatus(tokio::sync::oneshot::Sender<StateAudio>),
}

impl PartialEq for AudioCommand {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Play, Self::Play)
            | (Self::Pause, Self::Pause)
            | (Self::TogglePlayback, Self::TogglePlayback)
            | (Self::ClearPlayer, Self::ClearPlayer)
            | (Self::Clear, Self::Clear)
            | (Self::RestartSong, Self::RestartSong)
            | (Self::ShuffleQueue, Self::ShuffleQueue) => true,
            (Self::SkipForward(a), Self::SkipForward(b))
            | (Self::SkipBackward(a), Self::SkipBackward(b)) => a == b,
            (Self::AddToQueue(a), Self::AddToQueue(b)) => a == b,
            (Self::SetRepeatMode(a), Self::SetRepeatMode(b)) => a == b,
            (Self::Exit, Self::Exit) | (Self::ReportStatus(_), Self::ReportStatus(_)) => true,
            _ => false,
        }
    }
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
        async fn run(kernel: &AudioKernel, rx: Receiver<AudioCommand>) {
            let db = mecomp_storage::db::init_database().await.unwrap();

            loop {
                let command = rx.recv().unwrap();
                match command {
                    AudioCommand::Play => kernel.play(),
                    AudioCommand::Pause => kernel.pause(),
                    AudioCommand::TogglePlayback => kernel.toggle_playback(),
                    AudioCommand::RestartSong => kernel.restart_song(),
                    AudioCommand::ClearPlayer => kernel.clear_player(),
                    AudioCommand::Clear => kernel.clear(),
                    AudioCommand::SkipForward(n) => kernel.skip_forward(n),
                    AudioCommand::SkipBackward(n) => kernel.skip_backward(n),
                    AudioCommand::ShuffleQueue => kernel.queue.borrow_mut().shuffle(),
                    //AudioCommand::PlaySource(source) => kernel.append_to_player(source),
                    AudioCommand::AddToQueue(OneOrMany::None) => {}
                    AudioCommand::AddToQueue(OneOrMany::One(song)) => {
                        kernel.add_song_to_queue(song)
                    }
                    AudioCommand::AddToQueue(OneOrMany::Many(songs)) => {
                        kernel.add_songs_to_queue(songs)
                    }
                    AudioCommand::SetRepeatMode(mode) => {
                        kernel.queue.borrow_mut().set_repeat_mode(mode);
                    }
                    AudioCommand::Exit => break,
                    AudioCommand::ReportStatus(tx) => {
                        let current_song = kernel.queue.borrow().current_song().cloned();

                        let (current_album, current_artist) = tokio::join!(
                            async {
                                if let Some(song) = current_song.as_ref() {
                                    Song::read_album(&db, song.id.clone()).await
                                } else {
                                    Ok(None)
                                }
                            },
                            async {
                                if let Some(song) = current_song.as_ref() {
                                    Song::read_artist(&db, song.id.clone())
                                        .await
                                        .map(Into::into)
                                } else {
                                    Ok(OneOrMany::None)
                                }
                            }
                        );

                        let state = StateAudio {
                            queue: kernel.queue.borrow().queued_songs(),
                            queue_position: kernel.queue.borrow().current_index(),
                            current_song,
                            repeat_mode: kernel.queue.borrow().get_repeat_mode(),
                            runtime: kernel.queue.borrow().current_song().map(|song| {
                                StateRuntime {
                                    duration: song.runtime.into(),
                                    seek_position: todo!(
                                        "determine how much of a Source has been played"
                                    ),
                                    seek_percent: todo!(
                                        "determine how much of a Source has been played"
                                    ),
                                }
                            }),
                            paused: kernel.player.is_paused(),
                            muted: todo!("implement volume control"),
                            volume: todo!("implement volume control"),
                            current_album: current_album.ok().flatten(),
                            current_artist: current_artist.ok().into(),
                        };

                        if tx.send(state).is_err() {
                            // report and ignore errors
                            error!("Audio kernel failed to report its state");
                        }
                    }
                }
            }
        }

        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(run(&self, rx));
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

    fn restart_song(&self) {
        self.clear_player();

        if let Some(song) = self.queue.borrow().current_song() {
            if let Err(e) = self.append_song_to_player(song) {
                error!("Failed to append song to player: {}", e);
            }
        }
    }

    fn clear_player(&self) {
        self.player.clear();
    }

    fn clear(&self) {
        self.clear_player();
        self.queue.borrow_mut().clear();
    }

    fn skip_forward(&self, n: usize) {
        self.clear_player();

        let mut binding = self.queue();
        let next_song = binding.skip_forward(n);

        if let Some(song) = next_song {
            if let Err(e) = self.append_song_to_player(song) {
                error!("Failed to append song to player: {}", e);
            }
        }
    }

    fn skip_backward(&self, n: usize) {
        self.clear_player();

        let mut binding = self.queue();
        let next_song = binding.skip_backward(n);

        if let Some(song) = next_song {
            if let Err(e) = self.append_song_to_player(song) {
                error!("Failed to append song to player: {}", e);
            }
        }
    }

    fn add_song_to_queue(&self, song: Song) {
        {
            let mut binding = self.queue();
            binding.add_song(song);
        }
        // if the player is empty, start playback
        if self.player.empty() {
            if let Some(song) = self.get_next_song() {
                if let Err(e) = self.append_song_to_player(&song) {
                    error!("Failed to append song to player: {}", e);
                }
                self.play();
            }
        }
    }

    fn add_songs_to_queue(&self, songs: Vec<Song>) {
        {
            let mut binding = self.queue();
            binding.add_songs(songs);
        }
        // if the player is empty, start playback
        if self.player.empty() {
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
    use std::time::Duration;

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

    #[rstest]
    #[timeout(Duration::from_secs(3))] // if the test takes longer than 3 seconds, this is a failure
    fn test_audio_player_kernel_spawn_and_exit() {
        let (tx, rx) = mpsc::channel();
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.spawn(async {
            let kernel = AudioKernel::new();
            kernel.spawn(rx);
        });

        let sender = AudioKernelSender { tx };
        sender.send(AudioCommand::Exit);
    }
}
