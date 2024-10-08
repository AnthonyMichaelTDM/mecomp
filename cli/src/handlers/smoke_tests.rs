use std::sync::Arc;

use clap::Parser;
use mecomp_core::{audio::AudioKernelSender, rpc::MusicPlayerClient};
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
use tempfile::tempdir;

use crate::handlers::{
    CollectionCommand, Command, CommandHandler, CurrentTarget, LibraryCommand, LibraryGetTarget,
    LibraryListTarget, PlaybackCommand, PlaylistAddCommand, PlaylistCommand, PlaylistGetMethod,
    QueueAddTarget, QueueCommand, RadioCommand, RandTarget, RepeatMode, SearchTarget, SeekCommand,
    StatusCommand, VolumeCommand,
};

#[test]
fn test_cli_args_parse() {
    let args = vec!["mecomp-cli", "--port", "6600"];
    let flags = crate::Flags::try_parse_from(args);
    assert!(flags.is_ok());
    let flags = flags.unwrap();
    assert_eq!(flags.port, 6600);
    assert!(flags.subcommand.is_none());
}

/// the id used for all the items in this fake library
pub fn item_id() -> &'static str {
    "01J1K5B6RJ84WJXCWYJ5WNE12E"
}

/// Create a test database with a simple state
async fn db_with_state() -> Arc<Surreal<Db>> {
    let db = Arc::new(init_test_database().await.unwrap());

    let album_id = Thing::from(("album", item_id()));
    let analysis_id = Thing::from(("analysis", item_id()));
    let artist_id = Thing::from(("artist", item_id()));
    let collection_id = Thing::from(("collection", item_id()));
    let playlist_id = Thing::from(("playlist", item_id()));
    let song_id = Thing::from(("song", item_id()));

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
async fn client() -> MusicPlayerClient {
    let music_dir = Arc::new(tempdir().unwrap());

    let db = db_with_state().await;
    let mut settings: Settings = Default::default();
    settings.daemon.library_paths = vec![music_dir.path().to_path_buf()].into_boxed_slice();
    let settings = Arc::new(settings);
    let audio_kernel = AudioKernelSender::start();

    init_test_client_server(db, settings, audio_kernel)
}

#[rstest]
#[tokio::test]
async fn test_ping_command(#[future] client: MusicPlayerClient) {
    let ctx = tarpc::context::current();
    let command = Command::Ping;

    let result = command.handle(ctx, client.await).await;
    assert!(result.is_ok());
}

#[rstest]
#[tokio::test]
async fn test_stop_command(#[future] client: MusicPlayerClient) {
    let ctx = tarpc::context::current();
    let command = Command::Stop;

    let result = command.handle(ctx, client.await).await;
    assert!(result.is_ok());
}

#[rstest]
#[case(LibraryCommand::Rescan)]
#[case(LibraryCommand::Recluster)]
#[case(LibraryCommand::Analyze)]
#[case(LibraryCommand::Full)]
#[case(LibraryCommand::Brief)]
#[case(LibraryCommand::Health)]
#[case(LibraryCommand::List {
    full: false,
    target: LibraryListTarget::Artists,
})]
#[case(LibraryCommand::List {
    full: true,
    target: LibraryListTarget::Artists,
})]
#[case(LibraryCommand::List {
    full: false,
    target: LibraryListTarget::Albums,
})]
#[case(LibraryCommand::List {
    full: true,
    target: LibraryListTarget::Albums,
})]
#[case(LibraryCommand::List {
    full: false,
    target: LibraryListTarget::Songs,
})]
#[case(LibraryCommand::List {
    full: true,
    target: LibraryListTarget::Songs,
})]
#[case(LibraryCommand::Get {
    target: LibraryGetTarget::Artist,
    id: item_id().to_string(),
})]
#[case(LibraryCommand::Get {
    target: LibraryGetTarget::Album,
    id: item_id().to_string(),
})]
#[case(LibraryCommand::Get {
    target: LibraryGetTarget::Song,
    id: item_id().to_string(),
})]
#[case(LibraryCommand::Get {
    target: LibraryGetTarget::Playlist,
    id: item_id().to_string(),
})]
#[tokio::test]
async fn test_library_command(
    #[future] client: MusicPlayerClient,
    #[case] command: LibraryCommand,
) {
    let ctx = tarpc::context::current();
    let command = Command::Library { command };

    let result = command.handle(ctx, client.await).await;
    assert!(result.is_ok());
}

