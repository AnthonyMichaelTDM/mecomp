#![allow(clippy::needless_continue)]

pub mod interfaces;

use std::time::Duration;

use anyhow::{Context as _, Result, anyhow};

use mecomp_core::{
    rpc::{MusicPlayerClient, init_client},
    state::{Percent, RepeatMode, StateAudio, Status},
    udp::{Event, Listener, Message, StateChange},
};
use mecomp_storage::db::schemas::song::SongBrief;
use mpris_server::{
    LoopStatus, Metadata, PlaybackStatus, Property, Server, Signal, Time, TrackId,
    zbus::{Error as ZbusError, zvariant::ObjectPath},
};
use tarpc::context::Context;
use tokio::sync::{RwLock, RwLockReadGuard};

pub struct Mpris {
    daemon: RwLock<Option<MusicPlayerClient>>,
    pub port: u16,
    pub state: RwLock<StateAudio>,
}

impl Mpris {
    /// Create a new Mpris instance pending a connection to a daemon.
    #[must_use]
    pub fn new(port: u16) -> Self {
        Self {
            daemon: RwLock::new(None),
            port,
            state: RwLock::new(StateAudio::default()),
        }
    }

    /// Give access to the inner Daemon client (checks if the daemon is connected first).
    pub async fn daemon(&self) -> RwLockReadGuard<'_, Option<MusicPlayerClient>> {
        let mut maybedaemon = self.daemon.write().await;
        if let Some(daemon) = maybedaemon.as_ref() {
            let context = Context::current();
            if daemon.ping(context).await.is_ok() {
                return maybedaemon.downgrade();
            }
        }

        // if we get here, either the daemon is not connected, or it's not responding
        *maybedaemon = None;
        log::info!("Lost connection to daemon, shutting down");
        // spawn a new thread to kill the server after some delay
        #[cfg(not(test))] // we don't want to exit the process in tests
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_secs(5));
            std::process::exit(0);
        });
        // if let Err(e) = self.connect_with_retry().await {
        //     log::error!("Failed to reconnect to daemon: {}", e);
        // } else {
        //     log::info!("Reconnected to daemon");
        // }
        maybedaemon.downgrade()
    }

    /// Create a new Mpris instance with a daemon already connected.
    #[must_use]
    pub fn new_with_daemon(daemon: MusicPlayerClient) -> Self {
        Self {
            daemon: RwLock::new(Some(daemon)),
            port: 0,
            state: RwLock::new(StateAudio::default()),
        }
    }

    /// Connect to the daemon.
    ///
    /// # Errors
    ///
    /// Returns an error if the daemon cannot be connected.
    pub async fn connect(&self) -> Result<()> {
        if self.daemon.read().await.is_some() {
            return Ok(());
        }

        let daemon = init_client(self.port).await.context(format!(
            "Failed to connect to daemon on port: {}",
            self.port
        ))?;

        *self.state.write().await = daemon
            .state_audio(Context::current())
            .await
            .context(
                "Failed to get initial state from daemon, please ensure the daemon is running",
            )?
            .ok_or_else(|| anyhow!("Failed to get initial state from daemon"))?;
        *self.daemon.write().await = Some(daemon);

        Ok(())
    }

    /// Connect to the daemon if not already connected.
    ///
    /// # Errors
    ///
    /// Returns an error if the daemon cannot be connected after 5 retries.
    pub async fn connect_with_retry(&self) -> Result<()> {
        const MAX_RETRIES: u8 = 5;
        const BASE_DELAY: Duration = Duration::from_secs(1);

        let mut retries = 0;

        while retries < MAX_RETRIES {
            if let Err(e) = self.connect().await {
                retries += 1;
                log::warn!("Failed to connect to daemon: {e}");
                tokio::time::sleep(BASE_DELAY * u32::from(retries)).await;
            } else {
                return Ok(());
            }
        }

        Err(anyhow!(
            "Failed to connect to daemon on port {} after {} retries",
            self.port,
            MAX_RETRIES
        ))
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

        let maybe_daemon = server.imp().daemon().await;
        if let Some(daemon) = maybe_daemon.as_ref() {
            daemon
                .register_listener(Context::current(), listener.local_addr()?)
                .await?;
        } else {
            return Err(anyhow!("Daemon not connected"));
        }
        drop(maybe_daemon);

        let mut ticker = tokio::time::interval(TICK_RATE);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        #[allow(clippy::redundant_pub_crate)]
        loop {
            let mut state = server.imp().state.write().await;

            tokio::select! {
                Ok(message) = listener.recv() => {
                    match self
                        .handle_message(message, &mut state, || async { server.imp().daemon().await.clone() })
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
    pub async fn handle_message<D>(
        &self,
        message: Message,
        state: &mut StateAudio,
        get_daemon: D,
    ) -> anyhow::Result<MessageOutcomes>
    where
        D: AsyncFnOnce() -> Option<MusicPlayerClient>,
    {
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
                let context = Context::current();
                // we'll need to update the internal state with the new song (and it's duration info and such)
                if let Some(daemon) = get_daemon().await.as_ref() {
                    *state = daemon
                        .state_audio(context)
                        .await
                        .context("Failed to get state from daemon")?
                        .ok_or_else(|| anyhow!("Failed to get state from daemon"))?;
                } else {
                    state.current_song = None;
                    state.runtime = None;
                }

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
                .artist(song.artist.iter().map(ToString::to_string))
                .album(song.album.to_string())
                .title(song.title.to_string())
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
            .handle_message(message, state, || async { Some(daemon) })
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
