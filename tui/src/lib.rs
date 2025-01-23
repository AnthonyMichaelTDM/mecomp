use std::{
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
};

use futures::{future, prelude::*};
use mecomp_core::rpc::{Application, Event, MusicPlayerClient};
use state::action::{Action, PopupAction};
use tarpc::{
    context::Context,
    server::{incoming::Incoming as _, BaseChannel, Channel as _},
    tokio_serde::formats::Json,
};
use termination::Interrupted;
use tokio::sync::{broadcast, mpsc};
use ui::widgets::popups::PopupType;

pub mod state;
pub mod termination;
#[cfg(test)]
mod test_utils;
pub mod ui;

#[derive(Clone, Debug)]
pub struct Subscriber {
    action_tx: mpsc::UnboundedSender<Action>,
}

async fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
    tokio::spawn(fut);
}

impl Subscriber {
    const fn new(action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self { action_tx }
    }

    /// Start the subscriber and register with daemon
    ///
    /// # Errors
    ///
    /// If fail to create the server, or fail to register with daemon
    ///
    /// # Panics
    ///
    /// Panics if the peer address of the underlying TCP transport cannot be determined.
    pub async fn connect(
        daemon: Arc<MusicPlayerClient>,
        action_tx: mpsc::UnboundedSender<Action>,
        mut interrupt_rx: broadcast::Receiver<Interrupted>,
    ) -> anyhow::Result<Interrupted> {
        let application_addr = (IpAddr::V4(Ipv4Addr::LOCALHOST), 0);

        let mut listener =
            tarpc::serde_transport::tcp::listen(application_addr, Json::default).await?;
        let application_addr = listener.local_addr();
        listener.config_mut().max_frame_length(usize::MAX);

        let server = Self::new(action_tx.clone());

        let (handler, abort_handle) = future::abortable(
            listener
                .filter_map(|r| future::ready(r.ok()))
                .map(BaseChannel::with_defaults)
                .max_channels_per_key(10, |t| t.transport().peer_addr().unwrap().ip())
                .map(move |channel| channel.execute(server.clone().serve()).for_each(spawn))
                .buffer_unordered(10)
                .for_each(|()| async {}),
        );

        daemon
            .clone()
            .register_application(Context::current(), application_addr.port())
            .await??;

        tokio::spawn(async move {
            if handler.await == Err(future::Aborted) {
                let _ = daemon
                    .clone()
                    .unregister_application(Context::current(), application_addr.port())
                    .await;
            }
        });

        let interrupted = interrupt_rx.recv().await;

        abort_handle.abort();

        Ok(interrupted?)
    }
}

impl Application for Subscriber {
    async fn notify_event(self, _: Context, event: Event) {
        let notification = match event {
            Event::LibraryRescanFinished => "Library rescan finished",
            Event::LibraryAnalysisFinished => "Library analysis finished",
            Event::LibraryReclusterFinished => "Library recluster finished",
        };

        self.action_tx
            .send(Action::Popup(PopupAction::Open(PopupType::Notification(
                notification.into(),
            ))))
            .unwrap();
    }
}

#[cfg(test)]
mod subscriber_tests {
    use super::*;
    use mecomp_core::{audio::AudioKernelSender, rpc::Application};
    use mecomp_daemon::{config::Settings, init_test_client_server};
    use mecomp_storage::{
        db::schemas::{
            album::Album, analysis::Analysis, artist::Artist, collection::Collection,
            playlist::Playlist, song::Song,
        },
        test_utils::{arb_analysis_features, init_test_database},
    };
    use one_or_many::OneOrMany;
    use rstest::{fixture, rstest};
    use surrealdb::{engine::local::Db, sql::Thing, Surreal};
    use tarpc::context::Context;
    use tempfile::tempdir;
    use termination::create_termination;
    use test_utils::item_id;
    use tokio::sync::oneshot;

    /// Create a test database with a simple state
    async fn db_with_state() -> Arc<Surreal<Db>> {
        let db = Arc::new(init_test_database().await.unwrap());

        let album_id = Thing::from(("album", surrealdb::sql::Id::from(item_id())));
        let analysis_id = Thing::from(("analysis", surrealdb::sql::Id::from(item_id())));
        let artist_id = Thing::from(("artist", surrealdb::sql::Id::from(item_id())));
        let collection_id = Thing::from(("collection", surrealdb::sql::Id::from(item_id())));
        let playlist_id = Thing::from(("playlist", surrealdb::sql::Id::from(item_id())));
        let song_id = Thing::from(("song", surrealdb::sql::Id::from(item_id())));

        // create a song, artist, album, collection, and playlist
        let song = Song {
            id: song_id.clone().into(),
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
            id: analysis_id.clone().into(),
            features: arb_analysis_features()(),
        };
        let artist = Artist {
            id: artist_id.clone().into(),
            name: song.artist[0].clone(),
            runtime: song.runtime,
            album_count: 1,
            song_count: 1,
        };
        let album = Album {
            id: album_id.clone().into(),
            title: song.album.clone(),
            artist: song.artist.clone(),
            release: song.release_year,
            runtime: song.runtime,
            song_count: 1,
            discs: 1,
            genre: song.genre.clone(),
        };
        let collection = Collection {
            id: collection_id.clone().into(),
            name: "Collection 0".into(),
            runtime: song.runtime,
            song_count: 1,
        };
        let playlist = Playlist {
            id: playlist_id.clone().into(),
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
        let audio_kernel = AudioKernelSender::start();

        init_test_client_server(db, settings, audio_kernel)
    }

    #[rstest]
    #[case(Event::LibraryRescanFinished, "Library rescan finished".into())]
    #[case(Event::LibraryAnalysisFinished, "Library analysis finished".into())]
    #[case(Event::LibraryReclusterFinished, "Library recluster finished".into())]
    #[tokio::test]
    async fn test_notify_event(#[case] event: Event, #[case] expected: String) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let subscriber = Subscriber::new(tx);

        let ctx = Context::current();

        subscriber.notify_event(ctx, event).await;

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
            let interrupted = Subscriber::connect(daemon_, action_tx, interrupt_rx.resubscribe())
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

        // if this fails, the daemon failed to register the application
        assert_eq!(
            action,
            Action::Popup(PopupAction::Open(PopupType::Notification(
                "Library rescan finished".into()
            )))
        );

        // ensure the daemon has the application registered
        assert_eq!(
            daemon
                .enumerate_applications(Context::current())
                .await
                .unwrap()
                .len(),
            1
        );

        // kill the application
        terminator.terminate(Interrupted::UserInt).unwrap();
        assert_eq!(rx.await.unwrap(), Interrupted::UserInt);

        // if this fails, the daemon failed to unregister the application
        assert!(daemon
            .enumerate_applications(Context::current())
            .await
            .unwrap()
            .is_empty());
    }
}
