pub mod radio;
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

const RADIO_SIZE: u32 = 20;

/// Data needed by the views (that isn't directly handled by a state store)
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ViewData {
    pub album: Option<AlbumViewProps>,
    pub artist: Option<ArtistViewProps>,
    pub collection: Option<CollectionViewProps>,
    pub playlist: Option<PlaylistViewProps>,
    pub song: Option<SongViewProps>,
    pub radio: Option<RadioViewProps>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlbumViewProps {
    pub id: Thing,
    pub album: Album,
    pub artists: OneOrMany<Artist>,
    pub songs: Box<[Song]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtistViewProps {
    pub id: Thing,
    pub artist: Artist,
    pub albums: Box<[Album]>,
    pub songs: Box<[Song]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionViewProps {
    pub id: Thing,
    pub collection: Collection,
    pub songs: Box<[Song]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistViewProps {
    pub id: Thing,
    pub playlist: Playlist,
    pub songs: Box<[Song]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongViewProps {
    pub id: Thing,
    pub song: Song,
    pub artists: OneOrMany<Artist>,
    pub album: Album,
    pub playlists: Box<[Playlist]>,
    pub collections: Box<[Collection]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioViewProps {
    /// The number of similar songs to get
    pub count: u32,
    /// The songs that are similar to the things
    pub songs: Box<[Song]>,
}

pub mod checktree_utils {
    use mecomp_storage::db::schemas::{
        album::Album, artist::Artist, collection::Collection, playlist::Playlist, song::Song, Thing,
    };
    use ratatui::{
        style::{Style, Stylize},
        text::{Line, Span},
    };

    use crate::{
        state::action::{Action, AudioAction, PopupAction, QueueAction},
        ui::{
            components::content_view::ActiveView,
            widgets::{
                popups::PopupType,
                tree::{item::CheckTreeItem, state::CheckTreeState},
            },
        },
    };

    use super::RADIO_SIZE;

    /// Construct an `Action` to add the checked things to a playlist, if there are any,
    /// otherwise add the thing being displayed by the view
    ///
    /// # Returns
    ///
    /// None - if there are no checked things and the current thing is None
    /// Some(Action) - if there are checked things or the current thing is Some
    #[must_use]
    pub fn construct_add_to_playlist_action(
        checked_things: Vec<Thing>,
        current_thing: Option<&Thing>,
    ) -> Option<Action> {
        if checked_things.is_empty() {
            current_thing
                .map(|id| Action::Popup(PopupAction::Open(PopupType::Playlist(vec![id.clone()]))))
        } else {
            Some(Action::Popup(PopupAction::Open(PopupType::Playlist(
                checked_things,
            ))))
        }
    }

    /// Construct an `Action` to add the checked things to the queue if there are any,
    /// otherwise add the thing being displayed by the view
    ///
    /// # Returns
    ///
    /// None - if there are no checked things and the current thing is None
    /// Some(Action) - if there are checked things or the current thing is Some
    #[must_use]
    pub fn construct_add_to_queue_action(
        checked_things: Vec<Thing>,
        current_thing: Option<&Thing>,
    ) -> Option<Action> {
        if checked_things.is_empty() {
            current_thing
                .map(|id| Action::Audio(AudioAction::Queue(QueueAction::Add(vec![id.clone()]))))
        } else {
            Some(Action::Audio(AudioAction::Queue(QueueAction::Add(
                checked_things,
            ))))
        }
    }

    /// Construct an `Action` to start a radio from the checked things if there are any,
    /// otherwise start a radio from the thing being displayed by the view
    ///
    /// # Returns
    ///
    /// None - if there are no checked things and the current thing is None
    /// Some(Action) - if there are checked things or the current thing is Some
    #[must_use]
    pub fn construct_start_radio_action(
        checked_things: Vec<Thing>,
        current_thing: Option<&Thing>,
    ) -> Option<Action> {
        if checked_things.is_empty() {
            current_thing
                .map(|id| Action::SetCurrentView(ActiveView::Radio(vec![id.clone()], RADIO_SIZE)))
        } else {
            Some(Action::SetCurrentView(ActiveView::Radio(
                checked_things,
                RADIO_SIZE,
            )))
        }
    }

    /// Get the checked things from the tree state
    #[must_use]
    pub fn get_checked_things_from_tree_state(tree_state: &CheckTreeState<String>) -> Vec<Thing> {
        tree_state
            .checked()
            .iter()
            .filter_map(|id| id.iter().find_map(|id| id.parse::<Thing>().ok()))
            .collect()
    }

    /// Get the selected thing from the tree state
    #[must_use]
    pub fn get_selected_things_from_tree_state(
        tree_state: &CheckTreeState<String>,
    ) -> Option<Thing> {
        tree_state
            .selected()
            .iter()
            .find_map(|id| id.parse::<Thing>().ok())
    }

    fn create_dummy_leaf() -> CheckTreeItem<'static, String> {
        CheckTreeItem::new_leaf("dummy".to_string(), "")
    }

    /// # Errors
    ///
    /// Returns an error if the tree item cannot be created (e.g. duplicate ids)
    pub fn create_album_tree_item(
        albums: &[Album],
    ) -> Result<CheckTreeItem<String>, std::io::Error> {
        let mut item = CheckTreeItem::new(
            "Albums".to_string(),
            format!("Albums ({}):", albums.len()),
            albums
                .iter()
                .map(|album| create_album_tree_leaf(album, None))
                .collect(),
        )?;
        if item.children().is_empty() {
            item.add_child(create_dummy_leaf())?;
        }
        Ok(item)
    }

    pub fn create_album_tree_leaf<'a>(
        album: &Album,
        prefix: Option<Span<'a>>,
    ) -> CheckTreeItem<'a, String> {
        CheckTreeItem::new_leaf(
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

    /// # Errors
    ///
    /// Returns an error if the tree item cannot be created (e.g. duplicate ids)
    pub fn create_artist_tree_item(
        artists: &[Artist],
    ) -> Result<CheckTreeItem<String>, std::io::Error> {
        let mut item = CheckTreeItem::new(
            "Artists".to_string(),
            format!("Artists ({}):", artists.len()),
            artists
                .iter()
                .map(|artist| create_artist_tree_leaf(artist))
                .collect(),
        )?;
        if item.children().is_empty() {
            item.add_child(create_dummy_leaf())?;
        }
        Ok(item)
    }

    #[must_use]
    pub fn create_artist_tree_leaf(artist: &Artist) -> CheckTreeItem<String> {
        CheckTreeItem::new_leaf(
            artist.id.to_string(),
            Line::from(vec![Span::styled(
                artist.name.to_string(),
                Style::default().bold(),
            )]),
        )
    }

    #[must_use]
    pub fn create_collection_tree_leaf(collection: &Collection) -> CheckTreeItem<String> {
        CheckTreeItem::new_leaf(
            collection.id.to_string(),
            Line::from(vec![Span::styled(
                collection.name.to_string(),
                Style::default().bold(),
            )]),
        )
    }

    /// # Errors
    ///
    /// Returns an error if the tree item cannot be created (e.g. duplicate ids)
    pub fn create_collection_tree_item(
        collections: &[Collection],
    ) -> Result<CheckTreeItem<String>, std::io::Error> {
        let mut item = CheckTreeItem::new(
            "Collections".to_string(),
            format!("Collections ({}):", collections.len()),
            collections
                .iter()
                .map(|collection| create_collection_tree_leaf(collection))
                .collect(),
        )?;
        if item.children().is_empty() {
            item.add_child(create_dummy_leaf())?;
        }
        Ok(item)
    }

    #[must_use]
    pub fn create_playlist_tree_leaf(playlist: &Playlist) -> CheckTreeItem<String> {
        CheckTreeItem::new_leaf(
            playlist.id.to_string(),
            Line::from(vec![Span::styled(
                playlist.name.to_string(),
                Style::default().bold(),
            )]),
        )
    }

    /// # Errors
    ///
    /// Returns an error if the tree item cannot be created (e.g. duplicate ids)
    pub fn create_playlist_tree_item(
        playlists: &[Playlist],
    ) -> Result<CheckTreeItem<String>, std::io::Error> {
        let mut item = CheckTreeItem::new(
            "Playlists".to_string(),
            format!("Playlists ({}):", playlists.len()),
            playlists
                .iter()
                .map(|playlist| create_playlist_tree_leaf(playlist))
                .collect(),
        )?;
        if item.children().is_empty() {
            item.add_child(create_dummy_leaf())?;
        }
        Ok(item)
    }

    /// # Errors
    ///
    /// Returns an error if the tree item cannot be created (e.g. duplicate ids)
    pub fn create_song_tree_item(songs: &[Song]) -> Result<CheckTreeItem<String>, std::io::Error> {
        let mut item = CheckTreeItem::new(
            "Songs".to_string(),
            format!("Songs ({}):", songs.len()),
            songs
                .iter()
                .map(|song| create_song_tree_leaf(song))
                .collect(),
        )?;
        if item.children().is_empty() {
            item.add_child(create_dummy_leaf())?;
        }
        Ok(item)
    }

    pub fn create_song_tree_leaf(song: &Song) -> CheckTreeItem<String> {
        CheckTreeItem::new_leaf(
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
