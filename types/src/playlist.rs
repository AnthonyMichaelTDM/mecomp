#![allow(clippy::module_name_repetitions)]
use std::sync::Arc;

use surrealdb::sql::{Duration, Id, Thing};
use surrealqlx::Table;

pub type PlaylistId = Thing;

pub const TABLE_NAME: &str = "playlist";

/// This struct holds all the metadata about a particular [`Playlist`].
/// A [`Playlist`] is a collection of [`super::song::Song`]s.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "surrealdb", derive(Table))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "surrealdb", Table("playlist"))]
pub struct Playlist {
    /// the unique identifier for this [`Playlist`].
    #[field(dt = "record")]
    pub id: PlaylistId,

    /// The [`Playlist`]'s name.
    #[field(dt = "string")]
    pub name: Arc<str>,

    /// Total runtime.
    #[field(dt = "duration")]
    pub runtime: Duration,

    /// the number of songs this playlist has.
    #[field(dt = "int")]
    pub song_count: usize,
}

impl Playlist {
    #[must_use]
    pub fn generate_id() -> PlaylistId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct PlaylistChangeSet {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub name: Option<Arc<str>>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlaylistBrief {
    pub id: PlaylistId,
    pub name: Arc<str>,
    pub runtime: std::time::Duration,
    pub songs: usize,
}

impl From<Playlist> for PlaylistBrief {
    fn from(playlist: Playlist) -> Self {
        Self {
            id: playlist.id,
            name: playlist.name,
            runtime: playlist.runtime.into(),
            songs: playlist.song_count,
        }
    }
}

impl From<&Playlist> for PlaylistBrief {
    fn from(playlist: &Playlist) -> Self {
        Self {
            id: playlist.id.clone(),
            name: playlist.name.clone(),
            runtime: playlist.runtime.into(),
            songs: playlist.song_count,
        }
    }
}
