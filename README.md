# Metadata Enhanced Collection Oriented Music Player (MECOMP)

(name subject to change)

## Introduction

MECOMP is a local music player inspired by [festival](https://github.com/hinto-janai/festival) that it is designed to solve a very specific problem:

> I have a large collection of music, organizing my music by artist and album is not enough, I want to be able to organize my music by other criteria, such as genre, mood, etc.
> Typically, I would have to create a playlist for each of these criteria, but that is tedious and time consuming.
> I want to have automatically curated playlists of similar songs (analogous to genres), dynamically updated playlists of songs that match a certain criteria (basically filters), and be able to create queues of songs that are similar to the current song (think Pandora).
> There are some services that let you do most of these, like Spotify, but I want to be able to do this with my *local* music collection and not have to rely on a third party service.

## Features

(this acts as a TODO list for now)

- [ ] Scan music collection from a directory (and it's subdirectories)
- [ ] maintain a persistent state of the music collection (henceforth referred to as "Library" or "Music Library") that allows users to create playlists, track play counts, "like" songs, etc.
  this library will not update dynamically (i.e. if a file is added/removed from the music collection), but will have a "rescan" feature that will update the library without losing any of the user's data (play counts, likes, playlists, etc.)
- [ ] allow users to create playlists
- [ ] allow users to "like" songs
- [ ] track play counts
- [ ] allow users to create "smart playlists" that are automatically updated based on a set of criteria (e.g. "all songs with a play count greater than 10", "all songs by Green Day", "all songs with a similarity to Foo greater than X", etc.)
  - [ ] these criteria should be able to be combined with set/boolean logic (union (AND), intersection (OR), difference (NOT))
  - [ ] criteria can be scoped to allow for more complex queries
- [ ] maintains multiple auto-curated "Collections" created by K-Means clustering on the Music Library, these collections will represent the broad themes within your music collection, but are not tied to human defined genres
  - I'm not sure how best to name these collections, may be "genres", "moods", "styles", etc. I'm not sure if these should be user editable or not, but I'm leaning towards not.
  - These collections are generated when the Music Library is scanned and will be updated when the Music Library is rescanned
  - [ ] Users can "freeze" a collection, which will convert it to a playlist
- [ ] allow users to start a "radio" based on a specific song, which will populate the queue with the `n` most similar songs to the current song
  - uses HNSW nearest neighbor search to find similar songs. This is a fast approximate nearest neighbor search algorithm that is well suited for high dimensional data (like audio features)
  - audio features are extracted by methods inspired by [bliss-rs](https://github.com/Polochon-street/bliss-rs), these are the same features we use for clustering
- [ ] suggest songs to add to a playlist based on the current playlist (the average of the audio features of the songs in the playlist)
- [ ] if a song is missing important metadata, and there is an internet connection, attempt to fetch the metadata from the [MusicBrainz API](https://musicbrainz.org/doc/MusicBrainz_API).
- [ ] properly handle compilation albums (i.e. albums with multiple artists)
  - do this by simply showing the album multiple times, once for each artist. This is how iTunes handles it, and I think it's the best way to do it.
- [ ] properly handle songs with multiple artists (i.e. collaborations) (this is different from compilation albums)
  - mark the song as being by multiple artists, but only show it for the artist(s) that are identified by the "album artist" tag in the metadata (if it exists)\
- [ ] properly handle songs with multiple genres (i.e. "Rock; Metal")
  - show the song for each genre

## Architecture

MECOMP is designed to be modular and extensible, and is composed of a daemon (which is the core of the application), and several clients that communicate with the daemon.

### MECOMP-Daemon

MECOMP-Daemon is a long-running RPC server that is the core of the application, it handles all the backend logic and state-management necessary for the application to function. the MECOMP clients are simply frontends to this server. It is written in rust and uses google's [tarpc](https://github.com/google/tarpc) library for inter-process communication via RPC.

### Clients

#### MECOMP-CLI

MECOMP-CLI is a command line interface for MECOMP, it provides a simple way to interact with the daemon.

#### MECOMP-TUI

MECOMP-TUI is a terminal user interface for MECOMP, it provides a more user friendly way to interact with the daemon, but still in a terminal.

#### MECOMP-GUI

MECOMP-GUI is a graphical user interface for MECOMP, it provides a more user friendly way to interact with the daemon.
