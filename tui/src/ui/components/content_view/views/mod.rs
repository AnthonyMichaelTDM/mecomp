pub mod dynamic;
use mecomp_core::format_duration;
use mecomp_prost::{
    Album, AlbumBrief, Artist, ArtistBrief, Collection, CollectionBrief, DynamicPlaylist, Playlist,
    PlaylistBrief, Song, SongBrief,
};
use mecomp_prost::{RecordId, convert_duration};
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
pub mod random;
pub mod search;
pub mod song;
pub mod sort_mode;
pub mod traits;

/// Data needed by the views (that isn't directly handled by a state store)
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ViewData {
    pub album: Option<AlbumViewProps>,
    pub artist: Option<ArtistViewProps>,
    pub collection: Option<CollectionViewProps>,
    pub dynamic_playlist: Option<DynamicPlaylistViewProps>,
    pub playlist: Option<PlaylistViewProps>,
    pub song: Option<SongViewProps>,
    pub radio: Option<RadioViewProps>,
    pub random: Option<RandomViewProps>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlbumViewProps {
    pub id: RecordId,
    pub album: Album,
    pub artists: OneOrMany<ArtistBrief>,
    pub songs: Box<[SongBrief]>,
}

impl ItemViewProps for AlbumViewProps {
    fn id(&self) -> &RecordId {
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
                Span::styled(&self.album.title, Style::default().bold()),
                Span::raw(" "),
                Span::styled(
                    self.album.artists.as_slice().join(", "),
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
                    format_duration(&convert_duration(self.album.runtime)),
                    Style::default().italic(),
                ),
            ]),
        ])
        .alignment(Alignment::Center)
    }

    fn tree_items(&self) -> Result<Vec<CheckTreeItem<'_, String>>, std::io::Error> {
        let artist_tree = checktree_utils::create_artist_tree_item(self.artists.as_slice())?;
        let song_tree = checktree_utils::create_song_tree_item(self.songs.as_ref())?;
        Ok(vec![artist_tree, song_tree])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtistViewProps {
    pub id: RecordId,
    pub artist: Artist,
    pub albums: Box<[AlbumBrief]>,
    pub songs: Box<[SongBrief]>,
}

impl ItemViewProps for ArtistViewProps {
    fn id(&self) -> &RecordId {
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
            Line::from(Span::styled(&self.artist.name, Style::default().bold())),
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
                    format_duration(&convert_duration(self.artist.runtime)),
                    Style::default().italic(),
                ),
            ]),
        ])
        .alignment(Alignment::Center)
    }

    fn tree_items(&self) -> Result<Vec<CheckTreeItem<'_, String>>, std::io::Error> {
        let album_tree = checktree_utils::create_album_tree_item(self.albums.as_ref())?;
        let song_tree = checktree_utils::create_song_tree_item(self.songs.as_ref())?;
        Ok(vec![album_tree, song_tree])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionViewProps {
    pub id: RecordId,
    pub collection: Collection,
    pub songs: Box<[SongBrief]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicPlaylistViewProps {
    pub id: RecordId,
    pub dynamic_playlist: DynamicPlaylist,
    pub songs: Vec<SongBrief>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistViewProps {
    pub id: RecordId,
    pub playlist: Playlist,
    pub songs: Vec<SongBrief>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongViewProps {
    pub id: RecordId,
    pub song: Song,
    pub artists: Vec<ArtistBrief>,
    pub album: AlbumBrief,
    pub playlists: Vec<PlaylistBrief>,
    pub collections: Vec<CollectionBrief>,
}

impl ItemViewProps for SongViewProps {
    fn id(&self) -> &RecordId {
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
        let runtime = convert_duration(self.song.runtime);

        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(&self.song.title, Style::default().bold()),
                Span::raw(" "),
                Span::styled(
                    self.song.artists.as_slice().join(", "),
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
                        runtime.as_secs() / 60,
                        runtime.as_secs_f32() % 60.0,
                    ),
                    Style::default().italic(),
                ),
                Span::raw("  Genre(s): "),
                Span::styled(
                    self.song.genres.as_slice().join(", "),
                    Style::default().italic(),
                ),
            ]),
        ])
        .alignment(Alignment::Center)
    }

    fn tree_items(&self) -> Result<Vec<CheckTreeItem<'_, String>>, std::io::Error> {
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
    pub songs: Vec<SongBrief>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomViewProps {
    /// id of a random album
    pub album: RecordId,
    /// id of a random artist
    pub artist: RecordId,
    /// id of a random song
    pub song: RecordId,
}

