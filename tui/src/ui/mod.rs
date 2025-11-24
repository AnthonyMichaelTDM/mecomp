//! This module contains the implementations of the TUI.
//!
//! The app is responsible for rendering the state of the application to the terminal.
//!
//! The app is updated every tick, and they use the state stores to get the latest state.

pub mod app;
pub mod colors;
pub mod components;
pub mod widgets;

use std::{
    io::{self, Stdout},
    time::Duration,
};

use anyhow::Context as _;
use app::App;
use components::{
    Component, ComponentRender,
    content_view::{
        ActiveView,
        views::{
            AlbumViewProps, ArtistViewProps, CollectionViewProps, DynamicPlaylistViewProps,
            PlaylistViewProps, RadioViewProps, RandomViewProps, SongViewProps, ViewData,
        },
    },
};
use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, EventStream, PopKeyboardEnhancementFlags,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use mecomp_core::{config::Settings, state::StateAudio};
use mecomp_prost::{LibraryBrief, MusicPlayerClient, RadioSimilarRequest, SearchResult, Ulid};
use ratatui::prelude::*;
use tokio::sync::{broadcast, mpsc};
use tokio_stream::StreamExt;

use crate::{
    state::{Receivers, action::Action, component::ActiveComponent},
    termination::Interrupted,
};

#[derive(Debug, Default)]
pub struct AppState {
    pub active_component: ActiveComponent,
    pub audio: StateAudio,
    pub search: SearchResult,
    pub library: LibraryBrief,
    pub active_view: ActiveView,
    pub additional_view_data: ViewData,
    pub settings: Settings,
}

const RENDERING_TICK_RATE: Duration = Duration::from_millis(250);

#[allow(clippy::module_name_repetitions)]
pub struct UiManager {
    action_tx: mpsc::UnboundedSender<Action>,
}

impl UiManager {
    #[must_use]
    pub const fn new(action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self { action_tx }
    }

    /// Main loop for the UI manager.
    ///
    /// This function will run until the user exits the application.
    ///
    /// # Errors
    ///
    /// This function will return an error if there was an issue rendering to the terminal.
    pub async fn main_loop(
        self,
        daemon: MusicPlayerClient,
        settings: Settings,
        mut state_rx: Receivers,
        mut interrupt_rx: broadcast::Receiver<Interrupted>,
    ) -> anyhow::Result<Interrupted> {
        // consume the first state to initialize the ui app
        let mut state = AppState {
            active_component: ActiveComponent::default(),
            audio: state_rx.audio.recv().await.unwrap_or_default(),
            search: state_rx.search.recv().await.unwrap_or_default(),
            library: state_rx.library.recv().await.unwrap_or_default(),
            active_view: state_rx.view.recv().await.unwrap_or_default(),
            additional_view_data: ViewData::default(),
            settings,
        };
        let mut app = App::new(&state, self.action_tx.clone());

        let mut terminal = setup_terminal()?;
        let mut ticker = tokio::time::interval(RENDERING_TICK_RATE);
        let mut crossterm_events = EventStream::new();

        let result: anyhow::Result<Interrupted> = loop {
            tokio::select! {
                // Tick to terminate the select every N milliseconds
                _ = ticker.tick() => (),
                // Catch and handle crossterm events
               maybe_event = crossterm_events.next() => match maybe_event {
                    Some(Ok(Event::Key(key)))  => {
                        app.handle_key_event(key);
                    },
                    Some(Ok(Event::Mouse(mouse))) => {
                        let terminal_size = terminal.size().context("could not get terminal size")?;
                        let area = Rect::new(0, 0, terminal_size.width, terminal_size.height);
                        app.handle_mouse_event(mouse, area);
                    },
                    None => break Ok(Interrupted::UserInt),
                    _ => (),
                },
                // Handle state updates
                Some(audio) = state_rx.audio.recv() => {
                    state = AppState {
                        audio,
                        ..state
                    };
                    app = app.move_with_audio(&state);
                },
                Some(search) = state_rx.search.recv() => {
                    state = AppState {
                        search,
                        ..state
                    };
                    app = app.move_with_search(&state);
                },
                Some(library) = state_rx.library.recv() => {
                    state = AppState {
                        library,
                        // Fixes edge case where user has a playlist open, modifies that playlist, and tries to view it again without first viewing another playlist
                        additional_view_data: Box::pin(handle_additional_view_data(daemon.clone(), &state, &state.active_view)).await.unwrap_or(state.additional_view_data),
                        ..state
                    };
                    app = app.move_with_library(&state);
                },
                Some(active_view) = state_rx.view.recv() => {
                    // update view_data
                    let additional_view_data = Box::pin(handle_additional_view_data(daemon.clone(), &state, &active_view)).await.unwrap_or(state.additional_view_data);

                    state = AppState {
                        active_view,
                        additional_view_data,
                        ..state
                    };
                    app = app.move_with_view(&state);
                },
                Some(active_component) = state_rx.component.recv() => {
                    state = AppState {
                        active_component,
                        ..state
                    };
                    app = app.move_with_component(&state);
                },
                Some(popup) = state_rx.popup.recv() => {
                     app = app.move_with_popup( popup.map(|popup| {
                         popup.into_popup(&state, self.action_tx.clone())
                     }));
                }
                // Catch and handle interrupt signal to gracefully shutdown
                Ok(interrupted) = interrupt_rx.recv() => {
                    break Ok(interrupted);
                }
            }

            if let Err(err) = terminal
                .draw(|frame| app.render(frame, frame.area()))
                .context("could not render to the terminal")
            {
                break Err(err);
            }
        };

        restore_terminal(&mut terminal)?;

        result
    }
}

