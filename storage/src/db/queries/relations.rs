//! This module contains the queries used to define the relation tables between the different entities in the database.

use surrealdb::{Connection, Surreal};
use surrealqlx::migrations::{M, Migrations};

pub const ALBUM_TO_SONG: &str = "album_to_song";
pub const ANALYSIS_TO_SONG: &str = "analysis_to_song";
pub const ARTIST_TO_ALBUM: &str = "artist_to_album";
pub const ARTIST_TO_SONG: &str = "artist_to_song";
pub const COLLECTION_TO_SONG: &str = "collection_to_song";
pub const PLAYLIST_TO_SONG: &str = "playlist_to_song";

/// Function to run the queries to define the relation tables
pub async fn define_relation_tables<C: Connection>(db: &Surreal<C>) -> surrealqlx::Result<()> {
    let migrations = Migrations::new(
        "relation_tables",
        vec![
            M::up(r"  
DEFINE TABLE IF NOT EXISTS album_to_song TYPE RELATION IN album OUT song ENFORCED;
DEFINE INDEX IF NOT EXISTS unique_album_to_song_relationships ON TABLE album_to_song COLUMNS in, out UNIQUE;
DEFINE TABLE IF NOT EXISTS analysis_to_song TYPE RELATION IN analysis OUT song ENFORCED;
DEFINE INDEX IF NOT EXISTS unique_analysis_to_song_relationships ON TABLE analysis_to_song COLUMNS in, out UNIQUE;
DEFINE TABLE IF NOT EXISTS artist_to_album TYPE RELATION IN artist OUT album ENFORCED; 
DEFINE INDEX IF NOT EXISTS unique_artist_to_album_relationships ON TABLE artist_to_album COLUMNS in, out UNIQUE;
DEFINE TABLE IF NOT EXISTS artist_to_song TYPE RELATION IN artist OUT song ENFORCED;
DEFINE INDEX IF NOT EXISTS unique_artist_to_song_relationships ON TABLE artist_to_song COLUMNS in, out UNIQUE;
DEFINE TABLE IF NOT EXISTS collection_to_song TYPE RELATION IN collection OUT song ENFORCED;
DEFINE INDEX IF NOT EXISTS unique_collection_to_song_relationships ON TABLE collection_to_song COLUMNS in, out UNIQUE;
DEFINE TABLE IF NOT EXISTS playlist_to_song TYPE RELATION IN playlist OUT song ENFORCED;
DEFINE INDEX IF NOT EXISTS unique_playlist_to_song_relationships ON TABLE playlist_to_song COLUMNS in, out UNIQUE;").comment("Define relation tables"),
        ],
    );

    migrations.to_latest(db).await?;

    Ok(())
}
