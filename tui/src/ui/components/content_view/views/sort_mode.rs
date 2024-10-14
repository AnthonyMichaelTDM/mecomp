use std::{fmt::Display, marker::PhantomData};

use mecomp_storage::db::schemas::{
    album::Album, artist::Artist, collection::Collection, playlist::Playlist, song::Song,
};

use super::traits::SortMode;

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SongSort {
    Title,
    #[default]
    Artist,
    Album,
    AlbumArtist,
    Genre,
}

impl Display for SongSort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Title => write!(f, "Title"),
            Self::Artist => write!(f, "Artist"),
            Self::Album => write!(f, "Album"),
            Self::AlbumArtist => write!(f, "Album Artist"),
            Self::Genre => write!(f, "Genre"),
        }
    }
}

impl SortMode<Song> for SongSort {
    fn next(&self) -> Self {
        match self {
            Self::Title => Self::Artist,
            Self::Artist => Self::Album,
            Self::Album => Self::AlbumArtist,
            Self::AlbumArtist => Self::Genre,
            Self::Genre => Self::Title,
        }
    }

    fn prev(&self) -> Self {
        match self {
            Self::Title => Self::Genre,
            Self::Artist => Self::Title,
            Self::Album => Self::Artist,
            Self::AlbumArtist => Self::Album,
            Self::Genre => Self::AlbumArtist,
        }
    }

    fn sort_items(&self, songs: &mut [Song]) {
        fn key<T: AsRef<str>>(input: T) -> String {
            input
                .as_ref()
                .to_lowercase() // ignore case
                .trim_start_matches(|c: char| !c.is_alphanumeric()) // ignore leading non-alphanumeric characters
                .trim_start_matches("the ") // ignore leading "the "
                .to_owned()
        }

        match self {
            Self::Title => songs.sort_by_key(|song| key(&song.title)),
            Self::Artist => {
                songs.sort_by_cached_key(|song| song.artist.iter().map(key).collect::<Vec<_>>());
            }
            Self::Album => songs.sort_by_key(|song| key(&song.album)),
            Self::AlbumArtist => {
                songs.sort_by_cached_key(|song| {
                    song.album_artist.iter().map(key).collect::<Vec<_>>()
                });
            }
            Self::Genre => {
                songs.sort_by_cached_key(|song| song.genre.iter().map(key).collect::<Vec<_>>());
            }
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum AlbumSort {
    Title,
    #[default]
    Artist,
    ReleaseYear,
}

impl Display for AlbumSort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Title => write!(f, "Title"),
            Self::Artist => write!(f, "Artist"),
            Self::ReleaseYear => write!(f, "Year"),
        }
    }
}

impl SortMode<Album> for AlbumSort {
    fn next(&self) -> Self {
        match self {
            Self::Title => Self::Artist,
            Self::Artist => Self::ReleaseYear,
            Self::ReleaseYear => Self::Title,
        }
    }

    fn prev(&self) -> Self {
        match self {
            Self::Title => Self::ReleaseYear,
            Self::Artist => Self::Title,
            Self::ReleaseYear => Self::Artist,
        }
    }

    fn sort_items(&self, albums: &mut [Album]) {
        fn key<T: AsRef<str>>(input: T) -> String {
            input
                .as_ref()
                .to_lowercase() // ignore case
                .trim_start_matches(|c: char| !c.is_alphanumeric()) // ignore leading non-alphanumeric characters
                .trim_start_matches("the ") // ignore leading "the "
                .to_owned()
        }

        match self {
            Self::Title => albums.sort_by_key(|album| key(&album.title)),
            Self::Artist => {
                albums.sort_by_cached_key(|album| album.artist.iter().map(key).collect::<Vec<_>>());
            }
            Self::ReleaseYear => {
                albums.sort_by_key(|album| album.release.unwrap_or(0));
                albums.reverse();
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NameSort<T>(PhantomData<T>);

impl<T> Display for NameSort<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Name")
    }
}

impl<T> NameSort<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> Default for NameSort<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl SortMode<Artist> for NameSort<Artist> {
    #[must_use]
    fn next(&self) -> Self {
        Self::new()
    }

    #[must_use]
    fn prev(&self) -> Self {
        Self::new()
    }

    #[allow(clippy::unused_self)]
    fn sort_items(&self, items: &mut [Artist]) {
        fn key<T: AsRef<str>>(input: T) -> String {
            input
                .as_ref()
                .to_lowercase() // ignore case
                .trim_start_matches(|c: char| !c.is_alphanumeric()) // ignore leading non-alphanumeric characters
                .trim_start_matches("the ") // ignore leading "the "
                .to_owned()
        }
        items.sort_by_key(|item| key(&item.name));
    }
}

impl SortMode<Collection> for NameSort<Collection> {
    #[must_use]
    fn next(&self) -> Self {
        Self::new()
    }

    #[must_use]
    fn prev(&self) -> Self {
        Self::new()
    }

    #[allow(clippy::unused_self)]
    fn sort_items(&self, items: &mut [Collection]) {
        fn key<T: AsRef<str>>(input: T) -> String {
            input
                .as_ref()
                .to_lowercase() // ignore case
                .trim_start_matches(|c: char| !c.is_alphanumeric()) // ignore leading non-alphanumeric characters
                .trim_start_matches("the ") // ignore leading "the "
                .to_owned()
        }
        items.sort_by_key(|item| key(&item.name));
    }
}

impl SortMode<Playlist> for NameSort<Playlist> {
    #[must_use]
    fn next(&self) -> Self {
        Self::new()
    }

    #[must_use]
    fn prev(&self) -> Self {
        Self::new()
    }

    #[allow(clippy::unused_self)]
    fn sort_items(&self, items: &mut [Playlist]) {
        fn key<T: AsRef<str>>(input: T) -> String {
            input
                .as_ref()
                .to_lowercase() // ignore case
                .trim_start_matches(|c: char| !c.is_alphanumeric()) // ignore leading non-alphanumeric characters
                .trim_start_matches("the ") // ignore leading "the "
                .to_owned()
        }
        items.sort_by_key(|item| key(&item.name));
    }
}