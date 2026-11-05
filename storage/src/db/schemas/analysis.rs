#![allow(clippy::module_name_repetitions)]

use super::Id;
use super::RecordId;
use mecomp_analysis::NUMBER_FEATURES;
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
    pub features: [f32; NUMBER_FEATURES],

    /// The [`Song`]'s embedding vector.
    pub embedding: [f32; mecomp_analysis::DIM_EMBEDDING],
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
            // NOTE: The hardcoded value 32 below must match `mecomp_analysis::DIM_EMBEDDING`.  
            // If the embedding dimension changes, create a new migration to update the index accordingly.
            M::up(surrql!("DEFINE INDEX IF NOT EXISTS analysis_embeddings_vector_index ON analysis FIELDS embedding MTREE DIMENSION 32;"))
                .down(surrql!("REMOVE INDEX analysis_embeddings_vector_index ON analysis;"))
                .comment("Define analysis embeddings index after adding embedding field"),
            // v0.7.4: weighted analysis features, migration needs to apply weights to pre-existing analyses features
            M::up(surrql!(r"
                LET $weights = [ 0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, math::sqrt(3.0 / 13.0), math::sqrt(3.0 / 13.0), math::sqrt(3.0 / 13.0), math::sqrt(3.0 / 13.0), math::sqrt(3.0 / 13.0), math::sqrt(3.0 / 13.0), math::sqrt(3.0 / 13.0), math::sqrt(3.0 / 13.0), math::sqrt(3.0 / 13.0), math::sqrt(3.0 / 13.0), math::sqrt(3.0 / 13.0), math::sqrt(3.0 / 13.0), math::sqrt(3.0 / 13.0) ];
                UPDATE analysis 
                SET features = vector::multiply(features, $weights);
            ")).comment("Apply feature weights to pre-existing analyses"),
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

#[cfg(test)]
#[cfg(feature = "db")]
mod tests {
    use mecomp_analysis::FEATURE_WEIGHTS;
    use pretty_assertions::assert_eq;
    use surrealdb::{Surreal, engine::local::Mem, opt::Config};
    use surrealqlx::migrations::Migrations;

    use super::*;

    #[tokio::test]
    async fn test_v0_7_4_migration() {
        // setup the DB
        let config = Config::new().strict();
        let db = Surreal::new::<Mem>(config).await.unwrap();

        db.query("DEFINE NAMESPACE IF NOT EXISTS test")
            .await
            .unwrap();
        db.use_ns("test").await.unwrap();
        db.query("DEFINE DATABASE IF NOT EXISTS test")
            .await
            .unwrap();
        db.use_db("test").await.unwrap();

        // run the migrations up to v0.7.4
        let migrations = Migrations::new(Analysis::TABLE_NAME, Analysis::migrations());
        migrations.to_version(&db, 7).await.unwrap();

        // insert a "test" analysis into the database
        let test_analysis = Analysis {
            id: Analysis::generate_id(),
            features: [1.0f32; 23],
            embedding: [1.0f32; 32],
        };
        let expected_features = FEATURE_WEIGHTS;
        let created: Option<Analysis> = db
            .create(&test_analysis.id)
            .content(test_analysis.clone())
            .await
            .unwrap();
        assert_eq!(created, Some(test_analysis.clone()));

        // run the v0.7.4 migration
        migrations.to_version(&db, 8).await.unwrap();

        // read that test analysis back and make sure it matches what we expect
        let row: Option<Analysis> = db.select(&test_analysis.id).await.unwrap();
        let row = row.expect("Expected analysis row to exist");
        assert_eq!(row.features, expected_features);
    }
}
