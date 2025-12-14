use std::{sync::Arc, time::Duration};

use clap::Parser;
use mecomp_core::{audio::AudioKernelSender, config::Settings};
use mecomp_daemon::init_test_client_server;
use mecomp_prost::MusicPlayerClient;
use mecomp_storage::{
    db::schemas::{
        album::Album, analysis::Analysis, artist::Artist, collection::Collection,
        dynamic::DynamicPlaylist, playlist::Playlist, song::Song,
    },
    test_utils::{arb_analysis_features, init_test_database},
};
use pretty_assertions::assert_str_eq;
use rstest::{fixture, rstest};
use surrealdb::{RecordId, Surreal, engine::local::Db};
use tempfile::tempdir;

use crate::handlers::{
    CollectionCommand, Command, CommandHandler, CurrentTarget, DynamicCommand, DynamicUpdate,
    LibraryCommand, LibraryGetCommand, LibraryListTarget, PlaybackCommand, PlaylistAddCommand,
    PlaylistCommand, PlaylistGetMethod, QueueCommand, RadioCommand, RandTarget, RepeatMode,
    SearchTarget, SeekCommand, StatusCommand, VolumeCommand,
    utils::{StdIn, WriteAdapter},
};

struct StdInMock {
    lines: Vec<String>,
    terminal: bool,
}

impl StdInMock {
    fn new(lines: Vec<String>, terminal: bool) -> Self {
        Self { lines, terminal }
    }
}

impl StdIn for StdInMock {
    fn is_terminal(&self) -> bool {
        self.terminal
    }

    fn lines(&self) -> impl Iterator<Item = std::io::Result<String>> {
        self.lines.clone().into_iter().map(|line| Ok(line))
    }
}

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
/// another id used for some items in this fake library, used to avoid conflicts
pub const fn other_item_id() -> &'static str {
    "03J1K5B6RJ84WJXCWYJ5WNE12E"
}

