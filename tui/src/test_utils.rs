use mecomp_prost::{
    Album, AlbumBrief, Artist, ArtistBrief, Collection, CollectionBrief, DynamicPlaylist,
    LibraryBrief, Playlist, PlaylistBrief, RecordId, SearchResult, Song, SongBrief, Ulid,
    convert_std_duration,
};
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
pub fn item_id() -> Ulid {
    Ulid::new("01J1K5B6RJ84WJXCWYJ5WNE12E")
}

/// Create an `AppState` that has the 1 of every type of item in the library (song, artist, album, ...)
/// and `ContentView` as the active component, also, every view has data to display.
#[allow(clippy::too_many_lines)]
pub fn state_with_everything() -> AppState {
    let album_id = RecordId::new("album", item_id());
    let artist_id = RecordId::new("artist", item_id());
    let collection_id = RecordId::new("collection", item_id());
    let playlist_id = RecordId::new("playlist", item_id());
    let song_id = RecordId::new("song", item_id());
    let dynamic_id = RecordId::new("dynamic", item_id());

    let song = Song {
        id: song_id.clone().into(),
        title: "Test Song".into(),
        artists: vec!["Test Artist".to_string()],
        album_artists: vec!["Test Artist".to_string()],
        album: "Test Album".into(),
        genres: vec!["Test Genre".to_string()],
        runtime: convert_std_duration(std::time::Duration::from_secs(180)),
        track: Some(0),
        disc: Some(0),
        release_year: Some(2021),
        extension: "mp3".into(),
        path: "test.mp3".into(),
    };
    let song_brief: SongBrief = song.clone().into();
    let artist = Artist {
        id: artist_id.clone().into(),
        name: song.artists[0].clone(),
        runtime: song.runtime,
        album_count: 1,
        song_count: 1,
    };
    let artist_brief: ArtistBrief = artist.clone().into();
    let album = Album {
        id: album_id.clone().into(),
        title: song.album.clone(),
        artists: vec![song.artists[0].clone()],
        release: song.release_year,
        runtime: song.runtime,
        song_count: 1,
        discs: 1,
        genres: song.genres.clone(),
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
        query: "title = \"Test Song\"".into(),
    };

    AppState {
        active_component: ActiveComponent::ContentView,
        library: LibraryBrief {
            artists: vec![artist_brief.clone()],
            albums: vec![album_brief.clone()],
            songs: vec![song_brief.clone()],
            playlists: vec![playlist_brief.clone()],
            collections: vec![collection_brief.clone()],
            dynamic_playlists: vec![dynamic.clone()],
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
                artists: vec![artist_brief.clone()],
                songs: vec![song_brief.clone()],
            }),
            artist: Some(ArtistViewProps {
                id: artist_id,
                artist,
                albums: vec![album_brief.clone()],
                songs: vec![song_brief.clone()],
            }),
            song: Some(SongViewProps {
                id: song_id,
                song,
                artists: vec![artist_brief.clone()],
                album: album_brief.clone(),
                playlists: vec![playlist.clone()],
                collections: vec![collection.clone()],
            }),
            collection: Some(CollectionViewProps {
                id: collection_id,
                collection,
                songs: vec![song_brief.clone()],
            }),
            playlist: Some(PlaylistViewProps {
                id: playlist_id,
                playlist,
                songs: vec![song_brief.clone()],
            }),
            dynamic_playlist: Some(DynamicPlaylistViewProps {
                id: dynamic_id,
                dynamic_playlist: dynamic,
                songs: vec![song_brief.clone()],
            }),
            radio: Some(RadioViewProps {
                count: 1,
                songs: vec![song_brief.clone()],
            }),
        },
        search: SearchResult {
            songs: vec![song_brief],
            albums: vec![album_brief],
            artists: vec![artist_brief],
        },
        ..Default::default()
    }
}
