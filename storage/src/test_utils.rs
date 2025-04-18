use std::{
    ops::{Range, RangeInclusive},
    path::PathBuf,
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use anyhow::Result;
use lofty::{config::WriteOptions, file::TaggedFileExt, prelude::*, probe::Probe, tag::Accessor};
use one_or_many::OneOrMany;
use rand::{seq::IteratorRandom, Rng};
#[cfg(feature = "db")]
use surrealdb::{
    engine::local::{Db, Mem},
    sql::Id,
    Connection, Surreal,
};

#[cfg(feature = "analysis")]
use crate::db::schemas::analysis::Analysis;
#[cfg(not(feature = "db"))]
use crate::db::schemas::Id;
use crate::db::schemas::{
    album::Album,
    artist::Artist,
    collection::Collection,
    playlist::Playlist,
    song::{Song, SongChangeSet, SongMetadata},
};

pub const ARTIST_NAME_SEPARATOR: &str = ", ";

/// Initialize a test database with the same tables as the main database.
/// This is useful for testing queries and mutations.
///
/// # Errors
///
/// This function will return an error if the database cannot be initialized.
#[cfg(feature = "db")]
#[allow(clippy::missing_inline_in_public_items)]
pub async fn init_test_database() -> surrealdb::Result<Surreal<Db>> {
    use crate::db::{
        queries::relations::define_relation_tables, schemas::dynamic::DynamicPlaylist,
    };

    let db = Surreal::new::<Mem>(()).await?;
    db.use_ns("test").use_db("test").await?;

    crate::db::register_custom_analyzer(&db).await?;
    surrealqlx::register_tables!(
        &db,
        Album,
        Artist,
        Song,
        Collection,
        Playlist,
        DynamicPlaylist
    )?;
    #[cfg(feature = "analysis")]
    surrealqlx::register_tables!(&db, Analysis)?;

    define_relation_tables(&db).await?;

    Ok(db)
}

/// Initialize a test database with some basic state
///
/// # What will be created:
///
/// - a playlist named "Playlist 0"
/// - a collection named "Collection 0"
/// - optionally, a passed `DynamicPlaylist`
/// - `song_count` arbitrary songs whose values are determined by the given `song_case_func`
/// - a file in the given `TempDir` for each song
///
/// Can optionally also create a dynamic playlist with given information
///
/// You can pass functions to be used to create the songs and playlists
///
/// `song_case_func` signature
/// `FnMut(usize) -> (SongCase, bool, bool)`
/// - `i`: which song this is, 0..`song_count`
/// - returns: `(the song_case to use when generating the song, whether the song should be added to the playlist, whether it should be added to the collection`
///
/// Note: will actually create files for the songs in the passed `TempDir`
///
/// # Panics
///
/// Panics if an error occurs during the above process, this is intended to only be used for testing
/// so panicking when something goes wrong ensures that tests will fail and the backtrace will point
/// to whatever line caused the panic in here.
#[cfg(feature = "db")]
#[allow(clippy::missing_inline_in_public_items)]
pub async fn init_test_database_with_state<SCF>(
    song_count: std::num::NonZero<usize>,
    mut song_case_func: SCF,
    dynamic: Option<crate::db::schemas::dynamic::DynamicPlaylist>,
    tempdir: &tempfile::TempDir,
) -> Arc<Surreal<Db>>
where
    SCF: FnMut(usize) -> (SongCase, bool, bool) + Send + Sync,
{
    use anyhow::Context;

    use crate::db::schemas::dynamic::DynamicPlaylist;

    let db = Arc::new(init_test_database().await.unwrap());

    // create the playlist, collection, and optionally the dynamic playlist
    let playlist = Playlist {
        id: Playlist::generate_id(),
        name: "Playlist 0".into(),
        runtime: Duration::from_secs(0),
        song_count: 0,
    };
    let playlist = Playlist::create(&db, playlist).await.unwrap().unwrap();

    let collection = Collection {
        id: Collection::generate_id(),
        name: "Collection 0".into(),
        runtime: Duration::from_secs(0),
        song_count: 0,
    };
    let collection = Collection::create(&db, collection).await.unwrap().unwrap();

    if let Some(dynamic) = dynamic {
        let _ = DynamicPlaylist::create(&db, dynamic)
            .await
            .unwrap()
            .unwrap();
    }

    // create the songs
    for i in 0..(song_count.get()) {
        let (song_case, add_to_playlist, add_to_collection) = song_case_func(i);

        let metadata = create_song_metadata(tempdir, song_case.clone())
            .context(format!(
                "failed to create metadata for song case {song_case:?}"
            ))
            .unwrap();

        let song = Song::try_load_into_db(&db, metadata)
            .await
            .context(format!(
                "Failed to load into db the song case: {song_case:?}"
            ))
            .unwrap();

        if add_to_playlist {
            Playlist::add_songs(&db, playlist.id.clone(), vec![song.id.clone()])
                .await
                .unwrap();
        }
        if add_to_collection {
            Collection::add_songs(&db, collection.id.clone(), vec![song.id.clone()])
                .await
                .unwrap();
        }
    }

    db
}

/// Create a song with the given case, and optionally apply the given overrides.
///
/// The created song is shallow, meaning that the artists, album artists, and album are not created in the database.
///
/// # Errors
///
/// This function will return an error if the song cannot be created.
///
/// # Panics
///
/// Panics if the song can't be read from the database after creation.
#[cfg(feature = "db")]
#[allow(clippy::missing_inline_in_public_items)]
pub async fn create_song_with_overrides<C: Connection>(
    db: &Surreal<C>,
    SongCase {
        song,
        artists,
        album_artists,
        album,
        genre,
    }: SongCase,
    overrides: SongChangeSet,
) -> Result<Song> {
    let id = Song::generate_id();
    let song = Song {
        id: id.clone(),
        title: Into::into(format!("Song {song}").as_str()),
        artist: artists
            .iter()
            .map(|a| format!("Artist {a}"))
            .collect::<Vec<_>>()
            .into(),
        album_artist: album_artists
            .iter()
            .map(|a| format!("Artist {a}"))
            .collect::<Vec<_>>()
            .into(),
        album: format!("Album {album}"),
        genre: OneOrMany::One(format!("Genre {genre}")),
        runtime: Duration::from_secs(120),
        track: None,
        disc: None,
        release_year: None,
        extension: "mp3".into(),
        path: PathBuf::from_str(&format!("{}.mp3", id.key()))?,
    };

    Song::create(db, song.clone()).await?;
    if overrides != SongChangeSet::default() {
        Song::update(db, song.id.clone(), overrides).await?;
    }
    let song = Song::read(db, song.id).await?.expect("Song should exist");
    Ok(song)
}

/// Creates a song file with the given case and overrides.
/// The song file is created in a temporary directory.
/// The song metadata is created from the song file.
/// The song is not added to the database.
///
/// # Errors
///
/// This function will return an error if the song metadata cannot be created.
#[allow(clippy::missing_inline_in_public_items)]
pub fn create_song_metadata(
    tempdir: &tempfile::TempDir,
    SongCase {
        song,
        artists,
        album_artists,
        album,
        genre,
    }: SongCase,
) -> Result<SongMetadata> {
    // we have an example mp3 in `assets/`, we want to take that and create a new audio file with psuedorandom id3 tags
    let base_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../assets/music.mp3")
        .canonicalize()?;

    let mut tagged_file = Probe::open(&base_path)?.read()?;
    let tag = match tagged_file.primary_tag_mut() {
        Some(primary_tag) => primary_tag,
        // If the "primary" tag doesn't exist, we just grab the
        // first tag we can find. Realistically, a tag reader would likely
        // iterate through the tags to find a suitable one.
        None => tagged_file
            .first_tag_mut()
            .ok_or_else(|| anyhow::anyhow!("ERROR: No tags found"))?,
    };

    tag.insert_text(
        ItemKey::AlbumArtist,
        album_artists
            .iter()
            .map(|a| format!("Artist {a}"))
            .collect::<Vec<_>>()
            .join(ARTIST_NAME_SEPARATOR),
    );

    tag.remove_artist();
    tag.set_artist(
        artists
            .iter()
            .map(|a| format!("Artist {a}"))
            .collect::<Vec<_>>()
            .join(ARTIST_NAME_SEPARATOR),
    );

    tag.remove_album();
    tag.set_album(format!("Album {album}"));

    tag.remove_title();
    tag.set_title(format!("Song {song}"));

    tag.remove_genre();
    tag.set_genre(format!("Genre {genre}"));

    let new_path = tempdir.path().join(format!("song_{}.mp3", Id::ulid()));
    // copy the base file to the new path
    std::fs::copy(&base_path, &new_path)?;
    // write the new tags to the new file
    tag.save_to_path(&new_path, WriteOptions::default())?;

    // now, we need to load a SongMetadata from the new file
    Ok(SongMetadata::load_from_path(
        new_path,
        &OneOrMany::One(ARTIST_NAME_SEPARATOR.to_string()),
        &OneOrMany::None,
        None,
    )?)
}

#[derive(Debug, Clone)]
pub struct SongCase {
    pub song: u8,
    pub artists: Vec<u8>,
    pub album_artists: Vec<u8>,
    pub album: u8,
    pub genre: u8,
}

impl SongCase {
    #[must_use]
    #[inline]
    pub const fn new(
        song: u8,
        artists: Vec<u8>,
        album_artists: Vec<u8>,
        album: u8,
        genre: u8,
    ) -> Self {
        Self {
            song,
            artists,
            album_artists,
            album,
            genre,
        }
    }
}

#[inline]
pub const fn arb_song_case() -> impl Fn() -> SongCase {
    || {
        let artist_item_strategy = move || {
            (0..=10u8)
                .choose(&mut rand::thread_rng())
                .unwrap_or_default()
        };
        let rng = &mut rand::thread_rng();
        let artists = arb_vec(&artist_item_strategy, 1..=10)()
            .into_iter()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let album_artists = arb_vec(&artist_item_strategy, 1..=10)()
            .into_iter()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let song = (0..=10u8).choose(rng).unwrap_or_default();
        let album = (0..=10u8).choose(rng).unwrap_or_default();
        let genre = (0..=10u8).choose(rng).unwrap_or_default();

        SongCase::new(song, artists, album_artists, album, genre)
    }
}

#[inline]
pub const fn arb_vec<T>(
    item_strategy: &impl Fn() -> T,
    range: RangeInclusive<usize>,
) -> impl Fn() -> Vec<T> + '_
where
    T: Clone + std::fmt::Debug + Sized,
{
    move || {
        let size = range
            .clone()
            .choose(&mut rand::thread_rng())
            .unwrap_or_default();
        std::iter::repeat_with(item_strategy).take(size).collect()
    }
}

pub enum IndexMode {
    InBounds,
    OutOfBounds,
}

#[inline]
pub const fn arb_vec_and_index<T>(
    item_strategy: &impl Fn() -> T,
    range: RangeInclusive<usize>,
    index_mode: IndexMode,
) -> impl Fn() -> (Vec<T>, usize) + '_
where
    T: Clone + std::fmt::Debug + Sized,
{
    move || {
        let vec = arb_vec(item_strategy, range.clone())();
        let index = match index_mode {
            IndexMode::InBounds => 0..vec.len(),
            #[allow(clippy::range_plus_one)]
            IndexMode::OutOfBounds => vec.len()..(vec.len() + vec.len() / 2 + 1),
        }
        .choose(&mut rand::thread_rng())
        .unwrap_or_default();
        (vec, index)
    }
}

pub enum RangeStartMode {
    Standard,
    Zero,
    OutOfBounds,
}

pub enum RangeEndMode {
    Start,
    Standard,
    OutOfBounds,
}

pub enum RangeIndexMode {
    InBounds,
    InRange,
    AfterRangeInBounds,
    OutOfBounds,
    BeforeRange,
}

// Returns a tuple of a Vec of T and a Range<usize>
// where the start is a random index in the Vec
// and the end is a random index in the Vec that is greater than or equal to the start
#[inline]
pub const fn arb_vec_and_range_and_index<T>(
    item_strategy: &impl Fn() -> T,
    range: RangeInclusive<usize>,
    range_start_mode: RangeStartMode,
    range_end_mode: RangeEndMode,
    index_mode: RangeIndexMode,
) -> impl Fn() -> (Vec<T>, Range<usize>, Option<usize>) + '_
where
    T: Clone + std::fmt::Debug + Sized,
{
    move || {
        let rng = &mut rand::thread_rng();
        let vec = arb_vec(item_strategy, range.clone())();
        let start = match range_start_mode {
            RangeStartMode::Standard => 0..vec.len(),
            #[allow(clippy::range_plus_one)]
            RangeStartMode::OutOfBounds => vec.len()..(vec.len() + vec.len() / 2 + 1),
            RangeStartMode::Zero => 0..1,
        }
        .choose(rng)
        .unwrap_or_default();
        let end = match range_end_mode {
            RangeEndMode::Standard => start..vec.len(),
            #[allow(clippy::range_plus_one)]
            RangeEndMode::OutOfBounds => vec.len()..(vec.len() + vec.len() / 2 + 1).max(start),
            #[allow(clippy::range_plus_one)]
            RangeEndMode::Start => start..(start + 1),
        }
        .choose(rng)
        .unwrap_or_default();

        let index = match index_mode {
            RangeIndexMode::InBounds => 0..vec.len(),
            RangeIndexMode::InRange => start..end,
            RangeIndexMode::AfterRangeInBounds => end..vec.len(),
            #[allow(clippy::range_plus_one)]
            RangeIndexMode::OutOfBounds => vec.len()..(vec.len() + vec.len() / 2 + 1),
            RangeIndexMode::BeforeRange => 0..start,
        }
        .choose(rng);

        (vec, start..end, index)
    }
}

#[inline]
pub const fn arb_analysis_features() -> impl Fn() -> [f64; 20] {
    move || {
        let rng = &mut rand::thread_rng();
        let mut features = [0.0; 20];
        for feature in &mut features {
            *feature = rng.gen_range(-1.0..1.0);
        }
        features
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_create_song() {
        let db = init_test_database().await.unwrap();
        // Create a test case
        let song_case = SongCase::new(0, vec![0], vec![0], 0, 0);

        // Call the create_song function
        let result = create_song_with_overrides(&db, song_case, SongChangeSet::default()).await;

        // Assert that the result is Ok
        if let Err(e) = result {
            panic!("Error creating song: {e:?}");
        }

        // Get the Song from the result
        let song = result.unwrap();

        // Assert that we can get the song from the database
        let song_from_db = Song::read(&db, song.id.clone()).await.unwrap().unwrap();

        // Assert that the song from the database is the same as the song we created
        assert_eq!(song, song_from_db);
    }
}