/// Create a test database with a simple state
async fn db_with_state() -> Arc<Surreal<Db>> {
    let db = Arc::new(init_test_database().await.unwrap());

    let album_id = RecordId::from_table_key("album", item_id());
    let analysis_id1 = RecordId::from_table_key("analysis", item_id());
    let analysis_id2 = RecordId::from_table_key("analysis", other_item_id());
    let artist_id = RecordId::from_table_key("artist", item_id());
    let collection_id = RecordId::from_table_key("collection", item_id());
    let playlist_id1 = RecordId::from_table_key("playlist", item_id());
    let playlist_id2 = RecordId::from_table_key("playlist", other_item_id());
    let song_id1 = RecordId::from_table_key("song", item_id());
    let song_id2 = RecordId::from_table_key("song", other_item_id());
    let dynamic_id = RecordId::from_table_key("dynamic", item_id());

    // create a song, artist, album, collection, and playlist
    let song = Song {
        id: song_id1.clone(),
        title: "Test Song".into(),
        artist: "Test Artist".to_string().into(),
        album_artist: "Test Artist".to_string().into(),
        album: "Test Album".into(),
        genre: "Test Genre".to_string().into(),
        runtime: std::time::Duration::from_secs(180),
        track: Some(0),
        disc: Some(0),
        release_year: Some(2021),
        extension: "mp3".into(),
        path: "test.mp3".into(),
    };
    let song2 = Song {
        id: song_id2.clone(),
        title: "Another Song".into(),
        artist: "Test Artist".to_string().into(),
        album_artist: "Test Artist".to_string().into(),
        album: "Test Album".into(),
        genre: "Test Genre".to_string().into(),
        runtime: std::time::Duration::from_secs(200),
        track: Some(0),
        disc: Some(0),
        release_year: Some(2020),
        extension: "flac".into(),
        path: "another.flac".into(),
    };
    let analysis1 = Analysis {
        id: analysis_id1.clone(),
        features: arb_analysis_features()(),
    };
    let analysis2 = Analysis {
        id: analysis_id2.clone(),
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
    let playlist1 = Playlist {
        id: playlist_id1.clone(),
        name: "Test Playlist".into(),
        runtime: song.runtime,
        song_count: 1,
    };
    let playlist2 = Playlist {
        id: playlist_id2.clone(),
        name: "Another Playlist".into(),
        runtime: Duration::from_secs(0),
        song_count: 0,
    };
    let dynamic = DynamicPlaylist {
        id: dynamic_id.clone(),
        name: "Test Dynamic".into(),
        query: "title = \"Test Song\"".parse().unwrap(),
    };

    // insert the items into the database
    Song::create(&db, song).await.unwrap();
    Song::create(&db, song2).await.unwrap();
    Analysis::create(&db, song_id1.clone(), analysis1)
        .await
        .unwrap();
    Analysis::create(&db, song_id2.clone(), analysis2)
        .await
        .unwrap();
    Artist::create(&db, artist).await.unwrap();
    Album::create(&db, album).await.unwrap();
    Collection::create(&db, collection).await.unwrap();
    Playlist::create(&db, playlist1).await.unwrap();
    Playlist::create(&db, playlist2).await.unwrap();
    DynamicPlaylist::create(&db, dynamic).await.unwrap();

    // add relationships between the items
    Album::add_songs(
        &db,
        album_id.clone(),
        vec![song_id1.clone(), song_id2.clone()],
    )
    .await
    .unwrap();
    Artist::add_album(&db, artist_id.clone(), album_id)
        .await
        .unwrap();
    Artist::add_songs(
        &db,
        artist_id.clone(),
        vec![song_id1.clone(), song_id2.clone()],
    )
    .await
    .unwrap();
    Collection::add_songs(&db, collection_id, vec![song_id1.clone()])
        .await
        .unwrap();
    Playlist::add_songs(&db, playlist_id1, vec![song_id1.clone()])
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
    let command = Command::Ping;

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], true);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[tokio::test]
async fn test_stop_command(#[future] client: MusicPlayerClient) {
    let command = Command::Stop;

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], true);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[case(LibraryCommand::Rescan)]
#[case(LibraryCommand::Recluster)]
#[case(LibraryCommand::Analyze{ overwrite: false })]
#[case(LibraryCommand::Analyze{ overwrite: true })]
#[case(LibraryCommand::Full)]
#[case(LibraryCommand::Brief)]
#[case(LibraryCommand::Health)]
#[case(LibraryCommand::List {
    quiet: false,
    target: LibraryListTarget::Artists,
})]
#[case(LibraryCommand::List {
    quiet: true,
    target: LibraryListTarget::Artists,
})]
#[case(LibraryCommand::List {
    quiet: false,
    target: LibraryListTarget::Albums,
})]
#[case(LibraryCommand::List {
    quiet: true,
    target: LibraryListTarget::Albums,
})]
#[case(LibraryCommand::List {
    quiet: false,
    target: LibraryListTarget::Songs,
})]
#[case(LibraryCommand::List {
    quiet: true,
    target: LibraryListTarget::Songs,
})]
#[case(LibraryCommand::List {
    quiet: false,
    target: LibraryListTarget::Playlists,
})]
#[case(LibraryCommand::List {
    quiet: true,
    target: LibraryListTarget::Playlists,
})]
#[case(LibraryCommand::List {
    quiet: false,
    target: LibraryListTarget::DynamicPlaylists,
})]
#[case(LibraryCommand::List {
    quiet: true,
    target: LibraryListTarget::DynamicPlaylists,
})]
#[case(LibraryCommand::List {
    quiet: false,
    target: LibraryListTarget::Collections,
})]
#[case(LibraryCommand::List {
    quiet: true,
    target: LibraryListTarget::Collections,
})]
#[case(LibraryCommand::Get {
    command: LibraryGetCommand::Artist {id: item_id().to_string()},
})]
#[case(LibraryCommand::Get {
    command: LibraryGetCommand::Album {id: item_id().to_string()},
})]
#[case(LibraryCommand::Get {
    command: LibraryGetCommand::Song {id: item_id().to_string()},
})]
#[case(LibraryCommand::Get {
    command: LibraryGetCommand::Playlist {id: item_id().to_string()},
})]
#[case(LibraryCommand::Get {
    command: LibraryGetCommand::Dynamic {id: item_id().to_string()},
})]
#[case(LibraryCommand::Get {
    command: LibraryGetCommand::Collection {id: item_id().to_string()},
})]
#[tokio::test]
async fn test_library_command(
    #[future] client: MusicPlayerClient,
    #[case] command: LibraryCommand,
) {
    let command = Command::Library { command };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], true);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
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
    let command = Command::Status { command };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], true);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[tokio::test]
async fn test_state_command(#[future] client: MusicPlayerClient) {
    let command = Command::State;

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], true);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
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
    let command = Command::Current { target };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], true);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
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
    let command = Command::Rand { target };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], true);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
    assert!(result.is_ok());

    if target == RandTarget::Song {
        // songs are random, so we can't snapshot test them reliably.
        // we can at least make sure the output is non-empty
        assert!(!stdout.0.is_empty());
        set_snapshot_suffix!("stderr");
        insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
        return;
    }

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
async fn test_search_command(
    #[future] client: MusicPlayerClient,
    #[case] target: SearchTarget,
    #[values(true, false)] quiet: bool,
) {
    let command = Command::Search {
        quiet,
        target,
        query: "test".to_string(),
        limit: 10,
    };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], true);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
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
    let command = Command::Playback { command };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], true);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[case(QueueCommand::Add {items: vec![format!("album:{}", item_id())] })]
