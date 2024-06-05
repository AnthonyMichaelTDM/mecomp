//! utilitites used for testing
//!
//! TODO: the other modules (core and storage) also have test_utils modules. Should we combine them into a new crate?

use std::{
    ops::{Range, RangeInclusive},
    sync::OnceLock,
    time::Duration,
};

use lazy_static::lazy_static;
use lofty::{config::WriteOptions, file::TaggedFileExt, prelude::*, probe::Probe, tag::Accessor};
use mecomp_storage::db::schemas::song::{Song, SongMetadata};
use rand::seq::IteratorRandom;
use surrealdb::{Connection, Surreal};
use tempfile;
use tracing::instrument;

use mecomp_core::logger::{init_logger, init_tracing};

lazy_static! {
    static ref TEMP_MUSIC_DIR: tempfile::TempDir = tempfile::tempdir().unwrap();
}
static INIT: OnceLock<()> = OnceLock::new();

pub const TIMEOUT: std::time::Duration = Duration::from_secs(30);

pub fn init() {
    INIT.get_or_init(|| {
        init_logger(log::LevelFilter::Debug);
        if let Err(e) = tracing::subscriber::set_global_default(init_tracing()) {
            panic!("Error setting global default tracing subscriber: {e:?}")
        }
    });
}

const ARTIST_NAME_SEPARATOR: &str = ", ";

#[instrument()]
pub async fn create_song<C: Connection>(
    db: &Surreal<C>,
    SongCase {
        song,
        artists,
        album_artists,
        album,
        genre,
    }: SongCase,
) -> anyhow::Result<Song> {
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

    let new_path =
        TEMP_MUSIC_DIR
            .path()
            .join(format!("song_{}_{}.mp3", song, rand::random::<u64>()));
    // copy the base file to the new path
    std::fs::copy(&base_path, &new_path)?;
    // write the new tags to the new file
    tag.save_to_path(new_path.to_str().unwrap(), WriteOptions::default())?;

    // now, we need to load a SongMetadata from the new file
    let song_metadata = SongMetadata::load_from_path(new_path, Some(ARTIST_NAME_SEPARATOR), None)?;

    // now, we need to create a Song from the SongMetadata
    Ok(Song::try_load_into_db(db, song_metadata).await?)
}

#[derive(Debug, Clone)]
pub struct SongCase {
    song: u8,
    artists: Vec<u8>,
    album_artists: Vec<u8>,
    album: u8,
    genre: u8,
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

#[rstest::fixture]
pub fn foo_sc() -> SongCase {
    SongCase::new(0, vec![0], vec![0], 0, 0)
}

#[rstest::fixture]
pub fn bar_sc() -> SongCase {
    SongCase::new(1, vec![1, 0], vec![1], 0, 1)
}

#[rstest::fixture]
pub fn baz_sc() -> SongCase {
    SongCase::new(2, vec![1], vec![1, 0], 1, 2)
}

pub fn arb_song_case() -> impl Fn() -> SongCase {
    || {
        let artist_item_strategy = move || {
            (0..=10u8)
                .choose(&mut rand::thread_rng())
                .unwrap_or_default()
        };
        let rng = &mut rand::thread_rng();
        let artists = arb_vec(&artist_item_strategy, 1..=10)();
        let album_artists = arb_vec(&artist_item_strategy, 1..=10)();
        let song = (0..=10u8).choose(rng).unwrap_or_default();
        let album = (0..=10u8).choose(rng).unwrap_or_default();
        let genre = (0..=10u8).choose(rng).unwrap_or_default();

        SongCase::new(song, artists, album_artists, album, genre)
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
        let vec = arb_vec(item_strategy, range.clone())();
        let start = match range_start_mode {
            RangeStartMode::Standard => 0..vec.len(),
            RangeStartMode::OutOfBounds => vec.len()..(vec.len() + vec.len() / 2 + 1),
            RangeStartMode::Zero => 0..1,
        }
        .choose(&mut rand::thread_rng())
        .unwrap_or_default();
        let end = match range_end_mode {
            RangeEndMode::Standard => start..vec.len(),
            RangeEndMode::OutOfBounds => vec.len()..(vec.len() + vec.len() / 2 + 1).max(start),
            RangeEndMode::Start => start..(start + 1),
        }
        .choose(&mut rand::thread_rng())
        .unwrap_or_default();

        let index = match index_mode {
            RangeIndexMode::InBounds => 0..vec.len(),
            RangeIndexMode::InRange => start..end,
            RangeIndexMode::AfterRangeInBounds => end..vec.len(),
            RangeIndexMode::OutOfBounds => vec.len()..(vec.len() + vec.len() / 2 + 1),
            RangeIndexMode::BeforeRange => 0..start,
        }
        .choose(&mut rand::thread_rng());

        (vec, start..end, index)
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

#[cfg(test)]
mod tests {
    use super::*;
    use mecomp_storage::db::init_test_database;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_create_song() {
        init();

        let db = init_test_database().await.unwrap();
        // Create a test case
        let song_case = SongCase::new(0, vec![0], vec![0], 0, 0);

        // Call the create_song function
        let result = create_song(&db, song_case).await;

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
