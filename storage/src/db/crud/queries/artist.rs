use crate::db::schemas::artist::TABLE_NAME;
use surrealdb::sql::{
    statements::{DeleteStatement, OutputStatement, RelateStatement, SelectStatement},
    Cond, Dir, Expression, Fields, Graph, Ident, Idiom, Limit, Operator, Param, Part, Subquery,
    Table, Tables, Value, Values,
};

/// Query to read an artist by their name.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM artist WHERE name = $name LIMIT 1
/// ```
pub fn read_by_name() -> SelectStatement {
    SelectStatement {
        expr: Fields::all(),
        what: Values(vec![Value::Table(Table(TABLE_NAME.to_string()))]),
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
pub fn read_by_names() -> SelectStatement {
    SelectStatement {
        expr: Fields::all(),
        what: Values(vec![Value::Table(Table(TABLE_NAME.to_string()))]),
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
pub fn read_albums_by_artist() -> SelectStatement {
    SelectStatement {
        expr: Fields::all(),
        what: Values(vec![Value::Idiom(Idiom(vec![
            Part::Start(Value::Param(Param(Ident("id".into())))),
            Part::Graph(Graph {
                dir: Dir::Out,
                what: Tables(vec![Table("artist_to_album".into())]),
                expr: Fields::all(),
                ..Default::default()
            }),
            Part::Graph(Graph {
                dir: Dir::Out,
                what: Tables(vec![Table("album".into())]),
                expr: Fields::all(),
                ..Default::default()
            }),
        ]))]),
        ..Default::default()
    }
}

/// Query to relate an artist to an album.
///
/// Compiles to:
/// ```sql, ignore
/// RELATE $id->artist_to_album->$album
/// ```
pub fn relate_artist_to_album() -> RelateStatement {
    RelateStatement {
        from: Value::Param(Param(Ident("id".into()))),
        kind: Value::Table(Table("artist_to_album".into())),
        with: Value::Param(Param(Ident("album".into()))),
        ..Default::default()
    }
}

/// Query to relate artists to an album.
///
/// Compiles to:
/// ```sql, ignore
/// RELATE $ids->artist_to_album->$album
/// ```
pub fn relate_artists_to_album() -> RelateStatement {
    RelateStatement {
        from: Value::Param(Param(Ident("ids".into()))),
        kind: Value::Table(Table("artist_to_album".into())),
        with: Value::Param(Param(Ident("album".into()))),
        ..Default::default()
    }
}

/// Query to relate an artists to songs.
///
/// Compiles to:
/// ```sql, ignore
/// RELATE $id->artist_to_song->$songs
/// ```
pub fn relate_artist_to_songs() -> RelateStatement {
    RelateStatement {
        from: Value::Param(Param(Ident("id".into()))),
        kind: Value::Table(Table("artist_to_song".into())),
        with: Value::Param(Param(Ident("songs".into()))),
        ..Default::default()
    }
}

/// Query to remove songs from an artist.
///
/// Compiles to:
/// ```sql, ignore
/// DELETE $artist->artist_to_song WHERE out IN $songs
/// ```
pub fn remove_songs_from_artist() -> DeleteStatement {
    DeleteStatement {
        what: Values(vec![Value::Idiom(Idiom(vec![
            Part::Start(Value::Param(Param(Ident("artist".into())))),
            Part::Graph(Graph {
                dir: Dir::Out,
                what: Tables(vec![Table("artist_to_song".into())]),
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

/// Query to read all the songs associated with an artist.
///
/// Compiles to:
/// ```sql, ignore
/// RETURN array::union((SELECT * FROM $artist->artist_to_song->song), (SELECT * FROM $artist->artist_to_album->album->album_to_song->song))
/// ```
pub fn read_songs_by_artist() -> OutputStatement {
    OutputStatement {
        what: Value::Function(Box::new(surrealdb::sql::Function::Normal(
            "array::union".into(),
            vec![
                Value::Subquery(Box::new(Subquery::Select(SelectStatement {
                    expr: Fields::all(),
                    what: Values(vec![Value::Idiom(Idiom(vec![
                        Part::Start(Value::Param(Param(Ident("artist".into())))),
                        Part::Graph(Graph {
                            dir: Dir::Out,
                            what: Tables(vec![Table("artist_to_song".into())]),
                            expr: Fields::all(),
                            ..Default::default()
                        }),
                        Part::Graph(Graph {
                            dir: Dir::Out,
                            what: Tables(vec![Table("song".into())]),
                            expr: Fields::all(),
                            ..Default::default()
                        }),
                    ]))]),
                    ..Default::default()
                }))),
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
                            what: Tables(vec![Table("album".into())]),
                            expr: Fields::all(),
                            ..Default::default()
                        }),
                        Part::Graph(Graph {
                            dir: Dir::Out,
                            what: Tables(vec![Table("album_to_song".into())]),
                            expr: Fields::all(),
                            ..Default::default()
                        }),
                        Part::Graph(Graph {
                            dir: Dir::Out,
                            what: Tables(vec![Table("song".into())]),
                            expr: Fields::all(),
                            ..Default::default()
                        }),
                    ]))]),
                    ..Default::default()
                }))),
            ],
        ))),
        ..Default::default()
    }
}
