use std::sync::Arc;

use readable::date::Date;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use super::OneOrMany;

#[derive(Clone, Debug, Deserialize, Serialize)]
/// This struct holds all the metadata about a particular [`Album`].
/// An [`Album`] is a collection of [`Song`]s owned by an [`Artist`].
pub struct Album {
    /// Title of the [`Album`].
    pub title: Arc<str>,
    /// Id of the [`Artist`] of this [`Album`].
    pub artist: OneOrMany<Thing>,

    /// Human-readable release date of this [`Album`].
    pub release: Date,
    /// Total runtime of this [`Album`].
    pub runtime: f64,
    /// [`Song`] count of this [`Album`].
    pub song_count: usize,
    // SOMEDAY:
    // This should be sorted based
    // off incrementing disc and track numbers, e.g:
    //
    // DISC 1:
    //   - 1. ...
    //   - 2. ...
    // DISC 2:
    //   - 1. ...
    //   - 2. ...
    //
    // So, doing `my_album.songs.iter()` will always
    // result in the correct `Song` order for `my_album`.
    /// The [`Id`]s of the [`Song`]s in this [`Album`].
    pub songs: Arc<[Thing]>,
    /// How many discs are in this `Album`?
    /// (Most will only have 1).
    pub discs: u32,

    /// This [`Album`]'s genre.
    pub genre: Option<OneOrMany<Arc<str>>>,
}
