#![allow(clippy::module_name_repetitions)]
//----------------------------------------------------------------------------------------- std lib
use std::path::PathBuf;
use std::sync::Arc;
//--------------------------------------------------------------------------------- other libraries
#[cfg(not(feature = "db"))]
use super::{Id, Thing};
use lofty::{file::TaggedFileExt, prelude::*, probe::Probe, tag::Accessor};
use std::time::Duration;
#[cfg(feature = "db")]
use surrealdb::sql::{Id, Thing};
use tracing::instrument;
//----------------------------------------------------------------------------------- local modules
use crate::errors::SongIOError;
use one_or_many::OneOrMany;

pub type SongId = Thing;

pub const TABLE_NAME: &str = "song";

/// This struct holds all the metadata about a particular [`Song`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "db", derive(surrealqlx::Table))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "db", Table("song"))]
pub struct Song {
    /// The unique identifier for this [`Song`].
    #[cfg_attr(feature = "db", field("any"))]
    pub id: SongId,
    /// Title of the [`Song`].
    #[cfg_attr(feature = "db", field(dt = "string", index(text("custom_analyzer"))))]
    pub title: Arc<str>,
    /// Artist of the [`Song`]. (Can be multiple)
    #[cfg_attr(
        feature = "db",
        field(dt = "option<set<string> | string>", index(text("custom_analyzer")))
    )]
    #[cfg_attr(feature = "serde", serde(default))]
    pub artist: OneOrMany<Arc<str>>,
    /// album artist, if not found then defaults to first artist
    #[cfg_attr(feature = "db", field(dt = "option<set<string> | string>"))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub album_artist: OneOrMany<Arc<str>>,
    /// album title
    #[cfg_attr(feature = "db", field(dt = "string"))]
    pub album: Arc<str>,
    /// Genre of the [`Song`]. (Can be multiple)
    #[cfg_attr(feature = "db", field(dt = "option<set<string> | string>"))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub genre: OneOrMany<Arc<str>>,

    /// Total runtime of this [`Song`].
    #[cfg_attr(feature = "db", field(dt = "duration"))]
    #[cfg_attr(
        feature = "db",
        serde(
            serialize_with = "super::serialize_duration_as_sql_duration",
            deserialize_with = "super::deserialize_duration_from_sql_duration"
        )
    )]
    pub runtime: Duration,
    // /// Sample rate of this [`Song`].
    // pub sample_rate: u32,
    /// The track number of this [`Song`].
    #[cfg_attr(feature = "db", field(dt = "option<int>"))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub track: Option<u16>,
    /// The disc number of this [`Song`].
    #[cfg_attr(feature = "db", field(dt = "option<int>"))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub disc: Option<u16>,
    /// the year the song was released
    #[cfg_attr(feature = "db", field(dt = "option<int>"))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub release_year: Option<i32>,

    // /// The `MIME` type of this [`Song`].
    // pub mime: Arc<str>,
    /// The file extension of this [`Song`].
    #[cfg_attr(feature = "db", field(dt = "string"))]
    pub extension: Arc<str>,

    /// The [`PathBuf`] this [`Song`] is located at.
    #[cfg_attr(feature = "db", field(dt = "string", index(unique)))]
    pub path: PathBuf,
}

impl Song {
    #[must_use]
    #[inline]
    pub fn generate_id() -> SongId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
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
    #[cfg_attr(
        feature = "db",
        serde(serialize_with = "super::serialize_duration_option_as_sql_duration")
    )]
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

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SongBrief {
    pub id: SongId,
    pub title: Arc<str>,
    pub artist: OneOrMany<Arc<str>>,
    pub album: Arc<str>,
    pub album_artist: OneOrMany<Arc<str>>,
    pub release_year: Option<i32>,
    pub runtime: std::time::Duration,
    pub path: PathBuf,
}

impl From<Song> for SongBrief {
    #[inline]
    fn from(song: Song) -> Self {
        Self {
            id: song.id,
            title: song.title,
            artist: song.artist,
            album: song.album,
            album_artist: song.album_artist,
            release_year: song.release_year,
            runtime: song.runtime,
            path: song.path,
        }
    }
}

