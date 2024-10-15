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

const DURATION_WATCHER_TICK_MS: u64 = 50;
const DURATION_WATCHER_NEXT_SONG_THRESHOLD_MS: u64 = 100;

/// The minimum volume that can be set, currently set to 0.0 (no sound)
const MIN_VOLUME: f32 = 0.0;
/// The maximum volume that can be set, currently set to 10.0 (10x volume)
const MAX_VOLUME: f32 = 10.0;

#[derive(Debug, Clone)]
pub struct AudioKernelSender {
    tx: Sender<(AudioCommand, tracing::Span)>,
}

impl AudioKernelSender {
    /// Starts the audio kernel in a detached thread and returns a sender to be used to send commands to the audio kernel.
    ///
    /// # Returns
    ///
    /// A sender to be used to send commands to the audio kernel.
    ///
    /// # Panics
    ///
    /// Panics if there is an issue spawning the audio kernel thread (if the name contains null bytes, which it doesn't, so this should never happen)
    #[must_use]
    pub fn start() -> Arc<Self> {
        let (tx, rx) = std::sync::mpsc::channel();
        let tx_clone = tx.clone();
        std::thread::Builder::new()
            .name(String::from("Audio Kernel"))
            .spawn(move || {
                let kernel = AudioKernel::new();
                kernel.init(tx_clone, rx);
            })
            .unwrap();
        Arc::new(Self::new(tx))
    }

    #[must_use]
    pub(crate) const fn new(tx: Sender<(AudioCommand, tracing::Span)>) -> Self {
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
struct DurationInfo {
    time_played: Duration,
    current_duration: Duration,
}

pub(crate) struct AudioKernel {
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
    muted: Arc<AtomicBool>,
    /// the current song duration and the time played
    duration_info: Arc<Mutex<DurationInfo>>,
    /// whether the audio kernel is paused
    paused: Arc<AtomicBool>,
}

impl AudioKernel {
    /// this function initializes the audio kernel
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
            muted: Arc::new(AtomicBool::new(false)),
            duration_info: Arc::new(Mutex::new(DurationInfo::default())),
            paused: Arc::new(AtomicBool::new(true)),
        }
    }

