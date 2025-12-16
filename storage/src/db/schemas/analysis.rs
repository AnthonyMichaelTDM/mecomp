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

    /// The [`Song`]'s embedding vector.
    pub embedding: [f64; mecomp_analysis::DIM_EMBEDDING],
}

#[cfg(feature = "db")]
impl Table for Analysis {
    const TABLE_NAME: &'static str = TABLE_NAME;

    fn migrations() -> Vec<M<'static>> {
        use surrealqlx::surrql;

        vec![
            M::up(surrql!("
DEFINE TABLE IF NOT EXISTS analysis SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS id ON analysis TYPE record;
DEFINE FIELD IF NOT EXISTS features ON analysis TYPE array<float>;
DEFINE INDEX IF NOT EXISTS analysis_features_vector_index ON analysis FIELDS features MTREE DIMENSION 20;")
            )
            .comment("Initial version"),
            // v0.6.0 changed the size of the features array from 20 to 23
            M::up(surrql!("DELETE analysis;")).comment("Clear the existing analyses"),
            M::up(surrql!("DEFINE INDEX OVERWRITE analysis_features_vector_index ON analysis FIELDS features MTREE DIMENSION 23;"))
            .comment("Update analysis features size from 20 to 23"),
            // v0.6.1 also clear the analysis_to_song relations table to ensure no dangling relations exist
            //
            // The analysis_to_song relations table doesn't exist in most tests, so we need to check for its existence first.
            // This isn't possible in SurrealDB <= 3.0, so we Define the table if it doesn't exist, then delete from it.
            M::up(surrql!("DEFINE TABLE IF NOT EXISTS analysis_to_song TYPE RELATION IN analysis OUT song ENFORCED;DELETE analysis_to_song;"))
                .down(surrql!("DEFINE TABLE IF NOT EXISTS analysis_to_song TYPE RELATION IN analysis OUT song ENFORCED;DELETE analysis_to_song;"))
                .comment("Clear analysis_to_song relations to prevent dangling relations"),
            // v0.7.0 added the embedding field
            M::up(surrql!("DELETE analysis;DELETE analysis_to_song;"))
                .comment("Clear existing analyses so we can modify indexes properly"),
            M::up(surrql!("DEFINE FIELD IF NOT EXISTS embedding ON analysis TYPE array<float>;"))
                .down(surrql!("REMOVE FIELD embedding ON analysis;"))
                .comment("Add embedding field to analysis table"),
            M::up(surrql!("DEFINE INDEX IF NOT EXISTS analysis_embeddings_vector_index ON analysis FIELDS embedding MTREE DIMENSION 32;"))
                .down(surrql!("REMOVE INDEX analysis_embeddings_vector_index ON analysis;"))
                .comment("Define analysis embeddings index after adding embedding field"),
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