#[case(QueueCommand::Add {items: vec![format!("artist:{}", item_id())] })]
#[case(QueueCommand::Add {items: vec![format!("song:{}", item_id())] })]
#[case(QueueCommand::Add {items: vec![format!("playlist:{}", item_id())] })]
#[case(QueueCommand::Add {items: vec![format!("collection:{}", item_id())] })]
#[case(QueueCommand::Add {items: vec![format!("dynamic:{}", item_id())] })]
#[case(QueueCommand::Remove { start: 0, end: 1 })]
#[case(QueueCommand::Clear)]
#[case(QueueCommand::List { quiet: false })]
#[case(QueueCommand::List { quiet: true })]
#[case(QueueCommand::Set { index: 0 })]
#[tokio::test]
async fn test_queue_command(#[future] client: MusicPlayerClient, #[case] command: QueueCommand) {
    let command = Command::Queue { command };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], true);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[case::mixed(QueueCommand::Add {items: vec![format!("song:{}", item_id())] }, vec![format!("song:{}", other_item_id())])]
#[case::args_only(QueueCommand::Add {items: vec![format!("song:{}", item_id())] }, vec![])]
#[case::pipe_only(QueueCommand::Add {items: vec![] }, vec![format!("song:{}", item_id())])]
/// invalid record ids that are piped in get ignored
#[case::pipe_invalid(QueueCommand::Add {items: vec![] }, vec![format!("song:{}", other_item_id()), "invalid:id".to_string()])]
#[tokio::test]
async fn test_queue_add_pipe(
    #[future] client: MusicPlayerClient,
    #[case] command: QueueCommand,
    #[case] pipe_lines: Vec<String>,
) {
    let command = Command::Queue { command };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(pipe_lines, false);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[tokio::test]
async fn test_queue_add_invalid(#[future] client: MusicPlayerClient) {
    let command = Command::Queue {
        command: QueueCommand::Add {
            items: vec![format!("invalid:{}", item_id())],
        },
    };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], false);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
    assert!(result.is_err());
    assert_str_eq!(
        result.unwrap_err().to_string(),
        "One or more provided IDs are invalid"
    );
}

#[rstest]
#[case(PlaylistCommand::Add (PlaylistAddCommand { id: item_id().to_string(), items: vec![format!("song:{}", item_id())] } ))]
#[case(PlaylistCommand::Add (PlaylistAddCommand { id: item_id().to_string(), items: vec![format!("album:{}", item_id())] } ))]
#[case(PlaylistCommand::Add (PlaylistAddCommand { id: item_id().to_string(), items: vec![format!("artist:{}", item_id())] } ))]
#[case(PlaylistCommand::Remove { id: item_id().to_string(), item_ids: vec![item_id().to_string()] })]
#[case(PlaylistCommand::List)]
#[case(PlaylistCommand::Get { method: PlaylistGetMethod::Name, target: "Test Playlist".to_string() })]
#[case(PlaylistCommand::Get { method: PlaylistGetMethod::Id, target: item_id().to_string() })]
#[case(PlaylistCommand::Update { id: item_id().to_string(), name: "Updated Test Playlist".to_string() })]
#[case(PlaylistCommand::Songs { id: item_id().to_string() })]
#[case(PlaylistCommand::Delete { id: item_id().to_string() })]
#[case(PlaylistCommand::Add (PlaylistAddCommand { id: item_id().to_string(), items: vec![format!("song:{}", other_item_id()), format!("song:{}", item_id())] } ))]
#[tokio::test]
async fn test_playlist_command(
    #[future] client: MusicPlayerClient,
    #[case] command: PlaylistCommand,
) {
    let command = Command::Playlist { command };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], true);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[case::mixed(PlaylistAddCommand { id: item_id().to_string(), items: vec![format!("album:{}", item_id())] }, vec![format!("song:{}", other_item_id())])]
