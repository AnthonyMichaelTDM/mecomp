#![allow(clippy::needless_continue)]

pub mod interfaces;

use std::time::Duration;

use anyhow::{Context as _, Result, anyhow};

use mecomp_core::{
    state::{Percent, RepeatMode, StateAudio, Status},
    udp::{Event, Listener, Message, StateChange},
};
use mecomp_prost::{MusicPlayerClient, RegisterListenerRequest};
use mecomp_storage::db::schemas::song::SongBrief;
use mpris_server::{
    LoopStatus, Metadata, PlaybackStatus, Property, Server, Signal, Time, TrackId,
    zbus::{Error as ZbusError, zvariant::ObjectPath},
};
use tokio::sync::{Mutex, RwLock};

#[derive(Debug)]
pub struct Mpris {
    pub daemon: Mutex<MusicPlayerClient>,
    pub port: u16,
    pub state: RwLock<StateAudio>,
}

impl Mpris {
    /// Create a new Mpris instance with a daemon already connected.
    #[must_use]
    pub fn new_with_daemon(daemon: MusicPlayerClient) -> Self {
        Self {
            daemon: Mutex::new(daemon),
            port: 0,
            state: RwLock::new(StateAudio::default()),
        }
    }

    /// Update the state from the daemon.
    ///
    /// # Errors
    ///
    /// Returns an error if the state cannot be retrieved from the daemon.
    pub async fn update_state(&self) -> Result<()> {
        let mut state = self.state.write().await;

        *state = self
            .daemon
            .lock()
            .await
            .state_audio(())
            .await
            .context("Failed to get state from daemon")?
            .into_inner()
            .state
            .ok_or_else(|| anyhow!("Failed to get state from daemon"))?
            .into();
        Ok(())
    }

    /// Start the Mpris server.
    ///
    /// Consumes self, but you can get back a reference to it by calling `imp()` on the returned `Server`.
    ///
    /// # Errors
    ///
    /// Returns an error if the server cannot be started.
    pub async fn start_server(self, bus_name_suffix: &str) -> Result<Server<Self>, ZbusError> {
        Server::new(bus_name_suffix, self).await
    }
}

#[derive(Debug)]
pub enum MessageOutcomes {
    Nothing,
    Signal(Signal),
    Properties(Vec<Property>),
    Quit,
}

/// Should be the same as the tick rate used by other clients (e.g. the TUI).
pub const TICK_RATE: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub struct Subscriber;