#[cfg(not(tarpaulin_include))]
fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    let mut stdout = io::stdout();

    enable_raw_mode()?;

    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

#[cfg(not(tarpaulin_include))]
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode()?;

    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        PopKeyboardEnhancementFlags,
    )?;

    Ok(terminal.show_cursor()?)
}

#[cfg(not(tarpaulin_include))]
pub fn init_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // intentionally ignore errors here since we're already in a panic
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);

        original_hook(panic_info);
    }));
}

async fn song_view_future(
    daemon: MusicPlayerClient,
    id: Ulid,
) -> anyhow::Result<(
    Option<mecomp_prost::Song>,
    Option<Vec<mecomp_prost::ArtistBrief>>,
    Option<mecomp_prost::AlbumBrief>,
    Option<Vec<mecomp_prost::PlaylistBrief>>,
    Option<Vec<mecomp_prost::CollectionBrief>>,
)> {
    let mut copy = daemon.clone();
    let song = copy.library_song_get(id.clone());
    let mut copy = daemon.clone();
    let artists = copy.library_song_get_artists(id.clone());
    let mut copy = daemon.clone();
    let album = copy.library_song_get_album(id.clone());
    let mut copy = daemon.clone();
    let playlists = copy.library_song_get_playlists(id.clone());
    let mut copy = daemon.clone();
    let collections = copy.library_song_get_collections(id.clone());

    Ok(
        tokio::try_join!(song, artists, album, playlists, collections,).map(
            |(song, artists, album, playlists, collections)| {
                (
                    song.into_inner().song,
                    artists.into_inner().artists.map(|a| a.artists),
                    album.into_inner().album,
                    playlists.into_inner().playlists.map(|p| p.playlists),
                    collections.into_inner().collections.map(|c| c.collections),
                )
            },
        )?,
    )
}

async fn album_view_future(
    daemon: MusicPlayerClient,
    id: Ulid,
) -> anyhow::Result<(
    Option<mecomp_prost::Album>,
    Option<Vec<mecomp_prost::ArtistBrief>>,
    Option<Vec<mecomp_prost::SongBrief>>,
)> {
    let mut copy = daemon.clone();
    let album = copy.library_album_get(id.clone());
    let mut copy = daemon.clone();
    let artists = copy.library_album_get_artists(id.clone());
    let mut copy = daemon.clone();
    let songs = copy.library_album_get_songs(id.clone());

    Ok(
        tokio::try_join!(album, artists, songs).map(|(album, artists, songs)| {
            (
                album.into_inner().album,
                artists.into_inner().artists.map(|a| a.artists),
                songs.into_inner().songs.map(|s| s.songs),
            )
        })?,
    )
}

