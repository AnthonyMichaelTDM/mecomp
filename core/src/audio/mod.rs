#![allow(clippy::module_name_repetitions)]
pub mod queue;

use std::{
    cell::RefCell,
    fs::File,
    io::BufReader,
    ops::Range,
    sync::{
        atomic::AtomicBool,
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
    state::{Percent, RepeatMode, StateAudio, StateRuntime},
};
use mecomp_storage::{db::schemas::song::Song, util::OneOrMany};

use self::queue::Queue;

#[cfg(not(tarpaulin_include))]
lazy_static! {
    pub static ref AUDIO_KERNEL: Arc<AudioKernelSender> = {
        let (tx, rx) = std::sync::mpsc::channel();
        tokio::spawn(async {
            let kernel = AudioKernel::new();
            kernel.init(rx);
        });
        Arc::new(AudioKernelSender { tx })
    };
}
/// Queue Commands
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueCommand {
    SkipForward(usize),
    SkipBackward(usize),
    SetPosition(usize),
    Shuffle,
    AddToQueue(OneOrMany<Song>),
    RemoveRange(Range<usize>),
    Clear,
    SetRepeatMode(RepeatMode),
}

/// Volume commands
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum VolumeCommand {
    Up(f32),
    Down(f32),
    Set(f32),
    Mute,
    Unmute,
    ToggleMute,
}

/// Commands that can be sent to the audio kernel
#[derive(Debug)]
pub enum AudioCommand {
    Play,
    Pause,
    TogglePlayback,
    RestartSong,
    /// only clear the player (i.e. stop playback)
    ClearPlayer,
    /// Queue Commands
    Queue(QueueCommand),
    /// Stop the audio kernel
    Exit,
    /// used to report information about the state of the audio kernel
    ReportStatus(tokio::sync::oneshot::Sender<StateAudio>),
    /// volume control commands
    Volume(VolumeCommand),
}

impl PartialEq for AudioCommand {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Play, Self::Play)
            | (Self::Pause, Self::Pause)
            | (Self::TogglePlayback, Self::TogglePlayback)
            | (Self::ClearPlayer, Self::ClearPlayer)
            | (Self::RestartSong, Self::RestartSong)
            | (Self::Exit, Self::Exit)
            | (Self::ReportStatus(_), Self::ReportStatus(_)) => true,
            (Self::Queue(a), Self::Queue(b)) => a == b,
            (Self::Volume(a), Self::Volume(b)) => a == b,
            _ => false,
        }
    }
}

impl std::fmt::Display for AudioCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Play => write!(f, "Play"),
            Self::Pause => write!(f, "Pause"),
            Self::TogglePlayback => write!(f, "Toggle Playback"),
            Self::RestartSong => write!(f, "Restart Song"),
            Self::ClearPlayer => write!(f, "Clear Player"),
            Self::Queue(command) => write!(f, "Queue: {:?}", command),
            Self::Exit => write!(f, "Exit"),
            Self::ReportStatus(_) => write!(f, "Report Status"),
            Self::Volume(command) => write!(f, "Volume: {:?}", command),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioKernelSender {
    tx: Sender<(AudioCommand, tracing::Span)>,
}

impl AudioKernelSender {
    #[must_use]
    pub const fn new(tx: Sender<(AudioCommand, tracing::Span)>) -> Self {
        Self { tx }
    }

    /// Send a command to the audio kernel
    #[instrument(skip(self))]
    pub fn send(&self, command: AudioCommand) {
        let ctx =
            tracing::info_span!("Sending Audio Command to Kernel", command = ?command).or_current();

        if let Err(e) = self.tx.send((command, ctx)) {
            error!("Failed to send command to audio kernel: {e}");
            panic!("Failed to send command to audio kernel: {e}");
        }
    }
}

pub struct AudioKernel {
    /// this is not used, but is needed to keep the stream alive
    _stream: rodio::OutputStream,
    /// this is not used, but is needed to keep the stream alive
    _stream_handle: rodio::OutputStreamHandle,
    player: rodio::Sink,
    queue: RefCell<Queue>,
    /// The value `1.0` is the "normal" volume (unfiltered input). Any value other than `1.0` will multiply each sample by this value.
    volume: RefCell<f32>,
    muted: AtomicBool,
}

