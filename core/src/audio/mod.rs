#![allow(clippy::module_name_repetitions)]

use std::{
    fs::File,
    io::BufReader,
    ops::Range,
    sync::{
        Arc, Mutex,
        atomic::AtomicBool,
        mpsc::{Receiver, Sender},
    },
    time::Duration,
};

use log::{debug, error};
use rodio::{
    Decoder, Source,
    source::{EmptyCallback, SeekError},
};
use tracing::instrument;

use crate::{
    errors::LibraryError,
    format_duration,
    state::{Percent, SeekType, StateAudio, StateRuntime, Status},
    udp::StateChange,
};
use mecomp_storage::db::schemas::song::Song;
use one_or_many::OneOrMany;

pub mod commands;
pub mod queue;

use commands::{AudioCommand, QueueCommand, VolumeCommand};
use queue::Queue;

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
    /// The audio kernel will transmit state changes to the provided event transmitter.
    ///
    /// # Returns
    ///
    /// A sender to be used to send commands to the audio kernel.
    ///
    /// # Panics
    ///
    /// Panics if there is an issue spawning the audio kernel thread (if the name contains null bytes, which it doesn't, so this should never happen)
    #[must_use]
    #[inline]
    pub fn start(event_tx: Sender<StateChange>) -> Arc<Self> {
        let (command_tx, command_rx) = std::sync::mpsc::channel();
        let tx_clone = command_tx.clone();
        std::thread::Builder::new()
            .name(String::from("Audio Kernel"))
            .spawn(move || {
                let kernel = AudioKernel::new(tx_clone, command_rx, event_tx);
                kernel.init();
            })
            .unwrap();
        Arc::new(Self::new(command_tx))
    }

    #[must_use]
    #[inline]
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
    /// The value `1.0` is the "normal" volume (unfiltered input).
    /// Any value other than `1.0` will multiply each sample by this value.
    volume: Arc<Mutex<f32>>,
    /// whether the audio is muted
    muted: Arc<AtomicBool>,
    /// whether the audio kernel is paused, playlist, or stopped
    status: Arc<Mutex<Status>>,
    /// Channel that the audio kernel might use to send `AudioCommand`'s
    /// to itself over (e.g., in a callback)
    command_tx: Sender<(AudioCommand, tracing::Span)>,
    /// Channel that the audio kernel recieves `AudioCommand`'s over
    command_rx: Receiver<(AudioCommand, tracing::Span)>,
    /// Event publisher for when the audio kernel changes state
    event_tx: Sender<StateChange>,
}

