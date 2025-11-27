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
