//! CRUD operations for the song table

use std::path::PathBuf;

use tracing::instrument;

use crate::{
    db::{
        db,
        schemas::{
            album::Album,
            artist::Artist,
            song::{Song, SongChangeSet, SongId, TABLE_NAME},
        },
    },
    errors::Error,
    util::OneOrMany,
};

impl Song {
    #[instrument]
    pub async fn create(song: Song) -> Result<Option<SongId>, Error> {
        let id = db()
            .await?
            .create((TABLE_NAME, song.id.clone()))
            .content(song)
            .await?
            .map(|x: Song| x.id);
        Ok(id)
    }

    #[instrument]
    pub async fn read_all() -> Result<Vec<Song>, Error> {
        Ok(db().await?.select(TABLE_NAME).await?)
    }

    #[instrument]
    pub async fn read(id: SongId) -> Result<Option<Song>, Error> {
        Ok(db().await?.select((TABLE_NAME, id)).await?)
    }

    #[instrument]
    pub async fn read_by_path(path: PathBuf) -> Result<Option<Song>, Error> {
        Ok(db()
            .await?
            .query("SELECT * FROM song WHERE path = $path LIMIT 1")
            .bind(("path", path))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn read_album(id: SongId) -> Result<Option<Album>, Error> {
        Ok(db()
            .await?
            .query("SELECT <-album_to_song<-album FROM $id")
            .bind(("id", id))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn read_artist(id: SongId) -> Result<OneOrMany<Artist>, Error> {
        let res: Vec<Artist> = db()
            .await?
            .query("SELECT <-artist_to_song<-artist FROM $id")
            .bind(("id", id))
            .await?
            .take(0)?;

        Ok(res.into())
    }

    #[instrument]
    pub async fn read_album_artist(id: SongId) -> Result<OneOrMany<Artist>, Error> {
        let res: Vec<Artist> = db()
            .await?
            .query("SELECT <-album_to_song<-album<-artist_to_album<-artist FROM $id")
            .bind(("id", id))
            .await?
            .take(0)?;

        Ok(res.into())
    }

    /// Update the information about a song, repairs relations if necessary
    ///
    /// repairs relations if:
    /// - the artist name(s) have changed
    /// - the album name has changed
    /// - the album artist name(s) have changed
    /// - TODO: The duration has changed
    #[instrument]
    pub async fn update(id: SongId, changes: SongChangeSet) -> Result<(), Error> {
        if changes.album.is_some() || changes.album_artist.is_some() {
            let old_album: Option<Album> = db()
                .await?
                .query("SELECT <-album_to_song<-album FROM $id")
                .bind(("id", id.clone()))
                .await?
                .take(0)?;
            let old_album = old_album.ok_or(Error::NotFound)?;

            // find/create the new album
            let new_album = match (&changes.album, &changes.album_artist) {
                (Some(album), Some(album_artist)) => {
                    Album::read_or_create_by_name_and_album_artist(&album, album_artist.to_owned())
                        .await?
                }
                (Some(album), None) => {
                    Album::read_or_create_by_name_and_album_artist(
                        &album,
                        old_album.artist.to_owned(),
                    )
                    .await?
                }
                (None, Some(album_artist)) => {
                    // find/create the new album
                    Album::read_or_create_by_name_and_album_artist(
                        &old_album.title,
                        album_artist.to_owned(),
                    )
                    .await?
                }
                (None, None) => unreachable!(),
            }
            .ok_or(Error::NotFound)?;

            // remove song from the old album
            Album::remove_songs(old_album.id, &[id.clone()]).await?;

            // add song to the new album
            Album::add_songs(new_album.id, &[id.clone()]).await?;
        }

        if let Some(artist) = &changes.artist {
            let old_artist: Vec<Artist> = db()
                .await?
                .query("SELECT <-artist_to_song<-artist FROM $id")
                .bind(("id", id.clone()))
                .await?
                .take(0)?;
            // find/create artists with the new names
            let new_artist = Artist::read_or_create_by_names(artist.clone()).await?;

            // remove song from the old artists
            for artist in old_artist.into_iter() {
                Artist::remove_songs(artist.id, &[id.clone()]).await?;
            }
            // add song to the new artists
            for artist in new_artist.into_iter() {
                Artist::add_songs(artist.id, &[id.clone()]).await?;
            }
        }

        db().await?
            .query(format!("UPDATE type::record($id) MERGE $changes"))
            .bind(("id", &id))
            .bind(("changes", &changes))
            .await?;

        Ok(())
    }

    /// Delete a song from the database,
    /// will also:
    /// - go through the artist and album tables and remove references to it from there.
    /// - remove the song from playlists.
    /// - remove the song from collections.
    #[instrument]
    pub async fn delete(id: SongId) -> Result<(), Error> {
        let _: Option<Song> = db().await?.delete((TABLE_NAME, id)).await?;
        Ok(())
    }
}
