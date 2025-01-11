//! Utility types and functions.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum MetadataConflictResolution {
    #[default]
    Overwrite,
    Skip,
}

#[cfg(all(test, feature = "serde"))]
mod metadata_conflict_resolution {
    use rstest::rstest;

    use super::*;

    #[test]
    fn test_default() {
        assert_eq!(
            MetadataConflictResolution::default(),
            MetadataConflictResolution::Overwrite
        );
    }

    #[rstest]
    #[case::lower(MetadataConflictResolution::Overwrite, "overwrite")]
    #[case::lower(MetadataConflictResolution::Skip, "skip")]
    fn test_deserialize<D, 'de>(#[case] expected: MetadataConflictResolution, #[case] input: D)
    where
        D: serde::de::IntoDeserializer<'de>,
    {
        let actual: MetadataConflictResolution =
            MetadataConflictResolution::deserialize(input.into_deserializer()).unwrap();
        assert_eq!(actual, expected);
    }
}
