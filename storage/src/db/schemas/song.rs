//----------------------------------------------------------------------------------------- std lib
use std::sync::Arc;
use std::{collections::HashSet, path::PathBuf};
//--------------------------------------------------------------------------------- other libraries
use metadata::media_file::MediaFileMetadata;
use serde::{Deserialize, Serialize};
use surrealdb::sql::{Duration, Id, Thing};
use surrealqlx::Table;
use tracing::instrument;
//----------------------------------------------------------------------------------- local modules
use super::{album::Album, artist::Artist};
use crate::{
    errors::{Error, SongIOError},
    util::OneOrMany,
};

pub type SongId = Thing;

pub const TABLE_NAME: &str = "song";

/// This struct holds all the metadata about a particular [`Song`].
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, Table)]
#[Table("song")]
pub struct Song {
    // / The unique identifier for this [`Song`].
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
    pub duration: Duration,
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
    #[instrument(skip(metadata))]
    pub async fn try_load_into_db(metadata: SongMetadata) -> Result<Self, Error> {
        // check if the file exists
        if !metadata.path_exists() {
            return Err(SongIOError::FileNotFound(metadata.path).into());
        }

        // for each artist, check if the artist exists in the database and get the id, if they don't then create a new artist and get the id
        let mut artists = Vec::with_capacity(metadata.artist.len());
        for artist in metadata.artist.iter() {
            if let Some(artist) = Artist::read_or_create_by_name(artist.as_ref()).await? {
                artists.push(artist);
            }
        }

        // check if the album artist exists, if they don't then create a new artist and get the id
        let mut album_artist_ids = Vec::with_capacity(metadata.artist.len());
        for artist in metadata.album_artist.iter() {
            if let Some(artist) = Artist::read_or_create_by_name(artist.as_ref()).await? {
                album_artist_ids.push(artist.id);
            }
        }

        // check if the album artist(s) have the album.
        // if they don't then create a new album, assign it to the artist.
        // get the id of the album
        let mut album = None;
        for artist_id in album_artist_ids.iter() {
            // try to find the album in the artist's albums
            let mut artist_has_album = false;
            for artist_album in Artist::read_albums(artist_id.clone()).await? {
                if artist_album.title.as_ref() == metadata.album.as_ref() {
                    artist_has_album = true;
                    if album.is_none() {
                        album = Some(artist_album);
                    }
                    break;
                }
            }

            // if we found the album, continue
            if artist_has_album {
                continue;
            }
            // if we didn't find the album, create a new album (if we haven't already)
            if album.is_none() {
                album = Album::create(Album {
                    id: Album::generate_id(),
                    title: metadata.album.clone(),
                    artist: metadata.album_artist.clone(),
                    release: metadata.release_year,
                    runtime: Duration::from_secs(0),
                    song_count: 0,
                    discs: 1,
                    genre: OneOrMany::None,
                })
                .await?;
                for artist_id in album_artist_ids.iter() {
                    let id = album.as_ref().unwrap().id.clone();
                    Artist::add_album(id, artist_id.clone()).await?;
                }
            };
        }
        let album = album.expect("Album not found or created, shouldn't happen");

        // create a new song
        let song = Self {
            id: Self::generate_id(),
            title: metadata.title,
            artist: metadata.artist,
            album_artist: metadata.album_artist,
            album: metadata.album,
            genre: metadata.genre,
            release_year: metadata.release_year,
            duration: metadata.duration,
            extension: metadata.extension,
            track: metadata.track,
            disc: metadata.disc,
            path: metadata.path,
        };
        // add that song to the database
        let song_id = Self::create(song.clone()).await?.unwrap().id;

        // add the song to the artists, if it's not already there
        for artist in artists.iter() {
            Artist::add_songs(artist.id.clone(), &[song_id.clone()]).await?;
        }

        // add the song to the album, if it's not already there
        Album::add_songs(album.id.clone(), &[song_id.clone()]).await?;

        // add the album to the album artist, if it's not already there
        Artist::add_album_to_artists(&album_artist_ids, album.id).await?;

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
    pub duration: Option<Duration>,
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
            duration: song.duration.into(),
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
            duration: song.duration.into(),
            path: song.path.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SongMetadata {
    pub title: Arc<str>,
    pub artist: OneOrMany<Arc<str>>,
    pub album: Arc<str>,
    pub album_artist: OneOrMany<Arc<str>>,
    pub genre: OneOrMany<Arc<str>>,
    pub duration: Duration,
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
            duration: song.duration,
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
            duration: song.duration,
            track: song.track,
            disc: song.disc,
            release_year: song.release_year,
            extension: song.extension,
            path: song.path,
        }
    }
}

impl SongMetadata {
    pub fn path_exists(&self) -> bool {
        self.path.exists() && self.path.is_file()
    }

    /// Check if the metadata of this song is likely the same song as the metadata of the other song.
    ///
    /// doesn't check for exact equality (use `==` for that),
    /// but is for checking if the song is the same song even if the metadata has been updated.
    pub fn is_same_song(&self, other: &Self) -> bool {
        // the title is the same
        self.title == other.title
            // the artist is the same
            && self.artist == other.artist
            // the album is the same
            && self.album == other.album
            // the duration is the same
            && self.duration == other.duration
            // the genre is the same, or the genre is not in self but is in other
            && (self.genre == other.genre || self.genre.is_none() && other.genre.is_some())
            // the track is the same, or the track is not in self but is in other
            && (self.track == other.track || self.track.is_none() && other.track.is_some())
            // the disc is the same, or the disc is not in self but is in other
            && (self.disc == other.disc || self.disc.is_none() && other.disc.is_some())
            // the release year is the same, or the release year is not in self but is in other
            && (self.release_year == other.release_year
                || self.release_year.is_none() && other.release_year.is_some())
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
            duration: base.duration,
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
        if self.duration != song.duration {
            changeset.duration = Some(self.duration);
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
        let tags = audiotags::Tag::default()
            .read_from_path(&path)
            .map_err(SongIOError::AudiotagError)?;
        let artist: OneOrMany<Arc<str>> = tags
            .artist()
            .map(|a| {
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
            })
            .unwrap_or(OneOrMany::One("Unknown Artist".into()));

        Ok(Self {
            title: tags
                .title()
                .map(|x| x.replace('\0', "").into())
                .unwrap_or_else(|| path.file_stem().unwrap().to_string_lossy())
                .into(),
            album: tags
                .album_title()
                .map(|x| x.replace('\0', ""))
                .unwrap_or("Unknown Album".into())
                .into(),
            album_artist: tags
                .album_artist()
                .map(|a| {
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
                })
                .unwrap_or_else(|| OneOrMany::One(artist.get(0).unwrap().clone())),
            artist,
            genre: tags
                .genre()
                .map(|genre| match (genre_separator, genre) {
                    (Some(sep), genre) if genre.contains(sep) => OneOrMany::Many(
                        genre.replace('\0', "").split(sep).map(Into::into).collect(),
                    ),
                    (_, genre) => OneOrMany::One(genre.into()),
                })
                .into(),
            duration: MediaFileMetadata::new(&path)
                .map_err(|_| SongIOError::DurationReadError)
                .map(|x| {
                    x._duration
                        .map(|d| Duration::from(std::time::Duration::from_secs_f64(d)))
                })?
                .ok_or(SongIOError::DurationNotFound)?,
            track: tags.track_number(),
            disc: tags.disc_number(),
            release_year: tags.year(),
            extension: path
                .extension()
                .expect("File without extension")
                .to_string_lossy()
                .into(),
            path,
        })
    }
}