impl From<&Song> for SongBrief {
    #[inline]
    fn from(song: &Song) -> Self {
        let song = song.clone();
        Self::from(song)
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
    #[inline]
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
    #[inline]
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
    #[inline]
    pub fn path_exists(&self) -> bool {
        self.path.exists() && self.path.is_file()
    }

    /// create a changeset from the difference between `self` and `song`
    #[instrument()]
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

    /// Load a [`SongMetadata`] from a file path.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the file.
    /// * `artist_name_separator` - The separator used to separate multiple artists in the metadata.
    /// * `genre_separator` - The separator used to separate multiple genres in the metadata.
    ///
    #[instrument()]
    pub fn load_from_path(
        path: PathBuf,
        artist_name_separator: &OneOrMany<String>,
        genre_separator: Option<&str>,
    ) -> Result<Self, SongIOError> {
        // check if the file exists
        if !path.exists() || !path.is_file() {
            return Err(SongIOError::FileNotFound(path));
        }
        // get metadata from the file
        let tagged_file = Probe::open(&path)
            .map_err(SongIOError::LoftyError)?
            .read()
            .map_err(SongIOError::LoftyError)?;
        let properties = tagged_file.properties();

        let tag = match tagged_file.primary_tag() {
            Some(primary_tag) => primary_tag,
            // If the "primary" tag doesn't exist, we just grab the
            // first tag we can find. Realistically, a tag reader would likely
            // iterate through the tags to find a suitable one.
            None => tagged_file.first_tag().ok_or(SongIOError::MissingTags)?,
        };

        let mut artist: OneOrMany<Arc<str>> = tag
            .artist()
            .as_deref()
            // split the artist string into multiple artists using user provided separators
            .map_or_else(
                || OneOrMany::One("Unknown Artist".into()),
                |a| {
                    // first we remove null characters from the string
                    // then, we replace all instances of any separator with a single separator (in this case, the null character)
                    // I'll use a fold here to make that all nice and pretty.
                    let a = artist_name_separator
                        .iter()
                        .fold(a.replace('\0', ""), |a, sep| a.replace(sep, "\0"));

                    // now we split the string into multiple artists
                    if a.contains('\0') {
                        OneOrMany::Many(a.split('\0').map(str::trim).map(Into::into).collect())
                    } else {
                        OneOrMany::One(a.trim().into())
                    }
                },
            );
        artist.dedup();

        let mut album_artist = tag.get_string(&ItemKey::AlbumArtist).map_or_else(
            || OneOrMany::One(artist.get(0).unwrap().clone()),
            |a| {
                let a = artist_name_separator
                    .iter()
                    .fold(a.replace('\0', ""), |a, sep| a.replace(sep, "\0"));

                if a.contains('\0') {
                    OneOrMany::Many(a.split('\0').map(str::trim).map(Into::into).collect())
                } else {
                    OneOrMany::One(a.trim().into())
                }
            },
        );
        album_artist.dedup();

        let mut genre: OneOrMany<_> = tag
            .genre()
            .map(|genre| match (genre_separator, genre) {
                (Some(sep), genre) if genre.contains(sep) => OneOrMany::Many(
                    genre
                        .replace('\0', "")
                        .split(sep)
                        .map(str::trim)
                        .map(Into::into)
                        .collect(),
                ),
                (_, genre) => OneOrMany::One(genre.trim().into()),
            })
            .into();
        genre.dedup();

        Ok(Self {
            title: tag
                .title()
                .map_or_else(
                    || path.file_stem().unwrap().to_string_lossy(),
                    |x| x.replace('\0', "").into(),
                )
                .into(),
            album: tag
                .album()
                .map_or("Unknown Album".into(), |x| x.replace('\0', ""))
                .trim()
                .into(),
            album_artist,
            artist,
            genre,
            runtime: properties.duration(),
            track: tag
                .get_string(&ItemKey::TrackNumber)
                .and_then(|x| x.parse().ok()),
            disc: tag
                .get_string(&ItemKey::DiscNumber)
                .and_then(|x| x.parse().ok()),
            release_year: tag.get_string(&ItemKey::Year).and_then(|x| x.parse().ok()),
            extension: path
                .extension()
                .expect("File without extension")
                .to_string_lossy()
                .into(),
            path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
    use rstest::{fixture, rstest};

    #[fixture]
    fn song() -> Song {
        Song {
            id: Thing::from((TABLE_NAME, "id")),
            title: Arc::from("song"),
            artist: OneOrMany::One(Arc::from("artist")),
            album_artist: OneOrMany::One(Arc::from("artist")),
            album: Arc::from("album"),
            genre: OneOrMany::One(Arc::from("genre")),
            runtime: Duration::from_secs(3600),
            track: Some(1),
            disc: Some(1),
            release_year: Some(2021),
            extension: Arc::from("mp3"),
            path: PathBuf::from("path"),
        }
    }

    #[fixture]
    fn song_brief() -> SongBrief {
        SongBrief {
            id: Thing::from((TABLE_NAME, "id")),
            title: Arc::from("song"),
            artist: OneOrMany::One(Arc::from("artist")),
            album: Arc::from("album"),
            album_artist: OneOrMany::One(Arc::from("artist")),
            release_year: Some(2021),
            runtime: Duration::from_secs(3600),
            path: PathBuf::from("path"),
        }
    }

    #[rstest]
    #[case(song(), song_brief())]
    #[case(&song(), song_brief())]
    fn test_song_brief_from_song<T: Into<SongBrief>>(#[case] song: T, #[case] brief: SongBrief) {
        let actual: SongBrief = song.into();
        assert_eq!(actual, brief);
    }

    #[rstest]
    #[case::same(SongMetadata {
        title: Arc::from("song"),
        artist: OneOrMany::One(Arc::from("artist")),
        album_artist: OneOrMany::One(Arc::from("artist")),
        album: Arc::from("album"),
        genre: OneOrMany::One(Arc::from("genre")),
        runtime: Duration::from_secs(3600),
        track: Some(1),
        disc: Some(1),
        release_year: Some(2021),
        extension: Arc::from("mp3"),
        path: PathBuf::from("path"),
    },
    Song {
        id: Thing::from((TABLE_NAME, "id")),
        title: Arc::from("song"),
        artist: OneOrMany::One(Arc::from("artist")),
        album_artist: OneOrMany::One(Arc::from("artist")),
        album: Arc::from("album"),
        genre: OneOrMany::One(Arc::from("genre")),
        runtime: Duration::from_secs(3600),
        track: Some(1),
        disc: Some(1),
        release_year: Some(2021),
        extension: Arc::from("mp3"),
        path: PathBuf::from("path"),
    },
    SongChangeSet::default())]
    #[case::different(SongMetadata {
        title: Arc::from("song 2"),
        artist: OneOrMany::One(Arc::from("artist")),
        album_artist: OneOrMany::One(Arc::from("artist")),
        album: Arc::from("album"),
        genre: OneOrMany::One(Arc::from("rock")),
        runtime: Duration::from_secs(3000),
        track: Some(1),
        disc: Some(3),
        release_year: Some(2021),
        extension: Arc::from("mp3"),
        path: PathBuf::from("path"),
    },
    Song {
        id: Thing::from((TABLE_NAME, "id")),
        title: Arc::from("song"),
        artist: OneOrMany::One(Arc::from("artist")),
        album_artist: OneOrMany::One(Arc::from("artist")),
        album: Arc::from("album"),
        genre: OneOrMany::One(Arc::from("genre")),
        runtime: Duration::from_secs(3600),
        track: Some(1),
        disc: Some(1),
        release_year: Some(2021),
        extension: Arc::from("mp3"),
        path: PathBuf::from("path"),
    },
    SongChangeSet{
        title: Some(Arc::from("song 2")),
        genre: Some(OneOrMany::One(Arc::from("rock"))),
        runtime: Some(Duration::from_secs(3000)),
        disc: Some(Some(3)),
        ..Default::default()
    })]
    fn test_merge_with_song(
        #[case] base: SongMetadata,
        #[case] other: Song,
        #[case] expected: SongChangeSet,
    ) {
        let actual = base.merge_with_song(&other);
        assert_eq!(actual, expected);
    }
}
