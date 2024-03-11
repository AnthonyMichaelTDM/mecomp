//! CRUD operations for the song table

use crate::{
    db::{
        schemas::{
            album::Album,
            artist::Artist,
            collection::Collection,
            playlist::Playlist,
            song::{Song, SongId, TABLE_NAME},
        },
        DB,
    },
    errors::Error,
};

impl Song {
    pub async fn create(song: Song) -> Result<Option<SongId>, Error> {
        let id = DB
            .create((TABLE_NAME, song.id.clone()))
            .content(song)
            .await?
            .map(|x: Song| x.id);
        Ok(id)
    }

    pub async fn read_all() -> Result<Vec<Song>, Error> {
        Ok(DB.select(TABLE_NAME).await?)
    }

    pub async fn read(id: SongId) -> Result<Option<Song>, Error> {
        Ok(DB.select((TABLE_NAME, id)).await?)
    }

    pub async fn update(id: SongId, song: Song) -> Result<(), Error> {
        let result = DB.update((TABLE_NAME, id)).content(song).await?;
        result.ok_or(Error::NotFound)
    }

    /// Delete a song from the database,
    /// will also:
    /// - go through the artist and album tables and remove references to it from there.
    /// - remove the song from playlists.
    /// - remove the song from collections.
    pub async fn delete(id: SongId) -> Result<(), Error> {
        let Some(song) = Song::read(id.clone()).await? else {
            return Ok(());
        };

        // remove the song from the artist's list of songs
        for artist_id in song.artist_id.iter() {
            Artist::remove_song(artist_id.clone(), id.clone()).await?;
        }

        // remove the song from the album's list of songs
        Album::remove_song(song.album_id, id.clone()).await?;

        // remove the song from playlists
        for playlist in Playlist::read_all().await? {
            if playlist.songs.contains(&id) {
                Playlist::remove_song(playlist.id, id.clone()).await?;
            }
        }

        // remove the song from collections
        for collection in Collection::read_all().await? {
            if collection.songs.contains(&id) {
                Collection::remove_song(collection.id, id.clone()).await?;
            }
        }

        let _: Option<Song> = DB.delete((TABLE_NAME, id)).await?;
        Ok(())
    }
}
