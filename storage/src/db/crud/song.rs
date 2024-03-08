//! CRUD operations for the song table

use crate::{
    db::{
        schemas::{
            artist::Artist,
            song::{Song, SongId, TABLE_NAME},
        },
        DB,
    },
    errors::Error,
};

impl Song {
    pub async fn read_all() -> Result<Vec<Song>, Error> {
        Ok(DB.select(TABLE_NAME).await?)
    }

    pub async fn read(id: SongId) -> Result<Option<Song>, Error> {
        Ok(DB.select((TABLE_NAME, id)).await?)
    }

    /// Delete a song from the database,
    /// will also:
    /// - go through the artist and album tables and remove references to it from there.
    /// - remove the song from playlists.
    pub async fn delete(id: SongId) -> Result<(), Error> {
        let Some(song) = Song::read(id.clone()).await? else {
            return Ok(());
        };

        // remove the song from the artist's list of songs
        for artist_id in song.artist_ids {
            let mut artist = Artist::read(artist_id).await?;
            if let Some(artist) = &mut artist {
                artist.remove_song(id.clone()).await?;
            }
        }

        let _: Option<Song> = DB.delete((TABLE_NAME, id)).await?;
        Ok(())
    }
}