    /// this function initializes the audio kernel
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
                .enable_time()
                .build()
                .unwrap()
                .block_on(async {
                    tokio::select! {
                        _ = rx => {},
                        () = async {
                            loop {
                                queue_rx.next();
                                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
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
            muted: Arc::new(AtomicBool::new(false)),
            duration_info: Arc::new(Mutex::new(DurationInfo::default())),
            paused: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Spawn the audio kernel, taking ownership of self
    ///
    /// this function should be called in a detached thread to keep the audio kernel running,
    /// this function will block until the `Exit` command is received
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
        let duration_info = self.duration_info.clone();
        let paused = self.paused.clone();

        // NOTE: as of rodio v0.19.0, we have access to the `get_pos` command, which allows us to get the current position of the audio stream
        // it may seem like this means we don't need to have a duration watcher, but the key point is that we need to know when to skip to the next song
        // the duration watcher both tracks the duration of the song, and skips to the next song when the song is over
        let _duration_watcher = std::thread::Builder::new().name(String::from("Duration Watcher")).spawn(move || {
            let sleep_time = std::time::Duration::from_millis(DURATION_WATCHER_TICK_MS);
            let duration_threshold =
                std::time::Duration::from_millis(DURATION_WATCHER_NEXT_SONG_THRESHOLD_MS);

            tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .unwrap()
                .block_on(async {
                    log::info!("Duration Watcher started");
                    tokio::select! {
                        _ = dw_rx => {},
                        () = async {
                            loop {
                                tokio::time::sleep(sleep_time).await;
                                let mut duration_info = duration_info.lock().unwrap();
                                if !paused.load(std::sync::atomic::Ordering::Relaxed) {
                                    // if we aren't paused, increment the time played
                                    duration_info.time_played += sleep_time;
                                    // if we're within the threshold of the end of the song, signal to the audio kernel to skip to the next song
                                    if duration_info.time_played >= duration_info.current_duration.saturating_sub(duration_threshold) {
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
                AudioCommand::Seek(seek, duration) => self.seek(seek, duration),
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
    fn clear(&self) {
        self.clear_player();
        self.queue.lock().unwrap().clear();
    }

    #[instrument(skip(self))]
    fn clear_player(&self) {
        self.player.clear();
        self.paused
            .store(true, std::sync::atomic::Ordering::Relaxed);
        *self.duration_info.lock().unwrap() = DurationInfo::default();
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
        let runtime = current_song.as_ref().map(|_| {
            let duration_info = self.duration_info.lock().unwrap();
            let seek_position = duration_info.time_played;
            let duration = duration_info.current_duration;
            drop(duration_info);
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
        let paused = self.player.is_paused();
        // if the current song is not being removed, we don't need to do anything special to the player
        let current_to_be_removed = self
            .queue
            .lock()
            .unwrap()
            .current_index()
            .map_or(false, |current_index| range.contains(&current_index));

        self.queue.lock().unwrap().remove_range(range);

        // if the current song was removed, clear the player and restart playback
        if current_to_be_removed {
            self.clear_player();
            if let Some(song) = self.get_current_song() {
                if let Err(e) = self.append_song_to_player(&song) {
                    error!("Failed to append song to player: {e}");
                }
                if !paused {
                    self.play();
                }
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
            *self.duration_info.lock().unwrap() = DurationInfo {
                time_played: Duration::from_secs(0),
                current_duration: duration,
            };
        }
        self.player.append(source);
    }

    #[instrument(skip(self))]
    fn append_song_to_player(&self, song: &Song) -> Result<(), LibraryError> {
        let source = Decoder::new(BufReader::new(File::open(&song.path)?))?.convert_samples();
        *self.duration_info.lock().unwrap() = DurationInfo {
            time_played: Duration::from_secs(0),
            current_duration: song.runtime,
        };
        self.append_to_player(source);

        Ok(())
    }

    #[instrument(skip(self))]
    fn volume_control(&self, command: VolumeCommand) {
        match command {
            VolumeCommand::Up(percent) => {
                let mut volume = self.volume.lock().unwrap();
                *volume = (*volume + percent).clamp(MIN_VOLUME, MAX_VOLUME);
            }
            VolumeCommand::Down(percent) => {
                let mut volume = self.volume.lock().unwrap();
                *volume = (*volume - percent).clamp(MIN_VOLUME, MAX_VOLUME);
            }
            VolumeCommand::Set(percent) => {
                let mut volume = self.volume.lock().unwrap();
                *volume = percent.clamp(MIN_VOLUME, MAX_VOLUME);
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

    #[instrument(skip(self))]
    fn seek(&self, seek: SeekType, duration: Duration) {
        // get a lock on the current song duration and time played
        let mut duration_info = self.duration_info.lock().unwrap();
        // calculate the new time based on the seek type
        let new_time = match seek {
            SeekType::Absolute => duration,
            SeekType::RelativeForwards => duration_info.time_played.saturating_add(duration),
            SeekType::RelativeBackwards => duration_info.time_played.saturating_sub(duration),
        };
        let new_time = if new_time > duration_info.current_duration {
            duration_info.current_duration
        } else if new_time < Duration::from_secs(0) {
            Duration::from_secs(0)
        } else {
            new_time
        };

        // try to seek to the new time.
        // if the seek fails, log the error and continue
        // if the seek succeeds, update the time_played to the new time
        match self.player.try_seek(new_time) {
            Ok(()) => {
                debug!("Seek to {} successful", format_duration(&new_time));
                duration_info.time_played = new_time;
                drop(duration_info);
            }
            Err(SeekError::NotSupported { underlying_source }) => {
                error!("Seek not supported by source: {underlying_source}");
            }
            Err(err) => {
                error!("Seeking failed with error: {err}");
            }
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
    use std::time::Duration;

    #[fixture]
    fn audio_kernel() -> AudioKernel {
        AudioKernel::default()
    }

    #[fixture]
    fn audio_kernel_sender() -> Arc<AudioKernelSender> {
        AudioKernelSender::start()
    }

    async fn get_state(sender: Arc<AudioKernelSender>) -> StateAudio {
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
        #[from(audio_kernel_sender)] sender: Arc<AudioKernelSender>,
    ) {
        init();

        sender.send(AudioCommand::Exit);
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
            #[from(audio_kernel_sender)] sender: Arc<AudioKernelSender>,
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
        #[timeout(Duration::from_secs(6))] // if the test takes longer than this, the test can be considered a failure
        #[tokio::test]
        async fn test_audio_kernel_skip_forward_sender(
            #[from(audio_kernel_sender)] sender: Arc<AudioKernelSender>,
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
            #[from(audio_kernel_sender)] sender: Arc<AudioKernelSender>,
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

            // pause the player
            sender.send(AudioCommand::Pause);

            // remove the current song from the queue, the player should still be paused
            sender.send(AudioCommand::Queue(QueueCommand::RemoveRange(0..1)));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert!(state.paused);
            assert_eq!(state.queue.len(), 1);
            assert_eq!(state.queue[0], song2);

            // unpause the player
            sender.send(AudioCommand::Play);

            // add the song back to the queue, should be playing
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
        #[timeout(Duration::from_secs(10))] // if the test takes longer than this, the test can be considered a failure
        #[tokio::test]
        async fn test_audio_kernel_skip_backward(
            #[from(audio_kernel_sender)] sender: Arc<AudioKernelSender>,
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
        #[timeout(Duration::from_secs(10))] // if the test takes longer than this, the test can be considered a failure
        #[tokio::test]
        async fn test_audio_kernel_set_position(
            #[from(audio_kernel_sender)] sender: Arc<AudioKernelSender>,
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
        async fn test_audio_kernel_clear(
            #[from(audio_kernel_sender)] sender: Arc<AudioKernelSender>,
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
        async fn test_audio_kernel_shuffle(
            #[from(audio_kernel_sender)] sender: Arc<AudioKernelSender>,
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
        async fn test_volume_commands(#[from(audio_kernel_sender)] sender: Arc<AudioKernelSender>) {
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

        #[rstest]
        #[timeout(Duration::from_secs(5))] // if the test takes longer than this, the test can be considered a failure
        #[tokio::test]
        async fn test_volume_out_of_bounds(
            #[from(audio_kernel_sender)] sender: Arc<AudioKernelSender>,
        ) {
            init();

            // try moving volume above/below the maximum/minimum
            sender.send(AudioCommand::Volume(VolumeCommand::Up(MAX_VOLUME + 0.5)));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.volume, MAX_VOLUME);
            assert!(!state.muted);
            sender.send(AudioCommand::Volume(VolumeCommand::Down(
                MAX_VOLUME + 0.5 - MIN_VOLUME,
            )));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.volume, MIN_VOLUME);
            assert!(!state.muted);

            // try setting volume above/below the maximum/minimum
            sender.send(AudioCommand::Volume(VolumeCommand::Set(MAX_VOLUME + 0.5)));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.volume, MAX_VOLUME);
            assert!(!state.muted);
            sender.send(AudioCommand::Volume(VolumeCommand::Set(MIN_VOLUME - 0.5)));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.volume, MIN_VOLUME);
            assert!(!state.muted);

            sender.send(AudioCommand::Exit);
        }

        #[rstest]
        #[timeout(Duration::from_secs(8))] // if the test takes longer than this, the test can be considered a failure
        #[tokio::test]
        async fn test_seek_commands(#[from(audio_kernel_sender)] sender: Arc<AudioKernelSender>) {
            init();
            let db = init_test_database().await.unwrap();
            let tempdir = tempfile::tempdir().unwrap();

            let song = Song::try_load_into_db(
                &db,
                create_song_metadata(&tempdir, arb_song_case()()).unwrap(),
            )
            .await
            .unwrap();

            // add a song to the queue
            // NOTE: this song has a duration of 10 seconds
            sender.send(AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(
                OneOrMany::One(song.clone()),
            ))));
            sender.send(AudioCommand::Pause);
            let state: StateAudio = get_state(sender.clone()).await;
            sender.send(AudioCommand::Seek(
                SeekType::Absolute,
                Duration::from_secs(0),
            ));
            assert_eq!(state.queue_position, Some(0));
            assert!(state.paused);
            assert_eq!(
                state.runtime.unwrap().duration,
                Duration::from_secs(10) + Duration::from_nanos(6)
            );

            // skip ahead a bit
            sender.send(AudioCommand::Seek(
                SeekType::RelativeForwards,
                Duration::from_secs(2),
            ));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.runtime.unwrap().seek_position, Duration::from_secs(2));
            assert_eq!(state.current_song, Some(song.clone()));
            assert!(state.paused);

            // skip back a bit
            sender.send(AudioCommand::Seek(
                SeekType::RelativeBackwards,
                Duration::from_secs(1),
            ));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.runtime.unwrap().seek_position, Duration::from_secs(1));
            assert_eq!(state.current_song, Some(song.clone()));
            assert!(state.paused);

            // skip to 9 seconds
            sender.send(AudioCommand::Seek(
                SeekType::Absolute,
                Duration::from_secs(9),
            ));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.runtime.unwrap().seek_position, Duration::from_secs(9));
            assert_eq!(state.current_song, Some(song.clone()));
            assert!(state.paused);

            // now we unpause, wait a bit, and check that the song has ended
            sender.send(AudioCommand::Play);
            tokio::time::sleep(Duration::from_millis(1001)).await;
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert!(state.paused);

            sender.send(AudioCommand::Exit);
        }
    }
}