impl AudioKernel {
    /// this function initializes the audio kernel
    ///
    /// # Panics
    ///
    /// panics if the rodio stream cannot be created
    #[must_use]
    #[cfg(not(feature = "mock_playback"))]
    pub fn new(
        command_tx: Sender<(AudioCommand, tracing::Span)>,
        command_rx: Receiver<(AudioCommand, tracing::Span)>,
        event_tx: Sender<StateChange>,
    ) -> Self {
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
            status: Arc::new(Mutex::new(Status::Stopped)),
            command_tx,
            command_rx,
            event_tx,
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
    pub fn new(
        command_tx: Sender<(AudioCommand, tracing::Span)>,
        command_rx: Receiver<(AudioCommand, tracing::Span)>,
        event_tx: Sender<StateChange>,
    ) -> Self {
        // most of the tests are playing the `assets/music.mp3` file, which is sampled at 44.1kHz
        // thus, we should poll the queue every 22 microseconds
        const QUEUE_POLLING_INTERVAL: Duration = Duration::from_micros(22);

        let (sink, mut queue_rx) = rodio::Sink::new_idle();

        // start a detached thread that continuously polls the queue_rx, until it receives a command to exit
        let (tx, mut rx) = tokio::sync::oneshot::channel();

        std::thread::spawn(move || {
            // basically, call rx.await and while it is waiting for a command, poll the queue_rx
            loop {
                if matches!(
                    rx.try_recv(),
                    Ok(()) | Err(tokio::sync::oneshot::error::TryRecvError::Closed)
                ) {
                    break;
                }
                queue_rx.next();
                std::thread::sleep(QUEUE_POLLING_INTERVAL);
            }
        });

        sink.pause();

        Self {
            player: sink.into(),
            queue_rx_end_tx: tx,
            queue: Arc::new(Mutex::new(Queue::new())),
            volume: Arc::new(Mutex::new(1.0)),
            muted: Arc::new(AtomicBool::new(false)),
            status: Arc::new(Mutex::new(Status::Stopped)),
            command_tx,
            command_rx,
            event_tx,
        }
    }

    /// Spawn the audio kernel, taking ownership of self
    ///
    /// this function should be called in a detached thread to keep the audio kernel running,
    /// this function will block until the `Exit` command is received
    ///
    /// # Panics
    ///
    /// The function may panic if one of the Mutexes is poisoned
    ///
    /// if the `mock_playback` feature is enabled, this function may panic if it is unable to signal the `queue_rx` thread to end.
    pub fn init(self) {
        for (command, ctx) in &self.command_rx {
            let _guard = ctx.enter();

            let prev_status = *self.status.lock().unwrap();

            match command {
                AudioCommand::Play => self.play(),
                AudioCommand::Pause => self.pause(),
                AudioCommand::TogglePlayback => self.toggle_playback(),
                AudioCommand::RestartSong => {
                    self.restart_song();
                    let _ = self
                        .event_tx
                        .send(StateChange::Seeked(Duration::from_secs(0)));
                }
                AudioCommand::ClearPlayer => self.clear_player(),
                AudioCommand::Queue(command) => self.queue_control(command),
                AudioCommand::Exit => break,
                AudioCommand::ReportStatus(tx) => {
                    let state = self.state();

                    if let Err(e) = tx.send(state) {
                        // if there was an error, then the receiver will never receive the state,
                        // this can cause a permanent hang
                        // so we stop the audio kernel if this happens
                        // (which will cause any future calls to `send` to panic)
                        error!(
                            "Audio Kernel failed to send state to the receiver, state receiver likely has been dropped. State: {e}"
                        );
                        break;
                    }
                }
                AudioCommand::Volume(command) => self.volume_control(command),
                AudioCommand::Seek(seek, duration) => {
                    self.seek(seek, duration);
                    let _ = self
                        .event_tx
                        .send(StateChange::Seeked(self.get_time_played()));
                }
                AudioCommand::Stop if prev_status != Status::Stopped => {
                    self.stop();
                    let _ = self
                        .event_tx
                        .send(StateChange::Seeked(Duration::from_secs(0)));
                }
                AudioCommand::Stop => {}
            }

            let new_status = *self.status.lock().unwrap();

            if prev_status != new_status {
                let _ = self.event_tx.send(StateChange::StatusChanged(new_status));
            }
        }

        #[cfg(feature = "mock_playback")]
        self.queue_rx_end_tx.send(()).unwrap();
    }

    #[instrument(skip(self))]
    fn play(&self) {
        if self.player.empty() {
            return;
        }
        self.player.play();
        *self.status.lock().unwrap() = Status::Playing;
    }

    #[instrument(skip(self))]
    fn pause(&self) {
        self.player.pause();
        *self.status.lock().unwrap() = Status::Paused;
    }

    #[instrument(skip(self))]
    fn stop(&self) {
        self.player.pause();
        self.seek(SeekType::Absolute, Duration::from_secs(0));
        *self.status.lock().unwrap() = Status::Stopped;
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
        let status = *self.status.lock().unwrap();
        self.clear_player();

        if let Some(song) = self.queue.lock().unwrap().current_song() {
            if let Err(e) = self.append_song_to_player(song) {
                error!("Failed to append song to player: {e}");
            }

            match status {
                // if it was previously stopped, we don't need to do anything here
                Status::Stopped => {}
                // if it was previously paused, we need to re-pause
                Status::Paused => self.pause(),
                // if it was previously playing, we need to play
                Status::Playing => self.play(),
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
        *self.status.lock().unwrap() = Status::Stopped;
    }

    #[instrument(skip(self))]
    fn queue_control(&self, command: QueueCommand) {
        let prev_song = self.queue.lock().unwrap().current_song().cloned();
        match command {
            QueueCommand::Clear => self.clear(),
            QueueCommand::PlayNextSong => self.start_next_song(),
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
            QueueCommand::SetRepeatMode(mode) => {
                self.queue.lock().unwrap().set_repeat_mode(mode);
                let _ = self.event_tx.send(StateChange::RepeatModeChanged(mode));
            }
        }

        let new_song = self.queue.lock().unwrap().current_song().cloned();

        if prev_song != new_song {
            let _ = self
                .event_tx
                .send(StateChange::TrackChanged(new_song.map(|s| s.id.into())));
        }
    }

    #[instrument(skip(self))]
    fn state(&self) -> StateAudio {
        let queue = self.queue.lock().unwrap();
        let queue_position = queue.current_index();
        let current_song = queue.current_song().cloned();
        let repeat_mode = queue.get_repeat_mode();
        let runtime = current_song.as_ref().map(|song| {
            let duration = song.runtime;
            let seek_position = self.get_time_played();
            let seek_percent =
                Percent::new(seek_position.as_secs_f32() / duration.as_secs_f32() * 100.0);
            StateRuntime {
                seek_position,
                seek_percent,
                duration,
            }
        });
        let status = *self.status.lock().unwrap();
        let status = if self.player.is_paused() {
            debug_assert!(matches!(status, Status::Paused | Status::Stopped));
            status
        } else {
            debug_assert_eq!(status, Status::Playing);
            Status::Playing
        };

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
            status,
            muted,
            volume,
        }
    }

    #[instrument(skip(self))]
    fn start_next_song(&self) {
        *self.status.lock().unwrap() = Status::Stopped;
        // we need to explicitly pause the player since
        // there is technically something in the sink right now
        // (the `EndCallback`), so it won't pause itself
        self.player.pause();

        let mut queue = self.queue.lock().unwrap();
        let next_song = queue.next_song().cloned();
        let repeat_mode = queue.get_repeat_mode();
        let current_index = queue.current_index();
        drop(queue);

        if let Some(song) = next_song {
            if let Err(e) = self.append_song_to_player(&song) {
                error!("Failed to append song to player: {e}");
            }

            // we have not just finished the queue
            // (this makes it so if we hit the end of the queue on RepeatMode::None, we don't start playing again)
            if current_index.is_some() || repeat_mode.is_all() {
                self.play();
            }
        }
    }

    #[instrument(skip(self))]
    fn skip_forward(&self, n: usize) {
        let status = *self.status.lock().unwrap();
        self.clear_player();

        let next_song = self.queue.lock().unwrap().skip_forward(n).cloned();

        if let Some(song) = next_song {
            if let Err(e) = self.append_song_to_player(&song) {
                error!("Failed to append song to player: {e}");
            }

            let binding = self.queue.lock().unwrap();
            match status {
                Status::Paused => self.pause(),
                // we were playing and we have not just finished the queue
                // (this makes it so if we hit the end of the queue on RepeatMode::None, we don't start playing again)
                Status::Playing
                    if binding.get_repeat_mode().is_all() || binding.current_index().is_some() =>
                {
                    self.play();
                }
                _ => {}
            }
        }
    }

    #[instrument(skip(self))]
    fn skip_backward(&self, n: usize) {
        let status = *self.status.lock().unwrap();
        self.clear_player();

        let next_song = self.queue.lock().unwrap().skip_backward(n).cloned();

        if let Some(song) = next_song {
            if let Err(e) = self.append_song_to_player(&song) {
                error!("Failed to append song to player: {e}");
            }
            match status {
                Status::Stopped => {}
                Status::Paused => self.pause(),
                Status::Playing => self.play(),
            }
        }
    }

    #[instrument(skip(self))]
    fn set_position(&self, n: usize) {
        let status = *self.status.lock().unwrap();
        self.clear_player();

        let mut binding = self.queue.lock().unwrap();
        binding.set_current_index(n);
        let next_song = binding.current_song().cloned();
        drop(binding);

        if let Some(song) = next_song {
            if let Err(e) = self.append_song_to_player(&song) {
                error!("Failed to append song to player: {e}");
            }

            match status {
                Status::Stopped => {}
                Status::Paused => self.pause(),
                Status::Playing => self.play(),
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
        // if the current song is not being removed, we don't need to do anything special to the player
        let current_to_be_removed = self
            .queue
            .lock()
            .unwrap()
            .current_index()
            .is_some_and(|current_index| range.contains(&current_index));

        self.queue.lock().unwrap().remove_range(range);

        // if the current song was removed, clear the player and restart playback
        if current_to_be_removed {
            self.clear_player();
            if let Some(song) = self.get_current_song() {
                if let Err(e) = self.append_song_to_player(&song) {
                    error!("Failed to append song to player: {e}");
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

    fn get_time_played(&self) -> Duration {
        self.player.get_pos()
    }

    #[instrument(skip(self, source))]
    fn append_to_player<T>(&self, source: T)
    where
        T: Source<Item = f32> + Send + 'static,
    {
        self.player.append(source);

        // establish a callback for starting the next song once the current one finishes
        let command_tx = self.command_tx.clone();
        self.player
            .append(EmptyCallback::<f32>::new(Box::new(move || {
                log::debug!("Song finished");
                if let Err(e) = command_tx.send((
                    AudioCommand::Queue(QueueCommand::PlayNextSong),
                    tracing::Span::current(),
                )) {
                    error!("Failed to send command to audio kernel: {e}");
                } else {
                    log::debug!("Sent PlayNextSong command to audio kernel");
                }
            })));
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
                let mut volume = self.volume.lock().unwrap();
                let percent = (*volume + percent).clamp(MIN_VOLUME, MAX_VOLUME);
                if (*volume - percent).abs() > 0.0001 {
                    *volume = percent;
                    let _ = self.event_tx.send(StateChange::VolumeChanged(*volume));
                }
            }
            VolumeCommand::Down(percent) => {
                let mut volume = self.volume.lock().unwrap();
                let percent = (*volume - percent).clamp(MIN_VOLUME, MAX_VOLUME);
                if (*volume - percent).abs() > 0.0001 {
                    *volume = percent;
                    let _ = self.event_tx.send(StateChange::VolumeChanged(*volume));
                }
            }
            VolumeCommand::Set(percent) => {
                let mut volume = self.volume.lock().unwrap();
                let percent = percent.clamp(MIN_VOLUME, MAX_VOLUME);
                if (*volume - percent).abs() > 0.0001 {
                    *volume = percent;
                    let _ = self.event_tx.send(StateChange::VolumeChanged(*volume));
                }
            }
            VolumeCommand::Mute => {
                self.muted.store(true, std::sync::atomic::Ordering::Relaxed);
                let _ = self.event_tx.send(StateChange::Muted);
            }
            VolumeCommand::Unmute => {
                self.muted
                    .store(false, std::sync::atomic::Ordering::Relaxed);
                let _ = self.event_tx.send(StateChange::Unmuted);
            }
            VolumeCommand::ToggleMute => {
                self.muted.store(
                    !self.muted.load(std::sync::atomic::Ordering::Relaxed),
                    std::sync::atomic::Ordering::Relaxed,
                );
                if self.muted.load(std::sync::atomic::Ordering::Relaxed) {
                    let _ = self.event_tx.send(StateChange::Muted);
                } else {
                    let _ = self.event_tx.send(StateChange::Unmuted);
                }
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
        // calculate the new time based on the seek type
        let new_time = match seek {
            SeekType::Absolute => duration,
            SeekType::RelativeForwards => self.get_time_played().saturating_add(duration),
            SeekType::RelativeBackwards => self.get_time_played().saturating_sub(duration),
        };

        // try to seek to the new time.
        // if the seek fails, log the error and continue
        // if the seek succeeds, update the time_played to the new time
        match self.player.try_seek(new_time) {
            Ok(()) => {
                debug!("Seek to {} successful", format_duration(&new_time));
                let mut status = self.status.lock().unwrap();
                if new_time > Duration::from_secs(0) && *status == Status::Stopped {
                    *status = Status::Paused;
                    drop(status);
                }
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
        // channel for commands
        let (tx, rx) = mpsc::channel();
        // channel for events
        let (event_tx, _) = mpsc::channel();
        AudioKernel::new(tx, rx, event_tx)
    }

    #[fixture]
    fn audio_kernel_sender() -> Arc<AudioKernelSender> {
        // channel for events
        let (tx, _) = mpsc::channel();
        AudioKernelSender::start(tx)
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
    #[should_panic = "Failed to send command to audio kernel: sending on a closed channel"]
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
        let volume = *audio_kernel.volume.lock().unwrap();
        assert!(f32::EPSILON > (volume - 1.1).abs(), "{volume} != 1.1");

        audio_kernel.volume_control(VolumeCommand::Down(0.1));
        let volume = *audio_kernel.volume.lock().unwrap();
        assert!(f32::EPSILON > (volume - 1.0).abs(), "{volume} != 1.0");

        audio_kernel.volume_control(VolumeCommand::Set(0.5));
        let volume = *audio_kernel.volume.lock().unwrap();
        assert!(f32::EPSILON > (volume - 0.5).abs(), "{volume} != 0.5");

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
            assert_eq!(state.status, Status::Playing);

            sender.send(AudioCommand::Pause);
            let state = get_state(sender.clone()).await;
            assert_eq!(state.status, Status::Paused);

            sender.send(AudioCommand::Play);
            let state = get_state(sender.clone()).await;
            assert_eq!(state.status, Status::Playing);

            sender.send(AudioCommand::RestartSong);
            let state = get_state(sender.clone()).await;
            assert_eq!(state.status, Status::Playing); // Note, unlike adding a song to the queue, RestartSong does not affect whether the player is paused

            sender.send(AudioCommand::TogglePlayback);
            let state = get_state(sender.clone()).await;
            assert_eq!(state.status, Status::Paused);

            sender.send(AudioCommand::RestartSong);
            let state = get_state(sender.clone()).await;
            assert_eq!(state.status, Status::Paused); // Note, unlike adding a song to the queue, RestartSong does not affect whether the player is paused

            sender.send(AudioCommand::Exit);
        }

        #[rstest]
        fn test_audio_kernel_stop(audio_kernel: AudioKernel) {
            init();
            audio_kernel.player.append(sound());
            audio_kernel.play();
            assert!(!audio_kernel.player.is_paused());
            audio_kernel.stop();
            assert!(audio_kernel.player.is_paused());
            assert_eq!(audio_kernel.player.get_pos(), Duration::from_secs(0));
            assert_eq!(*audio_kernel.status.lock().unwrap(), Status::Stopped);
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
            assert!(state.paused());
            assert_eq!(state.status, Status::Stopped);

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
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

            audio_kernel.queue_control(QueueCommand::SkipForward(1));

            // the second song should start playing
            let state = audio_kernel.state();
            assert_eq!(state.queue_position, Some(1));
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

            audio_kernel.queue_control(QueueCommand::SkipForward(1));

            // the third song should start playing
            let state = audio_kernel.state();
            assert_eq!(state.queue_position, Some(2));
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

            audio_kernel.queue_control(QueueCommand::SkipForward(1));

            // we were at the end of the queue and tried to skip forward with repeatmode not being Continuous, so the player should be paused and the queue position should be None
            let state = audio_kernel.state();
            assert_eq!(state.queue_position, None);
            assert!(state.paused());
            assert_eq!(state.status, Status::Stopped);
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
            assert!(state.paused());
            assert_eq!(state.status, Status::Stopped);

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
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

            sender.send(AudioCommand::Queue(QueueCommand::SkipForward(1)));
            // the second song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(1));
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

            sender.send(AudioCommand::Queue(QueueCommand::SkipForward(1)));
            // the third song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(2));
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

            sender.send(AudioCommand::Queue(QueueCommand::SkipForward(1)));
            // we were at the end of the queue and tried to skip forward, so the player should be paused and the queue position should be None
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert!(state.paused());
            assert_eq!(state.status, Status::Stopped);

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
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

            // pause the player
            sender.send(AudioCommand::Pause);

            // remove the current song from the queue, the player should still be paused(), but also stopped
            sender.send(AudioCommand::Queue(QueueCommand::RemoveRange(0..1)));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert!(state.paused());
            assert_eq!(state.status, Status::Stopped);
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
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);
            assert_eq!(state.queue.len(), 2);
            assert_eq!(state.queue[0], song2);
            assert_eq!(state.queue[1], song1);

            // remove the next song from the queue, player should still be playing
            sender.send(AudioCommand::Queue(QueueCommand::RemoveRange(1..2)));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);
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
            assert!(state.paused());
            assert_eq!(state.status, Status::Stopped);

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
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

            sender.send(AudioCommand::Queue(QueueCommand::SkipForward(2)));

            // the third song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(2));
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

            sender.send(AudioCommand::Queue(QueueCommand::SkipBackward(1)));

            // the second song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(1));
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

            sender.send(AudioCommand::Queue(QueueCommand::SkipBackward(1)));

            // the first song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert!(!state.paused());

            sender.send(AudioCommand::Queue(QueueCommand::SkipBackward(1)));

            // we were at the start of the queue and tried to skip backward, so the player should be paused and the queue position should be None
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert!(state.paused());
            assert_eq!(state.status, Status::Stopped);

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
            assert!(state.paused());
            assert_eq!(state.status, Status::Stopped);

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
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

            sender.send(AudioCommand::Queue(QueueCommand::SetPosition(1)));
            // the second song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(1));
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

            sender.send(AudioCommand::Queue(QueueCommand::SetPosition(2)));
            // the third song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(2));
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

            sender.send(AudioCommand::Queue(QueueCommand::SetPosition(0)));
            // the first song should start playing
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

            sender.send(AudioCommand::Queue(QueueCommand::SetPosition(3)));
            // we tried to set the position to an index that's out of pounds, so the player should be at the nearest valid index
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(2));
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

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
            assert!(state.paused());
            assert_eq!(state.status, Status::Stopped);

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
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

            sender.send(AudioCommand::ClearPlayer);
            // we only cleared the audio player, so the queue should still have the songs
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert_eq!(state.queue.len(), 3);
            assert!(state.paused());
            assert_eq!(state.status, Status::Stopped);

            sender.send(AudioCommand::Queue(QueueCommand::Clear));
            // we cleared the queue, so the player should be paused and the queue should be empty
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert_eq!(state.queue.len(), 0);
            assert!(state.paused());
            assert_eq!(state.status, Status::Stopped);

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
            assert!(state.paused());
            assert_eq!(state.status, Status::Stopped);

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
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

            // lets go to the second song
            sender.send(AudioCommand::Queue(QueueCommand::SkipForward(1)));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(1));
            assert_eq!(state.queue.len(), 3);
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

            // lets shuffle the queue
            sender.send(AudioCommand::Queue(QueueCommand::Shuffle));
            // we shuffled the queue, so the player should still be playing and the queue should still have 3 songs, and the previous current song should be the now first song
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert_eq!(state.queue.len(), 3);
            assert!(!state.paused());
            assert_eq!(state.status, Status::Playing);

            sender.send(AudioCommand::Exit);
        }

        #[rstest]
        #[timeout(Duration::from_secs(5))] // if the test takes longer than this, the test can be considered a failure
        #[tokio::test]
        async fn test_volume_commands(#[from(audio_kernel_sender)] sender: Arc<AudioKernelSender>) {
            init();

            let state = get_state(sender.clone()).await;
            assert!(
                f32::EPSILON > (state.volume - 1.0).abs(),
                "{} != 1.0",
                state.volume
            );
            assert!(!state.muted);

            sender.send(AudioCommand::Volume(VolumeCommand::Up(0.1)));
            let state = get_state(sender.clone()).await;
            assert!(
                f32::EPSILON > (state.volume - 1.1).abs(),
                "{} != 1.1",
                state.volume
            );
            assert!(!state.muted);

            sender.send(AudioCommand::Volume(VolumeCommand::Down(0.1)));
            let state = get_state(sender.clone()).await;
            assert!(
                f32::EPSILON > (state.volume - 1.0).abs(),
                "{} != 1.0",
                state.volume
            );
            assert!(!state.muted);

            sender.send(AudioCommand::Volume(VolumeCommand::Set(0.5)));
            let state = get_state(sender.clone()).await;
            assert!(
                f32::EPSILON > (state.volume - 0.5).abs(),
                "{} != 0.5",
                state.volume
            );
            assert!(!state.muted);

            sender.send(AudioCommand::Volume(VolumeCommand::Mute));
            let state = get_state(sender.clone()).await;
            assert!(
                f32::EPSILON > (state.volume - 0.5).abs(),
                "{} != 0.5",
                state.volume
            ); // although underlying volume is 0 (for the rodio player), the stored volume is still 0.5
            assert!(state.muted);

            sender.send(AudioCommand::Volume(VolumeCommand::Unmute));
            let state = get_state(sender.clone()).await;
            assert!(
                f32::EPSILON > (state.volume - 0.5).abs(),
                "{} != 0.5",
                state.volume
            );
            assert!(!state.muted);

            sender.send(AudioCommand::Volume(VolumeCommand::ToggleMute));
            let state = get_state(sender.clone()).await;
            assert!(
                f32::EPSILON > (state.volume - 0.5).abs(),
                "{} != 0.5",
                state.volume
            );
            assert!(state.muted);

            sender.send(AudioCommand::Volume(VolumeCommand::ToggleMute));
            let state = get_state(sender.clone()).await;
            assert!(
                f32::EPSILON > (state.volume - 0.5).abs(),
                "{} != 0.5",
                state.volume
            );
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
            assert!(
                f32::EPSILON > (state.volume - MAX_VOLUME).abs(),
                "{} != {}",
                state.volume,
                MAX_VOLUME
            );
            assert!(!state.muted);
            sender.send(AudioCommand::Volume(VolumeCommand::Down(
                MAX_VOLUME + 0.5 - MIN_VOLUME,
            )));
            let state = get_state(sender.clone()).await;
            assert!(
                f32::EPSILON > (state.volume - MIN_VOLUME).abs(),
                "{} != {}",
                state.volume,
                MIN_VOLUME
            );
            assert!(!state.muted);

            // try setting volume above/below the maximum/minimum
            sender.send(AudioCommand::Volume(VolumeCommand::Set(MAX_VOLUME + 0.5)));
            let state = get_state(sender.clone()).await;
            assert!(
                f32::EPSILON > (state.volume - MAX_VOLUME).abs(),
                "{} != {}",
                state.volume,
                MAX_VOLUME
            );
            assert!(!state.muted);
            sender.send(AudioCommand::Volume(VolumeCommand::Set(MIN_VOLUME - 0.5)));
            let state = get_state(sender.clone()).await;
            assert!(
                f32::EPSILON > (state.volume - MIN_VOLUME).abs(),
                "{} != {}",
                state.volume,
                MIN_VOLUME
            );
            assert!(!state.muted);

            sender.send(AudioCommand::Exit);
        }

        #[rstest]
        #[timeout(Duration::from_secs(9))] // if the test takes longer than this, the test can be considered a failure
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
            sender.send(AudioCommand::Stop);
            sender.send(AudioCommand::Seek(
                SeekType::Absolute,
                Duration::from_secs(0),
            ));
            let state: StateAudio = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, Some(0));
            assert_eq!(state.status, Status::Stopped);
            assert_eq!(
                state.runtime.unwrap().duration,
                Duration::from_secs(10) + Duration::from_millis(188)
            );
            assert_eq!(state.runtime.unwrap().seek_position, Duration::from_secs(0));

            // skip ahead a bit
            sender.send(AudioCommand::Seek(
                SeekType::RelativeForwards,
                Duration::from_secs(2),
            ));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.runtime.unwrap().seek_position, Duration::from_secs(2));
            assert_eq!(state.current_song, Some(song.clone()));
            assert_eq!(state.status, Status::Paused);

            // skip back a bit
            sender.send(AudioCommand::Seek(
                SeekType::RelativeBackwards,
                Duration::from_secs(1),
            ));
            let state = get_state(sender.clone()).await;
            assert_eq!(state.runtime.unwrap().seek_position, Duration::from_secs(1));
            assert_eq!(state.current_song, Some(song.clone()));
            assert_eq!(state.status, Status::Paused);

            // skip to 10 seconds
            sender.send(AudioCommand::Seek(
                SeekType::Absolute,
                Duration::from_secs(10),
            ));
            let state = get_state(sender.clone()).await;
            assert_eq!(
                state.runtime.unwrap().seek_position,
                Duration::from_secs(10)
            );
            assert_eq!(state.current_song, Some(song.clone()));
            assert_eq!(state.status, Status::Paused);

            // now we unpause, wait a bit, and check that the song has ended
            sender.send(AudioCommand::Play);
            tokio::time::sleep(Duration::from_millis(500)).await;
            let state = get_state(sender.clone()).await;
            assert_eq!(state.queue_position, None);
            assert_eq!(state.status, Status::Stopped);

            sender.send(AudioCommand::Exit);
        }
    }
}
