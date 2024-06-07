#![allow(clippy::module_name_repetitions)]
//----------------------------------------------------------------------------------------- std lib
use std::sync::Arc;
use std::{collections::HashSet, path::PathBuf};
//--------------------------------------------------------------------------------- other libraries
#[cfg(not(feature = "surrealdb"))]
use crate::surreal::Thing;
#[cfg(not(feature = "surrealdb"))]
use std::time::Duration;
#[cfg(feature = "surrealdb")]
use surrealdb::sql::{Duration, Id, Thing};
//----------------------------------------------------------------------------------- local modules
use one_or_many::OneOrMany;

pub type SongId = Thing;

pub const TABLE_NAME: &str = "song";

/// This struct holds all the metadata about a particular [`Song`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "surrealdb", derive(surrealqlx::Table))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "surrealdb", Table("song"))]
pub struct Song {
    /// The unique identifier for this [`Song`].
    #[cfg_attr(feature = "surrealdb", field(dt = "record"))]
    pub id: SongId,
    /// Title of the [`Song`].
    #[cfg_attr(feature = "surrealdb", field(dt = "string", index()))]
    pub title: Arc<str>,
    /// Artist of the [`Song`]. (Can be multiple)
    #[cfg_attr(feature = "surrealdb", field(dt = "option<set<string> | string>"))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub artist: OneOrMany<Arc<str>>,
    /// album artist, if not found then defaults to first artist
    #[cfg_attr(feature = "surrealdb", field(dt = "option<set<string> | string>"))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub album_artist: OneOrMany<Arc<str>>,
    /// album title
    #[cfg_attr(feature = "surrealdb", field(dt = "string"))]
    pub album: Arc<str>,
    /// Genre of the [`Song`]. (Can be multiple)
    #[cfg_attr(feature = "surrealdb", field(dt = "option<set<string> | string>"))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub genre: OneOrMany<Arc<str>>,

    /// Total runtime of this [`Song`].
    #[cfg_attr(feature = "surrealdb", field(dt = "duration"))]
    pub runtime: Duration,
    // /// Sample rate of this [`Song`].
    // pub sample_rate: u32,
    /// The track number of this [`Song`].
    #[cfg_attr(feature = "surrealdb", field(dt = "option<int>"))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub track: Option<u16>,
    /// The disc number of this [`Song`].
    #[cfg_attr(feature = "surrealdb", field(dt = "option<int>"))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub disc: Option<u16>,
    /// the year the song was released
    #[cfg_attr(feature = "surrealdb", field(dt = "option<int>"))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub release_year: Option<i32>,

    // /// The `MIME` type of this [`Song`].
    // pub mime: Arc<str>,
    /// The file extension of this [`Song`].
    #[cfg_attr(feature = "surrealdb", field(dt = "string"))]
    pub extension: Arc<str>,

    /// The [`PathBuf`] this [`Song`] is located at.
    #[cfg_attr(feature = "surrealdb", field(dt = "string", index(unique)))]
    pub path: PathBuf,
}

