//! This module contains the actions that the user can perform through the UI.
//! these actions are sent from the UI to the state stores, which then update the state accordingly.
#![allow(clippy::module_name_repetitions)]

use std::time::Duration;

use mecomp_core::state::{RepeatMode, SeekType};
use mecomp_storage::db::schemas::Thing;

use crate::ui::{components::content_view::ActiveView, widgets::popups::PopupType};

pub enum Action {
    /// General actions
    General(GeneralAction),
    /// Actions that effect the audio state store.
    Audio(AudioAction),
    /// Actions that effect the search state store.
    Search(String),
    /// Actions that effect the library state store.
    Library(LibraryAction),
    /// Actions that effect the current view.
    SetCurrentView(ActiveView),
    /// Actions regarding popups
    Popup(PopupAction),
}

pub enum GeneralAction {
    /// Exit the application.
    Exit,
}

pub enum AudioAction {
    /// Playback Commands
    Playback(PlaybackAction),
    /// Queue Commands
    Queue(QueueAction),
}

pub enum PlaybackAction {
    /// Toggle play/pause
    Toggle,
    /// Skip to the next song.
    Next,
    /// Skip to the previous song.
    Previous,
    /// Seek to a specific position in the current song.
    Seek(SeekType, Duration),
    /// Change the volume.
    Volume(VolumeAction),
    /// Toggle the mute state.
    ToggleMute,
}

pub enum VolumeAction {
    /// Increase the volume by a given amount (0 is mute, 100 is max)
    Increase(f32),
    /// Decrease the volume by a given amount (0 is mute, 100 is max)
    Decrease(f32),
}

pub enum QueueAction {
    /// Add a list of things to the queue (by id)
    #[allow(dead_code)]
    Add(Vec<Thing>),
    /// Remove something from the queue (by index)
    Remove(usize),
    /// Set the current queue position
    SetPosition(usize),
    /// Shuffle the queue
    Shuffle,
    /// Clear the queue
    Clear,
    /// Set the repeat mode
    SetRepeatMode(RepeatMode),
}

pub enum LibraryAction {
    /// Rescan the library
    Rescan,
    /// Tell the Library Store to get the latest library data
    #[allow(dead_code)]
    Update,
    /// Analyze the library
    Analyze,
    /// Recluster the collection
    Recluster,
    /// Create a new playlist with the given name
    CreatePlaylist(String),
    /// Delete a playlist by id
    RemovePlaylist(Thing),
    /// Remove a song from a playlist (`PlaylistId`, Vec<`SongId`>)
    RemoveSongsFromPlaylist(Thing, Vec<Thing>),
    /// Add a list of things to a playlist (`PlaylistId`, Vec<`SongId`>)
    AddThingsToPlaylist(Thing, Vec<Thing>),
    /// Create a new playlist with the given name (if it doesn't exist) and add the songs to it
    /// (`PlaylistName`, Vec<`SongId`>)
    CreatePlaylistAndAddThings(String, Vec<Thing>),
}

pub enum PopupAction {
    /// Open a popup
    Open(PopupType),
    /// Close the current popup
    Close,
}
