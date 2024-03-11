//----------------------------------------------------------------------------------------- std lib
use audiotags::Config;
use std::path::PathBuf;
use std::sync::Arc;
//--------------------------------------------------------------------------------- other libraries
use readable::run::Runtime;
use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};
//----------------------------------------------------------------------------------- local modules
use super::{
    album::{Album, AlbumId},
    artist::{Artist, ArtistId},
};
use crate::{
    errors::{Error, SongIOError},
    util::OneOrMany,
};

pub type SongId = Thing;

pub const TABLE_NAME: &str = "song";

#[derive(Clone, Debug, Deserialize, Serialize)]
/// This struct holds all the metadata about a particular [`Song`].
pub struct Song {
    // / The unique identifier for this [`Song`].
    pub id: SongId,
    /// Title of the [`Song`].
    pub title: Arc<str>,
    /// Artist of the [`Song`]. (Can be multiple)
    pub artist_id: OneOrMany<ArtistId>,
    /// Artist of the [`Song`]. (Can be multiple)
    pub artist: OneOrMany<Arc<str>>,
    /// album artist, if not found then defaults to first artist
    pub album_artist: OneOrMany<Arc<str>>,
    /// album artist id
    pub album_artist_id: OneOrMany<ArtistId>,

    /// Key to the [`Album`].
    pub album_id: AlbumId,
    /// album title
    pub album: Arc<str>,
    /// Genre of the [`Song`]. (Can be multiple)
    pub genre: Option<OneOrMany<Arc<str>>>,

    /// Total runtime of this [`Song`].
    pub duration: Runtime,
    // /// Sample rate of this [`Song`].
    // pub sample_rate: u32,
    /// The track number of this [`Song`].
    pub track: Option<u16>,
    /// The disc number of this [`Song`].
    pub disc: Option<u16>,
    /// the year the song was released
    pub release_year: Option<i32>,

    // /// The `MIME` type of this [`Song`].
    // pub mime: Arc<str>,
    /// The file extension of this [`Song`].
    pub extension: Arc<str>,

    /// The [`PathBuf`] this [`Song`] is located at.
    pub path: PathBuf,
}

impl Song {
    pub fn generate_id() -> SongId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }

    /// Create a new [`Song`] from a file path.
    /// This function will read the metadata from the file and create a new [`Song`] from it.
    /// If the file does not exist, or if the file is not a valid audio file, this function will return an error.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the file.
    /// * `artist_name_separator` - The separator used to separate multiple artists in the metadata.
    /// * `genre_separator` - The separator used to separate multiple genres in the metadata.
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
    pub async fn try_load(metadata: SongMetadata) -> Result<Self, Error> {
        // check if the file exists
        if !metadata.song_exists() {
            return Err(SongIOError::FileNotFound(metadata.path).into());
        }

        // for each artist, check if the artist exists in the database and get the id, if they don't then create a new artist and get the id
        let mut artist_ids = Vec::with_capacity(metadata.artist.len());
        for (i, artist) in metadata.artist.iter().enumerate() {
            if let Some(artist) = Artist::read_by_name(artist.as_ref()).await? {
                artist_ids[i] = artist.id;
            } else {
                let artist_id = Artist::create(Artist {
                    id: Artist::generate_id(),
                    name: artist.clone(),
                    songs: vec![].into_boxed_slice(),
                    albums: vec![].into_boxed_slice(),
                    runtime: 0.into(),
                })
                .await?
                .unwrap();
                artist_ids[i] = artist_id;
            }
        }

        // check if the album artist exists, if they don't then create a new artist and get the id
        let mut album_artist_ids = Vec::with_capacity(metadata.artist.len());
        for (i, artist) in metadata.artist.iter().enumerate() {
            if let Some(artist) = Artist::read_by_name(artist.as_ref()).await? {
                album_artist_ids[i] = artist.id;
            } else {
                let artist_id = Artist::create(Artist {
                    id: Artist::generate_id(),
                    name: artist.clone(),
                    songs: vec![].into_boxed_slice(),
                    albums: vec![].into_boxed_slice(),
                    runtime: 0.into(),
                })
                .await?
                .unwrap();
                album_artist_ids[i] = artist_id;
            }
        }

        // check if the album artist(s) have the album.
        // if they don't then create a new album, assign it to the artist.
        // get the id of the album
        let mut album_id = None;
        for artist_id in artist_ids.iter() {
            let artist = Artist::read(artist_id.clone()).await?.unwrap();

            // try to find the album in the artist's albums
            let mut artist_has_album = false;
            for id in artist.albums.iter() {
                let album = Album::read(id.clone()).await?.unwrap();
                if album.title.as_ref() == metadata.album.as_ref() {
                    artist_has_album = true;
                    if album_id.is_none() {
                        album_id = Some(album.id);
                    }
                    break;
                }
            }

            // if we didn't find the album, create a new album (if we didn't find the album earlier) and assign it to the artist
            if !artist_has_album {
                if album_id.is_none() {
                    album_id = Album::create(Album {
                        id: Album::generate_id(),
                        title: metadata.album.clone(),
                        artist_id: album_artist_ids.clone().into(),
                        artist: metadata.album_artist.clone(),
                        release: metadata.release_year,
                        runtime: 0.into(),
                        song_count: 0,
                        songs: vec![].into_boxed_slice(),
                        discs: 1,
                        genre: None,
                    })
                    .await?;
                }

                let updated_artist = Artist {
                    albums: artist
                        .albums
                        .iter()
                        .cloned()
                        .chain(std::iter::once(album_id.clone().unwrap()))
                        .collect(),
                    ..artist
                };
                Artist::update(artist_id.clone(), updated_artist).await?;
            }
        }
        let album_id = album_id.expect("Album not found or created, shouldn't happen");

        // create a new song
        let song = Self {
            id: Self::generate_id(),
            title: metadata.title,
            artist: metadata.artist,
            artist_id: artist_ids.clone().into(),
            album_artist: metadata.album_artist,
            album_artist_id: album_artist_ids.clone().into(),
            album: metadata.album,
            album_id: album_id.clone(),
            genre: metadata.genre,
            release_year: metadata.release_year,
            duration: metadata.duration,
            extension: metadata.extension,
            track: metadata.track,
            disc: metadata.disc,
            path: metadata.path,
        };
        // add that song to the database
        let song_id = Self::create(song.clone()).await?.unwrap();

        // add the song to the album artists and artists (if it's not already there)
        for artist_id in artist_ids.iter().chain(album_artist_ids.iter()) {
            let artist = Artist::read(artist_id.clone()).await?.unwrap();
            if !artist.songs.contains(&song_id) {
                Artist::update(
                    artist_id.clone(),
                    Artist {
                        songs: artist
                            .songs
                            .iter()
                            .cloned()
                            .chain(std::iter::once(song_id.clone()))
                            .collect(),
                        runtime: artist.runtime + song.duration,
                        ..artist
                    },
                )
                .await?;
            }
        }

        // add the song to the album
        let album = Album::read(album_id.clone()).await?.unwrap();
        if !album.songs.contains(&song_id) {
            Album::update(
                album_id.clone(),
                Album {
                    songs: album
                        .songs
                        .iter()
                        .cloned()
                        .chain(std::iter::once(song_id.clone()))
                        .collect(),
                    runtime: album.runtime + song.duration,
                    song_count: album.song_count + 1,
                    genre: {
                        // add all the genres of the song to the album, if the album doesn't have that genre
                        let mut genres = album.genre.unwrap_or_default();
                        if let Some(song_genres) = song.genre.as_ref() {
                            for genre in song_genres.iter() {
                                if !genres.contains(genre) {
                                    genres.push(genre.clone());
                                }
                            }
                        }
                        Some(genres)
                    },
                    ..album
                },
            )
            .await?;
        }

        Ok(song)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SongBrief {
    pub id: SongId,
    pub title: Arc<str>,
    pub artist: OneOrMany<Arc<str>>,
    pub album: Arc<str>,
    pub album_artist: OneOrMany<Arc<str>>,
    pub release_year: Option<i32>,
    pub duration: Runtime,
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
            duration: song.duration,
            path: song.path,
        }
    }
}