pub mod checktree_utils {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use mecomp_prost::{
        AlbumBrief, ArtistBrief, CollectionBrief, DynamicPlaylist, PlaylistBrief, RecordId,
        SongBrief,
    };
    use ratatui::{
        layout::Position,
        style::{Style, Stylize},
        text::{Line, Span, Text},
    };

    use crate::{
        state::action::{Action, AudioAction, PopupAction, QueueAction, ViewAction},
        ui::{
            components::content_view::ActiveView,
            widgets::{
                popups::PopupType,
                tree::{item::CheckTreeItem, state::CheckTreeState},
            },
        },
    };

    impl CheckTreeState<String> {
        /// Get the checked things from the tree state
        #[must_use]
        pub fn get_checked_things(&self) -> Vec<RecordId> {
            self.checked()
                .iter()
                .filter_map(|id| id.iter().find_map(|id| id.parse::<RecordId>().ok()))
                .collect()
        }

        /// Get the selected thing from the tree state
        #[must_use]
        pub fn get_selected_thing(&self) -> Option<RecordId> {
            self.selected()
                .iter()
                .find_map(|id| id.parse::<RecordId>().ok())
        }

        /// Handle mouse events interacting with the tree
        ///
        /// Assumes that the given area only includes the `CheckTree`
        ///
        /// # Arguments
        ///
        /// `event` - the mouse event to handle
        /// `area` - the area of the tree in the terminal
        /// `swap_ctrl_click_behavior` - whether to swap the behavior of ctrl+click and click:
        ///  - if `true`, ctrl+click will toggle the check state of the item, and click will open the item
        ///  - if `false` (default), ctrl+click will open the item, and click will toggle the check state of the item
        ///
        /// # Returns
        ///
        /// an action if the mouse event requires it
        pub fn handle_mouse_event(
            &mut self,
            event: MouseEvent,
            area: ratatui::layout::Rect,
            swap_ctrl_click_behavior: bool,
        ) -> Option<Action> {
            let MouseEvent {
                kind,
                column,
                row,
                modifiers,
            } = event;
            let mouse_position = Position::new(column, row);

            if !area.contains(mouse_position) {
                return None;
            }

            match kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // do a click (toggle check state or open item)
                    let click_result = self.mouse_click(mouse_position);

                    // if it was a control-click,
                    let condition = modifiers.contains(KeyModifiers::CONTROL) && click_result;
                    // and we aren't swapping the behavior,
                    let condition = if swap_ctrl_click_behavior {
                        !condition
                    } else {
                        condition
                    };

                    // then attempt to open the selected thing
                    if condition {
                        self.get_selected_thing()
                            .map(|thing| Action::ActiveView(ViewAction::Set(thing.into())))
                    } else {
                        None
                    }
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

    impl<'items> CheckTreeItem<'items, String> {
        /// Create a `CheckTreeState` from a list of things
        ///
        /// # Errors
        ///
        /// returns an error if the tree state cannot be created (e.g. duplicate ids)
        pub fn new_with_items<'a, 'text, Item, LeafFn>(
            items: &'items [Item],
            identifier: impl AsRef<str>,
            text: impl Into<Text<'text>>,
            leaf_fn: LeafFn,
        ) -> Result<Self, std::io::Error>
        where
            'a: 'text,
            'a: 'items,
            'text: 'items,
            LeafFn: FnMut(&'items Item) -> CheckTreeItem<'a, String>,
        {
            let identifier = identifier.as_ref().to_string();
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
        checked_things: Vec<RecordId>,
        current_thing: Option<&RecordId>,
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
        checked_things: Vec<RecordId>,
        current_thing: Option<&RecordId>,
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
        checked_things: Vec<RecordId>,
        current_thing: Option<&RecordId>,
    ) -> Option<Action> {
        if checked_things.is_empty() {
            current_thing
                .map(|id| Action::ActiveView(ViewAction::Set(ActiveView::Radio(vec![id.clone()]))))
        } else {
            Some(Action::ActiveView(ViewAction::Set(ActiveView::Radio(
                checked_things,
            ))))
        }
    }

    fn create_dummy_leaf() -> CheckTreeItem<'static, String> {
        CheckTreeItem::new_leaf("dummy".to_string(), "")
    }

    /// # Errors
    ///
    /// Returns an error if the tree item cannot be created (e.g. duplicate ids)
    pub fn create_album_tree_item(
        albums: &[AlbumBrief],
    ) -> Result<CheckTreeItem<'_, String>, std::io::Error> {
        CheckTreeItem::<String>::new_with_items(
            albums,
            "Albums",
            format!("Albums ({}):", albums.len()),
            |album| create_album_tree_leaf(album, None),
        )
    }

    #[must_use]
    pub fn create_album_tree_leaf<'a>(
        album: &'a AlbumBrief,
        prefix: Option<Span<'a>>,
    ) -> CheckTreeItem<'a, String> {
        CheckTreeItem::new_leaf(
            album.id.to_string(),
            Line::from(vec![
                prefix.unwrap_or_default(),
                Span::styled(&album.title, Style::default().bold()),
                Span::raw(" "),
                Span::styled(
                    album.artists.as_slice().join(", "),
                    Style::default().italic(),
                ),
            ]),
        )
    }

