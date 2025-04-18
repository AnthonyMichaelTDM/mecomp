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
pub fn relate<Source: AsRef<str>, Target: AsRef<str>, Rel: AsRef<str>>(
    source: Source,
    target: Target,
    rel: Rel,
) -> impl IntoQuery {
    fn relate_statement(source: &str, target: &str, rel: &str) -> impl IntoQuery + use<> {
        parse_query(format!("RELATE ${source}->{rel}->${target}"))
    }

    relate_statement(source.as_ref(), target.as_ref(), rel.as_ref())
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
pub fn unrelate<Source: AsRef<str>, Target: AsRef<str>, Rel: AsRef<str>>(
    source: Source,
    target: Target,
    rel: Rel,
) -> impl IntoQuery {
    fn unrelate_statement(source: &str, target: &str, rel: &str) -> impl IntoQuery + use<> {
        parse_query(format!("DELETE ${source}->{rel} WHERE out IN ${target}"))
    }

    unrelate_statement(source.as_ref(), target.as_ref(), rel.as_ref())
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
/// let statement = read_related_out("album", "album_to_song");
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $album->album_to_song.out".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn read_related_out<Source: AsRef<str>, Rel: AsRef<str>>(
    source: Source,
    rel: Rel,
) -> impl IntoQuery {
    fn read_related_statement(source: &str, rel: &str) -> impl IntoQuery + use<> {
        parse_query(format!("SELECT * FROM ${source}->{rel}.out"))
    }

    read_related_statement(source.as_ref(), rel.as_ref())
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
/// let statement = read_related_in("album", "artist_to_album");
/// assert_eq!(
///    statement.as_str().unwrap(),
///   "SELECT * FROM $album<-artist_to_album.in"
/// );
/// ```
#[must_use]
pub fn read_related_in<Target: AsRef<str>, Rel: AsRef<str>>(
    target: Target,
    rel: Rel,
) -> impl IntoQuery {
    fn read_related_statement(target: &str, rel: &str) -> impl IntoQuery + use<> {
        parse_query(format!("SELECT * FROM ${target}<-{rel}.in"))
    }

    read_related_statement(target.as_ref(), rel.as_ref())
}

/// Struct to assist deserializing the results of the count queries
#[derive(Debug, serde::Deserialize, PartialEq, Eq, Clone, Copy)]
pub struct Count {
    count: usize,
}

impl Count {
    #[cfg(test)]
    pub const fn new(count: usize) -> Self {
        Self { count }
    }

    pub async fn count<C: surrealdb::Connection>(
        db: &surrealdb::Surreal<C>,
        table: &str,
    ) -> Result<usize, Error> {
        let result: Option<Self> = db.query(count(table)).await?.take(0)?;
        Ok(result.map_or_else(
            || {
                log::warn!("When counting entries in table {table}, no count was returned",);
                0
            },
            |c| c.count,
        ))
    }

    pub async fn count_orphaned<C: surrealdb::Connection>(
        db: &surrealdb::Surreal<C>,
        table: &str,
        relation: &str,
    ) -> Result<usize, Error> {
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
    ) -> Result<usize, Error> {
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

/// Query to run a full text search on a given field of a given table.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM table WHERE field @@ $field ORDER BY relevance DESC LIMIT limit
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::generic::full_text_search;
/// use surrealdb::opt::IntoQuery;
///
/// // Example: search for songs with the word "hello" in the title
/// let statement = full_text_search("song", "title", 10);
/// assert_eq!(
///    statement.into_query().unwrap(),
///   "SELECT * FROM song WHERE title @@ $title ORDER BY relevance DESC LIMIT 10".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn full_text_search<Table: AsRef<str>, Field: AsRef<str>>(
    table: Table,
    field: Field,
    limit: i64,
) -> impl IntoQuery {
    fn full_text_search_statement(table: &str, field: &str, limit: i64) -> impl IntoQuery + use<> {
        parse_query(format!(
            "SELECT * FROM {table} WHERE {field} @@ ${field} ORDER BY relevance DESC LIMIT {limit}"
        ))
    }

    full_text_search_statement(table.as_ref(), field.as_ref(), limit)
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
        read_related_out("album", "album_to_song"),
        "SELECT * FROM $album->album_to_song.out"
    )]
    #[case::read_related_in(
        read_related_in("album", "artist_to_album"),
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
    #[case::full_text_search(
        full_text_search("song", "title", 10),
        "SELECT * FROM song WHERE title @@ $title ORDER BY relevance DESC LIMIT 10"
    )]
    fn test_queries(#[case] statement: impl IntoQuery, #[case] expected: &str) {
        validate_query(statement, expected);
    }
}
