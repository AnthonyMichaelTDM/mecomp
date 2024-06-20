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

### Search functionality

- [x] allow users to search their music library (search for songs, artists, albums, etc.)
  - [x] searching by songs includes the artist names in the index, so for example searching for "Green Day" will return all songs by Green Day (even if the song name doesn't contain "Green Day")

### Playlists

- [x] allow users to create playlists
- [ ] allow users to "like" songs
- [ ] track play counts
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
- [ ] Checklist widget: a tree/list that allows for multiple items to be selected
  - [ ] allow for multiple items to be selected (e.g. for adding to the queue, a playlist, starting a radio, etc.)
  - [ ] function to get the selected items
- [ ] Implement view pages for the following:
  - [ ] search results
    - [x] show the results of a search
    - [x] pressing enter on a result will take you to the appropriate view page for that result
    - [ ] be able to select multiple results and add them to the queue, a playlist, start a radio, etc,
  - [ ] radio results
    - [ ] show the results of a radio search
    - [ ] be able to select multiple results and add them to the queue, a playlist, start a radio, etc,
  - [ ] albums
    - [x] display all albums
    - [x] be able to "enter" an to go to the album view page
    - [x] ability to sort by name, artist, year, etc.
    - [ ] be able to select multiple items and add them to the queue, a playlist, start a radio, etc,
  - [ ] artists
    - [x] display all artists
    - [x] be able to "enter" an artist to go to the artist view page
    - [x] ability to sort by name, etc.
    - [ ] be able to select multiple items and add them to the queue, a playlist, start a radio, etc,
  - [x] songs
    - [x] display all songs
    - [x] be able to "enter" a song to go to the song view page
    - [x] ability to sort by name, artist, album, year, etc.
    - [ ] be able to select multiple items and add them to the queue, a playlist, start a radio, etc,
  - [ ] playlists
    - [ ] display all playlists
    - [ ] ability to sort by name, etc.
    - [ ] be able to "enter" a playlist to go to the playlist view page
  - [ ] collections
    - [ ] display all collections
    - [ ] be able to "enter" a collection to go to the collection view page
  - [x] a single album
    - [x] show info about the album, including all the songs contained
    - [ ] be able to add the album to the queue, a playlist, etc,
    - [ ] be able to start a radio from the album
    - [ ] be able to select multiple items and add them to the queue, a playlist, start a radio, etc,
  - [ ] a single artist
    - [x] show info about the artist, including all the albums and songs by the artist
    - [ ] be able to add all the artist's songs to the queue, a playlist, etc,
    - [ ] be able to select multiple items and add them to the queue, a playlist, start a radio, etc,
  - [ ] a single song
    - [x] show information about the song
    - [ ] be able to add the song to the queue, a playlist, etc,
    - [ ] be able to start a radio from the song
  - [ ] a single playlist
    - [ ] show the playlist's name and contents
    - [ ] be able to add the playlist to the queue
    - [ ] be able to select multiple songs and add them to the queue, a playlist, start a radio, etc,
    - [ ] show song suggestions based on the playlist (radio)
  - [ ] a single collection
    - [ ] show the collection's contents
    - [ ] be able to add the collection to the queue
    - [ ] be able to select multiple songs and add them to the queue, a playlist, start a radio, etc,

### MECOMP-GUI

- [ ] Implement basic GUI
