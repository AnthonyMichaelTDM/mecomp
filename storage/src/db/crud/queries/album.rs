use crate::db::schemas;
use surrealdb::sql::{
    statements::{DeleteStatement, RelateStatement, SelectStatement},
    Cond, Dir, Expression, Fields, Graph, Ident, Idiom, Operator, Param, Part, Table, Tables,
    Value, Values,
};

/// Query to read an album by its name.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM album WHERE title=$name
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
#[must_use]
pub fn relate_album_to_songs() -> RelateStatement {
    RelateStatement {
        from: Value::Param(Param(Ident("album".into()))),
        kind: Value::Table(Table("album_to_song".into())),
        with: Value::Param(Param(Ident("songs".into()))),
        ..Default::default()
    }
}

/// Query to read the songs of an album
///
/// Compiles to:
///
/// ```sql, ignore
/// SELECT * FROM $album->album_to_song.out
/// ```
#[must_use]
pub fn read_songs_in_album() -> SelectStatement {
    SelectStatement {
        expr: Fields::all(),
        what: Values(vec![Value::Idiom(Idiom(vec![
            Part::Start(Value::Param(Param(Ident("album".into())))),
            Part::Graph(Graph {
                dir: Dir::Out,
                what: Tables(vec![Table("album_to_song".into())]),
                expr: Fields::all(),
                ..Default::default()
            }),
            Part::Field(Ident("out".into())),
        ]))]),
        ..Default::default()
    }
}

/// Query to remove songs from an album
///
/// Compiles to:
///
/// ```sql, ignore
/// DELETE $album->album_to_song WHERE out IN $songs
/// ```
#[must_use]
pub fn remove_songs_from_album() -> DeleteStatement {
    DeleteStatement {
        what: Values(vec![Value::Idiom(Idiom(vec![
            Part::Start(Value::Param(Param(Ident("album".into())))),
            Part::Graph(Graph {
                dir: Dir::Out,
                what: Tables(vec![Table("album_to_song".into())]),
                expr: Fields::all(),
                ..Default::default()
            }),
        ]))]),
        cond: Some(Cond(Value::Expression(Box::new(Expression::Binary {
            l: Value::Idiom(Idiom(vec![Part::Field(Ident("out".into()))])),
            o: Operator::Inside,
            r: Value::Param(Param(Ident("songs".into()))),
        })))),
        ..Default::default()
    }
}

/// Query to read the artist of an album
///
/// Compiles to:
///
/// ```sql, ignore
/// SELECT * FROM $id<-artist_to_album<-artist
/// ```
#[must_use]
pub fn read_artist_of_album() -> SelectStatement {
    SelectStatement {
        expr: Fields::all(),
        what: Values(vec![Value::Idiom(Idiom(vec![
            Part::Start(Value::Param(Param(Ident("id".into())))),
            Part::Graph(Graph {
                dir: Dir::In,
                what: Tables(vec![Table("artist_to_album".into())]),
                expr: Fields::all(),

                ..Default::default()
            }),
            Part::Graph(Graph {
                dir: Dir::In,
                what: Tables(vec![Table(schemas::artist::TABLE_NAME.into())]),
                expr: Fields::all(),
                ..Default::default()
            }),
        ]))]),
        ..Default::default()
    }
}
