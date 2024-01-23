use std::sync::Arc;

use readable::run::Runtime;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Clone, Debug, Deserialize, Serialize)]
/// This struct holds all the metadata about a particular ['Artist'].
/// An ['Artist'] is a collection of ['Album']s.
pub struct Artist {
    /// The [`Artist`]'s name.
    pub name: Arc<str>,

    /// Total runtime.
    pub runtime: Runtime,

    // SOMEDAY:
    // This should be a Box<[AlbumKey]>.
    /// Keys to the associated [`Album`]\(s\).
    pub albums: Arc<[Thing]>,

    /// Keys to every [`Song`] by this [`Artist`].
    ///
    /// The order is [`Album`] release order, then [`Song`] track order.
    pub songs: Arc<[Thing]>,
}