    /// # Errors
    ///
    /// Returns an error if the tree item cannot be created (e.g. duplicate ids)
    pub fn create_artist_tree_item(
        artists: &[ArtistBrief],
    ) -> Result<CheckTreeItem<'_, String>, std::io::Error> {
        CheckTreeItem::<String>::new_with_items(
            artists,
            "Artists",
            format!("Artists ({}):", artists.len()),
            create_artist_tree_leaf,
        )
    }

    #[must_use]
    pub fn create_artist_tree_leaf(artist: &ArtistBrief) -> CheckTreeItem<'_, String> {
        CheckTreeItem::new_leaf(
            artist.id.to_string(),
            Line::from(vec![Span::styled(&artist.name, Style::default().bold())]),
        )
    }

    /// # Errors
    ///
    /// Returns an error if the tree item cannot be created (e.g. duplicate ids)
    pub fn create_collection_tree_item(
        collections: &[CollectionBrief],
    ) -> Result<CheckTreeItem<'_, String>, std::io::Error> {
        CheckTreeItem::<String>::new_with_items(
            collections,
            "Collections",
            format!("Collections ({}):", collections.len()),
            create_collection_tree_leaf,
        )
    }

    #[must_use]
    pub fn create_collection_tree_leaf(collection: &CollectionBrief) -> CheckTreeItem<'_, String> {
        CheckTreeItem::new_leaf(
            collection.id.to_string(),
            Line::from(vec![Span::styled(
                &collection.name,
                Style::default().bold(),
            )]),
        )
    }

    /// # Errors
    ///
    /// Returns an error if the tree item cannot be created (e.g. duplicate ids)
    pub fn create_playlist_tree_item(
        playlists: &[PlaylistBrief],
    ) -> Result<CheckTreeItem<'_, String>, std::io::Error> {
        CheckTreeItem::<String>::new_with_items(
            playlists,
            "Playlists",
            format!("Playlists ({}):", playlists.len()),
            create_playlist_tree_leaf,
        )
    }

    #[must_use]
    pub fn create_playlist_tree_leaf(playlist: &PlaylistBrief) -> CheckTreeItem<'_, String> {
        CheckTreeItem::new_leaf(
            playlist.id.to_string(),
            Line::from(vec![Span::styled(&playlist.name, Style::default().bold())]),
        )
    }

    /// # Errors
    ///
    /// Returns an error if the tree item cannot be created (e.g. duplicate ids)
    pub fn create_dynamic_playlist_tree_item(
        dynamic_playlists: &[DynamicPlaylist],
    ) -> Result<CheckTreeItem<'_, String>, std::io::Error> {
        CheckTreeItem::<String>::new_with_items(
            dynamic_playlists,
            "Dynamic Playlists",
            format!("Dynamic Playlists ({}):", dynamic_playlists.len()),
            create_dynamic_playlist_tree_leaf,
        )
    }

    #[must_use]
    pub fn create_dynamic_playlist_tree_leaf(
        dynamic_playlist: &DynamicPlaylist,
    ) -> CheckTreeItem<'_, String> {
        CheckTreeItem::new_leaf(
            dynamic_playlist.id.to_string(),
            Line::from(vec![Span::styled(
                &dynamic_playlist.name,
                Style::default().bold(),
            )]),
        )
    }

    /// # Errors
    ///
    /// Returns an error if the tree item cannot be created (e.g. duplicate ids)
    pub fn create_song_tree_item(
        songs: &[SongBrief],
    ) -> Result<CheckTreeItem<'_, String>, std::io::Error> {
        CheckTreeItem::<String>::new_with_items(
            songs,
            "Songs",
            format!("Songs ({}):", songs.len()),
            create_song_tree_leaf,
        )
    }

    #[must_use]
    pub fn create_song_tree_leaf(song: &SongBrief) -> CheckTreeItem<'_, String> {
        CheckTreeItem::new_leaf(
            song.id.to_string(),
            Line::from(vec![
                Span::styled(&song.title, Style::default().bold()),
                Span::raw(" "),
                Span::styled(
                    song.artists.as_slice().join(", "),
                    Style::default().italic(),
                ),
            ]),
        )
    }
}
