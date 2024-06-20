pub mod album;
pub mod artist;
pub mod collection;
pub mod none;
pub mod playlist;
pub mod search;
pub mod song;

pub mod utils {
    use mecomp_storage::db::schemas::{album::Album, artist::Artist, song::Song};
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
