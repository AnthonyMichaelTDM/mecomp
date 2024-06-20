//! The content view displays the contents of the current view (e.g. the songs in a playlist, the search results, etc.).

pub mod views;

use mecomp_storage::db::schemas::{album, artist, collection, playlist, song, Id, Thing};
use views::{
    none::NoneView,
    search::SearchView,
    song::{LibrarySongsView, SongView},
};

use crate::ui::AppState;

use super::{Component, ComponentRender, RenderProps};

pub struct ContentView {
    pub(crate) props: Props,
    //
    pub(crate) none_view: NoneView,
    pub(crate) search_view: SearchView,
    pub(crate) songs_view: LibrarySongsView,
    pub(crate) song_view: SongView,
    // pub(crate) albums_view: LibraryAlbumsView,
    // pub(crate) album_view: AlbumView,
    // pub(crate) artists_view: LibraryArtistsView,
    // pub(crate) artist_view: ArtistView,
    // pub(crate) playlists_view: LibraryPlaylistsView,
    // pub(crate) playlist_view: PlaylistView,
    // pub(crate) collections_view: LibraryCollectionsView,
    // pub(crate) collection_view: CollectionView,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Props {
    active_view: ActiveView,
}

impl From<&AppState> for Props {
    fn from(value: &AppState) -> Self {
        Self {
            active_view: value.active_view.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ActiveView {
    /// Blank view.
    #[default]
    None,
    /// A view with a search bar and search results.
    Search,
    /// A view with all the songs in the users library.
    Songs,
    /// A view of a specific song.
    Song(Id),
    /// A view with all the albums in the users library.
    Albums,
    /// A view of a specific album.
    Album(Id),
    /// A view with all the artists in the users library.
    Artists,
    /// A view of a specific artist.
    Artist(Id),
    /// A view with all the playlists in the users library.
    Playlists,
    /// A view of a specific playlist.
    Playlist(Id),
    /// A view with all the collections in the users library.
    Collections,
    /// A view of a specific collection.
    Collection(Id),
    // TODO: views for genres, settings, radios, etc.
}

impl From<Thing> for ActiveView {
    fn from(value: Thing) -> Self {
        match value.tb.as_str() {
            album::TABLE_NAME => Self::Album(value.id),
            artist::TABLE_NAME => Self::Artist(value.id),
            collection::TABLE_NAME => Self::Collection(value.id),
            playlist::TABLE_NAME => Self::Playlist(value.id),
            song::TABLE_NAME => Self::Song(value.id),
            _ => Self::None,
        }
    }
}

impl ContentView {
    fn get_active_view_component(&self) -> &dyn Component {
        match &self.props.active_view {
            ActiveView::None => &self.none_view,
            ActiveView::Search => &self.search_view,
            ActiveView::Songs => &self.songs_view,
            ActiveView::Song(_id) => &self.song_view,
            ActiveView::Albums => todo!(),
            ActiveView::Album(_id) => todo!(),
            ActiveView::Artists => todo!(),
            ActiveView::Artist(_id) => todo!(),
            ActiveView::Playlists => todo!(),
            ActiveView::Playlist(_id) => todo!(),
            ActiveView::Collections => todo!(),
            ActiveView::Collection(_id) => todo!(),
        }
    }

    fn get_active_view_component_mut(&mut self) -> &mut dyn Component {
        match &self.props.active_view {
            ActiveView::None => &mut self.none_view,
            ActiveView::Search => &mut self.search_view,
            ActiveView::Songs => &mut self.songs_view,
            ActiveView::Song(_id) => &mut self.song_view,
            ActiveView::Albums => todo!(),
            ActiveView::Album(_id) => todo!(),
            ActiveView::Artists => todo!(),
            ActiveView::Artist(_id) => todo!(),
            ActiveView::Playlists => todo!(),
            ActiveView::Playlist(_id) => todo!(),
            ActiveView::Collections => todo!(),
            ActiveView::Collection(_id) => todo!(),
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
            search_view: SearchView::new(state, action_tx.clone()),
            songs_view: LibrarySongsView::new(state, action_tx.clone()),
            song_view: SongView::new(state, action_tx),
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
            songs_view: self.songs_view.move_with_state(state),
            song_view: self.song_view.move_with_state(state),
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
        match &self.props.active_view {
            ActiveView::None => self.none_view.render(frame, props),
            ActiveView::Search => self.search_view.render(frame, props),
            ActiveView::Songs => self.songs_view.render(frame, props),
            ActiveView::Song(_) => self.song_view.render(frame, props),
            ActiveView::Albums => todo!(),
            ActiveView::Album(_) => todo!(),
            ActiveView::Artists => todo!(),
            ActiveView::Artist(_) => todo!(),
            ActiveView::Playlists => todo!(),
            ActiveView::Playlist(_) => todo!(),
            ActiveView::Collections => todo!(),
            ActiveView::Collection(_) => todo!(),
        }
    }
}
