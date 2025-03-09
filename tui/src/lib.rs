use std::sync::Arc;

use mecomp_core::{
    rpc::MusicPlayerClient,
    udp::{Event, Listener, Message},
};
use state::action::{Action, PopupAction};
use tarpc::context::Context;
use termination::Interrupted;
use tokio::sync::{broadcast, mpsc};
use ui::widgets::popups::PopupType;

pub mod state;
pub mod termination;
#[cfg(test)]
mod test_utils;
pub mod ui;

#[derive(Debug)]
pub struct Subscriber;

impl Subscriber {
    /// Main loop for the subscriber.
    ///
    /// # Errors
    ///
    /// Returns an error if the main loop cannot be started, or if an error occurs while handling a message.
    pub async fn main_loop(
        &self,
        daemon: Arc<MusicPlayerClient>,
        action_tx: mpsc::UnboundedSender<Action>,
        mut interrupt_rx: broadcast::Receiver<Interrupted>,
    ) -> anyhow::Result<Interrupted> {
        let mut listener = Listener::new().await?;
        daemon
            .register_listener(Context::current(), listener.local_addr()?)
            .await?;

        #[allow(clippy::redundant_pub_crate)]
        let result = loop {
            tokio::select! {
                Ok(message) = listener.recv() => {
                   self.handle_message(&action_tx, message)?;
                }
                Ok(interrupted) = interrupt_rx.recv() => {
                    break interrupted;
                }
            }
        };

        Ok(result)
    }