impl Song {
    #[must_use]
    #[cfg(feature = "surrealdb")]
    pub fn generate_id() -> SongId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SongChangeSet {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub title: Option<Arc<str>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub artist: Option<OneOrMany<Arc<str>>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub album_artist: Option<OneOrMany<Arc<str>>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub album: Option<Arc<str>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub genre: Option<OneOrMany<Arc<str>>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub runtime: Option<Duration>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub track: Option<Option<u16>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub disc: Option<Option<u16>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub release_year: Option<Option<i32>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub extension: Option<Arc<str>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SongBrief {
    pub id: SongId,
    pub title: Arc<str>,
    pub artist: OneOrMany<Arc<str>>,
    pub album: Arc<str>,
    pub album_artist: OneOrMany<Arc<str>>,
    pub release_year: Option<i32>,
    pub duration: std::time::Duration,
    pub path: PathBuf,
}

impl From<Song> for SongBrief {
    fn from(song: Song) -> Self {
        Self {
            id: song.id,
            title: song.title,
            artist: song.artist,
            album: song.album,
            album_artist: song.album_artist,
            release_year: song.release_year,
            #[cfg(not(feature = "surrealdb"))]
            duration: song.runtime,
            #[cfg(feature = "surrealdb")]
            duration: song.runtime.into(),
            path: song.path,
        }
    }
}

impl From<&Song> for SongBrief {
    fn from(song: &Song) -> Self {
        Self {
            id: song.id.clone(),
            title: song.title.clone(),
            artist: song.artist.clone(),
            album: song.album.clone(),
            album_artist: song.album_artist.clone(),
            release_year: song.release_year,
            #[cfg(not(feature = "surrealdb"))]
            duration: song.runtime,
            #[cfg(feature = "surrealdb")]
            duration: song.runtime.into(),
            path: song.path.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SongMetadata {
    pub title: Arc<str>,
    pub artist: OneOrMany<Arc<str>>,
    pub album: Arc<str>,
    pub album_artist: OneOrMany<Arc<str>>,
    pub genre: OneOrMany<Arc<str>>,
    pub runtime: Duration,
    pub release_year: Option<i32>,
    pub track: Option<u16>,
    pub disc: Option<u16>,
    pub extension: Arc<str>,
    pub path: PathBuf,
}

impl From<&Song> for SongMetadata {
    fn from(song: &Song) -> Self {
        Self {
            title: song.title.clone(),
            artist: song.artist.clone(),
            album: song.album.clone(),
            album_artist: song.album_artist.clone(),
            genre: song.genre.clone(),
            runtime: song.runtime,
            track: song.track,
            disc: song.disc,
            release_year: song.release_year,
            extension: song.extension.clone(),
            path: song.path.clone(),
        }
    }
}

impl From<Song> for SongMetadata {
    fn from(song: Song) -> Self {
        Self {
            title: song.title,
            artist: song.artist,
            album: song.album,
            album_artist: song.album_artist,
            genre: song.genre,
            runtime: song.runtime,
            track: song.track,
            disc: song.disc,
            release_year: song.release_year,
            extension: song.extension,
            path: song.path,
        }
    }
}

impl SongMetadata {
    #[must_use]
    pub fn path_exists(&self) -> bool {
        self.path.exists() && self.path.is_file()
    }

    /// Check if the metadata of this song is likely the same song as the metadata of the other song.
    ///
    /// doesn't check for exact equality (use `==` for that),
    /// but is for checking if the song is the same song even if the metadata has been updated.
    #[must_use]
    #[allow(clippy::suspicious_operation_groupings)]
    pub fn is_same_song(&self, other: &Self) -> bool {
        // the title is the same
        self.title == other.title
            // the artist is the same
            && self.artist == other.artist
            // the album is the same
            && self.album == other.album
            // the duration is the same
            && self.runtime == other.runtime
            // the genre is the same, or the genre is not in self but is in other
            && (self.genre == other.genre || (self.genre.is_none() && other.genre.is_some()))
            // the track is the same, or the track is not in self but is in other
            && (self.track == other.track || (self.track.is_none() && other.track.is_some()))
            // the disc is the same, or the disc is not in self but is in other
            && (self.disc == other.disc || (self.disc.is_none() && other.disc.is_some()))
            // the release year is the same, or the release year is not in self but is in other
            && (self.release_year == other.release_year
                || (self.release_year.is_none() && other.release_year.is_some()))
    }

    /// Merge the metadata of two songs.
    /// This function will merge the metadata of two songs into a new song metadata.
    ///
    /// for fields that can't be merged (like the title, album, or duration), the metadata of `self` will be used.
    ///
    /// Therefore, you should check that the songs are the same song before merging (use `is_same_song`).
    #[must_use]
    pub fn merge(base: &Self, other: &Self) -> Self {
        Self {
            title: base.title.clone(),
            // merge the artists, if the artist is in `self` and not in `other`, then add it to the merged metadata
            artist: {
                base.artist
                    .iter()
                    .chain(other.artist.iter())
                    .cloned()
                    .collect::<HashSet<_>>() // remove duplicates
                    .into_iter()
                    .collect()
            },
            album: base.album.clone(),
            album_artist: base
                .album_artist
                .iter()
                .chain(other.album_artist.iter())
                .cloned()
                .collect::<HashSet<_>>() // remove duplicates
                .into_iter()
                .collect(),
            genre: base
                .genre
                .iter()
                .chain(other.genre.iter())
                .cloned()
                .collect::<HashSet<Arc<str>>>()
                .into_iter()
                .collect(),
            runtime: base.runtime,
            track: base.track.or(other.track),
            disc: base.disc.or(other.disc),
            release_year: base.release_year.or(other.release_year),
            extension: base.extension.clone(),
            path: base.path.clone(),
        }
    }

    /// create a changeset from the difference between `self` and `song`
    #[must_use]
    pub fn merge_with_song(&self, song: &Song) -> SongChangeSet {
        let mut changeset = SongChangeSet::default();

        if self.title != song.title {
            changeset.title = Some(self.title.clone());
        }
        if self.artist != song.artist {
            changeset.artist = Some(self.artist.clone());
        }
        if self.album_artist != song.album_artist {
            changeset.album_artist = Some(self.album_artist.clone());
        }
        if self.genre != song.genre {
            changeset.genre = Some(self.genre.clone());
        }
        if self.runtime != song.runtime {
            changeset.runtime = Some(self.runtime);
        }
        if self.track != song.track {
            changeset.track = Some(self.track);
        }
        if self.disc != song.disc {
            changeset.disc = Some(self.disc);
        }
        if self.release_year != song.release_year {
            changeset.release_year = Some(self.release_year);
        }
        if self.extension != song.extension {
            changeset.extension = Some(self.extension.clone());
        }
        if self.path != song.path {
            changeset.path = Some(self.path.clone());
        }

        changeset
    }
}
