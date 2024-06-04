#![allow(clippy::module_name_repetitions)]
pub mod queue;

use std::{
    cell::RefCell,
    fmt::Display,
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

lazy_static! {
    pub static ref AUDIO_KERNEL: Arc<AudioKernelSender> = {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
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

impl Display for QueueCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueueCommand::SkipForward(n) => write!(f, "Skip Forward by {n}"),
            QueueCommand::SkipBackward(n) => write!(f, "Skip Backward by {n}"),
            QueueCommand::SetPosition(n) => write!(f, "Set Position to {n}"),
            QueueCommand::Shuffle => write!(f, "Shuffle"),
            QueueCommand::AddToQueue(OneOrMany::None) => write!(f, "Add nothing"),
            QueueCommand::AddToQueue(OneOrMany::One(song)) => {
                write!(f, "Add \"{}\"", song.title.to_string())
            }
            QueueCommand::AddToQueue(OneOrMany::Many(songs)) => {
                write!(
                    f,
                    "Add {:?}",
                    songs
                        .iter()
                        .map(|song| song.title.to_string())
                        .collect::<Vec<_>>()
                )
            }
            QueueCommand::RemoveRange(range) => {
                write!(f, "Remove items {}..{}", range.start, range.end)
            }
            QueueCommand::Clear => write!(f, "Clear"),
            QueueCommand::SetRepeatMode(mode) => {
                write!(f, "Set Repeat Mode to {mode}", mode = mode)
            }
        }
    }
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

impl Display for VolumeCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VolumeCommand::Up(percent) => write!(f, "+{percent:.0}%", percent = percent * 100.0),
            VolumeCommand::Down(percent) => write!(f, "-{percent:.0}%", percent = percent * 100.0),
            VolumeCommand::Set(percent) => write!(f, "={percent:.0}%", percent = percent * 100.0),
            VolumeCommand::Mute => write!(f, "Mute"),
            VolumeCommand::Unmute => write!(f, "Unmute"),
            VolumeCommand::ToggleMute => write!(f, "Toggle Mute"),
        }
    }
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
    // TODO: seek commands
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
            #[cfg(not(tarpaulin_include))]
            _ => false,
        }
    }
}

