#![allow(clippy::module_name_repetitions)]
use std::sync::Arc;

#[cfg(not(feature = "surrealdb"))]
use crate::surreal::Thing;
#[cfg(not(feature = "surrealdb"))]
use std::time::Duration;
#[cfg(feature = "surrealdb")]
use surrealdb::sql::{Duration, Id, Thing};

use one_or_many::OneOrMany;

pub type AlbumId = Thing;

pub const TABLE_NAME: &str = "album";

/// This struct holds all the metadata about a particular [`Album`].
/// An [`Album`] is a collection of [`super::song::Song`]s owned by an [`super::artist::Artist`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "surrealdb", derive(surrealqlx::Table))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "surrealdb", Table("album"))]
pub struct Album {
    /// The unique identifier for this [`Album`].
    #[cfg_attr(feature = "surrealdb", field(dt = "record"))]
    pub id: AlbumId,
    /// Title of the [`Album`].
    #[cfg_attr(feature = "surrealdb", field(dt = "string", index()))]
    pub title: Arc<str>,
    /// Artist of the [`Album`]. (Can be multiple)
    #[cfg_attr(feature = "surrealdb", field(dt = "option<set<string> | string>"))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub artist: OneOrMany<Arc<str>>,
    /// Release year of this [`Album`].
    #[cfg_attr(feature = "surrealdb", field(dt = "option<int>"))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub release: Option<i32>,
    /// Total runtime of this [`Album`].
    #[cfg_attr(feature = "surrealdb", field(dt = "duration"))]
    pub runtime: Duration,
    /// [`Song`] count of this [`Album`].
    #[cfg_attr(feature = "surrealdb", field(dt = "int"))]
    pub song_count: usize,
    /// How many discs are in this [`Album`]?
    /// (Most will only have 1).
    #[cfg_attr(feature = "surrealdb", field(dt = "int"))]
    pub discs: u32,
    /// This [`Album`]'s genre.
    #[cfg_attr(feature = "surrealdb", field(dt = "option<set<string> | string>"))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub genre: OneOrMany<Arc<str>>,
}

impl Album {
    #[must_use]
    #[cfg(feature = "surrealdb")]
    pub fn generate_id() -> AlbumId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct AlbumChangeSet {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub title: Option<Arc<str>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub artist: Option<OneOrMany<Arc<str>>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub release: Option<Option<i32>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub runtime: Option<Duration>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub song_count: Option<usize>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub discs: Option<u32>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub genre: Option<OneOrMany<Arc<str>>>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlbumBrief {
    pub id: AlbumId,
    pub title: Arc<str>,
    pub artist: OneOrMany<Arc<str>>,
    pub release: Option<i32>,
    pub runtime: std::time::Duration,
    pub song_count: usize,
    pub discs: u32,
    pub genre: OneOrMany<Arc<str>>,
}

impl From<Album> for AlbumBrief {
    fn from(album: Album) -> Self {
        Self {
            id: album.id,
            title: album.title,
            artist: album.artist,
            release: album.release,
            #[cfg(not(feature = "surrealdb"))]
            runtime: album.runtime,
            #[cfg(feature = "surrealdb")]
            runtime: album.runtime.into(),
            song_count: album.song_count,
            discs: album.discs,
            genre: album.genre,
        }
    }
}

impl From<&Album> for AlbumBrief {
    fn from(album: &Album) -> Self {
        Self {
            id: album.id.clone(),
            title: album.title.clone(),
            artist: album.artist.clone(),
            release: album.release,
            #[cfg(not(feature = "surrealdb"))]
            runtime: album.runtime,
            #[cfg(feature = "surrealdb")]
            runtime: album.runtime.into(),
            song_count: album.song_count,
            discs: album.discs,
            genre: album.genre.clone(),
        }
    }
}