#[rstest]
#[case(StatusCommand::Rescan)]
#[case(StatusCommand::Recluster)]
#[case(StatusCommand::Analyze)]
#[tokio::test]
async fn test_status_command(#[future] client: MusicPlayerClient, #[case] command: StatusCommand) {
    let ctx = tarpc::context::current();
    let command = Command::Status { command };

    let result = command.handle(ctx, client.await).await;
    assert!(result.is_ok());
}

#[rstest]
#[tokio::test]
async fn test_state_command(#[future] client: MusicPlayerClient) {
    let ctx = tarpc::context::current();
    let command = Command::State;

    let result = command.handle(ctx, client.await).await;
    assert!(result.is_ok());
}

#[rstest]
#[case(CurrentTarget::Album)]
#[case(CurrentTarget::Artist)]
#[case(CurrentTarget::Song)]
#[tokio::test]
async fn test_current_command(#[future] client: MusicPlayerClient, #[case] target: CurrentTarget) {
    let ctx = tarpc::context::current();
    let command = Command::Current { target };

    let result = command.handle(ctx, client.await).await;
    assert!(result.is_ok());
}

#[rstest]
#[case(RandTarget::Album)]
#[case(RandTarget::Artist)]
#[case(RandTarget::Song)]
#[tokio::test]
async fn test_rand_command(#[future] client: MusicPlayerClient, #[case] target: RandTarget) {
    let ctx = tarpc::context::current();
    let command = Command::Rand { target };

    let result = command.handle(ctx, client.await).await;
    assert!(result.is_ok());
}

#[rstest]
#[case(SearchTarget::Album)]
#[case(SearchTarget::Artist)]
#[case(SearchTarget::Song)]
#[case(SearchTarget::All)]
#[tokio::test]
async fn test_search_command(#[future] client: MusicPlayerClient, #[case] target: SearchTarget) {
    let ctx = tarpc::context::current();
    let command = Command::Search {
        target,
        query: "test".to_string(),
        limit: 10,
    };

    let result = command.handle(ctx, client.await).await;
    assert!(result.is_ok());
}

#[rstest]
#[case(PlaybackCommand::Play)]
#[case(PlaybackCommand::Pause)]
#[case(PlaybackCommand::Stop)]
#[case(PlaybackCommand::Next)]
#[case(PlaybackCommand::Previous)]
#[case(PlaybackCommand::Seek { command: SeekCommand::Absolute { position: 0. } })]
#[case(PlaybackCommand::Seek { command: SeekCommand::Forward { amount: 0. } })]
#[case(PlaybackCommand::Seek { command: SeekCommand::Backward { amount: 0. } })]
#[case(PlaybackCommand::Volume { command: VolumeCommand::Decrease { amount: 0. } })]
#[case(PlaybackCommand::Volume { command: VolumeCommand::Increase { amount: 0. } })]
#[case(PlaybackCommand::Volume { command: VolumeCommand::Set { volume: 0. } })]
#[case(PlaybackCommand::Volume { command: VolumeCommand::Mute })]
#[case(PlaybackCommand::Volume { command: VolumeCommand::Unmute })]
#[case(PlaybackCommand::Toggle)]
#[case(PlaybackCommand::Restart)]
#[case(PlaybackCommand::Shuffle)]
#[case(PlaybackCommand::Repeat { mode: RepeatMode::None })]
#[case(PlaybackCommand::Repeat { mode: RepeatMode::Once })]
#[case(PlaybackCommand::Repeat { mode: RepeatMode::Continuous })]
#[tokio::test]
async fn test_playback_command(
    #[future] client: MusicPlayerClient,
    #[case] command: PlaybackCommand,
) {
    let ctx = tarpc::context::current();
    let command = Command::Playback { command };

    let result = command.handle(ctx, client.await).await;
    assert!(result.is_ok());
}