impl AudioKernel {
    /// this function initializes the audio kernel
    /// it is not meant to be called directly, use `AUDIO_KERNEL` instead to send commands
    ///
    /// # Panics
    ///
    /// panics if the rodio stream cannot be created
    #[must_use]
    pub fn new() -> Self {
        let (stream, stream_handle) = rodio::OutputStream::try_default().unwrap();

        let player = rodio::Sink::try_new(&stream_handle).unwrap();
        player.pause();
        let queue = Queue::new();

        Self {
            _stream: stream,
            _stream_handle: stream_handle,
            player,
            queue: RefCell::new(queue),
            volume: RefCell::new(1.0),
            muted: AtomicBool::new(false),
        }
    }

    /// Spawn the audio kernel, taking ownership of self
    ///
    /// this function should be called in a detached thread to keep the audio kernel running,
    /// this function will block until the `Exit` command is received
    ///
    /// # Example
    ///
    /// ```
    /// use mecomp_core::audio::{AudioKernel, AudioCommand, AudioKernelSender};
    ///
    /// // create a channel to send commands to the audio kernel
    /// let (tx, rx) = std::sync::mpsc::channel();
    /// // spawn the audio kernel in a detached thread
    /// std::thread::spawn(move || {
    ///    let kernel = AudioKernel::new();
    ///    kernel.init(rx);
    /// });
    /// // create a sender to send commands to the audio kernel
    /// let sender = AudioKernelSender::new(tx);
    ///
    /// // send a command to the audio kernel
    /// // e.g. to shutdown the audio kernel:
    /// sender.send(AudioCommand::Exit);
    /// ```
    pub fn init(self, rx: Receiver<(AudioCommand, tracing::Span)>) {
        // for command in rx {
        while let Ok((command, ctx)) = rx.recv() {
            let _guard = ctx.enter();

            match command {
                AudioCommand::Play => self.play(),
                AudioCommand::Pause => self.pause(),
                AudioCommand::TogglePlayback => self.toggle_playback(),
                AudioCommand::RestartSong => self.restart_song(),
                AudioCommand::ClearPlayer => self.clear_player(),
                AudioCommand::Queue(command) => self.queue_control(command),
                AudioCommand::Exit => break,
                AudioCommand::ReportStatus(tx) => {
                    let state = self.state();

                    if let Err(e) = tx.send(state) {
                        // if there was an error, then the receiver will never receive the state, this can cause a permanent hang
                        // so we stop the audio kernel if this happens (which will cause any future calls to `send` to panic)
                        error!("Audio Kernel failed to send state to the receiver, state receiver likely has been dropped. State: {e}");
                        break;
                    }
                }
                AudioCommand::Volume(command) => self.volume_control(command),
            }
        }
    }

    #[instrument(skip(self))]
    fn play(&self) {
        self.player.play();
    }

    #[instrument(skip(self))]
    fn pause(&self) {
        self.player.pause();
    }

    #[instrument(skip(self))]
    fn toggle_playback(&self) {
        if self.player.is_paused() {
            self.player.play();
        } else {
            self.player.pause();
        }
    }

    #[instrument(skip(self))]
    fn restart_song(&self) {
        self.clear_player();

        if let Some(song) = self.queue.borrow().current_song() {
            if let Err(e) = self.append_song_to_player(song) {
                error!("Failed to append song to player: {}", e);
            }
        }
    }

    #[instrument(skip(self))]
    fn clear_player(&self) {
        self.player.clear();
    }

    #[instrument(skip(self))]
    fn queue_control(&self, command: QueueCommand) {
        match command {
            QueueCommand::Clear => self.clear(),
            QueueCommand::SkipForward(n) => self.skip_forward(n),
            QueueCommand::SkipBackward(n) => self.skip_backward(n),
            QueueCommand::SetPosition(n) => self.set_position(n),
            QueueCommand::Shuffle => self.queue.borrow_mut().shuffle(),
            QueueCommand::AddToQueue(OneOrMany::None) => {}
            QueueCommand::AddToQueue(OneOrMany::One(song)) => self.add_song_to_queue(song),
            QueueCommand::AddToQueue(OneOrMany::Many(songs)) => self.add_songs_to_queue(songs),
            QueueCommand::RemoveRange(range) => self.remove_range_from_queue(range),
            QueueCommand::SetRepeatMode(mode) => self.queue.borrow_mut().set_repeat_mode(mode),
        }
    }

