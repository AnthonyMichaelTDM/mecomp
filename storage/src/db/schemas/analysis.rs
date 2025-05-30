#![allow(clippy::module_name_repetitions)]
use super::Id;
#[cfg(not(feature = "db"))]
use super::RecordId;
#[cfg(feature = "db")]
use surrealdb::RecordId;

pub type AnalysisId = RecordId;

pub const TABLE_NAME: &str = "analysis";

/// This struct holds the [`Analysis`] of a particular [`Song`].
///
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
    #[cfg_attr(feature = "db", field(dt = "array<float>"))]
    #[cfg_attr(feature = "db", index(vector(dim = 20)))]
    pub features: [f64; 20],
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
