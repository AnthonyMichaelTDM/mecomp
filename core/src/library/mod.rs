use mecomp_storage::db::schemas::{album::Album, artist::Artist, playlist::Playlist, song::Song};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LibraryBrief {
    pub artists: usize,
    pub albums: usize,
    pub songs: usize,
    pub playlists: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LibraryFull {
    pub artists: Box<[Artist]>,
    pub albums: Box<[Album]>,
    pub songs: Box<[Song]>,
    pub playlists: Box<[Playlist]>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LibraryHealth {
    pub artists: usize,
    pub albums: usize,
    pub songs: usize,
    pub playlists: usize,
    pub missing_artists: usize,
    pub missing_albums: usize,
    pub missing_songs: usize,
    pub missing_playlists: usize,
    pub missing_files: usize,
}
