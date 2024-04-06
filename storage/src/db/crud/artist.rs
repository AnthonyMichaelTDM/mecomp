//! CRUD operations for the artist table
use std::sync::Arc;

use surrealdb::sql::Duration;
use tracing::instrument;

use crate::{
    db::{
        db,
        schemas::{
            album::{Album, AlbumId},
            artist::{Artist, ArtistChangeSet, ArtistId, TABLE_NAME},
            song::{Song, SongId},
        },
    },
    errors::Error,
    util::OneOrMany,
};

impl Artist {
    #[instrument]
    pub async fn create(artist: Artist) -> Result<Option<Artist>, Error> {
        Ok(db()
            .await?
            .create((TABLE_NAME, artist.id.clone()))
            .content(artist)
            .await?)
    }

    #[instrument]
    pub async fn read_or_create_by_name(name: &str) -> Result<Option<Artist>, Error> {
        if let Some(artist) = Artist::read_by_name(name).await? {
            Ok(Some(artist))
        } else {
            Artist::create(Artist {
                id: Artist::generate_id(),
                name: name.into(),
                song_count: 0,
                album_count: 0,
                runtime: Duration::from_secs(0),
            })
            .await
        }
    }

    #[instrument]
    pub async fn read_or_create_by_names(names: OneOrMany<Arc<str>>) -> Result<Vec<Artist>, Error> {
        let mut artists = Vec::with_capacity(names.len());
        for name in names.iter() {
            if let Some(id) = Artist::read_or_create_by_name(name).await? {
                artists.push(id);
            }
        }
        Ok(artists)
    }

    pub async fn read_by_name(name: &str) -> Result<Option<Artist>, Error> {
        Ok(db()
            .await?
            .query("SELECT * FROM artist WHERE name = $name LIMIT 1")
            .bind(("name", name))
            .await?
            .take(0)?)
    }

    pub async fn read_by_names(names: &[Arc<str>]) -> Result<Vec<Artist>, Error> {
        // select artists records whose `name` field is in $names
        Ok(db()
            .await?
            .query("SELECT * FROM artist WHERE name IN $names")
            .bind(("names", names))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn read_all() -> Result<Vec<Artist>, Error> {
        Ok(db().await?.select(TABLE_NAME).await?)
    }

    #[instrument]
    pub async fn read(id: ArtistId) -> Result<Option<Artist>, Error> {
        Ok(db().await?.select((TABLE_NAME, id)).await?)
    }

    #[instrument]
    pub async fn read_one_or_many(ids: OneOrMany<ArtistId>) -> Result<OneOrMany<Artist>, Error> {
        match ids {
            OneOrMany::One(id) => Ok(Artist::read(id).await?.into()),
            OneOrMany::Many(ids) => Artist::read_many(ids).await.map(|v| v.into()),
            OneOrMany::None => Ok(OneOrMany::None),
        }
    }

    #[instrument]
    pub async fn read_many(ids: Vec<ArtistId>) -> Result<Vec<Artist>, Error> {
        Ok(db()
            .await?
            .query("SELECT * FROM $ids")
            .bind(("ids", ids))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn update(id: ArtistId, artist: ArtistChangeSet) -> Result<(), Error> {
        db().await?
            .query("UPDATE $id MERGE $artist")
            .bind(("id", &id))
            .bind(("artist", &artist))
            .await?;
        Ok(())
    }

    #[instrument]
    pub async fn read_albums(id: ArtistId) -> Result<Vec<Album>, Error> {
        Ok(db()
            .await?
            .query("SELECT ->artist_to_album FROM $id")
            .bind(("id", id))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn add_album(id: ArtistId, album_id: AlbumId) -> Result<(), Error> {
        db().await?
            // relate this artist to the album
            .query("RELATE $id->artist_to_album->$album;")
            // relate this artist to the songs in the album
            .query("RELATE $id->artist_to_song->(SELECT ->album_to_song<-album FROM $album);")
            // update runtime, and song/album count
            .query("UPDATE $id SET album_count += 1, runtime += (SELECT runtime FROM $album LIMIT 1)[0], songs += (SELECT song_count FROM $album LIMIT 1)[0];")
            .bind(("id", &id))
            .bind(("album", &album_id))
            .await?;

        Ok(())
    }

    #[instrument]
    pub async fn add_album_to_artists(ids: &[ArtistId], album_id: AlbumId) -> Result<(), Error> {
        db().await?
            // relate this artist to the album
            .query("RELATE $ids->artist_to_album->$album")
            // update runtime, and song/album count
            .query("UPDATE $ids SET album_count += 1, runtime += (SELECT runtime FROM $album LIMIT 1), songs += (SELECT song_count FROM $album LIMIT 1)")
            .bind(("ids", &ids))
            .bind(("album", &album_id))
            .await?;

        Ok(())
    }

    #[instrument]
    pub async fn add_songs(id: ArtistId, songs: &[SongId]) -> Result<(), Error> {
        db().await?
            // relate this artist to these songs
            .query("RELATE $id->artist_to_song->$songs")
            // update runtime, and song count
            .query("UPDATE $ids SET runtime += math::sum(SELECT runtime FROM $songs), songs += array::len($songs)")
            .bind(("id", &id))
            .bind(("songs", songs))
            .await?;

        Ok(())
    }

    #[instrument]
    pub async fn remove_songs(id: ArtistId, song_ids: &[SongId]) -> Result<(), Error> {
        for song in song_ids {
            let _ = db()
                .await?
                .query("DELETE $artist->album_to_song WHERE out=$song")
                .query(
                    "UPDATE $artist SET song_count-=1, runtime-=(SELECT runtime FROM ONLY $song LIMIT 1)",
                )
                .bind(("artist", &id))
                .bind(("song", song))
                .await?;
        }
        Ok(())
    }

    /// updates the album count, song count, and runtime of the artist, removes the artist if they have no songs or albums
    ///
    /// # Arguments
    ///
    /// * `id` - the id of the artist to repair
    ///
    /// # Returns
    ///
    /// * `bool` - whether the artist was removed or not (if it has no songs or albums, it should be removed)
    #[instrument]
    pub async fn repair(id: ArtistId) -> Result<bool, Error> {
        let album_count: Option<usize> = db()
            .await?
            .query("RETURN array::len(SELECT ->artist_to_album FROM $artist)")
            .bind(("artist", id.clone()))
            .await?
            .take(0)?;

        let songs: Vec<Song> = db().await?
            .query("RETURN array::union((SELECT ->artist_to_song FROM $artist), (SELECT ->album_to_song<-album FROM artist_to_album<-$artist))")
            .bind(("artist", id.clone())).await?.take(0)?;

        if album_count.is_none() && songs.is_empty() {
            let _: Option<Artist> = db().await?.delete((TABLE_NAME, id)).await?;
            Ok(true)
        } else {
            let mut runtime = Duration::from_secs(0);
            let mut song_count = 0;
            for song in songs {
                runtime = runtime + song.duration;
                song_count += 1;
            }

            db().await?.query("UPDATE $artist SET album_count=$album_count, song_count=$song_count, runtime=$runtime")
                .bind(("artist", id.clone()))
                .bind(("album_count", album_count))
                .bind(("song_count", song_count))
                .bind(("runtime", runtime))
                .await?;

            Ok(false)
        }
    }
}
