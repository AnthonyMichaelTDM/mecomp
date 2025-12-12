#![allow(clippy::module_name_repetitions)]
use super::Id;
#[cfg(not(feature = "db"))]
use super::RecordId;
use mecomp_analysis::NUMBER_FEATURES;
#[cfg(feature = "db")]
use surrealdb::RecordId;
#[cfg(feature = "db")]
use surrealqlx::{migrations::M, traits::Table};

pub type AnalysisId = RecordId;

pub const TABLE_NAME: &str = "analysis";

/// This struct holds the [`Analysis`] of a particular [`Song`].
///
/// An [`Analysis`] is the features extracted by the `mecomp-analysis` library and are used for recommendations (nearest neighbor search)
/// and Collection generation (clustering).
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Analysis {
    /// the unique identifier for this [`Analysis`].
    pub id: AnalysisId,

    /// The [`Song`]'s audio features.
    pub features: [f64; NUMBER_FEATURES],
}

#[cfg(feature = "db")]
impl Table for Analysis {
    const TABLE_NAME: &'static str = TABLE_NAME;

    fn migrations() -> Vec<M<'static>> {
        vec![
            M::up(
                r"DEFINE TABLE IF NOT EXISTS analysis SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS id ON analysis TYPE record;
DEFINE FIELD IF NOT EXISTS features ON analysis TYPE array<float>;
DEFINE INDEX IF NOT EXISTS analysis_features_vector_index ON analysis FIELDS features MTREE DIMENSION 20;",
            )
            .comment("Initial version"),
            // v0.6.0 changed the size of the features array from 20 to 23
            M::up(
                r"
-- Clear the existing analyses since they have the wrong feature size
DELETE analysis;
-- Recreate the index with the new dimension size
DEFINE INDEX OVERWRITE analysis_features_vector_index ON analysis FIELDS features MTREE DIMENSION 23;",
            ).down(
                r"
-- Clear the existing analyses since they have the wrong feature size
DELETE analysis;
-- Recreate the index with the old dimension size
DEFINE INDEX OVERWRITE analysis_features_vector_index ON analysis FIELDS features MTREE DIMENSION 20;",
            ).comment("Update analysis features size from 20 to 23"),
        ]
    }
}

impl Analysis {
    #[must_use]
    #[inline]
    pub fn generate_id() -> AnalysisId {
        RecordId::from_table_key(TABLE_NAME, Id::ulid())
    }
}

impl From<&Analysis> for mecomp_analysis::Analysis {
    #[inline]
    fn from(analysis: &Analysis) -> Self {
        Self::new(analysis.features)
    }
}

impl From<Analysis> for mecomp_analysis::Analysis {
    #[inline]
    fn from(analysis: Analysis) -> Self {
        Self::new(analysis.features)
    }
}
