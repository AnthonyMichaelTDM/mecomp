#[macro_use]
extern crate surrealqlx_macros;
/// This struct holds all the metadata about a particular [`Album`].
/// An [`Album`] is a collection of [`Song`]s owned by an [`Artist`].
#[derive(Table)]
#[Table("album")]
pub struct Album {
    /// The unique identifier for this [`Album`].
    #[field(dt = "record")]
    pub id: AlbumId,
    /// Title of the [`Album`].
    #[field(dt = "string", index())]
    pub title: Arc<str>,
    /// Ids of the [`Artist`] of this [`Album`] (Can be multiple)
    #[field(dt = "set<record> | record")]
    pub artist_id: OneOrMany<ArtistId>,
    /// Artist of the [`Album`]. (Can be multiple)
    #[field(dt = "set<string> | string")]
    pub artist: OneOrMany<Arc<str>>,
    /// Release year of this [`Album`].
    #[field(dt = "option<int>")]
    pub release: Option<i32>,
    /// Total runtime of this [`Album`].
    #[field(dt = "duration")]
    pub runtime: Duration,
    /// [`Song`] count of this [`Album`].
    #[field(dt = "int")]
    pub song_count: usize,
    // SOMEDAY:
    // This should be sorted based
    // off incrementing disc and track numbers, e.g:
    //
    // DISC 1:
    //   - 1. ...
    //   - 2. ...
    // DISC 2:
    //   - 1. ...
    //   - 2. ...
    //
    // So, doing `my_album.songs.iter()` will always
    // result in the correct `Song` order for `my_album`.
    /// The [`Id`]s of the [`Song`]s in this [`Album`].
    #[field(dt = "set<record>")]
    pub songs: Box<[SongId]>,
    /// How many discs are in this `Album`?
    /// (Most will only have 1).
    #[field(dt = "int")]
    pub discs: u32,
    /// This [`Album`]'s genre.
    #[field(dt = "set<string> | string")]
    pub genre: OneOrMany<Arc<str>>,
}
