use mecomp_storage::{
    db::schemas::{
        analysis::Analysis,
        song::{Song, SongId},
        Thing,
    },
    errors::StorageResult,
};
use surrealdb::{Connection, Surreal};

use super::get_songs_from_things;

/// Get the 'n' most similar songs to the given list of things
///
/// # Errors
///
/// Returns an error if there is an issue with the database
#[inline]
pub async fn get_similar<C: Connection>(
    db: &Surreal<C>,
    things: Vec<Thing>,
    n: u32,
) -> StorageResult<Vec<Song>> {
    // go through the list, and get songs for each thing (depending on what it is)
    let songs: Vec<SongId> = get_songs_from_things(db, &things)
        .await?
        .into_iter()
        .map(|s| s.id)
        .collect();

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
