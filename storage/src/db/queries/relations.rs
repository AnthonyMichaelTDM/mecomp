//! This module contains the queries used to define the relation tables between the different entities in the database.

use surrealdb::{Connection, Surreal};

pub const ALBUM_TO_SONG: &str = "album_to_song";
pub const ANALYSIS_TO_SONG: &str = "analysis_to_song";
pub const ARTIST_TO_ALBUM: &str = "artist_to_album";
pub const ARTIST_TO_SONG: &str = "artist_to_song";
pub const COLLECTION_TO_SONG: &str = "collection_to_song";
pub const PLAYLIST_TO_SONG: &str = "playlist_to_song";

/// Function to run the queries to define the relation tables
pub async fn define_relation_tables<C: Connection>(db: &Surreal<C>) -> surrealdb::Result<()> {
    db.query(
        r"  
    DEFINE TABLE album_to_song TYPE RELATION IN album OUT song ENFORCED;
    DEFINE TABLE analysis_to_song TYPE RELATION IN analysis OUT song ENFORCED;
    DEFINE TABLE artist_to_album TYPE RELATION IN artist OUT album ENFORCED; 
    DEFINE TABLE artist_to_song TYPE RELATION IN artist OUT song ENFORCED;
    DEFINE TABLE collection_to_song TYPE RELATION IN collection OUT song ENFORCED;
    DEFINE TABLE playlist_to_song TYPE RELATION IN playlist OUT song ENFORCED;
    ",
    )
    .await?;

    Ok(())
}
