use mecomp_core::{rpc::SearchResult, state::library::LibraryFull};
use mecomp_storage::db::schemas::{
    album::Album, artist::Artist, collection::Collection, playlist::Playlist, song::Song, Id, Thing,
};
use one_or_many::OneOrMany;
use ratatui::{backend::TestBackend, Terminal};

use crate::ui::{
    app::ActiveComponent,
    components::content_view::views::{
        AlbumViewProps, ArtistViewProps, CollectionViewProps, PlaylistViewProps, RadioViewProps,
        SongViewProps, ViewData,
    },
    AppState,
};

/// Setup a test terminal with the given width and height.
///
/// # Panics
///
/// Panics if the terminal cannot be created.
pub fn setup_test_terminal(width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let terminal = Terminal::new(backend).unwrap();
    terminal
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
pub fn state_with_everything() -> AppState {
    let album_id = Thing::from(("album", item_id()));
    let artist_id = Thing::from(("artist", item_id()));
    let collection_id = Thing::from(("collection", item_id()));
    let playlist_id = Thing::from(("playlist", item_id()));
    let song_id = Thing::from(("song", item_id()));

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
    let artist = Artist {
        id: artist_id.clone().into(),
        name: song.artist[0].clone(),
        runtime: song.runtime.clone(),
        album_count: 1,
        song_count: 1,
    };
    let album = Album {
        id: album_id.clone().into(),
        title: song.album.clone(),
        artist: song.artist.clone(),
        release: song.release_year,
        runtime: song.runtime.clone(),
        song_count: 1,
        discs: 1,
        genre: song.genre.clone(),
    };
    let collection = Collection {
        id: collection_id.clone().into(),
        name: "Collection 0".into(),
        runtime: song.runtime.clone(),
        song_count: 1,
    };
    let playlist = Playlist {
        id: playlist_id.clone().into(),
        name: "Test Playlist".into(),
        runtime: song.runtime.clone(),
        song_count: 1,
    };

    AppState {
        active_component: ActiveComponent::ContentView,
        library: LibraryFull {
            artists: vec![artist.clone()].into_boxed_slice(),
            albums: vec![album.clone()].into_boxed_slice(),
            songs: vec![song.clone()].into_boxed_slice(),
            playlists: vec![playlist.clone()].into_boxed_slice(),
            collections: vec![collection.clone()].into_boxed_slice(),
        },
        additional_view_data: ViewData {
            album: Some(AlbumViewProps {
                id: album_id.into(),
                album: album.clone(),
                artists: OneOrMany::One(artist.clone()),
                songs: vec![song.clone()].into_boxed_slice(),
            }),
            artist: Some(ArtistViewProps {
                id: artist_id.into(),
                artist: artist.clone(),
                albums: vec![album.clone()].into_boxed_slice(),
                songs: vec![song.clone()].into_boxed_slice(),
            }),
            collection: Some(CollectionViewProps {
                id: collection_id.into(),
                collection: collection.clone(),
                songs: vec![song.clone()].into_boxed_slice(),
            }),
            playlist: Some(PlaylistViewProps {
                id: playlist_id.into(),
                playlist: playlist.clone(),
                songs: vec![song.clone()].into_boxed_slice(),
            }),
            song: Some(SongViewProps {
                id: song_id.into(),
                song: song.clone(),
                artists: OneOrMany::One(artist.clone()),
                album: album.clone(),
            }),
            radio: Some(RadioViewProps {
                count: 1,
                songs: vec![song.clone()].into_boxed_slice(),
            }),
        },
        search: SearchResult {
            songs: vec![song.clone()].into_boxed_slice(),
            albums: vec![album.clone()].into_boxed_slice(),
            artists: vec![artist.clone()].into_boxed_slice(),
        },
        ..Default::default()
    }
}
