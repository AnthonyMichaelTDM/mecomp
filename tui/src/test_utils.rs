use mecomp_core::{rpc::SearchResult, state::library::LibraryBrief};
use mecomp_storage::db::schemas::{
    Id, RecordId,
    album::{Album, AlbumBrief},
    artist::{Artist, ArtistBrief},
    collection::{Collection, CollectionBrief},
    dynamic::DynamicPlaylist,
    playlist::{Playlist, PlaylistBrief},
    song::{Song, SongBrief},
};
use one_or_many::OneOrMany;
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

use crate::{
    state::component::ActiveComponent,
    ui::{
        AppState,
        components::content_view::views::{
            AlbumViewProps, ArtistViewProps, CollectionViewProps, DynamicPlaylistViewProps,
            PlaylistViewProps, RadioViewProps, RandomViewProps, SongViewProps, ViewData,
        },
    },
};

/// Setup a test terminal with the given width and height.
///
/// # Panics
///
/// Panics if the terminal cannot be created.
pub fn setup_test_terminal(width: u16, height: u16) -> (Terminal<TestBackend>, Rect) {
    let backend = TestBackend::new(width, height);
    (
        Terminal::new(backend).unwrap(),
        Rect::new(0, 0, width, height),
    )
}

/// check if the area and content (raw text) of two buffers are the same
pub fn assert_buffer_eq(buffer: &ratatui::buffer::Buffer, expected: &ratatui::buffer::Buffer) {
    // if this is false, the test passes
    if buffer.area() != expected.area()
        || !buffer
            .content()
            .iter()
            .zip(expected.content().iter())
            .all(|(a, b)| a.symbol() == b.symbol())
    {
        // otherwise, let's "assert" that they are the same, simply so that `pretty_assertions::assert_eq` will print the diff
        pretty_assertions::assert_eq!(buffer, expected);
    }
}

/// the id used for all the items in this fake library
pub fn item_id() -> Id {
    Id::String("01J1K5B6RJ84WJXCWYJ5WNE12E".into())
}

/// Create an `AppState` that has the 1 of every type of item in the library (song, artist, album, ...)
/// and `ContentView` as the active component, also, every view has data to display.
#[allow(clippy::too_many_lines)]
pub fn state_with_everything() -> AppState {
    let album_id = RecordId::from(("album", item_id()));
    let artist_id = RecordId::from(("artist", item_id()));
    let collection_id = RecordId::from(("collection", item_id()));
    let playlist_id = RecordId::from(("playlist", item_id()));
    let song_id = RecordId::from(("song", item_id()));
    let dynamic_id = RecordId::from(("dynamic", item_id()));

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
    let song_brief: SongBrief = song.clone().into();
    let artist = Artist {
        id: artist_id.clone().into(),
        name: song.artist[0].clone(),
        runtime: song.runtime,
        album_count: 1,
        song_count: 1,
    };
    let artist_brief: ArtistBrief = artist.clone().into();
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
    let album_brief: AlbumBrief = album.clone().into();
    let collection = Collection {
        id: collection_id.clone().into(),
        name: "Collection 0".into(),
        runtime: song.runtime,
        song_count: 1,
    };
    let collection_brief: CollectionBrief = collection.clone().into();
    let playlist = Playlist {
        id: playlist_id.clone().into(),
        name: "Test Playlist".into(),
        runtime: song.runtime,
        song_count: 1,
    };
    let playlist_brief: PlaylistBrief = playlist.clone().into();
    let dynamic = DynamicPlaylist {
        id: dynamic_id.clone().into(),
        name: "Test Dynamic".into(),
        query: "title = \"Test Song\"".parse().unwrap(),
    };

    AppState {
        active_component: ActiveComponent::ContentView,
        library: LibraryBrief {
            artists: vec![artist_brief.clone()].into_boxed_slice(),
            albums: vec![album_brief.clone()].into_boxed_slice(),
            songs: vec![song_brief.clone()].into_boxed_slice(),
            playlists: vec![playlist_brief.clone()].into_boxed_slice(),
            collections: vec![collection_brief.clone()].into_boxed_slice(),
            dynamic_playlists: vec![dynamic.clone()].into_boxed_slice(),
        },
        additional_view_data: ViewData {
            random: Some(RandomViewProps {
                album: album_id.clone(),
                artist: artist_id.clone(),
                song: song_id.clone(),
            }),
            album: Some(AlbumViewProps {
                id: album_id,
                album,
                artists: OneOrMany::One(artist_brief.clone()),
                songs: vec![song_brief.clone()].into_boxed_slice(),
            }),
            artist: Some(ArtistViewProps {
                id: artist_id,
                artist,
                albums: vec![album_brief.clone()].into_boxed_slice(),
                songs: vec![song_brief.clone()].into_boxed_slice(),
            }),
            song: Some(SongViewProps {
                id: song_id,
                song,
                artists: OneOrMany::One(artist_brief.clone()),
                album: album_brief.clone(),
                playlists: vec![playlist_brief].into_boxed_slice(),
                collections: vec![collection_brief].into_boxed_slice(),
            }),
            collection: Some(CollectionViewProps {
                id: collection_id,
                collection,
                songs: vec![song_brief.clone()].into_boxed_slice(),
            }),
            playlist: Some(PlaylistViewProps {
                id: playlist_id,
                playlist,
                songs: vec![song_brief.clone()].into_boxed_slice(),
            }),
            dynamic_playlist: Some(DynamicPlaylistViewProps {
                id: dynamic_id,
                dynamic_playlist: dynamic,
                songs: vec![song_brief.clone()].into_boxed_slice(),
            }),
            radio: Some(RadioViewProps {
                count: 1,
                songs: vec![song_brief.clone()].into_boxed_slice(),
            }),
        },
        search: SearchResult {
            songs: vec![song_brief].into_boxed_slice(),
            albums: vec![album_brief].into_boxed_slice(),
            artists: vec![artist_brief].into_boxed_slice(),
        },
        ..Default::default()
    }
}