    /// Handle a message received from the UDP socket.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be handled.
    pub fn handle_message(
        &self,
        action_tx: &mpsc::UnboundedSender<Action>,
        message: Message,
    ) -> anyhow::Result<()> {
        match message {
            Message::Event(event) => {
                let notification = match event {
                    Event::DaemonShutdown => {
                        action_tx.send(Action::General(state::action::GeneralAction::Exit))?;
                        return Ok(()); // exit early
                    }
                    Event::LibraryRescanFinished => "Library rescan finished",
                    Event::LibraryAnalysisFinished => "Library analysis finished",
                    Event::LibraryReclusterFinished => "Library recluster finished",
                };
                action_tx.send(Action::Library(state::action::LibraryAction::Update))?;

                action_tx.send(Action::Popup(PopupAction::Open(PopupType::Notification(
                    notification.into(),
                ))))?;
            }
            Message::StateChange(state_change) => {
                action_tx.send(Action::Audio(state::action::AudioAction::StateChange(
                    state_change,
                )))?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod subscriber_tests {
    use super::*;
    use mecomp_core::{audio::AudioKernelSender, config::Settings};
    use mecomp_daemon::init_test_client_server;
    use mecomp_storage::{
        db::schemas::{
            album::Album, analysis::Analysis, artist::Artist, collection::Collection,
            playlist::Playlist, song::Song,
        },
        test_utils::{arb_analysis_features, init_test_database},
    };
    use one_or_many::OneOrMany;
    use rstest::{fixture, rstest};
    use surrealdb::{engine::local::Db, RecordId, Surreal};
    use tarpc::context::Context;
    use tempfile::tempdir;
    use termination::create_termination;
    use test_utils::item_id;
    use tokio::sync::oneshot;

    /// Create a test database with a simple state
    async fn db_with_state() -> Arc<Surreal<Db>> {
        let db = Arc::new(init_test_database().await.unwrap());

        let album_id = RecordId::from_table_key("album", item_id());
        let analysis_id = RecordId::from_table_key("analysis", item_id());
        let artist_id = RecordId::from_table_key("artist", item_id());
        let collection_id = RecordId::from_table_key("collection", item_id());
        let playlist_id = RecordId::from_table_key("playlist", item_id());
        let song_id = RecordId::from_table_key("song", item_id());

        // create a song, artist, album, collection, and playlist
        let song = Song {
            id: song_id.clone(),
            title: "Test Song".into(),
            artist: OneOrMany::One("Test Artist".into()),
            album_artist: OneOrMany::One("Test Artist".into()),
            album: "Test Album".into(),
            genre: OneOrMany::One("Test Genre".into()),
            runtime: std::time::Duration::from_secs(180),
            track: Some(0),
            disc: Some(0),
            release_year: Some(2021),
            extension: "mp3".into(),
            path: "test.mp3".into(),
        };
        let analysis = Analysis {
            id: analysis_id.clone(),
            features: arb_analysis_features()(),
        };
        let artist = Artist {
            id: artist_id.clone(),
            name: song.artist[0].clone(),
            runtime: song.runtime,
            album_count: 1,
            song_count: 1,
        };
        let album = Album {
            id: album_id.clone(),
            title: song.album.clone(),
            artist: song.artist.clone(),
            release: song.release_year,
            runtime: song.runtime,
            song_count: 1,
            discs: 1,
            genre: song.genre.clone(),
        };
        let collection = Collection {
            id: collection_id.clone(),
            name: "Collection 0".into(),
            runtime: song.runtime,
            song_count: 1,
        };
        let playlist = Playlist {
            id: playlist_id.clone(),
            name: "Test Playlist".into(),
            runtime: song.runtime,
            song_count: 1,
        };

        // insert the items into the database
        Song::create(&db, song).await.unwrap();
        Analysis::create(&db, song_id.clone(), analysis)
            .await
            .unwrap();
        Artist::create(&db, artist).await.unwrap();
        Album::create(&db, album).await.unwrap();
        Collection::create(&db, collection).await.unwrap();
        Playlist::create(&db, playlist).await.unwrap();

        // add relationships between the items
        Album::add_songs(&db, album_id.clone(), vec![song_id.clone()])
            .await
            .unwrap();
        Artist::add_album(&db, artist_id.clone(), album_id)
            .await
            .unwrap();
        Artist::add_songs(&db, artist_id.clone(), vec![song_id.clone()])
            .await
            .unwrap();
        Collection::add_songs(&db, collection_id, vec![song_id.clone()])
            .await
            .unwrap();
        Playlist::add_songs(&db, playlist_id, vec![song_id.clone()])
            .await
            .unwrap();

        db
    }

    #[fixture]
    async fn daemon() -> MusicPlayerClient {
        let music_dir = Arc::new(tempdir().unwrap());

        let db = db_with_state().await;
        let mut settings: Settings = Default::default();
        settings.daemon.library_paths = vec![music_dir.path().to_path_buf()].into_boxed_slice();
        let settings = Arc::new(settings);
        let (tx, _) = std::sync::mpsc::channel();
        let audio_kernel = AudioKernelSender::start(tx);

        init_test_client_server(db, settings, audio_kernel)
            .await
            .unwrap()
    }

    #[rstest]
    #[case(Message::Event(Event::LibraryRescanFinished), "Library rescan finished".into())]
    #[case(Message::Event(Event::LibraryAnalysisFinished), "Library analysis finished".into())]
    #[case(Message::Event(Event::LibraryReclusterFinished), "Library recluster finished".into())]
    #[tokio::test]
    async fn test_handle_message(#[case] message: Message, #[case] expected: String) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let subscriber = Subscriber;

        subscriber.handle_message(&tx, message).unwrap();

        let action = rx.recv().await.unwrap();

        assert_eq!(
            action,
            Action::Library(state::action::LibraryAction::Update)
        );

        let action = rx.recv().await.unwrap();

        assert_eq!(
            action,
            Action::Popup(PopupAction::Open(PopupType::Notification(expected.into())))
        );
    }

    #[rstest]
    #[tokio::test]
    async fn test_connect(#[future] daemon: MusicPlayerClient) {
        let daemon = Arc::new(daemon.await);

        let (mut terminator, interrupt_rx) = create_termination();
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        terminator.terminate(Interrupted::UserInt).unwrap();

        let (tx, rx) = oneshot::channel();

        let daemon_ = daemon.clone();

        tokio::spawn(async move {
            let interrupted = Subscriber
                .main_loop(daemon_, action_tx, interrupt_rx.resubscribe())
                .await
                .unwrap();

            tx.send(interrupted).unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;

        daemon
            .library_rescan(Context::current())
            .await
            .unwrap()
            .unwrap();

        let action = action_rx.recv().await.unwrap();

        assert_eq!(
            action,
            Action::Library(state::action::LibraryAction::Update)
        );

        let action = action_rx.recv().await.unwrap();

        assert_eq!(
            action,
            Action::Popup(PopupAction::Open(PopupType::Notification(
                "Library rescan finished".into()
            )))
        );

        // kill the application
        terminator.terminate(Interrupted::UserInt).unwrap();
        assert_eq!(rx.await.unwrap(), Interrupted::UserInt);
    }
}
