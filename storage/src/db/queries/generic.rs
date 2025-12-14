use surrealdb::opt::IntoQuery;
use surrealqlx::surrql;

use crate::{db::queries::parse_query, errors::Error};

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
    surrql!("SELECT * FROM $ids")
}

/// Query to read `n` items from the given `table` at random
///
/// Compiles to:
/// ```sql, ignore
/// SELECT type::fields($fields) FROM type::table($table) ORDER BY RAND() LIMIT type::int($n)
/// ```
#[must_use]
pub const fn read_rand() -> impl IntoQuery {
    surrql!(
        "SELECT type::fields($fields) FROM type::table($table) ORDER BY RAND() LIMIT type::int($n)"
    )
}