impl Display for AudioCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Play => write!(f, "Play"),
            Self::Pause => write!(f, "Pause"),
            Self::TogglePlayback => write!(f, "Toggle Playback"),
            Self::RestartSong => write!(f, "Restart Song"),
            Self::ClearPlayer => write!(f, "Clear Player"),
            Self::Queue(command) => write!(f, "Queue: {command}"),
            Self::Exit => write!(f, "Exit"),
            Self::ReportStatus(_) => write!(f, "Report Status"),
            Self::Volume(command) => write!(f, "Volume: {command}"),
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
    #[cfg(not(feature = "mock_playback"))]
    // in tests, we have no audio devices to play audio on so we don't need to keep the stream alive
    _stream: rodio::OutputStream,
    /// this is not used, but is needed to keep the stream alive
    #[cfg(not(feature = "mock_playback"))]
    _stream_handle: rodio::OutputStreamHandle,
    #[cfg(feature = "mock_playback")]
    _queue_rx_end_tx: tokio::sync::oneshot::Sender<()>,
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
    #[cfg(not(feature = "mock_playback"))]
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

    /// this function initializes the audio kernel
    /// it is not meant to be called directly, use `AUDIO_KERNEL` instead to send command
    ///
    /// this is the version for tests, where we don't create the actual audio stream since we don't need to play audio
    #[must_use]
    #[cfg(feature = "mock_playback")]
    pub fn new() -> Self {
        let (sink, mut queue_rx) = rodio::Sink::new_idle();

        // start a detached thread that continuously polls the queue_rx, until it receives a command to exit
        let (tx, rx) = tokio::sync::oneshot::channel();

        std::thread::spawn(move || {
            // basically, call rx.await and while it is waiting for a command, poll the queue_rx
            let _ = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    tokio::select! {
                        _ = rx => {},
                        _ = async {
                            loop {
                                queue_rx.next();
                                std::thread::sleep(std::time::Duration::from_millis(1));
                            }
                        } => {},
                    }
                });
        });

        sink.pause();

        Self {
            player: sink,
            _queue_rx_end_tx: tx,
            queue: RefCell::new(Queue::new()),
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
        for (command, ctx) in rx {
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

        #[cfg(feature = "mock_playback")]
        self._queue_rx_end_tx.send(()).unwrap();
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
        let paused = self.player.is_paused();
        self.clear_player();

        if let Some(song) = self.queue.borrow().current_song() {
            if let Err(e) = self.append_song_to_player(song) {
                error!("Failed to append song to player: {}", e);
            }

            if !paused {
                self.play();
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
    use pretty_assertions::assert_str_eq;
    use rstest::{fixture, rstest};

    use crate::test_utils::{arb_song_case, create_song, init};

    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    #[rstest]
    #[case(AudioCommand::Play, AudioCommand::Play, true)]
    #[case(AudioCommand::Play, AudioCommand::Pause, false)]
    #[case(AudioCommand::Pause, AudioCommand::Pause, true)]
    #[case(AudioCommand::TogglePlayback, AudioCommand::TogglePlayback, true)]
    #[case(AudioCommand::RestartSong, AudioCommand::RestartSong, true)]
    #[case(
        AudioCommand::Queue(QueueCommand::Clear),
        AudioCommand::Queue(QueueCommand::Clear),
        true
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::Clear),
        AudioCommand::Queue(QueueCommand::Shuffle),
        false
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SkipForward(1)),
        AudioCommand::Queue(QueueCommand::SkipForward(1)),
        true
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SkipForward(1)),
        AudioCommand::Queue(QueueCommand::SkipForward(2)),
        false
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SkipBackward(1)),
        AudioCommand::Queue(QueueCommand::SkipBackward(1)),
        true
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SkipBackward(1)),
        AudioCommand::Queue(QueueCommand::SkipBackward(2)),
        false
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SetPosition(1)),
        AudioCommand::Queue(QueueCommand::SetPosition(1)),
        true
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SetPosition(1)),
        AudioCommand::Queue(QueueCommand::SetPosition(2)),
        false
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::Shuffle),
        AudioCommand::Queue(QueueCommand::Shuffle),
        true
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::Shuffle),
        AudioCommand::Queue(QueueCommand::Clear),
        false
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Up(0.1)),
        AudioCommand::Volume(VolumeCommand::Up(0.1)),
        true
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Up(0.1)),
        AudioCommand::Volume(VolumeCommand::Up(0.2)),
        false
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Down(0.1)),
        AudioCommand::Volume(VolumeCommand::Down(0.1)),
        true
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Down(0.1)),
        AudioCommand::Volume(VolumeCommand::Down(0.2)),
        false
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Set(0.1)),
        AudioCommand::Volume(VolumeCommand::Set(0.1)),
        true
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Set(0.1)),
        AudioCommand::Volume(VolumeCommand::Set(0.2)),
        false
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Mute),
        AudioCommand::Volume(VolumeCommand::Mute),
        true
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Mute),
        AudioCommand::Volume(VolumeCommand::Unmute),
        false
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Unmute),
        AudioCommand::Volume(VolumeCommand::Unmute),
        true
    )]
    fn test_audio_command_equality(
        #[case] lhs: AudioCommand,
        #[case] rhs: AudioCommand,
        #[case] expected: bool,
    ) {
        assert_eq!(lhs == rhs, expected);
        assert_eq!(rhs == lhs, expected);
    }

    // dummy song used for display tests, makes the tests more readable
    fn dummy_song() -> Song {
        Song {
            id: Song::generate_id(),
            title: "Song 1".into(),
            artist: OneOrMany::None,
            album_artist: OneOrMany::None,
            album: "album".into(),
            genre: OneOrMany::None,
            runtime: surrealdb::sql::Duration::from_secs(100),
            track: None,
            disc: None,
            release_year: None,
            extension: "mp3".into(),
            path: "foo/bar.mp3".into(),
        }
    }

    #[rstest]
    #[case(AudioCommand::Play, "Play")]
    #[case(AudioCommand::Pause, "Pause")]
    #[case(AudioCommand::TogglePlayback, "Toggle Playback")]
    #[case(AudioCommand::ClearPlayer, "Clear Player")]
    #[case(AudioCommand::RestartSong, "Restart Song")]
    #[case(AudioCommand::Queue(QueueCommand::Clear), "Queue: Clear")]
    #[case(AudioCommand::Queue(QueueCommand::Shuffle), "Queue: Shuffle")]
    #[case(
        AudioCommand::Queue(QueueCommand::AddToQueue(OneOrMany::None)),
        "Queue: Add nothing"
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::AddToQueue(OneOrMany::One(dummy_song()))),
        "Queue: Add \"Song 1\""
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::AddToQueue(OneOrMany::Many(vec![dummy_song()]))),
        "Queue: Add [\"Song 1\"]"
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::RemoveRange(0..1)),
        "Queue: Remove items 0..1"
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SetRepeatMode(RepeatMode::None)),
        "Queue: Set Repeat Mode to None"
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SkipForward(1)),
        "Queue: Skip Forward by 1"
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SkipBackward(1)),
        "Queue: Skip Backward by 1"
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SetPosition(1)),
        "Queue: Set Position to 1"
    )]
    #[case(AudioCommand::Volume(VolumeCommand::Up(0.1)), "Volume: +10%")]
    #[case(AudioCommand::Volume(VolumeCommand::Down(0.1)), "Volume: -10%")]
    #[case(AudioCommand::Volume(VolumeCommand::Set(0.1)), "Volume: =10%")]
    #[case(AudioCommand::Volume(VolumeCommand::Mute), "Volume: Mute")]
    #[case(AudioCommand::Volume(VolumeCommand::Unmute), "Volume: Unmute")]
    #[case(AudioCommand::Volume(VolumeCommand::ToggleMute), "Volume: Toggle Mute")]
    #[case(AudioCommand::Exit, "Exit")]
    #[case(AudioCommand::ReportStatus(tokio::sync::oneshot::channel().0), "Report Status")]
    fn test_audio_command_display(#[case] command: AudioCommand, #[case] expected: &str) {
        assert_str_eq!(command.to_string(), expected);
    }

    #[fixture]
    fn audio_kernel() -> AudioKernel {
        AudioKernel::default()
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

    #[test]
    #[should_panic]
    fn test_audio_kernel_send_closed_channel() {
        let (tx, _) = mpsc::channel();
        let sender = AudioKernelSender::new(tx);
        sender.send(AudioCommand::Play);
    }

    #[rstest]
    #[timeout(Duration::from_secs(3))] // if the test takes longer than 3 seconds, this is a failure
    fn test_audio_player_kernel_spawn_and_exit(
        #[from(audio_kernel_sender)] sender: AudioKernelSender,
    ) {
        init();

        sender.send(AudioCommand::Exit);
    }

    #[rstest]
    #[timeout(Duration::from_secs(3))] // if the test takes longer than 3 seconds, this is a failure
    fn test_audio_player_global_kernel_exit() {
        init();

        AUDIO_KERNEL.send(AudioCommand::Exit);
    }

    #[rstest]
    fn test_volume_control(audio_kernel: AudioKernel) {
        audio_kernel.volume_control(VolumeCommand::Up(0.1));
        assert_eq!(*audio_kernel.volume.borrow(), 1.1);

        audio_kernel.volume_control(VolumeCommand::Down(0.1));
        assert_eq!(*audio_kernel.volume.borrow(), 1.0);

        audio_kernel.volume_control(VolumeCommand::Set(0.5));
        assert_eq!(*audio_kernel.volume.borrow(), 0.5);

        audio_kernel.volume_control(VolumeCommand::Mute);
        assert_eq!(
            audio_kernel
                .muted
                .load(std::sync::atomic::Ordering::Relaxed),
            true
        );

        audio_kernel.volume_control(VolumeCommand::Unmute);
        assert_eq!(
            audio_kernel
                .muted
                .load(std::sync::atomic::Ordering::Relaxed),
            false
        );

        audio_kernel.volume_control(VolumeCommand::ToggleMute);
        assert_eq!(
            audio_kernel
                .muted
                .load(std::sync::atomic::Ordering::Relaxed),
            true
        );

        audio_kernel.volume_control(VolumeCommand::ToggleMute);
        assert_eq!(
            audio_kernel
                .muted
                .load(std::sync::atomic::Ordering::Relaxed),
            false
        );
    }

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
        #[timeout(Duration::from_secs(5))] // if the test takes longer than this, the test can be considered a failure
        #[tokio::test]
        async fn test_play_pause_toggle_restart(
            #[from(audio_kernel_sender)] sender: AudioKernelSender,
        ) {
            init();
            let db = init_test_database().await.unwrap();

            let song = create_song(&db, arb_song_case()()).await.unwrap();

            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(
                OneOrMany::One(song.clone()),
            )));

            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert!(!state.paused);

            sender.send(AudioCommand::Pause);
            let state = get_state(sender.clone()).await;
            assert!(state.paused);

            sender.send(AudioCommand::Play);
            let state = get_state(sender.clone()).await;
            assert!(!state.paused);

            sender.send(AudioCommand::RestartSong);
            let state = get_state(sender.clone()).await;
            assert!(!state.paused); // Note, unlike adding a song to the queue, RestartSong does not affect whether the player is paused

            sender.send(AudioCommand::TogglePlayback);
            let state = get_state(sender.clone()).await;
            assert!(state.paused);

            sender.send(AudioCommand::RestartSong);
            let state = get_state(sender.clone()).await;
            assert!(state.paused); // Note, unlike adding a song to the queue, RestartSong does not affect whether the player is paused

            sender.send(AudioCommand::Exit);
        }

        #[rstest]
        #[timeout(Duration::from_secs(5))] // if the test takes longer than this, the test can be considered a failure
        #[tokio::test]
        async fn test_audio_kernel_skip_forward(audio_kernel: AudioKernel) {
            init();
            let db = init_test_database().await.unwrap();

            let state = audio_kernel.state();
            assert_eq!(state.queue_position, None);
            assert!(state.paused);

            audio_kernel.queue_control(QueueCommand::AddToQueue(OneOrMany::Many(vec![
                create_song(&db, arb_song_case()()).await.unwrap(),
                create_song(&db, arb_song_case()()).await.unwrap(),
                create_song(&db, arb_song_case()()).await.unwrap(),
            ])));

            // songs were added to an empty queue, so the first song should start playing
            let state = audio_kernel.state();
            assert_eq!(state.queue_position, Some(0));
            assert!(!state.paused);

            audio_kernel.queue_control(QueueCommand::SkipForward(1));

            // the second song should start playing
            let state = audio_kernel.state();
            assert_eq!(state.queue_position, Some(1));
            assert!(!state.paused);

            audio_kernel.queue_control(QueueCommand::SkipForward(1));

            // the third song should start playing
            let state = audio_kernel.state();
            assert_eq!(state.queue_position, Some(2));
            assert!(!state.paused);

            audio_kernel.queue_control(QueueCommand::SkipForward(1));

            // we were at the end of the queue and tried to skip forward, so the player should be paused and the queue position should be None
            let state = audio_kernel.state();
            assert_eq!(state.queue_position, None);
            assert!(state.paused);
        }

        #[rstest]
        #[timeout(Duration::from_secs(5))] // if the test takes longer than this, the test can be considered a failure
        #[tokio::test]
        async fn test_audio_kernel_skip_forward_sender(
            #[from(audio_kernel_sender)] sender: AudioKernelSender,
        ) {
            // set up tracing
            init();

            let db = init_test_database().await.unwrap();

            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert!(state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(
                OneOrMany::Many(vec![
                    create_song(&db, arb_song_case()()).await.unwrap(),
                    create_song(&db, arb_song_case()()).await.unwrap(),
                    create_song(&db, arb_song_case()()).await.unwrap(),
                ]),
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

        #[rstest]
        #[timeout(Duration::from_secs(5))] // if the test takes longer than this, the test can be considered a failure
        #[tokio::test]
        async fn test_remove_range_from_queue(
            #[from(audio_kernel_sender)] sender: AudioKernelSender,
        ) {
            init();
            let db = init_test_database().await.unwrap();
            let song1 = create_song(&db, arb_song_case()()).await.unwrap();
            let song2 = create_song(&db, arb_song_case()()).await.unwrap();

            // add songs to the queue, starts playback
            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(
                OneOrMany::Many(vec![song1.clone(), song2.clone()]),
            )));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert!(!state.paused);

            // remove the current song from the queue, player should be paused
            sender.send(AudioCommand::Queue(QueueCommand::RemoveRange(0..1)));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert!(state.paused);
            assert_eq!(state.queue.len(), 1);
            assert_eq!(state.queue[0], song2);

            // add the song back to the queue, starts playback
            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(
                OneOrMany::One(song1.clone()),
            )));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert!(!state.paused);
            assert_eq!(state.queue.len(), 2);
            assert_eq!(state.queue[0], song2);
            assert_eq!(state.queue[1], song1);

            // remove the next song from the queue, player should still be playing
            sender.send(AudioCommand::Queue(QueueCommand::RemoveRange(1..2)));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert!(!state.paused);
            assert_eq!(state.queue.len(), 1);
            assert_eq!(state.queue[0], song2);

            sender.send(AudioCommand::Exit);
        }

        #[rstest]
        #[timeout(Duration::from_secs(5))] // if the test takes longer than this, the test can be considered a failure
        #[tokio::test]
        async fn test_audio_kernel_skip_backward(
            #[from(audio_kernel_sender)] sender: AudioKernelSender,
        ) {
            init();
            let db = init_test_database().await.unwrap();

            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert!(state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(
                OneOrMany::Many(vec![
                    create_song(&db, arb_song_case()()).await.unwrap(),
                    create_song(&db, arb_song_case()()).await.unwrap(),
                    create_song(&db, arb_song_case()()).await.unwrap(),
                ]),
            )));

            // songs were added to an empty queue, so the first song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert!(!state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::SkipForward(2)));

            // the third song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(2));
            assert!(!state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::SkipBackward(1)));

            // the second song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(1));
            assert!(!state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::SkipBackward(1)));

            // the first song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert!(!state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::SkipBackward(1)));

            // we were at the start of the queue and tried to skip backward, so the player should be paused and the queue position should be None
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert!(state.paused);

            sender.send(AudioCommand::Exit);
        }

        #[rstest]
        #[timeout(Duration::from_secs(5))] // if the test takes longer than this, the test can be considered a failure
        #[tokio::test]
        async fn test_audio_kernel_set_position(
            #[from(audio_kernel_sender)] sender: AudioKernelSender,
        ) {
            init();
            let db = init_test_database().await.unwrap();

            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert!(state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(
                OneOrMany::Many(vec![
                    create_song(&db, arb_song_case()()).await.unwrap(),
                    create_song(&db, arb_song_case()()).await.unwrap(),
                    create_song(&db, arb_song_case()()).await.unwrap(),
                ]),
            )));
            // songs were added to an empty queue, so the first song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert!(!state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::SetPosition(1)));
            // the second song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(1));
            assert!(!state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::SetPosition(2)));
            // the third song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(2));
            assert!(!state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::SetPosition(0)));
            // the first song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert!(!state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::SetPosition(3)));
            // we tried to set the position to an index that's out of pounds, so the player should be at the nearest valid index
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(2));
            assert!(!state.paused);

            sender.send(AudioCommand::Exit);
        }

        #[rstest]
        #[timeout(Duration::from_secs(5))] // if the test takes longer than this, the test can be considered a failure
        #[tokio::test]
        async fn test_audio_kernel_clear(#[from(audio_kernel_sender)] sender: AudioKernelSender) {
            init();
            let db = init_test_database().await.unwrap();

            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert!(state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(
                OneOrMany::Many(vec![
                    create_song(&db, arb_song_case()()).await.unwrap(),
                    create_song(&db, arb_song_case()()).await.unwrap(),
                    create_song(&db, arb_song_case()()).await.unwrap(),
                ]),
            )));
            // songs were added to an empty queue, so the first song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert_eq!(state.queue.len(), 3);
            assert!(!state.paused);

            sender.send(AudioCommand::ClearPlayer);
            // we only cleared the audio player, so the queue should still have the songs
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert_eq!(state.queue.len(), 3);
            assert!(state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::Clear));
            // we cleared the queue, so the player should be paused and the queue should be empty
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert_eq!(state.queue.len(), 0);
            assert!(state.paused);

            sender.send(AudioCommand::Exit);
        }

        #[rstest]
        #[timeout(Duration::from_secs(5))] // if the test takes longer than this, the test can be considered a failure
        #[tokio::test]
        async fn test_audio_kernel_shuffle(#[from(audio_kernel_sender)] sender: AudioKernelSender) {
            init();
            let db = init_test_database().await.unwrap();

            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert!(state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(
                OneOrMany::Many(vec![
                    create_song(&db, arb_song_case()()).await.unwrap(),
                    create_song(&db, arb_song_case()()).await.unwrap(),
                    create_song(&db, arb_song_case()()).await.unwrap(),
                ]),
            )));
            // songs were added to an empty queue, so the first song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert_eq!(state.queue.len(), 3);
            assert!(!state.paused);

            // lets go to the second song
            sender.send(AudioCommand::Queue(QueueCommand::SkipForward(1)));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(1));
            assert_eq!(state.queue.len(), 3);
            assert!(!state.paused);

            // lets shuffle the queue
            sender.send(AudioCommand::Queue(QueueCommand::Shuffle));
            // we shuffled the queue, so the player should still be playing and the queue should still have 3 songs, and the previous current song should be the now first song
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert_eq!(state.queue.len(), 3);
            assert!(!state.paused);

            sender.send(AudioCommand::Exit);
        }

        #[rstest]
        #[timeout(Duration::from_secs(5))] // if the test takes longer than this, the test can be considered a failure
        #[tokio::test]
        async fn test_volume_commands(#[from(audio_kernel_sender)] sender: AudioKernelSender) {
            init();

            let state = get_state(sender.clone()).await;
            assert_eq!(state.volume, 1.0);
            assert!(!state.muted);

            sender.send(AudioCommand::Volume(VolumeCommand::Up(0.1)));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.volume, 1.1);
            assert!(!state.muted);

            sender.send(AudioCommand::Volume(VolumeCommand::Down(0.1)));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.volume, 1.0);
            assert!(!state.muted);

            sender.send(AudioCommand::Volume(VolumeCommand::Set(0.5)));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.volume, 0.5);
            assert!(!state.muted);

            sender.send(AudioCommand::Volume(VolumeCommand::Mute));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.volume, 0.5); // although underlying volume is 0 (for the rodio player), the stored volume is still 0.5
            assert!(state.muted);

            sender.send(AudioCommand::Volume(VolumeCommand::Unmute));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.volume, 0.5);
            assert!(!state.muted);

            sender.send(AudioCommand::Volume(VolumeCommand::ToggleMute));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.volume, 0.5);
            assert!(state.muted);

            sender.send(AudioCommand::Volume(VolumeCommand::ToggleMute));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.volume, 0.5);
            assert!(!state.muted);

            sender.send(AudioCommand::Exit);
        }
    }
}
