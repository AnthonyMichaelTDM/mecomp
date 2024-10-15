use mecomp_core::format_duration;
use mecomp_storage::db::schemas::{
    album::Album, artist::Artist, collection::Collection, playlist::Playlist, song::Song, Thing,
};
use one_or_many::OneOrMany;
use ratatui::{
    layout::Alignment,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};
use traits::ItemViewProps;

use crate::ui::widgets::tree::item::CheckTreeItem;

pub mod album;
pub mod artist;
pub mod collection;
pub mod generic;
pub mod none;
pub mod playlist;
pub mod radio;
pub mod search;
pub mod song;
pub mod sort_mode;
pub mod traits;

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

impl ItemViewProps for AlbumViewProps {
    fn id(&self) -> &Thing {
        &self.id
    }

    fn retrieve(view_data: &ViewData) -> Option<Self> {
        view_data.album.clone()
    }

    fn title() -> &'static str {
        "Album View"
    }

    fn name() -> &'static str
    where
        Self: Sized,
    {
        "album"
    }

    fn none_checked_string() -> &'static str
    where
        Self: Sized,
    {
        "entire album"
    }

    fn info_widget(&self) -> impl Widget {
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(self.album.title.to_string(), Style::default().bold()),
                Span::raw(" "),
                Span::styled(
                    self.album
                        .artist
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<String>>()
                        .join(", "),
                    Style::default().italic(),
                ),
            ]),
            Line::from(vec![
                Span::raw("Release Year: "),
                Span::styled(
                    self.album
                        .release
                        .map_or_else(|| "unknown".to_string(), |y| y.to_string()),
                    Style::default().italic(),
                ),
                Span::raw("  Songs: "),
                Span::styled(self.album.song_count.to_string(), Style::default().italic()),
                Span::raw("  Duration: "),
                Span::styled(
                    format_duration(&self.album.runtime),
                    Style::default().italic(),
                ),
            ]),
        ])
        .alignment(Alignment::Center)
    }

    fn tree_items(&self) -> Result<Vec<CheckTreeItem<String>>, std::io::Error> {
        let artist_tree = checktree_utils::create_artist_tree_item(self.artists.as_slice())?;
        let song_tree = checktree_utils::create_song_tree_item(self.songs.as_ref())?;
        Ok(vec![artist_tree, song_tree])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtistViewProps {
    pub id: Thing,
    pub artist: Artist,
    pub albums: Box<[Album]>,
    pub songs: Box<[Song]>,
}

impl ItemViewProps for ArtistViewProps {
    fn id(&self) -> &Thing {
        &self.id
    }

    fn retrieve(view_data: &ViewData) -> Option<Self> {
        view_data.artist.clone()
    }

    fn title() -> &'static str {
        "Artist View"
    }

    fn name() -> &'static str
    where
        Self: Sized,
    {
        "artist"
    }

    fn none_checked_string() -> &'static str
    where
        Self: Sized,
    {
        "entire artist"
    }

    fn info_widget(&self) -> impl Widget {
        Paragraph::new(vec![
            Line::from(Span::styled(
                self.artist.name.to_string(),
                Style::default().bold(),
            )),
            Line::from(vec![
                Span::raw("Albums: "),
                Span::styled(
                    self.artist.album_count.to_string(),
                    Style::default().italic(),
                ),
                Span::raw("  Songs: "),
                Span::styled(
                    self.artist.song_count.to_string(),
                    Style::default().italic(),
                ),
                Span::raw("  Duration: "),
                Span::styled(
                    format_duration(&self.artist.runtime),
                    Style::default().italic(),
                ),
            ]),
        ])
        .alignment(Alignment::Center)
    }

    fn tree_items(&self) -> Result<Vec<CheckTreeItem<String>>, std::io::Error> {
        let album_tree = checktree_utils::create_album_tree_item(self.albums.as_ref())?;
        let song_tree = checktree_utils::create_song_tree_item(self.songs.as_ref())?;
        Ok(vec![album_tree, song_tree])
    }
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

impl ItemViewProps for SongViewProps {
    fn id(&self) -> &Thing {
        &self.id
    }

    fn retrieve(view_data: &ViewData) -> Option<Self> {
        view_data.song.clone()
    }

    fn title() -> &'static str {
        "Song View"
    }

    fn name() -> &'static str
    where
        Self: Sized,
    {
        "song"
    }

    fn none_checked_string() -> &'static str
    where
        Self: Sized,
    {
        "the song"
    }

    fn info_widget(&self) -> impl Widget {
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(self.song.title.to_string(), Style::default().bold()),
                Span::raw(" "),
                Span::styled(
                    self.song
                        .artist
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<String>>()
                        .join(", "),
                    Style::default().italic(),
                ),
            ]),
            Line::from(vec![
                Span::raw("Track/Disc: "),
                Span::styled(
                    format!(
                        "{}/{}",
                        self.song.track.unwrap_or_default(),
                        self.song.disc.unwrap_or_default()
                    ),
                    Style::default().italic(),
                ),
                Span::raw("  Duration: "),
                Span::styled(
                    format!(
                        "{}:{:04.1}",
                        self.song.runtime.as_secs() / 60,
                        self.song.runtime.as_secs_f32() % 60.0,
                    ),
                    Style::default().italic(),
                ),
                Span::raw("  Genre(s): "),
                Span::styled(
                    self.song
                        .genre
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<String>>()
                        .join(", "),
                    Style::default().italic(),
                ),
            ]),
        ])
        .alignment(Alignment::Center)
    }

    fn tree_items(&self) -> Result<Vec<CheckTreeItem<String>>, std::io::Error> {
        let artist_tree = checktree_utils::create_artist_tree_item(self.artists.as_slice())?;
        let album_tree =
            checktree_utils::create_album_tree_leaf(&self.album, Some(Span::raw("Album: ")));
        let playlist_tree = checktree_utils::create_playlist_tree_item(&self.playlists)?;
        let collection_tree = checktree_utils::create_collection_tree_item(&self.collections)?;
        Ok(vec![
            artist_tree,
            album_tree,
            playlist_tree,
            collection_tree,
        ])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioViewProps {
    /// The number of similar songs to get
    pub count: u32,
    /// The songs that are similar to the things
    pub songs: Box<[Song]>,
}

pub mod checktree_utils {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    use mecomp_storage::db::schemas::{
        album::Album, artist::Artist, collection::Collection, playlist::Playlist, song::Song, Thing,
    };
    use ratatui::{
        layout::Position,
        style::{Style, Stylize},
        text::{Line, Span, Text},
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

    impl CheckTreeState<String> {
        /// Get the checked things from the tree state
        #[must_use]
        pub fn get_checked_things(&self) -> Vec<Thing> {
            self.checked()
                .iter()
                .filter_map(|id| id.iter().find_map(|id| id.parse::<Thing>().ok()))
                .collect()
        }

        /// Get the selected thing from the tree state
        #[must_use]
        pub fn get_selected_thing(&self) -> Option<Thing> {
            self.selected()
                .iter()
                .find_map(|id| id.parse::<Thing>().ok())
        }

        /// Handle mouse events interacting with the tree
        ///
        /// Assumes that the given area only includes the `CheckTree`
        ///
        /// # Returns
        ///
        /// an action if the mouse event requires it
        pub fn handle_mouse_event(
            &mut self,
            event: MouseEvent,
            area: ratatui::layout::Rect,
        ) -> Option<Action> {
            let MouseEvent {
                kind, column, row, ..
            } = event;
            let mouse_position = Position::new(column, row);

            if !area.contains(mouse_position) {
                return None;
            }

            match kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    let selected_things = self.get_selected_thing();
                    self.mouse_click(mouse_position);

                    // if the selection didn't change, open the selected view
                    (selected_things == self.get_selected_thing())
                        .then_some(selected_things)
                        .flatten()
                        .map(|thing| Action::SetCurrentView(thing.into()))
                }
                MouseEventKind::ScrollDown => {
                    self.key_down();
                    None
                }
                MouseEventKind::ScrollUp => {
                    self.key_up();
                    None
                }
                _ => None,
            }
        }
    }

    impl CheckTreeItem<'_, String> {
        /// Create a `CheckTreeState` from a list of things
        ///
        /// # Errors
        ///
        /// returns an error if the tree state cannot be created (e.g. duplicate ids)
        #[allow(clippy::needless_pass_by_value)]
        pub fn new_with_items<'a, 'items, 'text, Item, LeafFn>(
            items: &'items [Item],
            identifier: impl ToString,
            text: impl Into<Text<'text>>,
            leaf_fn: LeafFn,
        ) -> Result<CheckTreeItem<String>, std::io::Error>
        where
            'a: 'text,
            'a: 'items,
            'text: 'items,
            LeafFn: FnMut(&Item) -> CheckTreeItem<'a, String>,
        {
            let identifier = identifier.to_string();
            let mut tree =
                CheckTreeItem::new(identifier, text, items.iter().map(leaf_fn).collect())?;
            if tree.children().is_empty() {
                tree.add_child(create_dummy_leaf())?;
            }
            Ok(tree)
        }
    }

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

    fn create_dummy_leaf() -> CheckTreeItem<'static, String> {
        CheckTreeItem::new_leaf("dummy".to_string(), "")
    }

    /// # Errors
    ///
    /// Returns an error if the tree item cannot be created (e.g. duplicate ids)
    pub fn create_album_tree_item(
        albums: &[Album],
    ) -> Result<CheckTreeItem<String>, std::io::Error> {
        CheckTreeItem::<String>::new_with_items(
            albums,
            "Albums",
            format!("Albums ({}):", albums.len()),
            |album| create_album_tree_leaf(album, None),
        )
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
        CheckTreeItem::<String>::new_with_items(
            artists,
            "Artists",
            format!("Artists ({}):", artists.len()),
            create_artist_tree_leaf,
        )
    }

    #[must_use]
    pub fn create_artist_tree_leaf<'a>(artist: &Artist) -> CheckTreeItem<'a, String> {
        CheckTreeItem::new_leaf(
            artist.id.to_string(),
            Line::from(vec![Span::styled(
                artist.name.to_string(),
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
        CheckTreeItem::<String>::new_with_items(
            collections,
            "Collections",
            format!("Collections ({}):", collections.len()),
            create_collection_tree_leaf,
        )
    }

    #[must_use]
    pub fn create_collection_tree_leaf<'a>(collection: &Collection) -> CheckTreeItem<'a, String> {
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
    pub fn create_playlist_tree_item(
        playlists: &[Playlist],
    ) -> Result<CheckTreeItem<String>, std::io::Error> {
        CheckTreeItem::<String>::new_with_items(
            playlists,
            "Playlists",
            format!("Playlists ({}):", playlists.len()),
            create_playlist_tree_leaf,
        )
    }

    #[must_use]
    pub fn create_playlist_tree_leaf<'a>(playlist: &Playlist) -> CheckTreeItem<'a, String> {
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
    pub fn create_song_tree_item(songs: &[Song]) -> Result<CheckTreeItem<String>, std::io::Error> {
        CheckTreeItem::<String>::new_with_items(
            songs,
            "Songs",
            format!("Songs ({}):", songs.len()),
            create_song_tree_leaf,
        )
    }

    pub fn create_song_tree_leaf<'a>(song: &Song) -> CheckTreeItem<'a, String> {
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
