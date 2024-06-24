//! Utility types and functions.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MetadataConflictResolution {
    #[default]
    Overwrite,
    Skip,
}
