//! Utility types and functions.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MetadataConflictResolution {
    Merge,
    Overwrite,
    Skip,
}
