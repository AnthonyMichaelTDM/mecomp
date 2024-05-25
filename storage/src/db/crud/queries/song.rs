use surrealdb::sql::{
    statements::SelectStatement, Cond, Dir, Expression, Fields, Graph, Ident, Idiom, Limit,
    Operator, Param, Part, Table, Tables, Value, Values,
};

use super::generic::read_related_in;

/// Query to read a song by its path
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM song WHERE path = $path LIMIT 1
/// ```
///
/// # Example
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::song::read_song_by_path;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_song_by_path();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM song WHERE path = $path LIMIT 1".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn read_song_by_path() -> SelectStatement {
    SelectStatement {
        expr: Fields::all(),
        what: Values(vec![Value::Table(Table("song".into()))]),
        cond: Some(Cond(Value::Expression(Box::new(Expression::Binary {
            l: Value::Idiom(Idiom(vec![Ident("path".into()).into()])),
            o: Operator::Equal,
            r: Value::Param(Param(Ident("path".into()))),
        })))),
        limit: Some(Limit(1.into())),
        ..Default::default()
    }
}

/// query to read the album of a song
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $id<-album_to_song.in
/// ```
///
/// # Example
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::song::read_album;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_album();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $id<-album_to_song.in".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn read_album() -> SelectStatement {
    read_related_in("id", "album_to_song")
}

/// Query to read the artist of a song
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $id<-artist_to_song.in
/// ```
///
/// # Example
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::song::read_artist;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_artist();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $id<-artist_to_song.in".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn read_artist() -> SelectStatement {
    read_related_in("id", "artist_to_song")
}

/// Query to read the album artist of a song
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $id<-album_to_song<-album<-artist_to_album.in
/// ```
///
/// # Example
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::song::read_album_artist;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_album_artist();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $id<-album_to_song<-album<-artist_to_album.in".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn read_album_artist() -> SelectStatement {
    SelectStatement {
        expr: Fields::all(),
        what: Values(vec![Value::Idiom(Idiom(vec![
            Part::Start(Value::Param(Param(Ident("id".into())))),
            Part::Graph(Graph {
                dir: Dir::In,
                what: Tables(vec![Table("album_to_song".into())]),
                expr: Fields::all(),
                ..Default::default()
            }),
            Part::Graph(Graph {
                dir: Dir::In,
                what: Tables(vec![Table("album".into())]),
                expr: Fields::all(),
                ..Default::default()
            }),
            Part::Graph(Graph {
                dir: Dir::In,
                what: Tables(vec![Table("artist_to_album".into())]),
                expr: Fields::all(),
                ..Default::default()
            }),
            Part::Field(Ident("in".into())),
        ]))]),
        ..Default::default()
    }
}
