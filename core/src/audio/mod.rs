#![allow(clippy::module_name_repetitions)]

use std::{
    fs::File,
    io::BufReader,
    ops::Range,
    sync::{
        atomic::AtomicBool,
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
    time::Duration,
};

use lazy_static::lazy_static;
use log::{debug, error};
use rodio::{source::SeekError, Decoder, Source};
use tracing::instrument;

use crate::{
    errors::LibraryError,
    format_duration,
    state::{Percent, SeekType, StateAudio, StateRuntime},
};
use mecomp_storage::db::schemas::song::Song;
use one_or_many::OneOrMany;

pub mod commands;
pub mod queue;

use commands::{AudioCommand, QueueCommand, VolumeCommand};
use queue::Queue;

lazy_static! {
    pub static ref AUDIO_KERNEL: Arc<AudioKernelSender> = {
        let (tx, rx) = std::sync::mpsc::channel();
        let tx_clone = tx.clone();
        std::thread::spawn(move || {
            let kernel = AudioKernel::new();
            kernel.init(tx_clone, rx);
        });
        Arc::new(AudioKernelSender { tx })
    };
}

const DURATION_WATCHER_TICK_MS: u64 = 50;
const DURATION_WATCHER_NEXT_SONG_THRESHOLD_MS: u64 = 100;

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
    _music_output: (rodio::OutputStream, rodio::OutputStreamHandle),
    #[cfg(feature = "mock_playback")]
    queue_rx_end_tx: tokio::sync::oneshot::Sender<()>,
    // /// Transmitter used to send commands to the audio kernel
    // tx: Sender<(AudioCommand, tracing::Span)>,
    /// the rodio sink used to play audio
    player: Arc<rodio::Sink>,
    /// the queue of songs to play
    queue: Arc<Mutex<Queue>>,
    /// The value `1.0` is the "normal" volume (unfiltered input). Any value other than `1.0` will multiply each sample by this value.
    volume: Arc<Mutex<f32>>,
    /// whether the audio is muted
    muted: AtomicBool,
    /// the amount of time the current sound has been playing
    time_played: Arc<Mutex<Duration>>,
    /// the duration of the current sound
    current_song_runtime: Arc<Mutex<Duration>>,
    /// whether the audio kernel is paused
    paused: Arc<AtomicBool>,
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

        let sink = rodio::Sink::try_new(&stream_handle).unwrap();
        sink.pause();
        let queue = Queue::new();

        Self {
            _music_output: (stream, stream_handle),
            player: sink.into(),
            queue: Arc::new(Mutex::new(queue)),
            volume: Arc::new(Mutex::new(1.0)),
            muted: AtomicBool::new(false),
            time_played: Arc::new(Mutex::new(Duration::from_secs(0))),
            current_song_runtime: Arc::new(Mutex::new(Duration::from_secs(0))),
            paused: Arc::new(AtomicBool::new(true)),
        }
    }

    /// this function initializes the audio kernel
    /// it is not meant to be called directly, use `AUDIO_KERNEL` instead to send command
    ///
    /// this is the version for tests, where we don't create the actual audio stream since we don't need to play audio
    ///
    /// # Panics
    ///
    /// panics if the tokio runtime cannot be created
    #[must_use]
    #[cfg(feature = "mock_playback")]
    pub fn new() -> Self {
        let (sink, mut queue_rx) = rodio::Sink::new_idle();

        // start a detached thread that continuously polls the queue_rx, until it receives a command to exit
        let (tx, rx) = tokio::sync::oneshot::channel();

        std::thread::spawn(move || {
            // basically, call rx.await and while it is waiting for a command, poll the queue_rx
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    tokio::select! {
                        _ = rx => {},
                        () = async {
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
            player: sink.into(),
            queue_rx_end_tx: tx,
            queue: Arc::new(Mutex::new(Queue::new())),
            volume: Arc::new(Mutex::new(1.0)),
            muted: AtomicBool::new(false),
            time_played: Arc::new(Mutex::new(Duration::from_secs(0))),
            current_song_runtime: Arc::new(Mutex::new(Duration::from_secs(0))),
            paused: Arc::new(AtomicBool::new(true)),
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
    /// let tx_clone = tx.clone();
    /// // spawn the audio kernel in a detached thread
    /// std::thread::spawn(move || {
    ///    let kernel = AudioKernel::new();
    ///    kernel.init(tx_clone, rx);
    /// });
    /// // create a sender to send commands to the audio kernel
    /// let sender = AudioKernelSender::new(tx);
    ///
    /// // send a command to the audio kernel
    /// // e.g. to shutdown the audio kernel:
    /// sender.send(AudioCommand::Exit);
    /// ```
    ///
    /// # Arguments
    ///
    /// * `tx` - the transmitter used to send commands to the audio kernel (this is used by the duration watcher to tell the kernel when to skip to the next song)
    /// * `rx` - the receiver used to receive commands from the audio kernel, this is what the audio kernel receives commands from
    ///
    /// # Panics
    ///
    /// The function may panic if one of the Mutexes is poisoned
    ///
    /// if the `mock_playback` feature is enabled, this function may panic if it is unable to signal the `queue_rx` thread to end.
    pub fn init(
        self,
        tx: Sender<(AudioCommand, tracing::Span)>,
        rx: Receiver<(AudioCommand, tracing::Span)>,
    ) {
        // duration watcher signalers
        let (dw_tx, dw_rx) = tokio::sync::oneshot::channel();

        // we won't be able to access this AudioKernel instance reliably, so we need to clone Arcs to all the values we need
        let time_played = self.time_played.clone();
        let current_song_runtime = self.current_song_runtime.clone();
        let paused = self.paused.clone();

        let _duration_water = std::thread::spawn(move || {
            let sleep_time = std::time::Duration::from_millis(DURATION_WATCHER_TICK_MS);
            let duration_threshold =
                std::time::Duration::from_millis(DURATION_WATCHER_NEXT_SONG_THRESHOLD_MS);

            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    log::info!("Duration Watcher started");
                    tokio::select! {
                        _ = dw_rx => {},
                        () = async {
                            loop {
                                tokio::time::sleep(sleep_time).await;
                                if !paused.load(std::sync::atomic::Ordering::Relaxed) {
                                    let mut time_played = time_played.lock().unwrap();
                                    // if we aren't paused, increment the time played
                                    *time_played += sleep_time;
                                    // if we're within the threshold of the end of the song, signal to the audio kernel to skip to the next song
                                    let current_song_runtime = current_song_runtime.lock().unwrap();
                                    if *time_played >= *current_song_runtime - duration_threshold {
                                        if let Err(e) = tx.send((AudioCommand::Queue(QueueCommand::SkipForward(1)), tracing::Span::current())) {
                                            error!("Failed to send command to audio kernel: {e}");
                                            panic!("Failed to send command to audio kernel: {e}");
                                        }
                                    }
                                }
                            }
                        } => {},
                    }
                });
        });

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
                AudioCommand::Seek(_seek, _duration) => todo!(),
            }
        }

        #[cfg(feature = "mock_playback")]
        self.queue_rx_end_tx.send(()).unwrap();
        dw_tx.send(()).unwrap();
    }

    #[instrument(skip(self))]
    fn play(&self) {
        self.player.play();
        self.paused
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    #[instrument(skip(self))]
    fn pause(&self) {
        self.player.pause();
        self.paused
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    #[instrument(skip(self))]
    fn toggle_playback(&self) {
        if self.player.is_paused() {
            self.play();
        } else {
            self.pause();
        }
    }

    #[instrument(skip(self))]
    fn restart_song(&self) {
        let paused = self.player.is_paused();
        self.clear_player();

        if let Some(song) = self.queue.lock().unwrap().current_song() {
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
        self.paused
            .store(true, std::sync::atomic::Ordering::Relaxed);
        *self.time_played.lock().unwrap() = Duration::from_secs(0);
        self.player.clear();
    }

    #[instrument(skip(self))]
    fn queue_control(&self, command: QueueCommand) {
        match command {
            QueueCommand::Clear => self.clear(),
            QueueCommand::SkipForward(n) => self.skip_forward(n),
            QueueCommand::SkipBackward(n) => self.skip_backward(n),
            QueueCommand::SetPosition(n) => self.set_position(n),
            QueueCommand::Shuffle => self.queue.lock().unwrap().shuffle(),
            QueueCommand::AddToQueue(song_box) => match *song_box {
                OneOrMany::None => {}
                OneOrMany::One(song) => self.add_song_to_queue(song),
                OneOrMany::Many(songs) => self.add_songs_to_queue(songs),
            },
            QueueCommand::RemoveRange(range) => self.remove_range_from_queue(range),
            QueueCommand::SetRepeatMode(mode) => self.queue.lock().unwrap().set_repeat_mode(mode),
        }
    }

    #[instrument(skip(self))]
    fn state(&self) -> StateAudio {
        let queue = self.queue.lock().unwrap();
        let queue_position = queue.current_index();
        let current_song = queue.current_song().cloned();
        let repeat_mode = queue.get_repeat_mode();
        let runtime = current_song.as_ref().map(|song| {
            let seek_position = *self.time_played.lock().unwrap();
            let duration = song.runtime;
            let seek_percent =
                Percent::new(seek_position.as_secs_f32() / duration.as_secs_f32() * 100.0);
            StateRuntime {
                seek_position,
                seek_percent,
                duration,
            }
        });
        let paused = self.player.is_paused();
        debug_assert_eq!(
            self.paused.load(std::sync::atomic::Ordering::Relaxed),
            paused
        );
        let muted = self.muted.load(std::sync::atomic::Ordering::Relaxed);
        let volume = *self.volume.lock().unwrap();

        let queued_songs = queue.queued_songs();
        drop(queue);

        StateAudio {
            queue: queued_songs,
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
        self.queue.lock().unwrap().clear();
    }

    #[instrument(skip(self))]
    fn skip_forward(&self, n: usize) {
        let paused = self.player.is_paused();
        self.clear_player();

        let next_song = self.queue.lock().unwrap().skip_forward(n).cloned();

        if let Some(song) = next_song {
            if let Err(e) = self.append_song_to_player(&song) {
                error!("Failed to append song to player: {}", e);
            }

            let binding = self.queue.lock().unwrap();
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

        let next_song = self.queue.lock().unwrap().skip_backward(n).cloned();

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

        let mut binding = self.queue.lock().unwrap();
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
        self.queue.lock().unwrap().add_song(song);

        // if the player is empty, start playback
        if self.player.empty() {
            let current_index = self.queue.lock().unwrap().current_index();

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
        self.queue.lock().unwrap().add_songs(songs);

        // if the player is empty, start playback
        if self.player.empty() {
            let current_index = self.queue.lock().unwrap().current_index();

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
    fn remove_range_from_queue(&self, range: Range<usize>) {
        let paused = if self.player.is_paused() {
            true
        } else if let Some(current_index) = self.queue.lock().unwrap().current_index() {
            // still true if the current song is to be removed
            range.start <= current_index && range.end > current_index
        } else {
            false
        };
        self.clear_player();

        self.queue.lock().unwrap().remove_range(range);

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
        self.queue.lock().unwrap().current_song().cloned()
    }

    #[instrument(skip(self))]
    fn get_next_song(&self) -> Option<Song> {
        self.queue.lock().unwrap().next_song().cloned()
    }

    #[instrument(skip(self, source))]
    fn append_to_player<T>(&self, source: T)
    where
        T: Source<Item = f32> + Send + 'static,
    {
        if let Some(duration) = source.total_duration() {
            *self.current_song_runtime.lock().unwrap() = duration;
        }
        self.player.append(source);
    }

    #[instrument(skip(self))]
    fn append_song_to_player(&self, song: &Song) -> Result<(), LibraryError> {
        let source = Decoder::new(BufReader::new(File::open(&song.path)?))?.convert_samples();
        *self.current_song_runtime.lock().unwrap() = song.runtime;
        self.append_to_player(source);

        Ok(())
    }

    #[instrument(skip(self))]
    fn volume_control(&self, command: VolumeCommand) {
        match command {
            VolumeCommand::Up(percent) => {
                *self.volume.lock().unwrap() += percent;
            }
            VolumeCommand::Down(percent) => {
                *self.volume.lock().unwrap() -= percent;
            }
            VolumeCommand::Set(percent) => {
                *self.volume.lock().unwrap() = percent;
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
            self.player
                .set_volume(self.volume.lock().unwrap().to_owned());
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
    use pretty_assertions::assert_eq;
    use rstest::{fixture, rstest};

    use crate::test_utils::init;

    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    #[fixture]
    fn audio_kernel() -> AudioKernel {
        AudioKernel::default()
    }

    #[fixture]
    fn audio_kernel_sender() -> AudioKernelSender {
        let (tx, rx) = mpsc::channel();
        let tx_clone = tx.clone();
        thread::spawn(move || {
            let kernel = AudioKernel::new();
            kernel.init(tx_clone, rx);
        });

        AudioKernelSender::new(tx)
    }

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
        assert_eq!(*audio_kernel.volume.lock().unwrap(), 1.1);

        audio_kernel.volume_control(VolumeCommand::Down(0.1));
        assert_eq!(*audio_kernel.volume.lock().unwrap(), 1.0);

        audio_kernel.volume_control(VolumeCommand::Set(0.5));
        assert_eq!(*audio_kernel.volume.lock().unwrap(), 0.5);

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

        use mecomp_storage::test_utils::{arb_song_case, create_song_metadata, init_test_database};
        use pretty_assertions::assert_eq;
        use rstest::rstest;

        use crate::test_utils::init;

        use super::{super::*, audio_kernel, audio_kernel_sender, get_state, sound};

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
            let tempdir = tempfile::tempdir().unwrap();

            let song = Song::try_load_into_db(
                &db,
                create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
            )
            .await
            .unwrap();

            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(
                OneOrMany::One(song.clone()),
            ))));

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
            let tempdir = tempfile::tempdir().unwrap();

            let state = audio_kernel.state();
            assert_eq!(state.queue_position, None);
            assert!(state.paused);

            audio_kernel.queue_control(QueueCommand::AddToQueue(Box::new(OneOrMany::Many(vec![
                Song::try_load_into_db(
                    &db,
                    create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
                )
                .await
                .unwrap(),
                Song::try_load_into_db(
                    &db,
                    create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
                )
                .await
                .unwrap(),
                Song::try_load_into_db(
                    &db,
                    create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
                )
                .await
                .unwrap(),
            ]))));

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
            let tempdir = tempfile::tempdir().unwrap();

            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert!(state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(
                OneOrMany::Many(vec![
                    Song::try_load_into_db(
                        &db,
                        create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
                    )
                    .await
                    .unwrap(),
                    Song::try_load_into_db(
                        &db,
                        create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
                    )
                    .await
                    .unwrap(),
                    Song::try_load_into_db(
                        &db,
                        create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
                    )
                    .await
                    .unwrap(),
                ]),
            ))));
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
            let tempdir = tempfile::tempdir().unwrap();
            let song1 = Song::try_load_into_db(
                &db,
                create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
            )
            .await
            .unwrap();
            let song2 = Song::try_load_into_db(
                &db,
                create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
            )
            .await
            .unwrap();

            // add songs to the queue, starts playback
            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(
                OneOrMany::Many(vec![song1.clone(), song2.clone()]),
            ))));
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
            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(
                OneOrMany::One(song1.clone()),
            ))));
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
            let tempdir = tempfile::tempdir().unwrap();

            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert!(state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(
                OneOrMany::Many(vec![
                    Song::try_load_into_db(
                        &db,
                        create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
                    )
                    .await
                    .unwrap(),
                    Song::try_load_into_db(
                        &db,
                        create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
                    )
                    .await
                    .unwrap(),
                    Song::try_load_into_db(
                        &db,
                        create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
                    )
                    .await
                    .unwrap(),
                ]),
            ))));

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
            let tempdir = tempfile::tempdir().unwrap();

            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert!(state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(
                OneOrMany::Many(vec![
                    Song::try_load_into_db(
                        &db,
                        create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
                    )
                    .await
                    .unwrap(),
                    Song::try_load_into_db(
                        &db,
                        create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
                    )
                    .await
                    .unwrap(),
                    Song::try_load_into_db(
                        &db,
                        create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
                    )
                    .await
                    .unwrap(),
                ]),
            ))));
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
            let tempdir = tempfile::tempdir().unwrap();

            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert!(state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(
                OneOrMany::Many(vec![
                    Song::try_load_into_db(
                        &db,
                        create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
                    )
                    .await
                    .unwrap(),
                    Song::try_load_into_db(
                        &db,
                        create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
                    )
                    .await
                    .unwrap(),
                    Song::try_load_into_db(
                        &db,
                        create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
                    )
                    .await
                    .unwrap(),
                ]),
            ))));
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
            let tempdir = tempfile::tempdir().unwrap();

            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert!(state.paused);

            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(
                OneOrMany::Many(vec![
                    Song::try_load_into_db(
                        &db,
                        create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
                    )
                    .await
                    .unwrap(),
                    Song::try_load_into_db(
                        &db,
                        create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
                    )
                    .await
                    .unwrap(),
                    Song::try_load_into_db(
                        &db,
                        create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
                    )
                    .await
                    .unwrap(),
                ]),
            ))));
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
