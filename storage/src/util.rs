//! Utility types and functions.

use one_or_many::OneOrMany;
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

/// Splits an artist name into multiple names based on the given separators.
///
/// The function takes into account exceptions, which are artist names that should not be split
/// even if they contain the separator.
///
/// The function returns a `OneOrMany<String>` containing the split artist names.
///
/// Worst Case Runtime: O(n(m + k)), where n is the length of the artist name, m is the number of separators, and k is the number of exceptions.
///
/// In general, it will be faster then this since the number of separators and exceptions is usually small.
#[inline]
#[must_use]
pub fn split_artist_name(
    artist: &str,
    artist_name_separator: &OneOrMany<String>,
    exceptions: &OneOrMany<String>,
) -> OneOrMany<String> {
    let mut artists = OneOrMany::None;

    // for the separators with exclusions, we can use a 2 pointers approach
    let mut left = 0;
    let mut right = 0;
    while right < artist.len() {
        let rest_right = &artist[right..];
        // are we at a separator?
        if let Some(sep) = artist_name_separator
            .iter()
            .find(|sep| rest_right.starts_with(*sep))
        {
            let rest_left = &artist[left..];
            // if we are at a separator, we need to check if are we at the beginning of an exception
            if let Some(exception) = exceptions
                .iter()
                .find(|exception| rest_left.starts_with(*exception))
            {
                let exception_len = exception.len();
                // if we are at the beginning of an exception, we need to check if after it the string ends or there is a separator
                let after_exception = &artist[left + exception_len..];
                // if the string ends, we can add the artist
                if after_exception.is_empty() {
                    break;
                }
                if let Some(sep) = artist_name_separator
                    .iter()
                    .find(|sep| after_exception.starts_with(*sep))
                {
                    // if there is a separator after it, we split the string there instead
                    let new = artist[left..left + exception_len].trim().replace('\0', "");
                    if !new.is_empty() {
                        artists.push(new);
                    }
                    left += exception_len + sep.len();
                    right = left;
                    continue;
                }
            }
            // otherwise, we can split the string at this separator
            let new = artist[left..right].trim().replace('\0', "");
            if !new.is_empty() {
                artists.push(new);
            }
            right += sep.len();
            left = right;
        } else {
            // if we are not at a separator, we just move the right pointer to the right
            right += 1;
            // continue incrementing the right pointer if we're not at a character boundary
            while !artist.is_char_boundary(right) && right < artist.len() {
                right += 1;
            }
        }
    }

    // add the last artist, if any
    if left < artist.len() {
        let new = artist[left..].trim().replace('\0', "");
        if !new.is_empty() {
            artists.push(new);
        }
    }
    artists
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::no_separation("Foo & Bar", &[], &[], vec!["Foo & Bar"])]
    #[case::redundant_separation("Foo & Bar", &["&", " ", " &", " & "], &[], vec!["Foo", "Bar"])]
    #[case::separation_no_exclusions("Foo & BarBaz", &["&", ";"], &[], vec!["Foo","BarBaz"])]
    #[case::separation_no_exclusions("Foo & Bar; Baz", &["&", ";"], &[], vec!["Foo", "Bar", "Baz"])]
    #[case::separation_excluded("Foo & Bar", &["&", ";"], &["Foo & Bar"], vec!["Foo & Bar"])]
    #[case::separation_excluded("Foo & BarBaz", &["&", ";"], &["Foo & Bar"], vec!["Foo","BarBaz"])]
    #[case::separation_excluded("Foo & Bar; Baz", &["&", ";"], &["Foo & Bar"], vec!["Foo & Bar", "Baz"])]
    #[case::separation_excluded("Foo & BarBaz; Zing", &["&", ";"], &["Foo & Bar"], vec!["Foo","BarBaz", "Zing"])]
    #[case::separation_excluded("Zing; Foo & BarBaz", &["&", ";"], &["Foo & Bar"], vec!["Zing","Foo","BarBaz"])]
    #[case::separation_excluded("Foo & Bar; Baz; Zing", &["&", ";"], &["Foo & Bar"], vec!["Foo & Bar", "Baz", "Zing"])]
    fn test_split_artist_name(
        #[case] artist: &str,
        #[case] separators: &[&str],
        #[case] exceptions: &[&str],
        #[case] expected: Vec<&str>,
    ) {
        let separators = separators.iter().map(|s| (*s).to_string()).collect();
        let exceptions = exceptions.iter().map(|s| (*s).to_string()).collect();
        let expected = expected
            .into_iter()
            .map(std::string::ToString::to_string)
            .collect::<OneOrMany<String>>();
        let artists = split_artist_name(artist, &OneOrMany::Many(separators), &exceptions);
        assert_eq!(artists, expected);
    }
}
