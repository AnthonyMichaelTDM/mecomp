use log::warn;
use mecomp_storage::{
    db::schemas::{
        album::{Album, AlbumId, TABLE_NAME as ALBUM_TABLE_NAME},
        analysis::Analysis,
        artist::{Artist, ArtistId, TABLE_NAME as ARTIST_TABLE_NAME},
        collection::{Collection, TABLE_NAME as COLLECTION_TABLE_NAME},
        playlist::{Playlist, PlaylistId, TABLE_NAME as PLAYLIST_TABLE_NAME},
        song::{Song, SongId, TABLE_NAME as SONG_TABLE_NAME},
    },
    errors::{Error, StorageResult},
};
use surrealdb::{sql::Thing, Connection, Surreal};

/// Get the 'n' most similar songs to the given list of things
///
/// # Errors
///
/// Returns an error if there is an issue with the database
pub async fn get_similar<C: Connection>(
    db: &Surreal<C>,
    ids: impl Iterator<Item = Thing> + Clone + Send,
    n: u32,
) -> StorageResult<Vec<Song>> {
    // go through the list, and get songs for each thing (depending on what it is)
    let mut songs: Vec<SongId> = Vec::new();
    for thing in ids {
        match thing.tb.as_str() {
            ALBUM_TABLE_NAME => {
                for song in Album::read_songs(db, thing.clone()).await? {
                    songs.push(song.id);
                }
            }
            ARTIST_TABLE_NAME => {
                for song in Artist::read_songs(db, thing.clone()).await? {
                    songs.push(song.id);
                }
            }
            COLLECTION_TABLE_NAME => {
                for song in Collection::read_songs(db, thing.clone()).await? {
                    songs.push(song.id);
                }
            }
            PLAYLIST_TABLE_NAME => {
                for song in Playlist::read_songs(db, thing.clone()).await? {
                    songs.push(song.id);
                }
            }
            SONG_TABLE_NAME => songs.push(
                Song::read(db, thing.clone())
                    .await?
                    .ok_or(Error::NotFound)?
                    .id,
            ),
            _ => {
                warn!("Unknown thing type: {}", thing.tb);
            }
        }
    }

    let neighbors = Analysis::nearest_neighbors_to_many(
        db,
        Analysis::read_for_songs(db, songs)
            .await?
            .into_iter()
            .filter_map(|a| a.map(|a| a.id))
            .collect(),
        n,
    )
    .await?;
    Ok(
        Analysis::read_songs(db, neighbors.iter().map(|a| a.id.clone()).collect())
            .await?
            .into(),
    )
}

/// Get the 'n' most similar songs to the given album
///
/// # Errors
///
/// Returns an error if there is an issue with the database
pub async fn get_similar_to_album<C: Connection>(
    db: &Surreal<C>,
    id: AlbumId,
    n: u32,
) -> StorageResult<Vec<Song>> {
    let neighbors = Analysis::nearest_neighbors_to_many(
        db,
        Analysis::read_for_songs(
            db,
            Album::read_songs(db, id)
                .await?
                .iter()
                .map(|s| s.id.clone())
                .collect(),
        )
        .await?
        .into_iter()
        .filter_map(|a| a.map(|a| a.id))
        .collect(),
        n,
    )
    .await?;
    Ok(
        Analysis::read_songs(db, neighbors.iter().map(|a| a.id.clone()).collect())
            .await?
            .into(),
    )
}

/// Get the 'n' most similar songs to the given artist
///
/// # Errors
///
/// Returns an error if there is an issue with the database
pub async fn get_similar_to_artist<C: Connection>(
    db: &Surreal<C>,
    id: ArtistId,
    n: u32,
) -> StorageResult<Vec<Song>> {
    let neighbors = Analysis::nearest_neighbors_to_many(
        db,
        Analysis::read_for_songs(
            db,
            Artist::read_songs(db, id)
                .await?
                .iter()
                .map(|s| s.id.clone())
                .collect(),
        )
        .await?
        .into_iter()
        .filter_map(|a| a.map(|a| a.id))
        .collect(),
        n,
    )
    .await?;
    Ok(
        Analysis::read_songs(db, neighbors.iter().map(|a| a.id.clone()).collect())
            .await?
            .into(),
    )
}

/// Get the 'n' most similar songs to the given playlist
///
/// # Errors
///
/// Returns an error if there is an issue with the database
pub async fn get_similar_to_playlist<C: Connection>(
    db: &Surreal<C>,
    id: PlaylistId,
    n: u32,
) -> StorageResult<Vec<Song>> {
    let neighbors = Analysis::nearest_neighbors_to_many(
        db,
        Analysis::read_for_songs(
            db,
            Playlist::read_songs(db, id)
                .await?
                .iter()
                .map(|s| s.id.clone())
                .collect(),
        )
        .await?
        .into_iter()
        .filter_map(|a| a.map(|a| a.id))
        .collect(),
        n,
    )
    .await?;
    Ok(
        Analysis::read_songs(db, neighbors.iter().map(|a| a.id.clone()).collect())
            .await?
            .into(),
    )
}

/// Get the 'n' most similar songs to the given song
///
/// # Errors
///
/// Returns an error if there is an issue with the database
pub async fn get_similar_to_song<C: Connection>(
    db: &Surreal<C>,
    id: SongId,
    n: u32,
) -> StorageResult<Vec<Song>> {
    let neighbors = Analysis::nearest_neighbors(
        db,
        Analysis::read_for_song(db, id)
            .await?
            .ok_or(Error::NotFound)?
            .id,
        n,
    )
    .await?;
    Ok(
        Analysis::read_songs(db, neighbors.iter().map(|a| a.id.clone()).collect())
            .await?
            .into(),
    )
}
