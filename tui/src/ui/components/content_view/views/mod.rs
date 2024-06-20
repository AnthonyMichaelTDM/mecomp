use mecomp_storage::db::schemas::{
    album::Album, artist::Artist, collection::Collection, playlist::Playlist, song::Song, Thing,
};
use one_or_many::OneOrMany;

pub mod album;
pub mod artist;
pub mod collection;
pub mod none;
pub mod playlist;
pub mod search;
pub mod song;

/// Data neaded by the views (that isn't directly handled by a state store)
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, Default)]
pub struct ViewData {
    pub album: Option<AlbumViewProps>,
    pub artist: Option<ArtistViewProps>,
    pub collection: Option<CollectionViewProps>,
    pub playlist: Option<PlaylistViewProps>,
    pub song: Option<SongViewProps>,
}

#[derive(Debug, Clone)]
pub struct AlbumViewProps {
    pub id: Thing,
    pub album: Album,
    pub artists: OneOrMany<Artist>,
    pub songs: Box<[Song]>,
}

#[derive(Debug, Clone)]
pub struct ArtistViewProps {
    pub id: Thing,
    pub artist: Artist,
    pub albums: Box<[Album]>,
    pub songs: Box<[Song]>,
}

#[derive(Debug, Clone)]
pub struct CollectionViewProps {
    pub id: Thing,
    pub collection: Collection,
    pub songs: Box<[Song]>,
}

#[derive(Debug, Clone)]
pub struct PlaylistViewProps {
    pub id: Thing,
    pub playlist: Playlist,
    pub songs: Box<[Song]>,
}

#[derive(Debug, Clone)]
pub struct SongViewProps {
    pub id: Thing,
    pub song: Song,
    pub artists: OneOrMany<Artist>,
    pub album: Album,
}

pub mod utils {
    use mecomp_storage::db::schemas::{
        album::Album, artist::Artist, collection::Collection, playlist::Playlist, song::Song,
    };
    use ratatui::{
        style::{Style, Stylize},
        text::{Line, Span},
    };
    use tui_tree_widget::TreeItem;

    pub fn create_album_tree_item(albums: &[Album]) -> Result<TreeItem<String>, std::io::Error> {
        TreeItem::new(
            "Albums".to_string(),
            format!("Albums ({}):", albums.len()),
            albums
                .iter()
                .map(|album| create_album_tree_leaf(album, None))
                .collect(),
        )
    }

    pub fn create_album_tree_leaf<'a>(
        album: &Album,
        prefix: Option<Span<'a>>,
    ) -> TreeItem<'a, String> {
        TreeItem::new_leaf(
            album.id.to_string(),
            Line::from(vec![
                prefix.unwrap_or_default(),
                Span::styled(album.title.to_string(), Style::default().bold()),
                Span::raw(" "),
                Span::styled(
                    album
                        .artist
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<String>>()
                        .join(", "),
                    Style::default().italic(),
                ),
            ]),
        )
    }

    pub fn create_artist_tree_item(artists: &[Artist]) -> Result<TreeItem<String>, std::io::Error> {
        TreeItem::new(
            "Artists".to_string(),
            format!("Artists ({}):", artists.len()),
            artists
                .iter()
                .map(|artist| create_artist_tree_leaf(artist))
                .collect(),
        )
    }

    pub fn create_artist_tree_leaf(artist: &Artist) -> TreeItem<String> {
        TreeItem::new_leaf(
            artist.id.to_string(),
            Line::from(vec![Span::styled(
                artist.name.to_string(),
                Style::default().bold(),
            )]),
        )
    }

    pub fn create_collection_tree_leaf(collection: &Collection) -> TreeItem<String> {
        TreeItem::new_leaf(
            collection.id.to_string(),
            Line::from(vec![Span::styled(
                collection.name.to_string(),
                Style::default().bold(),
            )]),
        )
    }

    pub fn create_playlist_tree_leaf(playlist: &Playlist) -> TreeItem<String> {
        TreeItem::new_leaf(
            playlist.id.to_string(),
            Line::from(vec![Span::styled(
                playlist.name.to_string(),
                Style::default().bold(),
            )]),
        )
    }

    pub fn create_song_tree_item(songs: &[Song]) -> Result<TreeItem<String>, std::io::Error> {
        TreeItem::new(
            "Songs".to_string(),
            format!("Songs ({}):", songs.len()),
            songs
                .iter()
                .map(|song| create_song_tree_leaf(song))
                .collect(),
        )
    }

    pub fn create_song_tree_leaf(song: &Song) -> TreeItem<String> {
        TreeItem::new_leaf(
            song.id.to_string(),
            Line::from(vec![
                Span::styled(song.title.to_string(), Style::default().bold()),
                Span::raw(" "),
                Span::styled(
                    song.artist
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<String>>()
                        .join(", "),
                    Style::default().italic(),
                ),
            ]),
        )
    }
}