async fn artist_view_future(
    daemon: MusicPlayerClient,
    id: Ulid,
) -> anyhow::Result<(
    Option<mecomp_prost::Artist>,
    Option<Vec<mecomp_prost::AlbumBrief>>,
    Option<Vec<mecomp_prost::SongBrief>>,
)> {
    let mut copy = daemon.clone();
    let artist = copy.library_artist_get(id.clone());
    let mut copy = daemon.clone();
    let albums = copy.library_artist_get_albums(id.clone());
    let mut copy = daemon.clone();
    let songs = copy.library_artist_get_songs(id.clone());

    Ok(
        tokio::try_join!(artist, albums, songs,).map(|(artist, albums, songs)| {
            (
                artist.into_inner().artist,
                albums.into_inner().albums.map(|a| a.albums),
                songs.into_inner().songs.map(|s| s.songs),
            )
        })?,
    )
}

async fn playlist_view_future(
    daemon: MusicPlayerClient,
    id: Ulid,
) -> anyhow::Result<(
    Option<mecomp_prost::Playlist>,
    Option<Vec<mecomp_prost::SongBrief>>,
)> {
    let mut copy = daemon.clone();
    let playlist = copy.library_playlist_get(id.clone());
    let mut copy = daemon.clone();
    let songs = copy.library_playlist_get_songs(id.clone());
    Ok(tokio::try_join!(playlist, songs,).map(|(playlist, songs)| {
        (
            playlist.into_inner().playlist,
            songs.into_inner().songs.map(|s| s.songs),
        )
    })?)
}

async fn dynamic_playlist_view_future(
    daemon: MusicPlayerClient,
    id: Ulid,
) -> anyhow::Result<(
    Option<mecomp_prost::DynamicPlaylist>,
    Option<Vec<mecomp_prost::SongBrief>>,
)> {
    let mut copy = daemon.clone();
    let dynamic_playlist = copy.library_dynamic_playlist_get(id.clone());
    let mut copy = daemon.clone();
    let songs = copy.library_dynamic_playlist_get_songs(id.clone());
    Ok(
        tokio::try_join!(dynamic_playlist, songs,).map(|(dynamic_playlist, songs)| {
            (
                dynamic_playlist.into_inner().playlist,
                songs.into_inner().songs.map(|s| s.songs),
            )
        })?,
    )
}

async fn collection_view_future(
    daemon: MusicPlayerClient,
    id: Ulid,
) -> anyhow::Result<(
    Option<mecomp_prost::Collection>,
    Option<Vec<mecomp_prost::SongBrief>>,
)> {
    let mut copy = daemon.clone();
    let collection = copy.library_collection_get(id.clone());
    let mut copy = daemon.clone();
    let songs = copy.library_collection_get_songs(id.clone());
    Ok(
        tokio::try_join!(collection, songs,).map(|(collection, songs)| {
            (
                collection.into_inner().collection,
                songs.into_inner().songs.map(|s| s.songs),
            )
        })?,
    )
}

async fn random_view_future(
    daemon: MusicPlayerClient,
) -> anyhow::Result<(
    Option<mecomp_prost::AlbumBrief>,
    Option<mecomp_prost::ArtistBrief>,
    Option<mecomp_prost::SongBrief>,
)> {
    let mut copy = daemon.clone();
    let album = copy.rand_album(());
    let mut copy = daemon.clone();
    let artist = copy.rand_artist(());
    let mut copy = daemon.clone();
    let song = copy.rand_song(());

    Ok(
        tokio::try_join!(album, artist, song).map(|(album, artist, song)| {
            (
                album.into_inner().album,
                artist.into_inner().artist,
                song.into_inner().song,
            )
        })?,
    )
}

