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

### MECOMP-TUI

- [ ] Implement basic TUI

### MECOMP-GUI

- [ ] Implement basic GUI
