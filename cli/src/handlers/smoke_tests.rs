use std::sync::Arc;

use clap::Parser;
use mecomp_core::{audio::AudioKernelSender, config::Settings, rpc::MusicPlayerClient};
use mecomp_daemon::init_test_client_server;
use mecomp_storage::{
    db::schemas::{
        album::Album, analysis::Analysis, artist::Artist, collection::Collection,
        dynamic::DynamicPlaylist, playlist::Playlist, song::Song,
    },
    test_utils::{arb_analysis_features, init_test_database},
};
use one_or_many::OneOrMany;
use rstest::{fixture, rstest};
use surrealdb::{engine::local::Db, sql::Thing, Surreal};
use tempfile::tempdir;

use crate::handlers::{
    utils::WriteAdapter, CollectionCommand, Command, CommandHandler, CurrentTarget, DynamicCommand,
    DynamicUpdate, LibraryCommand, LibraryGetTarget, LibraryListTarget, PlaybackCommand,
    PlaylistAddCommand, PlaylistCommand, PlaylistGetMethod, QueueAddTarget, QueueCommand,
    RadioCommand, RandTarget, RepeatMode, SearchTarget, SeekCommand, StatusCommand, VolumeCommand,
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

#[test]
fn test_cli_args_parse_query() {
    let args = vec![
        "mecomp-cli",
        "dynamic",
        "create",
        "new dp",
        "title = \"Test Song\"",
    ];
    let flags = crate::Flags::try_parse_from(args);
    assert!(flags.is_ok());
    let flags = flags.unwrap();
    assert_eq!(flags.port, 6600);
    assert!(flags.subcommand.is_some());
    let subcommand = flags.subcommand.unwrap();
    assert_eq!(
        subcommand,
        Command::Dynamic {
            command: DynamicCommand::Create {
                name: "new dp".to_string(),
                query: "title = \"Test Song\"".parse().unwrap(),
            },
        }
    );

    let args = vec!["mecomp-cli", "dynamic", "create", "new dp", "invalid query"];
    let flags = crate::Flags::try_parse_from(args);
    assert!(flags.is_err());
}

/// the id used for all the items in this fake library
pub const fn item_id() -> &'static str {
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
    let dynamic_id = Thing::from(("dynamic", item_id()));

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
    let dynamic = DynamicPlaylist {
        id: dynamic_id.clone(),
        name: "Test Dynamic".into(),
        query: "title = \"Test Song\"".parse().unwrap(),
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
    DynamicPlaylist::create(&db, dynamic).await.unwrap();

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
    let mut settings: Settings = Settings::default();
    settings.daemon.library_paths = vec![music_dir.path().to_path_buf()].into_boxed_slice();
    let settings = Arc::new(settings);
    let (tx, _) = std::sync::mpsc::channel();
    let audio_kernel = AudioKernelSender::start(tx);

    init_test_client_server(db, settings, audio_kernel)
        .await
        .unwrap()
}

#[fixture]
fn testname() -> String {
    std::thread::current()
        .name()
        .unwrap()
        .to_string()
        .replace("::", "_")
}

macro_rules! set_snapshot_suffix {
    ($($expr:expr),*) => {
        let mut settings = insta::Settings::clone_current();
        settings.set_snapshot_suffix(format!($($expr,)*));
        let _guard = settings.bind_to_scope();
    }
}

#[rstest]
#[tokio::test]
async fn test_ping_command(#[future] client: MusicPlayerClient) {
    let ctx = tarpc::context::current();
    let command = Command::Ping;

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());

    let result = command.handle(ctx, client.await, stdout, stderr).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[tokio::test]
async fn test_stop_command(#[future] client: MusicPlayerClient) {
    let ctx = tarpc::context::current();
    let command = Command::Stop;

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());

    let result = command.handle(ctx, client.await, stdout, stderr).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
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

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());

    let result = command.handle(ctx, client.await, stdout, stderr).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[case(StatusCommand::Rescan)]
