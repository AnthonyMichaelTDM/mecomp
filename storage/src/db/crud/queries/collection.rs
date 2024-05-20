use surrealdb::sql::{
    statements::{DeleteStatement, RelateStatement, SelectStatement, UpdateStatement},
    Cond, Data, Dir, Expression, Fields, Graph, Ident, Idiom, Operator, Param, Part, Table, Tables,
    Value, Values,
};

/// Query to relate a collection to its songs.
///
/// Compiles to:
/// ```sql, ignore
/// RELATE $id->collection_to_song->$songs
/// ```
#[must_use]
pub fn relate_collection_to_songs() -> RelateStatement {
    RelateStatement {
        from: Value::Param(Param(Ident("id".into()))),
        kind: Value::Table(Table("collection_to_song".into())),
        with: Value::Param(Param(Ident("songs".into()))),
        ..Default::default()
    }
}

/// Query to read the songs of a collection
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $id->collection_to_song.out
/// ```
#[must_use]
pub fn read_songs_in_collection() -> SelectStatement {
    SelectStatement {
        expr: Fields::all(),
        what: Values(vec![Value::Idiom(Idiom(vec![
            Part::Start(Value::Param(Param(Ident("id".into())))),
            Part::Graph(Graph {
                dir: Dir::Out,
                what: Tables(vec![Table("collection_to_song".into())]),
                expr: Fields::all(),
                ..Default::default()
            }),
            Part::Field(Ident("out".into())),
        ]))]),
        ..Default::default()
    }
}

/// Query to remove songs from a collection
///
/// Compiles to:
/// ```sql, ignore
/// DELETE $id->collection_to_song WHERE out IN $songs
/// ```
#[must_use]
pub fn remove_songs_from_collection() -> DeleteStatement {
    DeleteStatement {
        what: Values(vec![Value::Idiom(Idiom(vec![
            Part::Start(Value::Param(Param(Ident("id".into())))),
            Part::Graph(Graph {
                dir: Dir::Out,
                what: Tables(vec![Table("collection_to_song".into())]),
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

/// Query to "repair" a collection.
///
/// This query updates the `song_count` and runtime of the collection.
///
/// Compiles to:
/// ```sql, ignore
/// UPDATE $id SET song_count=$songs, runtime=$runtime
/// ```
#[must_use]
pub fn repair() -> UpdateStatement {
    UpdateStatement {
        what: Values(vec![Value::Param(Param(Ident("id".into())))]),
        data: Some(Data::SetExpression(vec![
            (
                Idiom(vec![Part::Field(Ident("song_count".into()))]),
                Operator::Equal,
                Value::Param(Param(Ident("songs".into()))),
            ),
            (
                Idiom(vec![Part::Field(Ident("runtime".into()))]),
                Operator::Equal,
                Value::Param(Param(Ident("runtime".into()))),
            ),
        ])),
        ..Default::default()
    }
}
