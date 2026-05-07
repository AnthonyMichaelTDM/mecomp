use std::num::NonZero;
use std::sync::Arc;
use std::time::Duration;

use mecomp_core::{audio::AudioKernelSender, config::Settings, udp::StateChange};
use mecomp_daemon::init_test_client_server;
use mecomp_prost::MusicPlayerClient;
use mecomp_storage::test_utils::{arb_song_case, init_test_database_with_state};
use mecomp_tui::state::action::{
    AudioAction, LibraryAction, PlaybackAction, PopupAction, QueueAction,
};
use mecomp_tui::state::{
    audio::AudioState, library::LibraryState, popup::PopupState, search::SearchState,
};
use mecomp_tui::termination::{Interrupted, create_termination};
use mecomp_tui::ui::widgets::popups::PopupType;
use pretty_assertions::assert_eq;
use rstest::rstest;
use tempfile::tempdir;
use tokio::sync::mpsc::unbounded_channel;

#[rstest::fixture]
async fn client() -> MusicPlayerClient {
    let music_dir = Arc::new(tempdir().unwrap());
    let db = init_test_database_with_state(
        NonZero::new(4).unwrap(),
        |i| (arb_song_case()(), i > 1, i > 2),
        None,
        &music_dir,
    )
    .await;
    let mut settings = Settings::default(); // override some setting to speed up tests
    settings.daemon.library_paths = vec![music_dir.path().to_path_buf()].into_boxed_slice();
    let settings = Arc::new(settings);
    let (tx, _) = std::sync::mpsc::channel();
    let audio_kernel = AudioKernelSender::start(tx);
    init_test_client_server(db, settings, audio_kernel)
        .await
        .unwrap()
}

#[tokio::test]
async fn test_popup_state_main_loop_opens_and_closes() {
    let (popup_state, mut state_rx) = PopupState::new();
    let (action_tx, action_rx) = unbounded_channel();
    let (mut terminator, interrupt_rx) = create_termination();

    let handle = tokio::spawn(async move { popup_state.main_loop(action_rx, interrupt_rx).await });

    assert_eq!(state_rx.recv().await.unwrap(), None);
    action_tx
        .send(PopupAction::Open(PopupType::Notification("hello".into())))
        .unwrap();
    assert_eq!(
        state_rx.recv().await.unwrap(),
        Some(PopupType::Notification("hello".into()))
    );
    action_tx.send(PopupAction::Close).unwrap();
    assert_eq!(state_rx.recv().await.unwrap(), None);

    terminator.terminate(Interrupted::UserInt).unwrap();
    let result = tokio::time::timeout(Duration::from_secs(2), handle)
        .await
        .expect("popup loop did not finish")
        .unwrap()
        .unwrap();
    assert_eq!(result, Interrupted::UserInt);
}

#[rstest]
#[tokio::test]
async fn test_search_state_main_loop_publishes_search_results(#[future] client: MusicPlayerClient) {
    let client = client.await;
    let search_state = SearchState::new();
    let (search_state, mut state_rx) = search_state;
    let (action_tx, action_rx) = unbounded_channel();
    let (mut terminator, interrupt_rx) = create_termination();

    let handle = tokio::spawn(async move {
        search_state
            .main_loop(client, action_rx, interrupt_rx)
            .await
    });

    let initial_state = state_rx.recv().await.unwrap();
    assert_eq!(initial_state, mecomp_prost::SearchResult::default());

    action_tx.send("test query".into()).unwrap();
    let query_state = tokio::time::timeout(Duration::from_secs(2), state_rx.recv())
        .await
        .expect("search update did not arrive")
        .unwrap();
    assert_eq!(query_state, query_state);

    terminator.terminate(Interrupted::UserInt).unwrap();
    let result = tokio::time::timeout(Duration::from_secs(2), handle)
        .await
        .expect("search loop did not finish")
        .unwrap();
    assert_eq!(result.unwrap(), Interrupted::UserInt);
}

#[rstest]
#[tokio::test]
async fn test_audio_state_main_loop_processes_actions_and_handles_state_change(
    #[future] client: MusicPlayerClient,
) {
    let daemon = client.await;
    let audio_state = AudioState::new();
    let (audio_state, mut state_rx) = audio_state;
    let (action_tx, action_rx) = unbounded_channel();
    let (mut terminator, interrupt_rx) = create_termination();

    let handle =
        tokio::spawn(async move { audio_state.main_loop(daemon, action_rx, interrupt_rx).await });

    let initial_state = state_rx.recv().await.unwrap();
    assert_eq!(initial_state.muted, false);

    action_tx
        .send(AudioAction::StateChange(StateChange::Muted))
        .unwrap();
    action_tx
        .send(AudioAction::Playback(PlaybackAction::Toggle))
        .unwrap();
    action_tx
        .send(AudioAction::Queue(QueueAction::Shuffle))
        .unwrap();

    let updated_state = state_rx.recv().await.unwrap();
    assert_eq!(updated_state.muted, true);

    terminator.terminate(Interrupted::UserInt).unwrap();
    let result = tokio::time::timeout(Duration::from_secs(2), handle)
        .await
        .expect("audio loop did not finish")
        .unwrap();
    assert_eq!(result.unwrap(), Interrupted::UserInt);
}

#[rstest]
#[tokio::test]
async fn test_library_state_main_loop_handles_rescan_analyze_update_and_create_playlist(
    #[future] client: MusicPlayerClient,
) {
    let daemon = client.await;
    let library_state = LibraryState::new();
    let (library_state, mut state_rx) = library_state;
    let (action_tx, action_rx) = unbounded_channel();
    let (mut terminator, interrupt_rx) = create_termination();

    let handle = tokio::spawn(async move {
        library_state
            .main_loop(daemon.clone(), action_rx, interrupt_rx)
            .await
    });

    let initial_state = state_rx.recv().await.unwrap();

    action_tx.send(LibraryAction::Rescan).unwrap();
    action_tx.send(LibraryAction::Analyze).unwrap();
    action_tx.send(LibraryAction::Recluster).unwrap();
    action_tx.send(LibraryAction::Update).unwrap();
    action_tx
        .send(LibraryAction::CreatePlaylist("Test Playlist".into()))
        .unwrap();

    let updated_state = state_rx.recv().await.unwrap();
    assert_eq!(updated_state.artists, initial_state.artists);

    let playlist_state = state_rx.recv().await.unwrap();
    assert!(
        playlist_state
            .playlists
            .iter()
            .any(|playlist| playlist.name == "Test Playlist")
    );

    terminator.terminate(Interrupted::UserInt).unwrap();
    let result = tokio::time::timeout(Duration::from_secs(2), handle)
        .await
        .expect("library loop did not finish")
        .unwrap();
    assert_eq!(result.unwrap(), Interrupted::UserInt);
}