impl Subscriber {
    /// Main loop for the UDP subscriber.
    ///
    /// # Errors
    ///
    /// Returns an error if the main loop cannot be started, or if an error occurs while handling a message.
    pub async fn main_loop(
        &self,
        server: &Server<Mpris>,
        // kill: tokio::sync::broadcast::Receiver<()>,
    ) -> anyhow::Result<()> {
        let mut listener = Listener::new().await?;

        server
            .imp()
            .daemon
            .lock()
            .await
            .register_listener(RegisterListenerRequest::new(listener.local_addr()?))
            .await?;

        let mut ticker = tokio::time::interval(TICK_RATE);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        #[allow(clippy::redundant_pub_crate)]
        loop {
            let mut state = server.imp().state.write().await;

            tokio::select! {
                Ok(message) = listener.recv() => {
                    match self
                        .handle_message(message, &mut state, &server.imp().daemon)
                        .await?
                    {
                        MessageOutcomes::Nothing => continue,
                        MessageOutcomes::Signal(signal) => server.emit(signal).await?,
                        MessageOutcomes::Properties(items) => server.properties_changed(items).await?,
                        MessageOutcomes::Quit => break,
                    }
                }
                _ = ticker.tick() => {
                    if state.paused() {
                        continue;
                    }
                    if let Some(runtime) = &mut state.runtime {
                        runtime.seek_position += TICK_RATE;
                        runtime.seek_percent = Percent::new(runtime.seek_position.as_secs_f32() / runtime.duration.as_secs_f32() * 100.0);
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle a message received from the UDP socket.
    ///
    /// Takes a closure that it can use to get a reference to the daemon client when needed.
    ///
    /// # Returns
    ///
    /// Either nothing, a signal, a list of changed properties, or a notification to quit.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be handled.
    pub async fn handle_message(
        &self,
        message: Message,
        state: &mut StateAudio,
        daemon: &Mutex<MusicPlayerClient>,
    ) -> anyhow::Result<MessageOutcomes> {
        log::info!("Received event: {message:?}");
        match message {
            Message::Event(
                Event::LibraryAnalysisFinished
                | Event::LibraryReclusterFinished
                | Event::LibraryRescanFinished,
            ) => Ok(MessageOutcomes::Nothing),
            Message::Event(Event::DaemonShutdown) => Ok(MessageOutcomes::Quit),
            Message::StateChange(StateChange::Muted) => {
                state.muted = true;
                Ok(MessageOutcomes::Properties(vec![Property::Volume(0.0)]))
            }
            Message::StateChange(StateChange::Unmuted) => {
                state.muted = false;
                Ok(MessageOutcomes::Properties(vec![Property::Volume(
                    state.volume.into(),
                )]))
            }
            Message::StateChange(StateChange::VolumeChanged(new_volume)) => {
                state.volume = new_volume;
                Ok(MessageOutcomes::Properties(vec![Property::Volume(
                    new_volume.into(),
                )]))
            }
            // generally speaking, a lot can change when a track is changed, therefore we update the entire internal state (even if we only emit the new metadata)
            Message::StateChange(StateChange::TrackChanged(_) | StateChange::QueueChanged) => {
                // we'll need to update the internal state with the new song (and it's duration info and such)
                *state = daemon
                    .lock()
                    .await
                    .state_audio(())
                    .await
                    .context("Failed to get state from daemon")?
                    .into_inner()
                    .state
                    .ok_or_else(|| anyhow!("Failed to get state from daemon"))?
                    .into();

                let metadata = metadata_from_opt_song(state.current_song.as_ref());
                Ok(MessageOutcomes::Properties(vec![Property::Metadata(
                    metadata,
                )]))
            }
            Message::StateChange(StateChange::RepeatModeChanged(new_mode)) => {
                state.repeat_mode = new_mode;
                Ok(MessageOutcomes::Properties(vec![Property::LoopStatus(
                    match new_mode {
                        RepeatMode::None => LoopStatus::None,
                        RepeatMode::One => LoopStatus::Track,
                        RepeatMode::All => LoopStatus::Playlist,
                    },
                )]))
            }
            Message::StateChange(StateChange::Seeked(position)) => {
                if let Some(runtime) = &mut state.runtime {
                    runtime.seek_position = position;
                    runtime.seek_percent = Percent::new(
                        position.as_secs_f32() / runtime.duration.as_secs_f32() * 100.0,
                    );
                }
                Ok(MessageOutcomes::Signal(Signal::Seeked {
                    position: Time::from_micros(
                        i64::try_from(position.as_micros()).unwrap_or(i64::MAX),
                    ),
                }))
            }
            Message::StateChange(StateChange::StatusChanged(status)) => {
                state.status = status;
                Ok(MessageOutcomes::Properties(vec![Property::PlaybackStatus(
                    match status {
                        Status::Stopped => PlaybackStatus::Stopped,
                        Status::Paused => PlaybackStatus::Paused,
                        Status::Playing => PlaybackStatus::Playing,
                    },
                )]))
            }
        }
    }
}

#[must_use]
pub fn metadata_from_opt_song(song: Option<&SongBrief>) -> Metadata {
    song.map_or_else(
        || Metadata::builder().trackid(TrackId::NO_TRACK).build(),
        |song| {
            Metadata::builder()
                .trackid(object_path_from_thing(&song.id.clone().into()))
                .length(Time::from_micros(
                    i64::try_from(song.runtime.as_micros()).unwrap_or(i64::MAX),
                ))
                .artist(song.artist.as_slice())
                .album(&song.album)
                .title(&song.title)
                .build()
        },
    )
}

fn object_path_from_thing(thing: &mecomp_storage::db::schemas::RecordId) -> ObjectPath<'_> {
    ObjectPath::try_from(format!("/mecomp/{}/{}", thing.tb, thing.id))
        .unwrap_or_else(|e| panic!("Failed to convert {thing} to ObjectPath: {e}"))
}

#[cfg(test)]
mod subscriber_tests {
    use std::{num::NonZero, sync::Arc};

    use super::*;
    use mecomp_core::{audio::AudioKernelSender, config::Settings};
    use mecomp_daemon::init_test_client_server;
    use mecomp_storage::{
        db::schemas::song::Song,
        test_utils::{arb_song_case, init_test_database_with_state},
    };
    use mpris_server::Metadata;
    use pretty_assertions::assert_str_eq;
    use rstest::rstest;
    use tempfile::TempDir;

    #[rstest]
    #[case::nothing(
        Message::Event(Event::LibraryAnalysisFinished),
        MessageOutcomes::Nothing
    )]
    #[case::nothing(
        Message::Event(Event::LibraryReclusterFinished),
        MessageOutcomes::Nothing
    )]
    #[case::nothing(Message::Event(Event::LibraryRescanFinished), MessageOutcomes::Nothing)]
    #[case::quit(Message::Event(Event::DaemonShutdown), MessageOutcomes::Quit)]
    #[case::muted(Message::StateChange(StateChange::Muted), MessageOutcomes::Properties(vec![Property::Volume(0.0)]))]
    #[case::unmuted(Message::StateChange(StateChange::Unmuted), MessageOutcomes::Properties(vec![Property::Volume(1.0)]))]
    #[case::volume_changed(Message::StateChange(StateChange::VolumeChanged(0.75)), MessageOutcomes::Properties(vec![Property::Volume(0.75)]))]
    #[case::track_changed(Message::StateChange(StateChange::TrackChanged(None)), MessageOutcomes::Properties(vec![Property::Metadata(Metadata::builder().trackid(TrackId::NO_TRACK).build())]))]
    #[case::track_changed(Message::StateChange(StateChange::TrackChanged(Some(Song::generate_id().into()))), MessageOutcomes::Properties(vec![Property::Metadata(Metadata::builder().trackid(TrackId::NO_TRACK).build())]))]
    #[case::repeat_mode_changed(Message::StateChange(StateChange::RepeatModeChanged(RepeatMode::One)), MessageOutcomes::Properties(vec![Property::LoopStatus(LoopStatus::Track)]))]
    #[case::seeked(Message::StateChange(StateChange::Seeked(Duration::from_secs(10))), MessageOutcomes::Signal(Signal::Seeked { position: Time::from_micros(10_000_000) }))]
    #[case::status_changed(Message::StateChange(StateChange::StatusChanged(Status::Playing)), MessageOutcomes::Properties(vec![Property::PlaybackStatus(PlaybackStatus::Playing)]))]
    #[case::status_changed(Message::StateChange(StateChange::StatusChanged(Status::Paused)), MessageOutcomes::Properties(vec![Property::PlaybackStatus(PlaybackStatus::Paused)]))]
    #[case::status_changed(Message::StateChange(StateChange::StatusChanged(Status::Stopped)), MessageOutcomes::Properties(vec![Property::PlaybackStatus(PlaybackStatus::Stopped)]))]
    #[tokio::test]
    async fn test_handle_message(#[case] message: Message, #[case] expected: MessageOutcomes) {
        let tempdir = TempDir::new().unwrap();

        let db = init_test_database_with_state(
            NonZero::new(4).unwrap(),
            |i| (arb_song_case()(), i > 1, i > 2),
            None,
            &tempdir,
        )
        .await;

        let settings = Arc::new(Settings::default());

        let (event_tx, _) = std::sync::mpsc::channel();

        let audio_kernel = AudioKernelSender::start(event_tx);

        let daemon = init_test_client_server(db, settings, audio_kernel.clone())
            .await
            .unwrap();

        let state = &mut StateAudio::default();

        let actual = Subscriber
            .handle_message(message, state, &daemon)
            .await
            .unwrap();

        // Since the structs from mpris_server don't implement PartialEq, we can't compare them directly, so instead we compare the Debug representations
        assert_str_eq!(format!("{actual:?}"), format!("{expected:?}"));
    }
}

