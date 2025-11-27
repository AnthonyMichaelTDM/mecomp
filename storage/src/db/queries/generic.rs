use surrealdb::opt::IntoQuery;

use crate::{db::queries::parse_query, errors::Error};

/// Query to add relations between two tables.
///
/// Compiles to:
///
/// ```sql, ignore
/// RELATE $source->rel->$target
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::generic::relate;
/// use surrealdb::opt::IntoQuery;
///
/// // Example: add a album to an artist
/// let statement = relate("id", "album", "artist_to_album");
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RELATE $id->artist_to_album->$album".into_query().unwrap()
/// );
///
/// // Example: add a album to multiple artists
/// let statement = relate("ids", "album", "artist_to_album");
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RELATE $ids->artist_to_album->$album".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn relate(source: &'static str, target: &'static str, rel: &'static str) -> impl IntoQuery {
    parse_query(format!("RELATE ${source}->{rel}->${target}"))
}

/// Query to unrelate two tables.
///
/// Compiles to:
/// ```sql, ignore
/// DELETE $source->rel WHERE out IN $target
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::generic::unrelate;
/// use surrealdb::opt::IntoQuery;
///
/// // Example: remove a album from an artist
/// let statement = unrelate("artist", "album", "artist_to_album");
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "DELETE $artist->artist_to_album WHERE out IN $album".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn unrelate(source: &'static str, target: &'static str, rel: &'static str) -> impl IntoQuery {
    parse_query(format!("DELETE ${source}->{rel} WHERE out IN ${target}"))
}
/// Query to read items related to a source.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $source->rel.out
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::generic::read_related_out;
/// use surrealdb::opt::IntoQuery;
///
/// // Example: read all the songs of an album
/// let statement = read_related_out("*", "album", "album_to_song");
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $album->album_to_song.out".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn read_related_out(
    selection: &'static str,
    source: &'static str,
    rel: &'static str,
) -> impl IntoQuery {
    parse_query(format!("SELECT {selection} FROM ${source}->{rel}.out"))
}

/// Query to read items related to a target
///
/// Compiles to:
///
/// ```sql, ignore
/// SELECT * FROM $target<-rel.in
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::generic::read_related_in;
/// use surrealdb::opt::IntoQuery;
///
/// // Example: read the artist of an album
/// let statement = read_related_in("*", "album", "artist_to_album");
/// assert_eq!(
///    statement.as_str().unwrap(),
///   "SELECT * FROM $album<-artist_to_album.in"
/// );
/// ```
#[must_use]
pub fn read_related_in(
    selection: &'static str,
    target: &'static str,
    rel: &'static str,
) -> impl IntoQuery {
    parse_query(format!("SELECT {selection} FROM ${target}<-{rel}.in"))
}

/// Struct to assist deserializing the results of the count queries
#[derive(Debug, serde::Deserialize, PartialEq, Eq, Clone, Copy)]
pub struct Count {
    count: u64,
}

impl Count {
    #[cfg(test)]
    pub const fn new(count: u64) -> Self {
        Self { count }
    }

    /// Count the number of items in a table.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails or if the result cannot be deserialized.
    pub async fn count<C: surrealdb::Connection>(
        db: &surrealdb::Surreal<C>,
        table: &str,
    ) -> Result<u64, Error> {
        let result: Option<Self> = db.query(count(table)).await?.take(0)?;
        Ok(result.map_or_else(
            || {
                log::warn!("When counting entries in table {table}, no count was returned",);
                0
            },
            |c| c.count,
        ))
    }

    /// Count the number of items in a table that are not included in a relation.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails or if the result cannot be deserialized.
    pub async fn count_orphaned<C: surrealdb::Connection>(
        db: &surrealdb::Surreal<C>,
        table: &str,
        relation: &str,
    ) -> Result<u64, Error> {
        let result: Option<Self> = db.query(count_orphaned(table, relation)).await?.take(0)?;
        Ok(result.map_or_else(
            || {
                log::warn!(
                    "When counting orphaned entries in table {table}, no count was returned",
                );
                0
            },
            |c| c.count,
        ))
    }

    pub async fn count_orphaned_both<C: surrealdb::Connection>(
        db: &surrealdb::Surreal<C>,
        table: &str,
        relation1: &str,
        relation2: &str,
    ) -> Result<u64, Error> {
        let result: Option<Self> = db
            .query(count_orphaned_both(table, relation1, relation2))
            .await?
            .take(0)?;
        Ok(result.map_or_else(
            || {
                log::warn!(
                    "When counting orphaned entries in table {table}, no count was returned",
                );
                0
            },
            |c| c.count,
        ))
    }
}