#[case::args_only(PlaylistAddCommand { id: other_item_id().to_string(), items: vec![format!("song:{}", item_id())] }, vec![])]
#[case::pipe_only(PlaylistAddCommand { id: other_item_id().to_string(), items: vec![] }, vec![format!("song:{}", item_id())])]
/// invalid record ids that are piped in get ignored
#[case::pipe_invalid(PlaylistAddCommand { id: other_item_id().to_string(), items: vec![] }, vec![format!("song:{}", other_item_id()), "invalid:id".to_string()])]
#[tokio::test]
async fn test_playlist_add_pipe(
    #[future] client: MusicPlayerClient,
    #[case] command: PlaylistAddCommand,
    #[case] pipe_lines: Vec<String>,
) {
    let playlist_id = command.id.clone();
    let command = Command::Playlist {
        command: PlaylistCommand::Add(command),
    };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(pipe_lines, false);

    let client = client.await;
    let result = command.handle(client.clone(), stdout, stderr, stdin).await;
    assert!(result.is_ok());

    // also get the songs in the playlist after adding to verify they were added correctly
    let get_command = Command::Playlist {
        command: PlaylistCommand::Songs { id: playlist_id },
    };
    let result = get_command
        .handle(client, stdout, stderr, &StdInMock::new(vec![], true))
        .await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[tokio::test]
async fn test_playlist_add_invalid(#[future] client: MusicPlayerClient) {
    let command = Command::Playlist {
        command: PlaylistCommand::Add(PlaylistAddCommand {
            id: item_id().to_string(),
            items: vec![format!("invalid:{}", item_id())],
        }),
    };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], false);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
    assert!(result.is_err());
    assert_str_eq!(
        result.unwrap_err().to_string(),
        "One or more provided IDs are invalid"
    );
}

#[rstest]
#[tokio::test]
async fn test_playlist_add_empty(#[future] client: MusicPlayerClient) {
    let command = Command::Playlist {
        command: PlaylistCommand::Add(PlaylistAddCommand {
            id: item_id().to_string(),
            items: vec![],
        }),
    };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], false);

    let client = client.await;

    let result = command.handle(client.clone(), stdout, stderr, stdin).await;
    assert!(result.is_err());
    assert_str_eq!(result.unwrap_err().to_string(), "no input provided");

    let stdin = &StdInMock::new(vec![], true);

    let result = command.handle(client, stdout, stderr, stdin).await;
    assert!(result.is_err());
    assert_str_eq!(result.unwrap_err().to_string(), "no input provided");
}

#[rstest]
#[tokio::test]
async fn test_playlist_create(#[future] client: MusicPlayerClient) {
    let command = Command::Playlist {
        command: PlaylistCommand::Create {
            name: "New Playlist".to_string(),
        },
    };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], true);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
    assert!(result.is_ok());

    let stdout = String::from_utf8(stdout.0.clone()).unwrap();
    assert!(
        stdout.starts_with("Daemon response:\nRecordId {"),
        "{stdout}"
    );
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
    let command = Command::Dynamic { command };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], true);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[tokio::test]
async fn test_dynamic_playlist_create(#[future] client: MusicPlayerClient) {
    let command = Command::Dynamic {
        command: DynamicCommand::Create {
            name: "New Dynamic Playlist".to_string(),
            query: "title = \"Test Song\"".parse().unwrap(),
        },
    };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], true);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
    assert!(result.is_ok());

    let stdout = String::from_utf8(stdout.0.clone()).unwrap();
    assert!(
        stdout.starts_with("Daemon response:\nRecordId {"),
        "{stdout}"
    );
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
    let command = Command::Collection { command };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], true);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
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
    let command = Command::Collection {
        command: CollectionCommand::Freeze {
            id: item_id().to_string(),
            name: "Test Collection".to_string(),
        },
    };

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], true);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
    assert!(result.is_ok());

    let stdout = String::from_utf8(stdout.0.clone()).unwrap();
    assert!(stdout.starts_with("Daemon response:\nplaylist:"));
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}

#[rstest]
#[case( RadioCommand { items: vec![format!("song:{}", item_id())], n: 1 } )]
#[case( RadioCommand { items: vec![format!("album:{}", item_id())], n: 1 } )]
#[case( RadioCommand { items: vec![format!("artist:{}", item_id())], n: 1 } )]
#[case( RadioCommand { items: vec![format!("playlist:{}", item_id())], n: 1 } )]
#[case( RadioCommand { items: vec![format!("dynamic:{}", item_id())], n: 1 } )]
#[case( RadioCommand { items: vec![format!("playlist:{}", item_id()), format!("song:{}", item_id())], n: 1 } )]
#[case( RadioCommand { items: vec![format!("song:{}", other_item_id())], n: 1 } )]
#[tokio::test]
async fn test_radio_command(#[future] client: MusicPlayerClient, #[case] command: RadioCommand) {
    let command = Command::Radio(command);

    let stdout = &mut WriteAdapter(Vec::new());
    let stderr = &mut WriteAdapter(Vec::new());
    let stdin = &StdInMock::new(vec![], true);

    let result = command.handle(client.await, stdout, stderr, stdin).await;
    assert!(result.is_ok());

    set_snapshot_suffix!("stdout");
    insta::assert_snapshot!(testname(), String::from_utf8(stdout.0.clone()).unwrap());
    set_snapshot_suffix!("stderr");
    insta::assert_snapshot!(testname(), String::from_utf8(stderr.0.clone()).unwrap());
}
