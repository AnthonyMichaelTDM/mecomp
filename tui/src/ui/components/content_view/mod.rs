//! The content view displays the contents of the current view (e.g. the songs in a playlist, the search results, etc.).

mod views;

use views::{none::NoneView, search::SearchView};

use crate::ui::AppState;

use super::{Component, ComponentRender, RenderProps};

pub struct ContentView {
    pub(crate) props: Props,
    //
    pub(crate) none_view: NoneView,
    pub(crate) search_view: SearchView,
    // pub(crate) songs_view: SongsView,
    // pub(crate) song_view: SongView,
    // pub(crate) albums_view: AlbumsView,
    // pub(crate) album_view: AlbumView,
    // pub(crate) artists_view: ArtistsView,
    // pub(crate) artist_view: ArtistView,
    // pub(crate) playlists_view: PlaylistsView,
    // pub(crate) playlist_view: PlaylistView,
    // pub(crate) collections_view: CollectionsView,
    // pub(crate) collection_view: CollectionView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Props {
    active_view: ActiveView,
}

impl From<&AppState> for Props {
    fn from(value: &AppState) -> Self {
        Self {
            active_view: value.view,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveView {
    /// Blank view.
    #[default]
    None,
    /// A view with a search bar and search results.
    Search,
    /// A view with all the songs in the users library.
    Songs,
    /// A view of a specific song.
    #[allow(dead_code)]
    Song,
    /// A view with all the albums in the users library.
    Albums,
    /// A view of a specific album.
    #[allow(dead_code)]
    Album,
    /// A view with all the artists in the users library.
    Artists,
    /// A view of a specific artist.
    #[allow(dead_code)]
    Artist,
    /// A view with all the playlists in the users library.
    Playlists,
    /// A view of a specific playlist.
    #[allow(dead_code)]
    Playlist,
    /// A view with all the collections in the users library.
    Collections,
    /// A view of a specific collection.
    #[allow(dead_code)]
    Collection,
    // TODO: views for genres, settings, etc.
}

impl ContentView {
    fn get_active_view_component(&self) -> &dyn Component {
        match self.props.active_view {
            ActiveView::None => &self.none_view,
            ActiveView::Search => &self.search_view,
            ActiveView::Songs => todo!(),
            ActiveView::Song => todo!(),
            ActiveView::Albums => todo!(),
            ActiveView::Album => todo!(),
            ActiveView::Artists => todo!(),
            ActiveView::Artist => todo!(),
            ActiveView::Playlists => todo!(),
            ActiveView::Playlist => todo!(),
            ActiveView::Collections => todo!(),
            ActiveView::Collection => todo!(),
        }
    }

    fn get_active_view_component_mut(&mut self) -> &mut dyn Component {
        match self.props.active_view {
            ActiveView::None => &mut self.none_view,
            ActiveView::Search => &mut self.search_view,
            ActiveView::Songs => todo!(),
            ActiveView::Song => todo!(),
            ActiveView::Albums => todo!(),
            ActiveView::Album => todo!(),
            ActiveView::Artists => todo!(),
            ActiveView::Artist => todo!(),
            ActiveView::Playlists => todo!(),
            ActiveView::Playlist => todo!(),
            ActiveView::Collections => todo!(),
            ActiveView::Collection => todo!(),
        }
    }
}

impl Component for ContentView {
    fn new(
        state: &AppState,
        action_tx: tokio::sync::mpsc::UnboundedSender<crate::state::action::Action>,
    ) -> Self
    where
        Self: Sized,
    {
        Self {
            props: Props::from(state),
            none_view: NoneView::new(state, action_tx.clone()),
            search_view: SearchView::new(state, action_tx),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        Self {
            props: Props::from(state),
            none_view: self.none_view.move_with_state(state),
            search_view: self.search_view.move_with_state(state),
        }
    }

    fn name(&self) -> &str {
        self.get_active_view_component().name()
    }

    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) {
        self.get_active_view_component_mut().handle_key_event(key);
    }
}

impl ComponentRender<RenderProps> for ContentView {
    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        match self.props.active_view {
            ActiveView::None => self.none_view.render(frame, props),
            ActiveView::Search => self.search_view.render(frame, props),
            ActiveView::Songs => todo!(),
            ActiveView::Song => todo!(),
            ActiveView::Albums => todo!(),
            ActiveView::Album => todo!(),
            ActiveView::Artists => todo!(),
            ActiveView::Artist => todo!(),
            ActiveView::Playlists => todo!(),
            ActiveView::Playlist => todo!(),
            ActiveView::Collections => todo!(),
            ActiveView::Collection => todo!(),
        }
    }
}
