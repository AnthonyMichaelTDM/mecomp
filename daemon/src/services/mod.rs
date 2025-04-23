use log::warn;
use mecomp_storage::{
    db::schemas::{
        RecordId,
        album::{Album, TABLE_NAME as ALBUM_TABLE_NAME},
        artist::{Artist, TABLE_NAME as ARTIST_TABLE_NAME},
        collection::{Collection, TABLE_NAME as COLLECTION_TABLE_NAME},
        dynamic::{DynamicPlaylist, TABLE_NAME as DYNAMIC_PLAYLIST_TABLE_NAME},
        playlist::{Playlist, TABLE_NAME as PLAYLIST_TABLE_NAME},
        song::{Song, TABLE_NAME as SONG_TABLE_NAME},
    },
    errors::{Error, StorageResult},
};
use one_or_many::OneOrMany;
use surrealdb::{Connection, Surreal};

pub mod backup;
pub mod library;
#[cfg(feature = "analysis")]
pub mod radio;

/// Get the songs associated with every thing in the list.
///
/// This function will go through the list of things and get the songs associated with each thing.
///
/// It will then remove duplicates from the list of songs.
///
/// # Errors
///
/// This function will return an error if there is an issue reading the songs from the database.
#[inline]
pub async fn get_songs_from_things<C: Connection>(
    db: &Surreal<C>,
    things: &[RecordId],
) -> StorageResult<OneOrMany<Song>> {
    // go through the list, and get songs for each thing (depending on what it is)
    let mut songs: OneOrMany<Song> = OneOrMany::None;
    for thing in things {
        let thing = thing.clone();
        match thing.tb.as_str() {
            ALBUM_TABLE_NAME => songs.extend(Album::read_songs(db, thing.into()).await?),
            ARTIST_TABLE_NAME => songs.extend(Artist::read_songs(db, thing.into()).await?),
            COLLECTION_TABLE_NAME => songs.extend(Collection::read_songs(db, thing.into()).await?),
            PLAYLIST_TABLE_NAME => songs.extend(Playlist::read_songs(db, thing.into()).await?),
            SONG_TABLE_NAME => {
                songs.push(Song::read(db, thing.into()).await?.ok_or(Error::NotFound)?);
            }
            DYNAMIC_PLAYLIST_TABLE_NAME => {
                if let Some(new_songs) = DynamicPlaylist::run_query_by_id(db, thing.into()).await? {
                    songs.extend(new_songs);
                }
            }
            _ => warn!("Unknown thing type: {}", thing.tb),
        }
    }

    // remove duplicates
    songs.dedup_by_key(|song| song.id.clone());

    Ok(songs)
}
