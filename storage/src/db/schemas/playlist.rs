use std::{sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};

use super::song::SongId;

pub type PlaylistId = Thing;

pub const TABLE_NAME: &str = "playlist";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
/// This struct holds all the metadata about a particular [`Playlist`].
/// A [`Playlist`] is a collection of [`Song`]s.
pub struct Playlist {
    /// the unique identifier for this [`Playlist`].
    pub id: PlaylistId,

    /// The [`Artist`]'s name.
    pub name: Arc<str>,

    /// Total runtime.
    pub runtime: Duration,

    /// Keys to every [`Song`] in this [`Playlist`].
    pub songs: Box<[SongId]>,
}

impl Playlist {
    pub fn generate_id() -> PlaylistId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlaylistBrief {
    pub id: PlaylistId,
    pub name: Arc<str>,
    pub runtime: Duration,
    pub songs: usize,
}

impl From<Playlist> for PlaylistBrief {
    fn from(playlist: Playlist) -> Self {
        Self {
            id: playlist.id,
            name: playlist.name,
            runtime: playlist.runtime,
            songs: playlist.songs.len(),
        }
    }
}

impl From<&Playlist> for PlaylistBrief {
    fn from(playlist: &Playlist) -> Self {
        Self {
            id: playlist.id.clone(),
            name: playlist.name.clone(),
            runtime: playlist.runtime,
            songs: playlist.songs.len(),
        }
    }
}