/// Returns `None` if new data is not needed
#[allow(clippy::too_many_lines)]
async fn handle_additional_view_data(
    mut daemon: MusicPlayerClient,
    state: &AppState,
    active_view: &ActiveView,
) -> Option<ViewData> {
    match active_view {
        ActiveView::Song(id) => {
            if let Ok((
                Some(song),
                Some(artists),
                Some(album),
                Some(playlists),
                Some(collections),
            )) = song_view_future(daemon, id.clone()).await
            {
                let album = album.into();
                let song_view_props = SongViewProps {
                    id: song.id.clone(),
                    song,
                    artists,
                    album,
                    playlists,
                    collections,
                };
                Some(ViewData {
                    song: Some(song_view_props),
                    ..state.additional_view_data.clone()
                })
            } else {
                Some(ViewData {
                    song: None,
                    ..state.additional_view_data.clone()
                })
            }
        }
        ActiveView::Album(id) => {
            if let Ok((Some(album), Some(artists), Some(songs))) =
                album_view_future(daemon, id.clone()).await
            {
                let artists = artists.into_iter().map(Into::into).collect();
                let songs = songs.into_iter().map(Into::into).collect();
                let album_view_props = AlbumViewProps {
                    id: album.id.clone(),
                    album,
                    artists,
                    songs,
                };
                Some(ViewData {
                    album: Some(album_view_props),
                    ..state.additional_view_data.clone()
                })
            } else {
                Some(ViewData {
                    album: None,
                    ..state.additional_view_data.clone()
                })
            }
        }
        ActiveView::Artist(id) => {
            if let Ok((Some(artist), Some(albums), Some(songs))) =
                artist_view_future(daemon, id.clone()).await
            {
                let albums = albums.into_iter().map(Into::into).collect();
                let songs = songs.into_iter().map(Into::into).collect();
                let artist_view_props = ArtistViewProps {
                    id: artist.id.clone(),
                    artist,
                    albums,
                    songs,
                };
                Some(ViewData {
                    artist: Some(artist_view_props),
                    ..state.additional_view_data.clone()
                })
            } else {
                Some(ViewData {
                    artist: None,
                    ..state.additional_view_data.clone()
                })
            }
        }
        ActiveView::Playlist(id) => {
            if let Ok((Some(playlist), Some(songs))) =
                playlist_view_future(daemon, id.clone()).await
            {
                let songs = songs.into_iter().map(Into::into).collect();
                let playlist_view_props = PlaylistViewProps {
                    id: playlist.id.clone(),
                    playlist,
                    songs,
                };
                Some(ViewData {
                    playlist: Some(playlist_view_props),
                    ..state.additional_view_data.clone()
                })
            } else {
                Some(ViewData {
                    playlist: None,
                    ..state.additional_view_data.clone()
                })
            }
        }
        ActiveView::DynamicPlaylist(id) => {
            if let Ok((Some(dynamic_playlist), Some(songs))) =
                dynamic_playlist_view_future(daemon, id.clone()).await
            {
                let songs = songs.into_iter().map(Into::into).collect();
                let dynamic_playlist_view_props = DynamicPlaylistViewProps {
                    id: dynamic_playlist.id.clone(),
                    dynamic_playlist,
                    songs,
                };
                Some(ViewData {
                    dynamic_playlist: Some(dynamic_playlist_view_props),
                    ..state.additional_view_data.clone()
                })
            } else {
                Some(ViewData {
                    dynamic_playlist: None,
                    ..state.additional_view_data.clone()
                })
            }
        }
        ActiveView::Collection(id) => {
            if let Ok((Some(collection), Some(songs))) =
                collection_view_future(daemon, id.clone()).await
            {
                let songs = songs.into_iter().map(Into::into).collect();
                let collection_view_props = CollectionViewProps {
                    id: collection.id.clone(),
                    collection,
                    songs,
                };
                Some(ViewData {
                    collection: Some(collection_view_props),
                    ..state.additional_view_data.clone()
                })
            } else {
                Some(ViewData {
                    collection: None,
                    ..state.additional_view_data.clone()
                })
            }
        }
        ActiveView::Radio(ids) => {
            let count = state.settings.tui.radio_count;
            let radio_view_props = if let Ok(resp) = daemon
                .radio_get_similar(RadioSimilarRequest::new(ids.clone(), count))
                .await
            {
                let songs = resp.into_inner().songs;
                Some(RadioViewProps { count, songs })
            } else {
                None
            };
            Some(ViewData {
                radio: radio_view_props,
                ..state.additional_view_data.clone()
            })
        }
        ActiveView::Random => {
            if let Ok((Some(album), Some(artist), Some(song))) =
                random_view_future(daemon.clone()).await
            {
                let random_view_props = RandomViewProps {
                    album: album.id.into(),
                    artist: artist.id.into(),
                    song: song.id.into(),
                };
                Some(ViewData {
                    random: Some(random_view_props),
                    ..state.additional_view_data.clone()
                })
            } else {
                Some(ViewData {
                    random: None,
                    ..state.additional_view_data.clone()
                })
            }
        }

        ActiveView::None
        | ActiveView::Search
        | ActiveView::Songs
        | ActiveView::Albums
        | ActiveView::Artists
        | ActiveView::Playlists
        | ActiveView::DynamicPlaylists
        | ActiveView::Collections => None,
    }
}