#[rstest]
#[case(QueueCommand::Add { id: item_id().to_string(), target: QueueAddTarget::Album })]
#[case(QueueCommand::Add { id: item_id().to_string(), target: QueueAddTarget::Artist })]
#[case(QueueCommand::Add { id: item_id().to_string(), target: QueueAddTarget::Song })]
#[case(QueueCommand::Add { id: item_id().to_string(), target: QueueAddTarget::Playlist })]
#[case(QueueCommand::Add { id: item_id().to_string(), target: QueueAddTarget::Collection })]
#[case(QueueCommand::Remove { start: 0, end: 1 })]
#[case(QueueCommand::Clear)]
#[case(QueueCommand::List)]
#[case(QueueCommand::Set { index: 0 })]
#[tokio::test]
async fn test_queue_command(#[future] client: MusicPlayerClient, #[case] command: QueueCommand) {
    let ctx = tarpc::context::current();
    let command = Command::Queue { command };

    let result = command.handle(ctx, client.await).await;
    assert!(result.is_ok());
}

#[rstest]
#[case(PlaylistCommand::Add { command: PlaylistAddCommand::Song { id: item_id().to_string(), song_ids: vec![item_id().to_string()] } })]
#[case(PlaylistCommand::Add { command: PlaylistAddCommand::Album { id: item_id().to_string(), album_id: item_id().to_string() } })]
#[case(PlaylistCommand::Add { command: PlaylistAddCommand::Artist { id: item_id().to_string(), artist_id: item_id().to_string() } })]
#[case(PlaylistCommand::Remove { id: item_id().to_string(), item_ids: vec![item_id().to_string()] })]
#[case(PlaylistCommand::List)]
#[case(PlaylistCommand::Get { method: PlaylistGetMethod::Name, target: "Test Playlist".to_string() })]
#[case(PlaylistCommand::Get { method: PlaylistGetMethod::Id, target: item_id().to_string() })]
#[case(PlaylistCommand::Delete { id: item_id().to_string() })]
#[tokio::test]
async fn test_playlist_command(
    #[future] client: MusicPlayerClient,
    #[case] command: PlaylistCommand,
) {
    let ctx = tarpc::context::current();
    let command = Command::Playlist { command };

    let result = command.handle(ctx, client.await).await;
    assert!(result.is_ok());
}

#[rstest]
#[case(CollectionCommand::List)]
#[case(CollectionCommand::Get { id: item_id().to_string() })]
#[case(CollectionCommand::Recluster)]
#[case(CollectionCommand::Freeze { id: Playlist::generate_id().id.to_string(), name: "Test Collection".to_string() })]
#[tokio::test]
async fn test_collection_command(
    #[future] client: MusicPlayerClient,
    #[case] command: CollectionCommand,
) {
    let ctx = tarpc::context::current();
    let command = Command::Collection { command };

    let result = command.handle(ctx, client.await).await;
    assert!(result.is_ok());
}

#[rstest]
#[case( RadioCommand::Album { id: item_id().to_string(), n: 1 } )]
#[case( RadioCommand::Artist { id: item_id().to_string(), n: 1 } )]
#[case( RadioCommand::Song { id: item_id().to_string(), n: 1 } )]
#[case( RadioCommand::Playlist { id: item_id().to_string(), n: 1 } )]
#[tokio::test]
async fn test_radio_command(#[future] client: MusicPlayerClient, #[case] command: RadioCommand) {
    let ctx = tarpc::context::current();
    let command = Command::Radio { command };

    let result = command.handle(ctx, client.await).await;
    assert!(result.is_ok());
}
