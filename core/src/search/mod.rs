use mecomp_storage::db::schemas::{album::AlbumBrief, artist::ArtistBrief, song::SongBrief};
use serde::{Deserialize, Serialize};

#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum SearchResult {
    Album(AlbumBrief),
    Artist(ArtistBrief),
    Song(SongBrief),
}

// TODO: implement searching (might move to the storage crate)
