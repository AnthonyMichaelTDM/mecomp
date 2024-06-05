//! Utility types and functions.

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum MetadataConflictResolution {
    Merge,
    Overwrite,
    Skip,
}
