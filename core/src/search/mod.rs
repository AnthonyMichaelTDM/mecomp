use mecomp_storage::db::schemas::{album::AlbumBrief, artist::ArtistBrief, song::SongBrief};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum SearchResult {
    Album(AlbumBrief),
    Artist(ArtistBrief),
    Song(SongBrief),
}