pub struct SongMetadata {
    pub title: Arc<str>,
    pub artist: OneOrMany<Arc<str>>,
    pub album: Arc<str>,
    pub album_artist: OneOrMany<Arc<str>>,
    pub genre: Option<OneOrMany<Arc<str>>>,
    pub duration: Runtime,
    pub release_year: Option<i32>,
    pub track: Option<u16>,
    pub disc: Option<u16>,
    pub extension: Arc<str>,
    pub path: PathBuf,
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
    pub fn song_exists(&self) -> bool {
        self.path.exists() && self.path.is_file()
    }

    pub fn load_from_path(
        path: PathBuf,
        artist_name_separator: Option<&'static str>,
        genre_separator: Option<&'static str>,
    ) -> Result<Self, SongIOError> {
        // check if the file exists
        if !path.exists() || !path.is_file() {
            return Err(SongIOError::FileNotFound(path).into());
        }
        // get metadata from the file
        let tags = audiotags::Tag::default()
            .with_config(if let Some(sep) = artist_name_separator {
                Config::default()
                    .sep_artist(sep)
                    .parse_multiple_artists(true)
            } else {
                Config::default()
            })
            .read_from_path(&path)
            .map_err(|e| SongIOError::AudiotagError(e))?;
        let artist: OneOrMany<Arc<str>> = tags
            .artists()
            .map(|artists| {
                if artists.len() == 1 {
                    OneOrMany::One(artists[0].into())
                } else {
                    OneOrMany::Many(artists.into_iter().map(Into::into).collect())
                }
            })
            .unwrap_or(OneOrMany::One("Unknown Artist".into()));

        Ok(Self {
            title: tags
                .title()
                .map(|x| x.into())
                .unwrap_or_else(|| path.file_stem().unwrap().to_string_lossy())
                .into(),
            album: tags.album_title().unwrap_or("Unknown Album").into(),
            album_artist: tags
                .album_artists()
                .map(|artists| {
                    if artists.len() == 1 {
                        OneOrMany::One(artists[0].into())
                    } else {
                        OneOrMany::Many(artists.into_iter().map(Into::into).collect())
                    }
                })
                .unwrap_or_else(|| OneOrMany::One(artist.get(0).unwrap().clone())),
            artist,
            genre: tags.genre().map(|genre| match (genre_separator, genre) {
                (Some(sep), genre) if genre.contains(sep) => {
                    OneOrMany::Many(genre.split(sep).map(Into::into).collect())
                }
                (_, genre) => OneOrMany::One(genre.into()),
            }),
            duration: tags
                .duration()
                .map(|x| x.into())
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
