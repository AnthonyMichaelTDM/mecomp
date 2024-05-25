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
/// SELECT * FROM $id->artist_to_album->album
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
