use std::sync::Arc;

use readable::run::Runtime;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use super::song::SongId;

pub type PlaylistId = Thing;

pub const TABLE_NAME: &str = "playlist";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
/// This struct holds all the metadata about a particular [`Playlist`].
/// A [`Playlist`] is a collection of [`Song`]s.
pub struct Playlist {
    /// the unique identifier for this [`Playlist`].
    pub id: Option<PlaylistId>,

    /// The [`Artist`]'s name.
    pub name: Arc<str>,

    /// Total runtime.
    pub runtime: Runtime,

    /// Keys to every [`Song`] in this [`Playlist`].
    pub songs: Vec<SongId>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlaylistBrief {
    pub id: PlaylistId,
    pub name: Arc<str>,
    pub runtime: Runtime,
    pub songs: usize,
}

impl From<Playlist> for PlaylistBrief {
    fn from(playlist: Playlist) -> Self {
        Self {
            id: playlist.id.expect("Playlist has no id"),
            name: playlist.name,
            runtime: playlist.runtime,
            songs: playlist.songs.len(),
        }
    }
}