    #[instrument(skip(self))]
    fn state(&self) -> StateAudio {
        let queue = self.queue.borrow();
        let queue_position = queue.current_index();
        let current_song = queue.current_song().cloned();
        let repeat_mode = queue.get_repeat_mode();
        let runtime = current_song.as_ref().map(|song| {
            StateRuntime {
                duration: song.runtime.into(),
                seek_position: 0.0, // TODO: determine how much of a Source has been played
                seek_percent: Percent::new(0.0), // TODO: determine how much of a Source has been played
            }
        });
        let paused = self.player.is_paused();
        let muted = self.muted.load(std::sync::atomic::Ordering::Relaxed);
        let volume = *self.volume.borrow();

        let queue = queue.queued_songs();

        StateAudio {
            queue,
            queue_position,
            current_song,
            repeat_mode,
            runtime,
            paused,
            muted,
            volume,
        }
    }

    #[instrument(skip(self))]
    fn clear(&self) {
        self.clear_player();
        self.queue.borrow_mut().clear();
    }

    #[instrument(skip(self))]
    fn skip_forward(&self, n: usize) {
        let paused = self.player.is_paused();
        self.clear_player();

        let next_song = self.queue.borrow_mut().skip_forward(n).cloned();

        if let Some(song) = next_song {
            if let Err(e) = self.append_song_to_player(&song) {
                error!("Failed to append song to player: {}", e);
            }

            let binding = self.queue.borrow();
            if !(paused
                // and we have not just finished the queue 
                // (this makes it so if we hit the end of the queue on RepeatMode::None, we don't start playing again)
                || (binding.get_repeat_mode().is_none()
                    && binding.current_index().is_none()))
            {
                self.play();
            }
        }
    }

    #[instrument(skip(self))]
    fn skip_backward(&self, n: usize) {
        let paused = self.player.is_paused();
        self.clear_player();

        let next_song = self.queue.borrow_mut().skip_backward(n).cloned();

        if let Some(song) = next_song {
            if let Err(e) = self.append_song_to_player(&song) {
                error!("Failed to append song to player: {}", e);
            }
            if !paused {
                self.play();
            }
        }
    }

    #[instrument(skip(self))]
    fn set_position(&self, n: usize) {
        let paused = self.player.is_paused();
        self.clear_player();

        let mut binding = self.queue.borrow_mut();
        binding.set_current_index(n);
        let next_song = binding.current_song().cloned();
        drop(binding);

        if let Some(song) = next_song {
            if let Err(e) = self.append_song_to_player(&song) {
                error!("Failed to append song to player: {e}");
            }
            if !paused {
                self.play();
            }
        }
    }

    #[instrument(skip(self))]
    fn add_song_to_queue(&self, song: Song) {
        self.queue.borrow_mut().add_song(song);

        // if the player is empty, start playback
        if self.player.empty() {
            let current_index = self.queue.borrow().current_index();

            if let Some(song) =
                current_index.map_or_else(|| self.get_next_song(), |_| self.get_current_song())
            {
                if let Err(e) = self.append_song_to_player(&song) {
                    error!("Failed to append song to player: {e}");
                }
                self.play();
            }
        }
    }

    #[instrument(skip(self))]
    fn add_songs_to_queue(&self, songs: Vec<Song>) {
        self.queue.borrow_mut().add_songs(songs);

        // if the player is empty, start playback
        if self.player.empty() {
            let current_index = self.queue.borrow().current_index();

            if let Some(song) =
                current_index.map_or_else(|| self.get_next_song(), |_| self.get_current_song())
            {
                if let Err(e) = self.append_song_to_player(&song) {
                    error!("Failed to append song to player: {e}");
                }
                self.play();
            }
        }
    }

    // TODO: test that the player stops playing when the current song is removed,
    #[instrument(skip(self))]
    fn remove_range_from_queue(&self, range: Range<usize>) {
        let paused = if self.player.is_paused() {
            true
        } else if let Some(current_index) = self.queue.borrow().current_index() {
            // still true if the current song is to be removed
            range.start <= current_index && range.end > current_index
        } else {
            false
        };
        self.clear_player();

        self.queue.borrow_mut().remove_range(range);

        if let Some(song) = self.get_current_song() {
            if let Err(e) = self.append_song_to_player(&song) {
                error!("Failed to append song to player: {e}");
            }
            if !paused {
                self.play();
            }
        }
    }

    #[instrument(skip(self))]
    fn get_current_song(&self) -> Option<Song> {
        self.queue.borrow().current_song().cloned()
    }

