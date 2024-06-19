# MECOMP TUI

This is the TUI client for MECOMP. It is a terminal-based user interface that allows users to interact with the MECOMP daemon.

## Layout

The TUI is divided into 2 main sections: the main view and the control panel.

The control panel is a fixed height panel at the bottom of the screen that displays the current state of the player (playing, paused, stopped, etc.) and allows users to control the player (play, pause, stop, etc.), volume, etc.

The main view is the rest of the screen and has 3 main sections (from left to right): the sidebar, the content view, and the queue.

The sidebar allows users to navigate between different views (e.g. the library, playlists, search, etc.).

The content view displays the contents of the current view (e.g. the songs in a playlist, the search results, etc.).

The queue displays the current queue of songs that are going to be played next.

So all in all, there are 4 main components of the TUI:

- Sidebar
- Content view
- Queue
- Control panel

## State Stores

The TUI uses a few state stores to manage the state of the application, some are updated at regular intervals, some are updated in response to user input, and some are updated by both.

The main state stores are:

- `audio`: The state of the audio player (playing, paused, stopped, runtime info, the queue, etc.), updated every tick.
- `library`: The state of the music library (songs, artists, albums, playlists, etc.), updated every minute, and in response to the user initiating a rescan, recluster, or adding a new playlist.
- `search`: The query and state of the search results, updated in response to the user initiating a search.
