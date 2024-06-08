#![allow(clippy::module_name_repetitions)]
use std::sync::Arc;

#[cfg(not(any(test, feature = "db")))]
use super::Thing;
#[cfg(not(any(test, feature = "db")))]
use std::time::Duration;
#[cfg(any(test, feature = "db"))]
use surrealdb::sql::{Duration, Id, Thing};

pub type ArtistId = Thing;

pub const TABLE_NAME: &str = "artist";

/// This struct holds all the metadata about a particular [`Artist`].
/// An [`Artist`] is a collection of [`super::album::Album`]s.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(any(test, feature = "db"), derive(surrealqlx::Table))]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(any(test, feature = "db"), Table("artist"))]
pub struct Artist {
    /// the unique identifier for this [`Artist`].
    #[cfg_attr(any(test, feature = "db"), field(dt = "record"))]
    pub id: ArtistId,

    /// The [`Artist`]'s name.
    #[cfg_attr(any(test, feature = "db"), field(dt = "string", index(unique)))]
    pub name: Arc<str>,

    /// Total runtime.
    #[cfg_attr(any(test, feature = "db"), field(dt = "duration"))]
    pub runtime: Duration,

    /// the number of albums this artist has.
    #[cfg_attr(any(test, feature = "db"), field(dt = "int"))]
    pub album_count: usize,

    /// the number of songs this artist has.
    #[cfg_attr(any(test, feature = "db"), field(dt = "int"))]
    pub song_count: usize,
}

impl Artist {
    #[must_use]
    #[cfg(any(test, feature = "db"))]
    pub fn generate_id() -> ArtistId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }
}

#[derive(Debug, Default)]
#[cfg_attr(any(test, feature = "serde"), derive(serde::Serialize))]
pub struct ArtistChangeSet {
    #[cfg_attr(
        any(test, feature = "serde"),
        serde(skip_serializing_if = "Option::is_none")
    )]
    pub name: Option<Arc<str>>,
    #[cfg_attr(
        any(test, feature = "serde"),
        serde(skip_serializing_if = "Option::is_none")
    )]
    pub runtime: Option<Duration>,
    #[cfg_attr(
        any(test, feature = "serde"),
        serde(skip_serializing_if = "Option::is_none")
    )]
    pub album_count: Option<usize>,
    #[cfg_attr(
        any(test, feature = "serde"),
        serde(skip_serializing_if = "Option::is_none")
    )]
    pub song_count: Option<usize>,
}

/// This struct holds all the metadata about a particular [`Artist`].
/// An [`Artist`] is a collection of [`super::album::Album`]s.
#[derive(Clone, Debug)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct ArtistBrief {
    pub id: ArtistId,
    pub name: Arc<str>,
    pub runtime: std::time::Duration,
    pub albums: usize,
    pub songs: usize,
}

impl From<Artist> for ArtistBrief {
    fn from(artist: Artist) -> Self {
        Self {
            id: artist.id,
            name: artist.name,
            #[cfg(not(any(test, feature = "db")))]
            runtime: artist.runtime,
            #[cfg(any(test, feature = "db"))]
            runtime: artist.runtime.into(),
            albums: artist.album_count,
            songs: artist.song_count,
        }
    }
}

impl From<&Artist> for ArtistBrief {
    fn from(artist: &Artist) -> Self {
        Self {
            id: artist.id.clone(),
            name: artist.name.clone(),
            #[cfg(not(any(test, feature = "db")))]
            runtime: artist.runtime,
            #[cfg(any(test, feature = "db"))]
            runtime: artist.runtime.into(),
            albums: artist.album_count,
            songs: artist.song_count,
        }
    }
}
