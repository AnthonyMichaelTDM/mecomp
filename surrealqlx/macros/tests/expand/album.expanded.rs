#[macro_use]
extern crate surrealqlx_macros;
/// This struct holds all the metadata about a particular [`Album`].
/// An [`Album`] is a collection of [`Song`]s owned by an [`Artist`].
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
impl ::surrealqlx::traits::Table for Album {
    const TABLE_NAME: &'static str = "album";
    #[allow(manual_async_fn)]
    fn init_table<C: ::surrealdb::Connection>(
        db: &::surrealdb::Surreal<C>,
    ) -> impl ::std::future::Future<Output = ::surrealdb::Result<()>> + Send {
        async {
            let _ = db
                .query("BEGIN;")
                .query("DEFINE TABLE album SCHEMAFULL;")
                .query("COMMIT;")
                .query("BEGIN;")
                .query("DEFINE FIELD id ON album TYPE record;")
                .query("DEFINE FIELD title ON album TYPE string;")
                .query("DEFINE FIELD artist_id ON album TYPE set<record> | record;")
                .query("DEFINE FIELD artist ON album TYPE set<string> | string;")
                .query("DEFINE FIELD release ON album TYPE option<int>;")
                .query("DEFINE FIELD runtime ON album TYPE duration;")
                .query("DEFINE FIELD song_count ON album TYPE int;")
                .query("DEFINE FIELD songs ON album TYPE set<record>;")
                .query("DEFINE FIELD discs ON album TYPE int;")
                .query("DEFINE FIELD genre ON album TYPE set<string> | string;")
                .query("COMMIT;")
                .query("BEGIN;")
                .query("DEFINE INDEX album_title_index ON album FIELDS title;")
                .query("COMMIT;")
                .await?;
            Ok(())
        }
    }
}
