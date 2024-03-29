use audiotags;
use lazy_static::lazy_static;
use mecomp_storage::db::schemas::song::{Song, SongMetadata};
use proptest::prelude::*;
use tempfile;
use tokio::sync::Mutex;

use crate::logger::{init_logger, init_tracing};

lazy_static! {
    static ref TEMP_MUSIC_DIR: tempfile::TempDir = tempfile::tempdir().unwrap();
    static ref INIT: Mutex<Option<()>> = Mutex::new(None);
}

pub async fn init() -> anyhow::Result<()> {
    let mut init = INIT.lock().await;
    if init.is_some() {
        return Ok(());
    }
    init_logger(log::LevelFilter::Debug);
    tracing::subscriber::set_global_default(init_tracing())?;

    init.replace(());

    Ok(())
}

const ARTIST_NAME_SEPARATOR: &str = ", ";

pub async fn create_song(
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

    let mut tags = audiotags::Tag::default().read_from_path(&base_path)?;
    tags.remove_album_artist();
    tags.remove_title();
    tags.remove_artist();
    tags.remove_genre();

    tags.add_artist(
        &artists
            .iter()
            .map(|a| format!("Artist {}", a))
            .collect::<Vec<_>>()
            .join(ARTIST_NAME_SEPARATOR),
    );
    tags.add_album_artist(
        &album_artists
            .iter()
            .map(|a| format!("Album Artist {}", a))
            .collect::<Vec<_>>()
            .join(ARTIST_NAME_SEPARATOR),
    );

    tags.set_album_title(&format!("Album {}", album));

    tags.set_title(&format!("Song {}", song));

    tags.set_genre(&format!("Genre {}", genre));

    let new_path =
        TEMP_MUSIC_DIR
            .path()
            .join(format!("song_{}_{}.mp3", song, rand::random::<u64>()));
    // copy the base file to the new path
    std::fs::copy(&base_path, &new_path)?;
    // write the new tags to the new file
    tags.write_to_path(new_path.to_str().unwrap())?;

    // now, we need to load a SongMetadata from the new file
    let song_metadata = SongMetadata::load_from_path(new_path, Some(ARTIST_NAME_SEPARATOR), None)?;

    // now, we need to create a Song from the SongMetadata
    Ok(Song::try_load_into_db(song_metadata).await?)
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

prop_compose! {
    pub fn arb_song_case() (
        song in 0..=10u8,
        artists in arb_vec(any::<u8>(), 1..10),
        album_artists in arb_vec(any::<u8>(), 1..10),
        album in 0..=10u8, genre in 0..=10u8
    ) -> SongCase {
        SongCase::new(song, artists, album_artists, album, genre)
    }
}

pub fn arb_vec_and_index<T>(
    item_strategy: impl Strategy<Value = T>,
    range: std::ops::Range<usize>,
) -> impl Strategy<Value = (Vec<T>, usize)>
where
    T: Clone + std::fmt::Debug,
{
    prop::collection::vec(item_strategy, range).prop_flat_map(|vec| {
        let len = vec.len();
        (Just(vec), 0..len)
    })
}

pub fn arb_vec<T>(
    item_strategy: impl Strategy<Value = T>,
    range: std::ops::Range<usize>,
) -> impl Strategy<Value = Vec<T>>
where
    T: Clone + std::fmt::Debug,
{
    prop::collection::vec(item_strategy, range)
}