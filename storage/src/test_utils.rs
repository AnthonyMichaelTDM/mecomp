use std::{ops::RangeInclusive, path::PathBuf, str::FromStr, sync::Arc};

use anyhow::Result;
use lazy_static::lazy_static;
use lofty::{config::WriteOptions, file::TaggedFileExt, prelude::*, probe::Probe, tag::Accessor};
use rand::seq::IteratorRandom;
use rstest::fixture;
use surrealdb::{
    sql::{Duration, Id},
    Connection, Surreal,
};

lazy_static! {
    static ref TEMP_MUSIC_DIR: tempfile::TempDir = tempfile::tempdir().unwrap();
}

use crate::db::schemas::song::{Song, SongChangeSet, SongMetadata};
use one_or_many::OneOrMany;

pub async fn create_song<C: Connection>(
    db: &Surreal<C>,
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

    Song::create(db, song.clone()).await?;
    Song::update(db, song.id.clone(), overrides).await?;
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
        Vec::from_iter(std::iter::repeat_with(item_strategy).take(size))
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
            .map(|a| format!("Artist {a} {ulid}"))
            .collect::<Vec<_>>()
            .join(", "),
    );

    tag.remove_artist();
    tag.set_artist(
        artists
            .iter()
            .map(|a| format!("Artist {a} {ulid}"))
            .collect::<Vec<_>>()
            .join(", "),
    );

    tag.remove_album();
    tag.set_album(format!("Album {album}"));

    tag.remove_title();
    tag.set_title(format!("Song {song}"));

    tag.remove_genre();
    tag.set_genre(format!("Genre {genre}"));

    let new_path = TEMP_MUSIC_DIR
        .path()
        .join(format!("song_{song}_{ulid}.mp3"));
    // copy the base file to the new path
    std::fs::copy(&base_path, &new_path)?;
    // write the new tags to the new file
    tag.save_to_path(new_path.to_str().unwrap(), WriteOptions::default())?;

    // now, we need to load a SongMetadata from the new file
    Ok(SongMetadata::load_from_path(new_path, Some(", "), None)?)
}
