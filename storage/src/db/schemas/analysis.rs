#![allow(clippy::module_name_repetitions)]
//--------------------------------------------------------------------------------- other libraries
#[cfg(not(feature = "db"))]
use super::Thing;
#[cfg(feature = "db")]
use surrealdb::sql::Thing;
//----------------------------------------------------------------------------------- local modules and Mecomp libraries
use mecomp_analysis::NUMBER_FEATURES;

pub type AnalysisId = Thing;

pub const TABLE_NAME: &str = "analysis";

// TODO: make a new table, `SongFeatures`, with a relation to songs, and store the analysis there
// this will allow for faster indexing of music libraries.
// caveat:
// - need to adjust `Song::delete` to also delete the associated `SongFeatures`
// - need to start a worker thread when the daemon starts that finds all the songs without features, and gives them features
// - need to add an endpoint to the daemon that also starts that worker thread (similar to rescan)

/// This struct holds the [`Analysis`] of a particular [`Song`].
/// An [`Analysis`] is the features extracted by the `mecomp-analysis` library and are used for recommendations (nearest neighbor search)
/// and Collection generation (clustering).
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "db", derive(surrealqlx::Table))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "db", Table("analysis"))]
pub struct Analysis {
    /// the unique identifier for this [`Analysis`].
    #[cfg_attr(feature = "db", field(dt = "record"))]
    pub id: AnalysisId,

    /// The [`Song`]'s audio features.
    #[cfg_attr(feature = "db", field(dt = "array<float>", index(vector(dim = 20))))]
    pub features: [f32; NUMBER_FEATURES],
}
