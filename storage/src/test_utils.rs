use std::{ops::RangeInclusive, path::PathBuf, str::FromStr, sync::Arc};

use anyhow::Result;
use lazy_static::lazy_static;
use rand::seq::IteratorRandom;
use rstest::fixture;
use surrealdb::sql::{Duration, Id};

lazy_static! {
    static ref TEMP_MUSIC_DIR: tempfile::TempDir = tempfile::tempdir().unwrap();
}

use crate::{
    db::schemas::song::{Song, SongChangeSet, SongMetadata},
    util::OneOrMany,
};

pub async fn create_song(
    SongCase {
        song,
        artists,
        album_artists,
        album,
        genre,
    }: SongCase,
    overrides: SongChangeSet,
    ulid: &str,
) -> anyhow::Result<Song> {
    let song = Song {
        id: Song::generate_id(),
        title: Arc::from(format!("Song {song} {ulid}").as_str()),
        artist: artists
            .iter()
            .map(|a| format!("Artist {a} {ulid}"))
            .map(Arc::from)
            .collect::<Vec<_>>()
            .into(),
        album_artist: album_artists
            .iter()
            .map(|a| format!("Artist {a} {ulid}"))
            .map(Arc::from)
            .collect::<Vec<_>>()
            .into(),
        album: Arc::from(format!("Album {album} {ulid}").as_str()),
        genre: OneOrMany::One(Arc::from(format!("Genre {genre} {ulid}").as_str())),
        runtime: Duration::from_secs(120),
        track: None,
        disc: None,
        release_year: None,
        extension: Arc::from("mp3"),
        path: PathBuf::from_str(&format!(
            "song_{song}_{}_{ulid}.mp3",
            rand::random::<usize>()
        ))?,
    };

    Song::create(song.clone()).await?;
    Song::update(song.id.clone(), overrides).await?;
    Ok(song)
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
        let rng = &mut rand::thread_rng();

        SongCase::new(
            (0..=10u8).choose(rng).unwrap_or_default(),
            arb_vec(&rand::random::<u8>, 1..=10)(),
            arb_vec(&rand::random::<u8>, 1..=10)(),
            (0..=10u8).choose(rng).unwrap_or_default(),
            (0..=10u8).choose(rng).unwrap_or_default(),
        )
    }
}

pub fn arb_vec_and_index<T>(
    item_strategy: &impl Fn() -> T,
    range: RangeInclusive<usize>,
) -> impl Fn() -> (Vec<T>, usize) + '_
where
    T: Clone + std::fmt::Debug + Sized,
{
    move || {
        let vec = arb_vec(item_strategy, range.clone())();
        let index = (0..vec.len())
            .choose(&mut rand::thread_rng())
            .unwrap_or_default();
        (vec, index)
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
        Vec::from_iter(std::iter::repeat_with(|| item_strategy()).take(size))
    }
}

#[fixture]
/// a unique identifier generator for testing, so that tests don't interfere with each other
pub fn ulid() -> String {
    Id::ulid().to_string()
}

pub fn song_metadata_from_case(
    SongCase {
        song,
        artists,
        album_artists,
        album,
        genre,
    }: SongCase,
    ulid: &str,
) -> Result<SongMetadata> {
    // we have an example mp3 in `assets/`, we want to take that and create a new audio file with psuedorandom id3 tags
    let base_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../assets/music.mp3")
        .canonicalize()?;

    let mut tags = audiotags::Tag::default().read_from_path(&base_path)?;
    tags.remove_album_artist();
    tags.remove_title();
    tags.remove_artist();
    tags.remove_genre();

    tags.add_artist(
        &artists
            .iter()
            .map(|a| format!("Artist {a} {ulid}"))
            .collect::<Vec<_>>()
            .join(", "),
    );
    tags.add_album_artist(
        &album_artists
            .iter()
            .map(|a| format!("Artist {a} {ulid}"))
            .collect::<Vec<_>>()
            .join(", "),
    );

    tags.set_album_title(&format!("Album {album} {ulid}"));

    tags.set_title(&format!("Song {song} {ulid}"));

    tags.set_genre(&format!("Genre {genre} {ulid}"));

    let new_path = TEMP_MUSIC_DIR
        .path()
        .join(format!("song_{song}_{ulid}.mp3"));
    // copy the base file to the new path
    std::fs::copy(&base_path, &new_path)?;
    // write the new tags to the new file
    tags.write_to_path(new_path.to_str().unwrap())?;

    // now, we need to load a SongMetadata from the new file
    Ok(SongMetadata::load_from_path(new_path, Some(", "), None)?)
}