#[cfg(test)]
pub mod test_utils {
    use std::{num::NonZero, sync::Arc};

    use super::*;
    use mecomp_core::{audio::AudioKernelSender, config::Settings};
    use mecomp_daemon::init_test_client_server;
    use mecomp_storage::test_utils::{arb_song_case, init_test_database_with_state};
    use rstest::fixture;
    use surrealdb::{Surreal, engine::local::Db};
    use tempfile::TempDir;

    // Create a database with some songs, a playlist, and a collection
    async fn db(tempdir: &TempDir) -> Arc<Surreal<Db>> {
        init_test_database_with_state(
            NonZero::new(4).unwrap(),
            |i| (arb_song_case()(), i > 1, i > 2),
            None,
            tempdir,
        )
        .await
    }

    #[fixture]
    pub async fn fixtures() -> (
        Mpris,
        std::sync::mpsc::Receiver<StateChange>,
        TempDir,
        Arc<AudioKernelSender>,
    ) {
        let tempdir = TempDir::new().unwrap();

        let db = db(&tempdir).await;

        let settings = Arc::new(Settings::default());

        let (event_tx, event_rx) = std::sync::mpsc::channel();

        let audio_kernel = AudioKernelSender::start(event_tx);

        let daemon = init_test_client_server(db, settings, audio_kernel.clone())
            .await
            .unwrap();

        let mpris = Mpris::new_with_daemon(daemon);

        (mpris, event_rx, tempdir, audio_kernel)
    }
}
