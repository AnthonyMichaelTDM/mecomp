# MECOMP todo list

## Daemon/Backend

### Basic music playing

- [x] Implement basic RPC server/client capably of handling all the basic functionality of a music player (play, pause, stop, etc.)
- [x] Implement audio playback functionality
- [x] Scan music collection from a directory (and it's subdirectories)
- [x] maintain a persistent state of the music collection (henceforth referred to as "Library" or "Music Library") that allows users to create playlists, track play counts, "like" songs, etc.
- [x] rescan endpoint: updates the library while minimizing data loss (i.e. play counts, likes, playlists, etc. should be preserved as much as possible)
  - used when adding a new root directory to the music collection, or when there have been changes to the collection while the daemon was not running
- [x] music library watcher that dynamically updates the library when songs are added, removed, or modified as long as the daemon is running
- [x] properly handle compilation albums (i.e. albums with multiple artists)
- [x] add commands for reading songs by artists, in albums, in playlists, in collections, etc.
- [x] commands for reading the artist/album of songs
- [x] commands for reading the artists of albums
- [ ] allow users to "like" songs
- [ ] track play counts

### Search functionality

- [x] allow users to search their music library (search for songs, artists, albums, etc.)
  - [x] searching by songs includes the artist names in the index, so for example searching for "Green Day" will return all songs by Green Day (even if the song name doesn't contain "Green Day")

### Playlists

- [x] allow users to create playlists
- [ ] allow users to create "smart playlists" that are automatically updated based on a set of criteria (e.g. "all songs with a play count greater than 10", "all songs by Green Day", "all songs with a similarity to Foo greater than X", etc.)
  - [ ] these criteria should be able to be combined with set/boolean logic (union (AND), intersection (OR), difference (NOT))
  - [ ] criteria can be scoped to allow for more complex queries

### Radio (song suggestions)

- [x] analyze audio features of songs to create searchable vector space for nearest neighbor search (M-Tree)
- [x] use M-Tree index based nearest neighbor search to find similar songs
  - ability to find songs similar to a given:
    - [x] song
    - [x] artist
    - [x] album
    - [x] playlist
    - [x] collection
    - [ ] genre

### Collections

- [ ] maintains multiple auto-curated "Collections" created by K-Means clustering on the Music Library, these collections will represent the broad themes within your music collection, but are not tied to human defined genres
- [x] Users can "freeze" a collection, which will convert it to a playlist
- [ ] Functionality to actually create the collections (recluster endpoit)

### Metadata

- [ ] if a song is missing important metadata, and there is an internet connection, attempt to fetch the metadata from the [MusicBrainz API](https://musicbrainz.org/doc/MusicBrainz_API).

## Clients

### MECOMP-CLI

- [x] Implement basic functionality (mirror the daemon's functionality). Basically a direct translation of the daemon's API to a CLI.
- [ ] Allow users to pipe results of searches, radios, etc. to other commands (e.g. `mecomp-cli search "Green Day" | mecomp-cli radio`)
  - [x] pipe to the queue (append): `mecomp-cli search "Green Day" | mecomp-cli queue`
  - [ ] pipe to a new/existing playlist
  - [ ] pipe to library lookup
  - [ ] pipe to radio
- [ ] add commands for reading songs by artists, in albums, in playlists, in collections, etc.
- [ ] commands for reading the artist/album of songs
- [ ] commands for reading the artists of albums

### MECOMP-TUI

- [x] Implement basic TUI
- [ ] ThingTree widget: a tree/list that allows for multiple items to be selected
  - [ ] allow for multiple items to be selected and added to the queue, a playlist, or used to start a radio
  - [ ] function to get the selected items
- [ ] add confirmation dialogues for potentially destructive actions (e.g. deleting a playlist, initiating a rescan, etc.)
- [ ] Implement view pages for the following:
  - [ ] search results
    - [x] show the results of a search
    - [x] pressing enter on a result will take you to the appropriate view page for that result
    - [ ] use ThingTree widget instead of Tree
  - [ ] radio results
    - [x] show the results of a radio search
    - [x] keybind to add to queue
    - [ ] keybind to add to a playlist
    - [ ] use ThingTree widget instead of Tree
  - [ ] albums
    - [x] display all albums
    - [x] be able to "enter" an to go to the album view page
    - [x] ability to sort by name, artist, year, etc.
    - [ ] use ThingTree widget instead of Tree
  - [ ] artists
    - [x] display all artists
    - [x] be able to "enter" an artist to go to the artist view page
    - [x] ability to sort by name, etc.
    - [ ] use ThingTree widget instead of Tree
  - [ ] songs
    - [x] display all songs
    - [x] be able to "enter" a song to go to the song view page
    - [x] ability to sort by name, artist, album, year, etc.
    - [ ] use ThingTree widget instead of Tree
  - [x] playlists
    - [x] display all playlists
    - [x] ability to sort by name, etc.
    - [x] be able to "enter" a playlist to go to the playlist view page
    - [x] keybind to create a new playlist
    - [x] keybind to remove the selected playlist
  - [x] collections
    - [x] display all collections
    - [x] be able to "enter" a collection to go to the collection view page
  - [ ] a single album
    - [x] show info about the album, including all the songs contained
    - [x] keybind to add to queue
    - [x] keybind to start a radio
    - [ ] keybind to add to a playlist
    - [ ] use ThingTree widget instead of Tree
  - [ ] a single artist
    - [x] show info about the artist, including all the albums and songs by the artist
    - [x] keybind to add to queue
    - [x] keybind to start a radio
    - [ ] keybind to add to a playlist
    - [ ] use ThingTree widget instead of Tree
  - [ ] a single song
    - [x] show information about the song
    - [x] keybind to add to queue
    - [x] keybind to start a radio
    - [ ] keybind to add to a playlist
  - [ ] a single playlist
    - [x] show the playlist's name and contents
    - [x] keybind to add to queue
    - [x] keybind to start a radio
    - [ ] keybind to add to a playlist
    - [x] keybind to remove a song from the playlist
    - [ ] show song suggestions based on the playlist (radio)
  - [ ] a single collection
    - [x] show the collection's contents
    - [x] keybind to add to queue
    - [ ] keybind to freeze into a playlist
- [ ] at startup, check if the daemon is running, and if it isn't then start it in a detached process

### MECOMP-GUI

- [ ] Implement basic GUI
