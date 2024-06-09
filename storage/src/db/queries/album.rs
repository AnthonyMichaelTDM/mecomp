use crate::db::schemas;
use surrealdb::sql::{
    statements::{DeleteStatement, RelateStatement, SelectStatement},
    Cond, Expression, Fields, Ident, Idiom, Operator, Param, Table, Value, Values,
};

use super::generic::{read_related_in, read_related_out, relate, unrelate};

/// Query to read an album by its name.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM album WHERE title=$name
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::album::read_by_name;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_by_name();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM album WHERE title=$name".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn read_by_name() -> SelectStatement {
    SelectStatement {
        expr: Fields::all(),
        what: Values(vec![Value::Table(Table(
            schemas::album::TABLE_NAME.to_string(),
        ))]),
        cond: Some(Cond(Value::Expression(Box::new(Expression::Binary {
            l: Value::Idiom(Idiom(vec![Ident("title".into()).into()])),
            o: Operator::Equal,
            r: Value::Param(Param(Ident("name".into()))),
        })))),
        ..Default::default()
    }
}

/// Query to read an album by its name and album artist.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM album WHERE title=$title AND artist=$artist
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::album::read_by_name_and_album_artist;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_by_name_and_album_artist();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM album WHERE title=$title AND artist=$artist".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn read_by_name_and_album_artist() -> SelectStatement {
    SelectStatement {
        expr: Fields::all(),
        what: Values(vec![Value::Table(Table(
            schemas::album::TABLE_NAME.to_string(),
        ))]),
        cond: Some(Cond(Value::Expression(Box::new(Expression::Binary {
            l: Value::Expression(Box::new(Expression::Binary {
                l: Value::Idiom(Idiom(vec![Ident("title".into()).into()])),
                o: surrealdb::sql::Operator::Equal,
                r: Value::Param(Param(Ident("title".into()))),
            })),
            o: Operator::And,
            r: Value::Expression(Box::new(Expression::Binary {
                l: Value::Idiom(Idiom(vec![Ident("artist".into()).into()])),
                o: Operator::Equal,
                r: Value::Param(Param(Ident("artist".into()))),
            })),
        })))),
        ..Default::default()
    }
}

/// Query to relate an album to its songs.
///
/// Compiles to:
///
/// ```sql, ignore
/// RELATE $album->album_to_song->$songs
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::album::add_songs;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = add_songs();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RELATE $album->album_to_song->$songs".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn add_songs() -> RelateStatement {
    relate("album", "songs", "album_to_song")
}

/// Query to read the songs of an album
///
/// Compiles to:
///
/// ```sql, ignore
/// SELECT * FROM $album->album_to_song.out
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::album::read_songs;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_songs();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $album->album_to_song.out".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn read_songs() -> SelectStatement {
    read_related_out("album", "album_to_song")
}

/// Query to remove songs from an album
///
/// Compiles to:
///
/// ```sql, ignore
/// DELETE $album->album_to_song WHERE out IN $songs
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::album::remove_songs;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = remove_songs();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "DELETE $album->album_to_song WHERE out IN $songs".into_query().unwrap()
/// );
#[must_use]
#[inline]
pub fn remove_songs() -> DeleteStatement {
    unrelate("album", "songs", "album_to_song")
}

/// Query to read the artist of an album
///
/// Compiles to:
///
/// ```sql, ignore
/// SELECT * FROM $id<-artist_to_album.in
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::album::read_artist;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_artist();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $id<-artist_to_album.in".into_query().unwrap()
/// );
#[must_use]
#[inline]
pub fn read_artist() -> SelectStatement {
    read_related_in("id", "artist_to_album")
}

#[cfg(test)]
mod query_validation_tests {
    use pretty_assertions::assert_eq;
    use surrealdb::opt::IntoQuery;

    use super::*;

    #[test]
    fn test_read_by_name() {
        let statement = read_by_name();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM album WHERE title=$name"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_read_by_name_and_album_artist() {
        let statement = read_by_name_and_album_artist();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM album WHERE title=$title AND artist=$artist"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_add_songs() {
        let statement = add_songs();
        assert_eq!(
            statement.into_query().unwrap(),
            "RELATE $album->album_to_song->$songs".into_query().unwrap()
        );
    }

    #[test]
    fn test_read_songs() {
        let statement = read_songs();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM $album->album_to_song.out"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_remove_songs() {
        let statement = remove_songs();
        assert_eq!(
            statement.into_query().unwrap(),
            "DELETE $album->album_to_song WHERE out IN $songs"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_read_artist() {
        let statement = read_artist();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM $id<-artist_to_album.in"
                .into_query()
                .unwrap()
        );
    }
}
