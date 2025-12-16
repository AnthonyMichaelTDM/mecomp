use crate::db::{queries::parse_query, schemas};
use surrealdb::opt::IntoQuery;
use surrealqlx::surrql;

/// Query to relate an analysis to a song
///
/// Compiles to:
/// ```sql, ignore
/// RELATE $id->analysis_to_song->$song
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::analysis::add_to_song;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = add_to_song();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RELATE $id->analysis_to_song->$song".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub const fn add_to_song() -> impl IntoQuery {
    surrql!("RELATE $id->analysis_to_song->$song")
}

/// Query to read the analysis for a song
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $song<-analysis_to_song.in
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::analysis::read_for_song;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_for_song();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $song<-analysis_to_song.in".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub const fn read_for_song() -> impl IntoQuery {
    surrql!("SELECT * FROM $song<-analysis_to_song.in")
}

/// Query to read the analyses for a list of songs
///
/// Compiles to:
/// ```sql, ignore
/// "array::flatten($songs<-analysis_to_song<-analysis)"
/// ```
///
/// Let's break this down:
/// - `$songs<-analysis_to_song<-analysis`: for each song in `$songs`, get the list of analyses
///   `RecordIds` that are related to it.
///   - `Vec<Vec<AnalysisId>>`
/// - `array::flatten(...)`: flatten the list of lists into a single list.
///   - `Vec<AnalysisId>`
///
#[must_use]
#[inline]
pub const fn read_for_songs() -> impl IntoQuery {
    surrql!("array::flatten($songs<-analysis_to_song<-analysis)")
}

/// Query to read the song for an analysis
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $id->analysis_to_song.out
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::analysis::read_song;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_song();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $id->analysis_to_song.out".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub const fn read_song() -> impl IntoQuery {
    surrql!("SELECT * FROM $id->analysis_to_song.out")
}

/// Query to read the songs for a list of analyses
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM array::flatten($ids->analysis_to_song->song)
/// ```
#[must_use]
#[inline]
pub const fn read_songs() -> impl IntoQuery {
    surrql!("SELECT * FROM array::flatten($ids->analysis_to_song->song)")
}

/// Query to find all the songs that don't have an analysis
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM song WHERE count(<-analysis_to_song.in) = 0
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::analysis::read_songs_without_analysis;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_songs_without_analysis();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM song WHERE count(<-analysis_to_song.in) = 0".into_query().unwrap()
/// );
/// ```
#[allow(clippy::module_name_repetitions)]
#[must_use]
#[inline]
pub const fn read_songs_without_analysis() -> impl IntoQuery {
    surrql!("SELECT * FROM song WHERE count(<-analysis_to_song.in) = 0")
}

/// Query to find the `n` nearest neighbors to a given analysis
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM analysis WHERE features <|n|> $target
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::analysis::nearest_neighbors;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = nearest_neighbors(5);
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM analysis WHERE id IS NOT $id features <|5|> $target".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn nearest_neighbors(n: u32) -> impl IntoQuery {
    parse_query(format!(
        "SELECT * FROM {} WHERE id IS NOT $id AND features <|{n}|> $target",
        schemas::analysis::TABLE_NAME
    ))
}

/// Query to find the `n` nearest neighbors to a list of analyses, excluding the given analyses
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM analysis WHERE id NOT IN $ids AND features <|n|> $target
/// ```
///
/// # Example
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::analysis::nearest_neighbors_to_many;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = nearest_neighbors_to_many(5, false);
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM analysis WHERE id NOT IN $ids AND features <|5|> $target".into_query().unwrap()
/// );
/// let statement = nearest_neighbors_to_many(5, true);
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM analysis WHERE id NOT IN $ids AND embedding <|5|> $target".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn nearest_neighbors_to_many(n: u32, use_embeddings: bool) -> impl IntoQuery {
    parse_query(format!(
        "SELECT * FROM {} WHERE id NOT IN $ids AND {} <|{n}|> $target",
        schemas::analysis::TABLE_NAME,
        if use_embeddings {
            "embedding"
        } else {
            "features"
        }
    ))
}

#[cfg(test)]
mod query_validation_tests {
    use rstest::rstest;
    use surrealdb::opt::IntoQuery;

    use crate::db::queries::validate_query;

    use super::*;

    #[rstest]
    #[case::add_to_song(add_to_song(), "RELATE $id->analysis_to_song->$song")]
    #[case::read_for_song(read_for_song(), "SELECT * FROM $song<-analysis_to_song.in")]
    #[case::read_for_songs(read_for_songs(), "array::flatten($songs<-analysis_to_song<-analysis)")]
    #[case::read_song(read_song(), "SELECT * FROM $id->analysis_to_song.out")]
    #[case::read_songs(
        read_songs(),
        "SELECT * FROM array::flatten($ids->analysis_to_song->song)"
    )]
    #[case::read_songs_without_analysis(
        read_songs_without_analysis(),
        "SELECT * FROM song WHERE count(<-analysis_to_song.in) = 0"
    )]
    #[case::nearest_neighbors(
        nearest_neighbors(5),
        "SELECT * FROM analysis WHERE id IS NOT $id AND features <|5|> $target"
    )]
    #[case::nearest_neighbors_to_many(
        nearest_neighbors_to_many(5, false),
        "SELECT * FROM analysis WHERE id NOT IN $ids AND features <|5|> $target"
    )]
    #[case::nearest_neighbors_to_many_use_embeddings(
        nearest_neighbors_to_many(5, true),
        "SELECT * FROM analysis WHERE id NOT IN $ids AND embedding <|5|> $target"
    )]
    fn test_queries(#[case] query: impl IntoQuery, #[case] expected: &str) {
        validate_query(query, expected);
    }
}
