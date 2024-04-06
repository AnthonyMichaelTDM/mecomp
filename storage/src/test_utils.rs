use std::{ops::RangeInclusive, path::PathBuf, str::FromStr, sync::Arc};

use rand::seq::IteratorRandom;
use rstest::fixture;
use surrealdb::sql::{Duration, Id};

use crate::{
    db::schemas::song::{Song, SongChangeSet},
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
        duration: Duration::from_secs(120),
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
    song: u8,
    artists: Vec<u8>,
    album_artists: Vec<u8>,
    album: u8,
    genre: u8,
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
