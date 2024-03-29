//! CRUD operations for the song table

use std::path::PathBuf;

use log::info;
use tracing::instrument;

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
    #[instrument]
    pub async fn create(song: Song) -> Result<Option<SongId>, Error> {
        let id = DB
            .create((TABLE_NAME, song.id.clone()))
            .content(song)
            .await?
            .map(|x: Song| x.id);
        Ok(id)
    }

    #[instrument]
    pub async fn read_all() -> Result<Vec<Song>, Error> {
        Ok(DB.select(TABLE_NAME).await?)
    }

    #[instrument]
    pub async fn read(id: SongId) -> Result<Option<Song>, Error> {
        Ok(DB.select((TABLE_NAME, id)).await?)
    }

    #[instrument]
    pub async fn read_by_path(path: PathBuf) -> Result<Option<Song>, Error> {
        Ok(DB
            .select(TABLE_NAME)
            .await?
            .into_iter()
            .find(|x: &Song| x.path == path))
    }

    #[instrument]
    pub async fn update(id: SongId, song: Song) -> Result<(), Error> {
        let _: Song = DB
            .update((TABLE_NAME, id))
            .content(song)
            .await?
            .ok_or(Error::NotFound)?;
        Ok(())
    }

    /// Update the metadata of a song
    ///
    /// Also repairs references to the song in the artist and album tables if the song's artist_id, album_id, or album_artist_id have changed.
    ///
    /// When repairing, uses the names of the album, album artists, and artists as the source of truth, so if the song's metadata has changed,
    /// the song's artist_id, album_id, and album_artist_id will be updated to match the names of the album, album artists, and artists.
    ///
    /// If there are no albums or artists with the same name as the song's metadata, they will be created.
    #[instrument]
    pub async fn update_and_repair(id: SongId, new_song: Song) -> Result<(), Error> {
        let old_song = Song::read(id.clone()).await?.ok_or(Error::NotFound)?;
        let mut new_song = new_song;

        Song::update(id.clone(), new_song.clone()).await?;

        // if the artist name(s) have changed, we need to:
        // - update the new song's artist_id
        // - remove the song from the old artist's list of songs
        // - add the song to the new artist's list of songs
        // - repair the old artists
        if old_song.artist != new_song.artist {
            let artists = Artist::create_or_read_by_names(new_song.artist.as_slice()).await?;

            new_song.artist_id = artists.into_iter().map(|x| x.id).collect();

            Song::update(id.clone(), new_song.clone()).await?;

            for artist_id in old_song.artist_id.iter() {
                info!("Repairing artist {}", artist_id);
                if Artist::repair(artist_id.clone()).await? {
                    info!("Artist {} was deleted after repair", artist_id);
                }
            }
        }

        // if the album name has changed, we need to:
        // - update the new song's album_id
        // - remove the song from the old album's list of songs
        // - add the song to the new album's list of songs
        // - repair the old album
        // this should also repair the album artists
        if old_song.album != new_song.album {
            let album_id = Album::read_or_create_by_name_and_album_artist(
                new_song.album.as_ref(),
                new_song.album_artist.as_slice(),
            )
            .await?
            .ok_or(Error::NotFound)?;
            new_song.album_id = album_id;
            Song::update(id.clone(), new_song.clone()).await?;

            Album::repair(old_song.album_id).await?;
        }
        if old_song.album_artist != new_song.album_artist {
            let album_artists =
                Artist::create_or_read_by_names(new_song.album_artist.as_slice()).await?;

            new_song.album_artist_id = album_artists.into_iter().map(|x| x.id).collect();

            Song::update(id.clone(), new_song.clone()).await?;

            for album_artist_id in old_song.album_artist_id.iter() {
                info!("Repairing album artist {}", album_artist_id);
                if Artist::repair(album_artist_id.clone()).await? {
                    info!("Album artist {} was deleted after repair", album_artist_id);
                }
            }
        }

        Ok(())
    }

    /// Delete a song from the database,
    /// will also:
    /// - go through the artist and album tables and remove references to it from there.
    /// - remove the song from playlists.
    /// - remove the song from collections.
    #[instrument]
    pub async fn delete(id: SongId) -> Result<(), Error> {
        let Some(song) = Song::read(id.clone()).await? else {
            return Ok(());
        };

        // remove the song from the artist's list of songs
        for artist_id in song.artist_id.iter() {
            Artist::remove_songs(artist_id.clone(), &[id.clone()]).await?;
        }

        // remove the song from the album's list of songs
        Album::remove_songs(song.album_id, &[id.clone()]).await?;

        // remove the song from playlists
        for playlist in Playlist::read_all().await? {
            if playlist.songs.contains(&id) {
                Playlist::remove_songs(playlist.id, &[id.clone()]).await?;
            }
        }

        // remove the song from collections
        for collection in Collection::read_all().await? {
            if collection.songs.contains(&id) {
                Collection::remove_songs(collection.id, &[id.clone()]).await?;
            }
        }

        let _: Option<Song> = DB.delete((TABLE_NAME, id)).await?;
        Ok(())
    }
}
