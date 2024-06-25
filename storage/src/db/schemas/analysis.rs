#![allow(clippy::module_name_repetitions)]
#[cfg(not(feature = "db"))]
use super::Thing;
use mecomp_analysis::clustering::{Elem, Sample};
#[cfg(feature = "db")]
use surrealdb::sql::{Id, Thing};

pub type AnalysisId = Thing;

pub const TABLE_NAME: &str = "analysis";

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
    pub features: [f64; 20],
}

impl Analysis {
    #[must_use]
    #[cfg(feature = "db")]
    pub fn generate_id() -> AnalysisId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }
}

impl Elem for Analysis {
    fn dimensions(&self) -> usize {
        20
    }

    fn at(&self, i: usize) -> f64 {
        self.features[i]
    }
}

impl Sample for Analysis {
    fn inner(&self) -> &[f64; 20] {
        &self.features
    }
}
