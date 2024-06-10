use std::{
    ops::{Range, RangeInclusive},
    path::PathBuf,
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use anyhow::Result;
use lofty::{config::WriteOptions, file::TaggedFileExt, prelude::*, probe::Probe, tag::Accessor};
use rand::seq::IteratorRandom;
use surrealdb::{
    engine::local::{Db, Mem},
    sql::Id,
    Connection, Surreal,
};

use crate::db::{
    register_custom_analyzer,
    schemas::{
        album::Album,
        artist::Artist,
        collection::Collection,
        playlist::Playlist,
        song::{Song, SongChangeSet, SongMetadata},
    },
};
use one_or_many::OneOrMany;

pub const ARTIST_NAME_SEPARATOR: &str = ", ";

/// Initialize a test database with the same tables as the main database.
/// This is useful for testing queries and mutations.
///
/// # Errors
///
/// This function will return an error if the database cannot be initialized.
pub async fn init_test_database() -> surrealdb::Result<Surreal<Db>> {
    let db = Surreal::new::<Mem>(()).await?;
    db.use_ns("test").use_db("test").await?;

    register_custom_analyzer(&db).await?;
    surrealqlx::register_tables!(&db, Album, Artist, Song, Collection, Playlist)?;

    Ok(db)
}

/// Create a song with the given case, and optionally apply the given overrides.
///
/// The created song is shallow, meaning that the artists, album artists, and album are not created in the database.
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
        title: Arc::from(format!("Song {song}").as_str()),
        artist: artists
            .iter()
            .map(|a| format!("Artist {a}"))
            .map(Arc::from)
            .collect::<Vec<_>>()
            .into(),
        album_artist: album_artists
            .iter()
            .map(|a| format!("Artist {a}"))
            .map(Arc::from)
            .collect::<Vec<_>>()
            .into(),
        album: Arc::from(format!("Album {album}").as_str()),
        genre: OneOrMany::One(Arc::from(format!("Genre {genre}").as_str())),
        runtime: Duration::from_secs(120),
        track: None,
        disc: None,
        release_year: None,
        extension: Arc::from("mp3"),
        path: PathBuf::from_str(&format!("{}.mp3", id.id))?,
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
            .ok_or(anyhow::anyhow!("ERROR: No tags found"))?,
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

    let new_path = tempdir
        .path()
        .join(format!("song_{}.mp3", Id::ulid().to_string()));
    // copy the base file to the new path
    std::fs::copy(&base_path, &new_path)?;
    // write the new tags to the new file
    tag.save_to_path(&new_path, WriteOptions::default())?;

    // now, we need to load a SongMetadata from the new file
    Ok(SongMetadata::load_from_path(
        new_path,
        Some(ARTIST_NAME_SEPARATOR),
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
    pub fn new(song: u8, artists: Vec<u8>, album_artists: Vec<u8>, album: u8, genre: u8) -> Self {
        Self {
            song,
            artists,
            album_artists,
            album,
            genre,
        }
    }
}

pub fn arb_song_case() -> impl Fn() -> SongCase {
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

pub fn arb_vec<T>(
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
        Vec::from_iter(std::iter::repeat_with(item_strategy).take(size))
    }
}

pub enum IndexMode {
    InBounds,
    OutOfBounds,
}

pub fn arb_vec_and_index<T>(
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
pub fn arb_vec_and_range_and_index<T>(
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
            RangeStartMode::OutOfBounds => vec.len()..(vec.len() + vec.len() / 2 + 1),
            RangeStartMode::Zero => 0..1,
        }
        .choose(rng)
        .unwrap_or_default();
        let end = match range_end_mode {
            RangeEndMode::Standard => start..vec.len(),
            RangeEndMode::OutOfBounds => vec.len()..(vec.len() + vec.len() / 2 + 1).max(start),
            RangeEndMode::Start => start..(start + 1),
        }
        .choose(rng)
        .unwrap_or_default();

        let index = match index_mode {
            RangeIndexMode::InBounds => 0..vec.len(),
            RangeIndexMode::InRange => start..end,
            RangeIndexMode::AfterRangeInBounds => end..vec.len(),
            RangeIndexMode::OutOfBounds => vec.len()..(vec.len() + vec.len() / 2 + 1),
            RangeIndexMode::BeforeRange => 0..start,
        }
        .choose(rng);

        (vec, start..end, index)
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