#[case(StatusCommand::Recluster)]
#[case(StatusCommand::Analyze)]
#[tokio::test]
async fn test_status_command(#[future] client: MusicPlayerClient, #[case] command: StatusCommand) {
    let ctx = tarpc::context::current();
    let command = Command::Status { command };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());

    let result = command.handle(ctx, client.await, stdout, stderr).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[tokio::test]
async fn test_state_command(#[future] client: MusicPlayerClient) {
    let ctx = tarpc::context::current();
    let command = Command::State;

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());

    let result = command.handle(ctx, client.await, stdout, stderr).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[case(CurrentTarget::Album)]
#[case(CurrentTarget::Artist)]
#[case(CurrentTarget::Song)]
#[tokio::test]
async fn test_current_command(#[future] client: MusicPlayerClient, #[case] target: CurrentTarget) {
    let ctx = tarpc::context::current();
    let command = Command::Current { target };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());

    let result = command.handle(ctx, client.await, stdout, stderr).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[case(RandTarget::Album)]
#[case(RandTarget::Artist)]
#[case(RandTarget::Song)]
#[tokio::test]
async fn test_rand_command(#[future] client: MusicPlayerClient, #[case] target: RandTarget) {
    let ctx = tarpc::context::current();
    let command = Command::Rand { target };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());

    let result = command.handle(ctx, client.await, stdout, stderr).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
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

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());

    let result = command.handle(ctx, client.await, stdout, stderr).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
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
#[case(PlaybackCommand::Repeat { mode: RepeatMode::One })]
#[case(PlaybackCommand::Repeat { mode: RepeatMode::All })]
#[tokio::test]
async fn test_playback_command(
    #[future] client: MusicPlayerClient,
    #[case] command: PlaybackCommand,
) {
    let ctx = tarpc::context::current();
    let command = Command::Playback { command };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());

    let result = command.handle(ctx, client.await, stdout, stderr).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[case(QueueCommand::Add { id: item_id().to_string(), target: QueueAddTarget::Album })]
#[case(QueueCommand::Add { id: item_id().to_string(), target: QueueAddTarget::Artist })]
#[case(QueueCommand::Add { id: item_id().to_string(), target: QueueAddTarget::Song })]
#[case(QueueCommand::Add { id: item_id().to_string(), target: QueueAddTarget::Playlist })]
#[case(QueueCommand::Add { id: item_id().to_string(), target: QueueAddTarget::Collection })]
#[case(QueueCommand::Add { id: item_id().to_string(), target: QueueAddTarget::Dynamic })]
#[case(QueueCommand::Remove { start: 0, end: 1 })]
#[case(QueueCommand::Clear)]
#[case(QueueCommand::List)]
#[case(QueueCommand::Set { index: 0 })]
#[tokio::test]
async fn test_queue_command(#[future] client: MusicPlayerClient, #[case] command: QueueCommand) {
    let ctx = tarpc::context::current();
    let command = Command::Queue { command };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());

    let result = command.handle(ctx, client.await, stdout, stderr).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[case(PlaylistCommand::Add { command: PlaylistAddCommand::Song { id: item_id().to_string(), song_ids: vec![item_id().to_string()] } })]
#[case(PlaylistCommand::Add { command: PlaylistAddCommand::Album { id: item_id().to_string(), album_id: item_id().to_string() } })]
#[case(PlaylistCommand::Add { command: PlaylistAddCommand::Artist { id: item_id().to_string(), artist_id: item_id().to_string() } })]
#[case(PlaylistCommand::Remove { id: item_id().to_string(), item_ids: vec![item_id().to_string()] })]
#[case(PlaylistCommand::List)]
#[case(PlaylistCommand::Get { method: PlaylistGetMethod::Name, target: "Test Playlist".to_string() })]
#[case(PlaylistCommand::Get { method: PlaylistGetMethod::Id, target: item_id().to_string() })]
#[case(PlaylistCommand::Update { id: item_id().to_string(), name: "Updated Test Playlist".to_string() })]
#[case(PlaylistCommand::Songs { id: item_id().to_string() })]
#[case(PlaylistCommand::Delete { id: item_id().to_string() })]
#[tokio::test]
async fn test_playlist_command(
    #[future] client: MusicPlayerClient,
    #[case] command: PlaylistCommand,
) {
    let ctx = tarpc::context::current();
    let command = Command::Playlist { command };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());

    let result = command.handle(ctx, client.await, stdout, stderr).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[tokio::test]
