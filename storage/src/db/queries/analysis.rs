use crate::db::schemas;
use surrealdb::opt::IntoQuery;

use super::generic::{read_related_in, read_related_out, relate};

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
pub fn add_to_song() -> impl IntoQuery {
    relate("id", "song", "analysis_to_song")
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
pub fn read_for_song() -> impl IntoQuery {
    read_related_in("song", "analysis_to_song")
}

/// Query to read the song for an analyses
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
pub fn read_song() -> impl IntoQuery {
    read_related_out("id", "analysis_to_song")
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
pub fn read_songs_without_analysis() -> impl IntoQuery {
    format!(
        "SELECT * FROM {} WHERE count(<-analysis_to_song.in) = 0",
        schemas::song::TABLE_NAME
    )
    .into_query()
    .unwrap()
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
pub fn nearest_neighbors(n: u32) -> impl IntoQuery {
    format!(
        "SELECT * FROM {} WHERE id IS NOT $id AND features <|{n}|> $target",
        schemas::analysis::TABLE_NAME
    )
    .into_query()
    .unwrap()
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
/// let statement = nearest_neighbors_to_many(5);
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM analysis WHERE id NOT IN $ids AND features <|5|> $target".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn nearest_neighbors_to_many(n: u32) -> impl IntoQuery {
    format!(
        "SELECT * FROM {} WHERE id NOT IN $ids AND features <|{n}|> $target",
        schemas::analysis::TABLE_NAME
    )
    .into_query()
    .unwrap()
}

#[cfg(test)]
mod query_validation_tests {
    use pretty_assertions::assert_eq;
    use surrealdb::opt::IntoQuery;

    use super::*;

    #[test]
    fn test_add_to_song() {
        let statement = add_to_song();
        assert_eq!(
            statement.into_query().unwrap(),
            "RELATE $id->analysis_to_song->$song".into_query().unwrap()
        );
    }

    #[test]
    fn test_read_for_song() {
        let statement = read_for_song();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM $song<-analysis_to_song.in"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_read_song() {
        let statement = read_song();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM $id->analysis_to_song.out"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_read_songs_without_analysis() {
        let statement = read_songs_without_analysis();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM song WHERE count(<-analysis_to_song.in) = 0"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_nearest_neighbors() {
        let statement = nearest_neighbors(5);
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM analysis WHERE id IS NOT $id AND features <|5|> $target"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_nearest_neighbors_to_many() {
        let statement = nearest_neighbors_to_many(5);
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM analysis WHERE id NOT IN $ids AND features <|5|> $target"
                .into_query()
                .unwrap()
        );
    }
}
