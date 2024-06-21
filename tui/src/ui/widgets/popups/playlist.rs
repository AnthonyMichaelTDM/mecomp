//! A popup that prompts the user to select a playlist, or create a new one.
//!
//! The popup will consist of an input box for the playlist name, a list of playlists to select from, and a button to create a new playlist.
//!
//! The user can navigate the list of playlists using the arrow keys, and select a playlist by pressing the enter key.
//!
//! The user can create a new playlist by typing a name in the input box and pressing the enter key.
//!
//! The user can cancel the popup by pressing the escape key.
//!
//! TODO: because popups need to access the un-obstructed keyboard input, we need to have a "popup manager" that runs at the same level as the
//! other main components, and update the app state to show/hide the popup. When popups are visible, app state should be locked to prevent
//! other components from receiving input or something
//!  
