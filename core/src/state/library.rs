use mecomp_storage::db::schemas::{
    album::{Album, AlbumBrief},
    artist::{Artist, ArtistBrief},
    collection::{Collection, CollectionBrief},
    dynamic::DynamicPlaylist,
    playlist::{Playlist, PlaylistBrief},
    song::{Song, SongBrief},
};
use serde::{Deserialize, Serialize};

/// A brief representation of the library
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct LibraryBrief {
    pub artists: Box<[ArtistBrief]>,
    pub albums: Box<[AlbumBrief]>,
    pub songs: Box<[SongBrief]>,
    pub playlists: Box<[PlaylistBrief]>,
    pub collections: Box<[CollectionBrief]>,
    pub dynamic_playlists: Box<[DynamicPlaylist]>,
}

/// A full representation of the library
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct LibraryFull {
    pub artists: Box<[Artist]>,
    pub albums: Box<[Album]>,
    pub songs: Box<[Song]>,
    pub playlists: Box<[Playlist]>,
    pub collections: Box<[Collection]>,
    pub dynamic_playlists: Box<[DynamicPlaylist]>,
}

/// Health information about the library
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LibraryHealth {
    /// The number of artists in the library
    pub artists: usize,
    /// The number of albums in the library
    pub albums: usize,
    /// The number of songs in the library
    pub songs: usize,
    /// The number of unanalyzed songs in the library
    /// Optional because the analysis feature may not be enabled for the daemon
    pub unanalyzed_songs: Option<usize>,
    /// The number of playlists in the library
    pub playlists: usize,
    /// The number of collections in the library
    pub collections: usize,
    /// The number of dynamic playlists in the library
    pub dynamic_playlists: usize,
    /// The number of orphaned songs in the library
    /// This is the number of artists that have no songs, and no albums
    pub orphaned_artists: usize,
    /// The number of orphaned albums in the library
    /// This is the number of albums that have no songs
    pub orphaned_albums: usize,
    /// The number of orphaned playlists in the library
    /// This is the number of playlists that have no songs
    pub orphaned_playlists: usize,
    /// The number of orphaned collections in the library
    /// This is the number of collections that have no songs
    pub orphaned_collections: usize,
    // TODO: implement counting of missing items
    // /// The number of missing artists in the library
    // /// This is the number of artists of songs/albums that are not in the library
    // pub missing_artists: usize,
    // /// The number of missing albums in the library
    // /// This is the number of albums of songs that are not in the library
    // pub missing_albums: usize,
    // /// The number of missing songs in the library
    // /// This is the number of songs that are not in the library
    // pub missing_songs: usize,
    // /// The number of missing playlists in the library
    // /// This is the number of playlists that are not in the library
    // pub missing_playlists: usize,
    // /// The number of missing collections in the library
    // /// This is the number of collections that are not in the library
    // pub missing_files: usize,
}