    #[instrument(skip(self))]
    fn get_next_song(&self) -> Option<Song> {
        self.queue.borrow_mut().next_song().cloned()
    }

    #[instrument(skip(self, source))]
    fn append_to_player<T>(&self, source: T)
    where
        T: Source<Item = f32> + Send + 'static,
    {
        self.player.append(source);
    }

    #[instrument(skip(self))]
    fn append_song_to_player(&self, song: &Song) -> Result<(), LibraryError> {
        let source = Decoder::new(BufReader::new(File::open(&song.path)?))?.convert_samples();

        self.append_to_player(source);

        Ok(())
    }

    #[instrument(skip(self))]
    fn volume_control(&self, command: VolumeCommand) {
        match command {
            VolumeCommand::Up(percent) => {
                *self.volume.borrow_mut() += percent;
            }
            VolumeCommand::Down(percent) => {
                *self.volume.borrow_mut() -= percent;
            }
            VolumeCommand::Set(percent) => {
                *self.volume.borrow_mut() = percent;
            }
            VolumeCommand::Mute => {
                self.muted.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            VolumeCommand::Unmute => {
                self.muted
                    .store(false, std::sync::atomic::Ordering::Relaxed);
            }
            VolumeCommand::ToggleMute => {
                self.muted.store(
                    !self.muted.load(std::sync::atomic::Ordering::Relaxed),
                    std::sync::atomic::Ordering::Relaxed,
                );
            }
        }

        if self.muted.load(std::sync::atomic::Ordering::Relaxed) {
            self.player.set_volume(0.0);
        } else {
            self.player.set_volume(self.volume.borrow().to_owned());
        }
    }
}

impl Default for AudioKernel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use mecomp_storage::db::init_test_database;
    use rstest::{fixture, rstest};

    use crate::test_utils::{arb_song_case, create_song, init};

    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    #[fixture]
    fn audio_kernel() -> AudioKernel {
        AudioKernel::new()
    }

    #[fixture]
    fn audio_kernel_sender() -> AudioKernelSender {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let kernel = AudioKernel::new();
            kernel.init(rx);
        });

        AudioKernelSender::new(tx)
    }

    #[instrument]
    async fn get_state(sender: AudioKernelSender) -> StateAudio {
        let (tx, rx) = tokio::sync::oneshot::channel::<StateAudio>();
        sender.send(AudioCommand::ReportStatus(tx));
        rx.await.unwrap()
    }

    #[fixture]
    fn sound() -> impl Source<Item = f32> + Send + 'static {
        rodio::source::SineWave::new(440.0)
    }

    #[test]
    fn test_audio_kernel_sender_send() {
        let (tx, rx) = mpsc::channel();
        let sender = AudioKernelSender::new(tx);
        sender.send(AudioCommand::Play);
        let (recv, _) = rx.recv().unwrap();
        assert_eq!(recv, AudioCommand::Play);
    }

    #[rstest]
    #[timeout(Duration::from_secs(3))] // if the test takes longer than 3 seconds, this is a failure
    fn test_audio_player_kernel_spawn_and_exit(
        #[from(audio_kernel_sender)] sender: AudioKernelSender,
    ) {
        init();

        sender.send(AudioCommand::Exit);
    }

    #[cfg(not(tarpaulin))]
    mod playback_tests {
        //! These are tests that require the audio kernel to be able to play audio
        //! As such, they cannot be run on CI.
        //! Therefore, they are in a separate module so that they can be skipped when running tests on CI.

        use super::*;

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

        #[rstest]
        #[timeout(Duration::from_secs(3))] // if the test takes longer than 3 seconds, this is a failure
        #[tokio::test]
        async fn test_audio_kernel_skip_forward() {
            let sender = audio_kernel_sender();

            let db = init_test_database().await.unwrap();
            let song = create_song(&db, arb_song_case()()).await.unwrap();

            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert!(state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(
                OneOrMany::One(song.clone()),
            )));
            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(
                OneOrMany::One(song.clone()),
            )));
            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(
                OneOrMany::One(song.clone()),
            )));
            // songs were added to an empty queue, so the first song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert!(!state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::SkipForward(1)));
            // the second song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(1));
            assert!(!state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::SkipForward(1)));
            // the third song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(2));
            assert!(!state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::SkipForward(1)));
            // we were at the end of the queue and tried to skip forward, so the player should be paused and the queue position should be None
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert!(state.paused);

            sender.send(AudioCommand::Exit);
        }
    }
}
