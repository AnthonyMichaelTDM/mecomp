#![allow(clippy::module_name_repetitions)]
//----------------------------------------------------------------------------------------- std lib
use std::sync::Arc;
use std::{collections::HashSet, path::PathBuf};
//--------------------------------------------------------------------------------- other libraries
use lofty::{file::TaggedFileExt, prelude::*, probe::Probe, tag::Accessor};
use serde::{Deserialize, Serialize};
use surrealdb::sql::{Duration, Id, Thing};
use surrealdb::{Connection, Surreal};
use surrealqlx::Table;
use tracing::instrument;
//----------------------------------------------------------------------------------- local modules
use super::{album::Album, artist::Artist};
use crate::errors::{Error, SongIOError};
use one_or_many::OneOrMany;

pub type SongId = Thing;

pub const TABLE_NAME: &str = "song";

/// This struct holds all the metadata about a particular [`Song`].
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Table)]
#[Table("song")]
pub struct Song {
    /// The unique identifier for this [`Song`].
    #[field(dt = "record")]
    pub id: SongId,
    /// Title of the [`Song`].
    #[field(dt = "string", index())]
    pub title: Arc<str>,
    /// Artist of the [`Song`]. (Can be multiple)
    #[field(dt = "option<set<string> | string>")]
    #[serde(default)]
    pub artist: OneOrMany<Arc<str>>,
    /// album artist, if not found then defaults to first artist
    #[field(dt = "option<set<string> | string>")]
    #[serde(default)]
    pub album_artist: OneOrMany<Arc<str>>,
    /// album title
    #[field(dt = "string")]
    pub album: Arc<str>,
    /// Genre of the [`Song`]. (Can be multiple)
    #[field(dt = "option<set<string> | string>")]
    #[serde(default)]
    pub genre: OneOrMany<Arc<str>>,

    /// Total runtime of this [`Song`].
    #[field(dt = "duration")]
    pub runtime: Duration,
    // /// Sample rate of this [`Song`].
    // pub sample_rate: u32,
    /// The track number of this [`Song`].
    #[field(dt = "option<int>")]
    #[serde(default)]
    pub track: Option<u16>,
    /// The disc number of this [`Song`].
    #[field(dt = "option<int>")]
    #[serde(default)]
    pub disc: Option<u16>,
    /// the year the song was released
    #[field(dt = "option<int>")]
    #[serde(default)]
    pub release_year: Option<i32>,

    // /// The `MIME` type of this [`Song`].
    // pub mime: Arc<str>,
    /// The file extension of this [`Song`].
    #[field(dt = "string")]
    pub extension: Arc<str>,

    /// The [`PathBuf`] this [`Song`] is located at.
    #[field(dt = "string", index(unique))]
    pub path: PathBuf,
}

impl Song {
    #[must_use]
    pub fn generate_id() -> SongId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }

    /// Create a new [`Song`] from song metadata and load it into the database.
    ///
    /// # Arguments
    ///
    /// * `metadata` - The metadata of the song.
    ///
    /// # Errors
    ///
    /// This function will return an error if the file does not exist, or if the file is not a valid audio file.
    ///
    /// # Side Effects
    ///
    /// This function will create a new [`Song`], [`Artist`], and [`Album`] if they do not exist in the database.
    /// This function will also add the new [`Song`] to the [`Artist`] and the [`Album`].
    /// This function will also update the [`Artist`] and the [`Album`] in the database.
    #[instrument()]
    pub async fn try_load_into_db<C: Connection>(
        db: &Surreal<C>,
        metadata: SongMetadata,
    ) -> Result<Self, Error> {
        // check if the file exists
        if !metadata.path_exists() {
            return Err(SongIOError::FileNotFound(metadata.path).into());
        }

        // for each artist, check if the artist exists in the database and get the id, if they don't then create a new artist and get the id
        let artists = Artist::read_or_create_by_names(db, metadata.artist.clone()).await?;

        // check if the album artist exists, if they don't then create a new artist and get the id
        Artist::read_or_create_by_names(db, metadata.album_artist.clone()).await?;

        // read or create the album
        // if an album doesn't exist with the given title and album artists,
        // will create a new album with the given title and album artists
        let album = Album::read_or_create_by_name_and_album_artist(
            db,
            &metadata.album,
            metadata.album_artist.clone(),
        )
        .await?
        .ok_or(Error::NotCreated)?;

        // create a new song
        let song = Self {
            id: Self::generate_id(),
            title: metadata.title,
            artist: metadata.artist,
            album_artist: metadata.album_artist,
            album: metadata.album,
            genre: metadata.genre,
            release_year: metadata.release_year,
            runtime: metadata.runtime,
            extension: metadata.extension,
            track: metadata.track,
            disc: metadata.disc,
            path: metadata.path,
        };
        // add that song to the database
        let song_id = Self::create(db, song.clone()).await?.unwrap().id;

        // add the song to the artists, if it's not already there (which it won't be)
        for artist in &artists {
            Artist::add_songs(db, artist.id.clone(), &[song_id.clone()]).await?;
        }

        // add the song to the album, if it's not already there (which it won't be)
        Album::add_songs(db, album.id.clone(), &[song_id.clone()]).await?;

        Ok(song)
    }
}

