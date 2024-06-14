use mecomp_storage::{
    db::schemas::{
        album::{Album, AlbumId},
        analysis::Analysis,
        artist::{Artist, ArtistId},
        playlist::{Playlist, PlaylistId},
        song::{Song, SongId},
    },
    errors::{Error, StorageResult},
};
use surrealdb::{Connection, Surreal};

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
