use crate::db::schemas;

use surrealdb::sql::{
    statements::{DeleteStatement, OutputStatement, RelateStatement, SelectStatement},
    Cond, Dir, Expression, Fields, Graph, Ident, Idiom, Limit, Operator, Param, Part, Subquery,
    Table, Tables, Value, Values,
};

use super::generic::{read_related_out, relate, unrelate};

/// Query to read an artist by their name.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM artist WHERE name = $name LIMIT 1
/// ```
///
/// # Example
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::artist::read_by_name;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_by_name();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM artist WHERE name = $name LIMIT 1".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn read_by_name() -> SelectStatement {
    SelectStatement {
        expr: Fields::all(),
        what: Values(vec![Value::Table(Table(
            schemas::artist::TABLE_NAME.to_string(),
        ))]),
        cond: Some(Cond(Value::Expression(Box::new(Expression::Binary {
            l: Value::Idiom(Idiom(vec![Ident("name".into()).into()])),
            o: Operator::Equal,
            r: Value::Param(Param(Ident("name".into()))),
        })))),
        limit: Some(Limit(1.into())),
        ..Default::default()
    }
}

/// Query to read a artists by their names.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM artist WHERE name IN $names
/// ```
///
/// # Example
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::artist::read_by_names;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_by_names();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM artist WHERE name IN $names".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn read_by_names() -> SelectStatement {
    SelectStatement {
        expr: Fields::all(),
        what: Values(vec![Value::Table(Table(
            schemas::artist::TABLE_NAME.into(),
        ))]),
        cond: Some(Cond(Value::Expression(Box::new(Expression::Binary {
            l: Value::Idiom(Idiom(vec![Ident("name".into()).into()])),
            o: Operator::Inside,
            r: Value::Param(Param(Ident("names".into()))),
        })))),
        ..Default::default()
    }
}

/// Query to read many artists
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $ids
/// ```
///
/// # Example
///
/// ```
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
pub fn read_many() -> SelectStatement {
    SelectStatement {
        expr: Fields::all(),
        what: Values(vec![Value::Param(Param(Ident("ids".into())))]),
        ..Default::default()
    }
}

/// Query to read the albums by an artist.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $id->artist_to_album.out
/// ```
///
/// # Example
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::artist::read_albums;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_albums();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $id->artist_to_album.out".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn read_albums() -> SelectStatement {
    read_related_out("id", "artist_to_album")
}

/// Query to relate an artist to an album.
///
/// Compiles to:
/// ```sql, ignore
/// RELATE $id->artist_to_album->$album
/// ```
///
/// # Example
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::artist::add_album;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = add_album();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RELATE $id->artist_to_album->$album".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn add_album() -> RelateStatement {
    relate("id", "album", "artist_to_album")
}

/// Query to relate artists to an album.
///
/// Compiles to:
/// ```sql, ignore
/// RELATE $ids->artist_to_album->$album
/// ```
///
/// # Example
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::artist::add_album_to_artists;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = add_album_to_artists();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RELATE $ids->artist_to_album->$album".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn add_album_to_artists() -> RelateStatement {
    relate("ids", "album", "artist_to_album")
}

/// Query to relate an artists to songs.
///
/// Compiles to:
/// ```sql, ignore
/// RELATE $id->artist_to_song->$songs
/// ```
///
/// # Example
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::artist::add_songs;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = add_songs();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RELATE $id->artist_to_song->$songs".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn add_songs() -> RelateStatement {
    relate("id", "songs", "artist_to_song")
}

/// Query to remove songs from an artist.
///
/// Compiles to:
/// ```sql, ignore
/// DELETE $artist->artist_to_song WHERE out IN $songs
/// ```
///
/// # Example
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::artist::remove_songs;    
/// use surrealdb::opt::IntoQuery;
///
/// let statement = remove_songs();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "DELETE $artist->artist_to_song WHERE out IN $songs".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn remove_songs() -> DeleteStatement {
    unrelate("artist", "songs", "artist_to_song")
}

/// Query to read all the songs associated with an artist.
///
/// Compiles to:
/// ```sql, ignore
/// RETURN array::union((SELECT * FROM $artist->artist_to_song.out), (SELECT * FROM $artist->artist_to_album->album->album_to_song.out))
/// ```
///
/// # Example
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::artist::read_songs;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_songs();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RETURN array::union((SELECT * FROM $artist->artist_to_song.out), (SELECT * FROM $artist->artist_to_album->album->album_to_song.out))".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn read_songs() -> OutputStatement {
    OutputStatement {
        what: Value::Function(Box::new(surrealdb::sql::Function::Normal(
            "array::union".into(),
            vec![
                Value::Subquery(Box::new(Subquery::Select(read_related_out(
                    "artist",
                    "artist_to_song",
                )))),
                Value::Subquery(Box::new(Subquery::Select(SelectStatement {
                    expr: Fields::all(),
                    what: Values(vec![Value::Idiom(Idiom(vec![
                        Part::Start(Value::Param(Param(Ident("artist".into())))),
                        Part::Graph(Graph {
                            dir: Dir::Out,
                            what: Tables(vec![Table("artist_to_album".into())]),
                            expr: Fields::all(),
                            ..Default::default()
                        }),
                        Part::Graph(Graph {
                            dir: Dir::Out,
                            what: Tables(vec![Table(schemas::album::TABLE_NAME.into())]),
                            expr: Fields::all(),
                            ..Default::default()
                        }),
                        Part::Graph(Graph {
                            dir: Dir::Out,
                            what: Tables(vec![Table("album_to_song".into())]),
                            expr: Fields::all(),
                            ..Default::default()
                        }),
                        Part::Field(Ident("out".into())),
                    ]))]),
                    ..Default::default()
                }))),
            ],
        ))),
        ..Default::default()
    }
}