#[derive(Debug, Default, Serialize, Clone)]
pub struct SongChangeSet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Arc<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<OneOrMany<Arc<str>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_artist: Option<OneOrMany<Arc<str>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<Arc<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<OneOrMany<Arc<str>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<Duration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track: Option<Option<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc: Option<Option<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_year: Option<Option<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension: Option<Arc<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
            duration: song.runtime.into(),
            path: song.path.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
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
    #[instrument()]
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
        artist_name_separator: Option<&str>,
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

        let artist: OneOrMany<Arc<str>> =
            tag.artist()
                .as_deref()
                .map_or(OneOrMany::One("Unknown Artist".into()), |a| {
                    let a = a.replace('\0', "");
                    if let Some(sep) = artist_name_separator {
                        if a.contains(sep) {
                            OneOrMany::Many(a.split(&sep).map(Into::into).collect())
                        } else {
                            OneOrMany::One(a.into())
                        }
                    } else {
                        OneOrMany::One(a.into())
                    }
                });

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
                .into(),
            album_artist: tag.get_string(&ItemKey::AlbumArtist).map_or_else(
                || OneOrMany::One(artist.get(0).unwrap().clone()),
                |a| {
                    let a = a.replace('\0', "");
                    if let Some(sep) = artist_name_separator {
                        if a.contains(sep) {
                            OneOrMany::Many(a.split(&sep).map(Into::into).collect())
                        } else {
                            OneOrMany::One(a.into())
                        }
                    } else {
                        OneOrMany::One(a.into())
                    }
                },
            ),
            artist,
            genre: tag
                .genre()
                .map(|genre| match (genre_separator, genre) {
                    (Some(sep), genre) if genre.contains(sep) => OneOrMany::Many(
                        genre.replace('\0', "").split(sep).map(Into::into).collect(),
                    ),
                    (_, genre) => OneOrMany::One(genre.into()),
                })
                .into(),
            runtime: properties.duration().into(),
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
    use crate::{
        db::{
            init_test_database,
            schemas::{album::Album, artist::Artist},
        },
        test_utils::{arb_song_case, song_metadata_from_case, ulid},
    };

    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_try_load_into_db() {
        let db = init_test_database().await.unwrap();
        // Create a mock SongMetadata object for testing
        let metadata = song_metadata_from_case(arb_song_case()(), &ulid()).unwrap();

        // Call the try_load_into_db function
        let result = Song::try_load_into_db(&db, metadata.clone()).await;

        // Assert that the function returns a valid Song object
        if let Err(e) = result {
            panic!("Error: {e:?}");
        }
        let song = result.unwrap();

        // Assert that the song has been loaded into the database correctly
        assert_eq!(song.title, metadata.title);
        assert_eq!(song.artist.len(), metadata.artist.len());
        assert_eq!(song.album_artist.len(), metadata.album_artist.len());
        assert_eq!(song.album, metadata.album);
        assert_eq!(song.genre.len(), metadata.genre.len());
        assert_eq!(song.runtime, metadata.runtime);
        assert_eq!(song.track, metadata.track);
        assert_eq!(song.disc, metadata.disc);
        assert_eq!(song.release_year, metadata.release_year);
        assert_eq!(song.extension, metadata.extension);
        assert_eq!(song.path, metadata.path);

        // Assert that the artists and album have been created in the database
        let artists = Song::read_artist(&db, song.id.clone()).await.unwrap();
        assert_eq!(artists.len(), metadata.artist.len()); // 2 artists + 1 album artist

        let album = Song::read_album(&db, song.id.clone()).await;
        assert_eq!(album.is_ok(), true);
        let album = album.unwrap();
        assert_eq!(album.is_some(), true);
        let album = album.unwrap();

        // Assert that the song has been associated with the artists and album correctly
        let artist_songs = Artist::read_songs(&db, artists.get(0).unwrap().id.clone())
            .await
            .unwrap();
        assert_eq!(artist_songs.len(), 1);
        assert_eq!(artist_songs[0].id, song.id);

        let album_songs = Album::read_songs(&db, album.id.clone()).await.unwrap();
        assert_eq!(album_songs.len(), 1);
        assert_eq!(album_songs[0].id, song.id);
    }
}