/// Query to count the number of items in a table.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT count() FROM table GROUP ALL
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::generic::count;
/// use surrealdb::opt::IntoQuery;
///
/// // Example: count the number of songs in the database
/// let statement = count("song");
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT count() FROM song GROUP ALL".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn count<Table: AsRef<str>>(table: Table) -> impl IntoQuery {
    fn count_statement(table: &str) -> impl IntoQuery + use<> {
        parse_query(format!("SELECT count() FROM {table} GROUP ALL"))
    }

    count_statement(table.as_ref())
}

/// Query to count the number of items in a table that are not included in a relation.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT count() FROM table WHERE count(->relation) = 0 GROUP ALL
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::generic::count_orphaned;
/// use surrealdb::opt::IntoQuery;
///
/// // Example: count the number of orphaned albums in the database
/// let statement = count_orphaned("album", "album_to_song");
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT count() FROM album WHERE count(->album_to_song) = 0 GROUP ALL".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn count_orphaned<Table: AsRef<str>, Rel: AsRef<str>>(
    table: Table,
    rel: Rel,
) -> impl IntoQuery {
    fn count_orphaned_statement(table: &str, rel: &str) -> impl IntoQuery + use<> {
        parse_query(format!(
            "SELECT count() FROM {table} WHERE count(->{rel}) = 0 GROUP ALL"
        ))
    }

    count_orphaned_statement(table.as_ref(), rel.as_ref())
}

/// Query to count the number of items in a table that are not included in both of the provided relations.
///
/// This is useful for counting orphaned items that are not included in either of the provided relations.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT count() FROM artist WHERE count(->artist_to_album) = 0 AND count(->artist_to_song) = 0 GROUP ALL
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::generic::count_orphaned_both;
/// use surrealdb::opt::IntoQuery;
///
/// // Example: count the number of orphaned artists in the database
/// let statement = count_orphaned_both("artist", "artist_to_album", "artist_to_song");
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT count() FROM artist WHERE count(->artist_to_album) = 0 AND count(->artist_to_song) = 0 GROUP ALL".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn count_orphaned_both<Table: AsRef<str>, Rel1: AsRef<str>, Rel2: AsRef<str>>(
    table: Table,
    rel1: Rel1,
    rel2: Rel2,
) -> impl IntoQuery {
    fn count_orphaned_both_statement(
        table: &str,
        rel1: &str,
        rel2: &str,
    ) -> impl IntoQuery + use<> {
        parse_query(format!(
            "SELECT count() FROM {table} WHERE count(->{rel1}) = 0 AND count(->{rel2}) = 0 GROUP ALL"
        ))
    }

    count_orphaned_both_statement(table.as_ref(), rel1.as_ref(), rel2.as_ref())
}

/// Query to read many items from a table.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $ids
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::artist::read_many;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_many();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $ids".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub const fn read_many() -> impl IntoQuery {
    "SELECT * FROM $ids"
}

/// Query to read `n` items from the given `table` at random
#[must_use]
pub fn read_rand(selection: &'static str, table: &'static str, n: usize) -> impl IntoQuery {
    format!("SELECT {selection} FROM {table} ORDER BY RAND() LIMIT {n}")
}

#[cfg(test)]
mod query_validation_tests {
    use super::super::validate_query;
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::relate(
        relate("id", "album", "artist_to_album"),
        "RELATE $id->artist_to_album->$album"
    )]
    #[case::relate(
        relate("ids", "album", "artist_to_album"),
        "RELATE $ids->artist_to_album->$album"
    )]
    #[case::unrelate(
        unrelate("artist", "album", "artist_to_album"),
        "DELETE $artist->artist_to_album WHERE out IN $album"
    )]
    #[case::read_related_out(
        read_related_out("*", "album", "album_to_song"),
        "SELECT * FROM $album->album_to_song.out"
    )]
    #[case::read_related_in(
        read_related_in("*", "album", "artist_to_album"),
        "SELECT * FROM $album<-artist_to_album.in"
    )]
    #[case::count(count("song"), "SELECT count() FROM song GROUP ALL")]
    #[case::count_orphaned(
        count_orphaned("album", "album_to_song"),
        "SELECT count() FROM album WHERE count(->album_to_song) = 0 GROUP ALL"
    )]
    #[case::count_orphaned_both(
        count_orphaned_both("artist", "artist_to_album", "artist_to_song"),
        "SELECT count() FROM artist WHERE count(->artist_to_album) = 0 AND count(->artist_to_song) = 0 GROUP ALL"
    )]
    #[case::read_many(read_many(), "SELECT * FROM $ids")]
    #[case::read_rand(
        read_rand("*", "song", 5),
        "SELECT * FROM song ORDER BY RAND() LIMIT 5"
    )]
    fn test_queries(#[case] statement: impl IntoQuery, #[case] expected: &str) {
        validate_query(statement, expected);
    }
}