async fn test_playlist_create(#[future] client: MusicPlayerClient) {
    let ctx = tarpc::context::current();
    let command = Command::Playlist {
        command: PlaylistCommand::Create {
            name: "New Playlist".to_string(),
        },
    };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());

    let result = command.handle(ctx, client.await, stdout, stderr).await;
    assert!(result.is_ok());

    let stdout = String::from_utf8(stdout.0.clone()).unwrap();
    assert!(stdout.starts_with("Daemon response:\nThing {"));
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[case(DynamicCommand::List)]
#[case(DynamicCommand::Get { id: item_id().to_string() })]
#[case(DynamicCommand::Songs { id: item_id().to_string() })]
#[case(DynamicCommand::Delete { id: item_id().to_string() })]
#[case(DynamicCommand::Update {
    id: item_id().to_string(),
    update: DynamicUpdate {
        name: Some("Updated Dynamic Playlist".to_string()),
        query: None,
    },
})]
#[case(DynamicCommand::Update {
    id: item_id().to_string(),
    update: DynamicUpdate {
        name: None,
        query: Some("title = \"Test Song\"".parse().unwrap()),
    },
})]
#[case(DynamicCommand::Update {
    id: item_id().to_string(),
    update: DynamicUpdate {
        name: Some("Updated Dynamic Playlist".to_string()),
        query: Some("title = \"Test Song\"".parse().unwrap()),
    },
})]
#[case(DynamicCommand::ShowBNF)]
#[tokio::test]
async fn test_dynamic_playlist_command(
    #[future] client: MusicPlayerClient,
    #[case] command: DynamicCommand,
) {
    let ctx = tarpc::context::current();
    let command = Command::Dynamic { command };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());

    let result = command.handle(ctx, client.await, stdout, stderr).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[tokio::test]
async fn test_dynamic_playlist_create(#[future] client: MusicPlayerClient) {
    let ctx = tarpc::context::current();
    let command = Command::Dynamic {
        command: DynamicCommand::Create {
            name: "New Dynamic Playlist".to_string(),
            query: "title = \"Test Song\"".parse().unwrap(),
        },
    };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());

    let result = command.handle(ctx, client.await, stdout, stderr).await;
    assert!(result.is_ok());

    let stdout = String::from_utf8(stdout.0.clone()).unwrap();
    assert!(stdout.starts_with("Daemon response:\nThing {"));
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[case(CollectionCommand::List)]
#[case(CollectionCommand::Get { id: item_id().to_string() })]
#[case(CollectionCommand::Songs { id: item_id().to_string() })]
#[case(CollectionCommand::Recluster)]
#[tokio::test]
async fn test_collection_command(
    #[future] client: MusicPlayerClient,
    #[case] command: CollectionCommand,
) {
    let ctx = tarpc::context::current();
    let command = Command::Collection { command };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());

    let result = command.handle(ctx, client.await, stdout, stderr).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[tokio::test]
/// this is a separate test because the returned value depends on when the test is run,
/// the ulid of the new playlist if generated at runtime and will be different each time
async fn test_collection_freeze(#[future] client: MusicPlayerClient) {
    let ctx = tarpc::context::current();
    let command = Command::Collection {
        command: CollectionCommand::Freeze {
            id: item_id().to_string(),
            name: "Test Collection".to_string(),
        },
    };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());

    let result = command.handle(ctx, client.await, stdout, stderr).await;
    assert!(result.is_ok());

    let stdout = String::from_utf8(stdout.0.clone()).unwrap();
    assert!(stdout.starts_with("Daemon response:\nplaylist:"));
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
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

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());

    let result = command.handle(ctx, client.await, stdout, stderr).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}
