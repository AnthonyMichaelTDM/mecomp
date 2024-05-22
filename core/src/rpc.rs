//! This module contains the service definitions.

#![allow(clippy::future_not_send)]

use std::ops::Range;

use mecomp_storage::{
    db::schemas::{
        album::{Album, AlbumBrief, AlbumId},
        artist::{Artist, ArtistBrief, ArtistId},
        collection::{Collection, CollectionBrief, CollectionId},
        playlist::{Playlist, PlaylistBrief, PlaylistId},
        song::{Song, SongBrief, SongId},
    },
    util::OneOrMany,
};

use crate::{
    errors::SerializableLibraryError,
    search::SearchResult,
    state::{
        library::{LibraryBrief, LibraryFull, LibraryHealth},
        Percent, RepeatMode, SeekType, StateAudio, StateRuntime,
    },
};

#[tarpc::service]
pub trait MusicPlayer {
    // misc
    async fn ping() -> String;

    // Music library.
    /// Rescans the music library.
    async fn library_rescan() -> Result<(), SerializableLibraryError>;
    /// Returns brief information about the music library.
    async fn library_brief() -> Result<LibraryBrief, SerializableLibraryError>;
    /// Returns full information about the music library. (all songs, artists, albums, etc.)
    async fn library_full() -> Result<LibraryFull, SerializableLibraryError>;
    /// Returns brief information about the music library's artists.
    async fn library_artists_brief() -> Result<Box<[ArtistBrief]>, SerializableLibraryError>;
    /// Returns full information about the music library's artists.
    async fn library_artists_full() -> Result<Box<[Artist]>, SerializableLibraryError>;
    /// Returns brief information about the music library's albums.
    async fn library_albums_brief() -> Result<Box<[AlbumBrief]>, SerializableLibraryError>;
    /// Returns full information about the music library's albums.
    async fn library_albums_full() -> Result<Box<[Album]>, SerializableLibraryError>;
    /// Returns brief information about the music library's songs.
    async fn library_songs_brief() -> Result<Box<[SongBrief]>, SerializableLibraryError>;
    /// Returns full information about the music library's songs.
    async fn library_songs_full() -> Result<Box<[Song]>, SerializableLibraryError>;
    /// Returns information about the health of the music library (are there any missing files, etc.)
    async fn library_health() -> LibraryHealth;

    // music library CRUD operations
    /// Get a song by its ID.
    async fn library_song_get(id: SongId) -> Option<Song>;
    /// Get an album by its ID.
    async fn library_album_get(id: AlbumId) -> Option<Album>;
    /// Get an artist by its ID.
    async fn library_artist_get(id: ArtistId) -> Option<Artist>;
    /// Get a playlist by its ID.
    async fn library_playlist_get(id: PlaylistId) -> Option<Playlist>;

    // Daemon control.
    /// tells the daemon to shutdown.
    async fn daemon_shutdown() -> ();

    // State retrieval.
    /// returns full information about the current state of the audio player (queue, current song, etc.)
    async fn state_audio() -> Option<StateAudio>;
    /// returns information about the current queue.
    async fn state_queue() -> Option<Box<[Song]>>;
    /// returns the current queue position.
    async fn state_queue_position() -> Option<usize>;
    /// is the player currently playing?
    async fn state_paused() -> bool;
    /// what repeat mode is the player in?
    async fn state_repeat() -> Option<RepeatMode>;
    /// returns the current volume.
    async fn state_volume() -> Option<Percent>;
    /// returns the current volume mute state.
    async fn state_volume_muted() -> bool;
    /// returns information about the runtime of the current song (seek position and duration)
    async fn state_runtime() -> Option<StateRuntime>;

    // Current (audio state)
    /// returns the current artist.
    async fn current_artist() -> Option<OneOrMany<Artist>>;
    /// returns the current album.
    async fn current_album() -> Option<Album>;
    /// returns the current song.
    async fn current_song() -> Option<Song>;

    // Rand (audio state)
    /// returns a random artist.
    async fn rand_artist() -> Option<Artist>;
    /// returns a random album.
    async fn rand_album() -> Option<Album>;
    /// returns a random song.
    async fn rand_song() -> Option<Song>;

    // Search (fuzzy keys)
    /// returns a list of artists, albums, and songs matching the given search query.
    async fn search(query: String) -> Box<[SearchResult]>;
    /// returns a list of artists matching the given search query.
    async fn search_artist(query: String) -> Box<[Artist]>;
    /// returns a list of albums matching the given search query.
    async fn search_album(query: String) -> Box<[Album]>;
    /// returns a list of songs matching the given search query.
    async fn search_song(query: String) -> Box<[Song]>;

    // Playback control.
    /// toggles playback (play/pause).
    async fn playback_toggle() -> ();
    /// start playback (unpause).
    async fn playback_play() -> ();
    /// pause playback.
    async fn playback_pause() -> ();
    /// set the current song to be the next song in the queue.
    async fn playback_next() -> ();
    /// restart the current song.
    async fn playback_restart() -> ();
    /// skip forward by the given amount of songs
    async fn playback_skip_forward(amount: usize) -> ();
    /// go backwards by the given amount of songs.
    async fn playback_skip_backward(amount: usize) -> ();
    /// only clear the player (i.e. stop playback)
    async fn playback_clear_player() -> ();
    /// clears the queue and stops playback.
    async fn playback_clear() -> ();
    /// seek forwards, backwards, or to an absolute second in the current song.
    async fn playback_seek(seek: SeekType, seconds: u64) -> ();
    /// set the repeat mode.
    async fn playback_repeat(mode: RepeatMode) -> ();
    /// Shuffle the current queue, then start playing from the 1st Song in the queue.
    async fn playback_shuffle() -> ();
    /// set the volume to the given value (0-100).
    /// (if the value is greater than 100, it will be clamped to 100.)
    async fn playback_volume(volume: Percent) -> ();
    /// increase the volume by the given amount (0-100).
    /// (volume will be clamped to 100.)
    async fn playback_volume_up(amount: Percent) -> ();
    /// decrease the volume by the given amount (0-100).
    /// (volume will be clamped to 0.)
    async fn playback_volume_down(amount: Percent) -> ();
    /// toggle the volume mute.
    async fn playback_volume_toggle_mute() -> ();
    /// mute the volume.
    async fn playback_mute() -> ();
    /// unmute the volume.
    async fn playback_unmute() -> ();

    // Queue control.
    /// add a song to the queue.
    /// (if the queue is empty, it will start playing the song.)
    async fn queue_add_song(song: SongId) -> Result<(), SerializableLibraryError>;
    /// add an album to the queue.
    /// (if the queue is empty, it will start playing the album.)
    async fn queue_add_album(album: AlbumId) -> Result<(), SerializableLibraryError>;
    /// add an artist to the queue.
    /// (if the queue is empty, it will start playing the artist.)
    async fn queue_add_artist(artist: ArtistId) -> Result<(), SerializableLibraryError>;
    /// add a playlist to the queue.
    /// (if the queue is empty, it will start playing the playlist.)
    async fn queue_add_playlist(playlist: PlaylistId) -> Result<(), SerializableLibraryError>;
    /// add a collection to the queue.
    /// (if the queue is empty, it will start playing the collection.)
    async fn queue_add_collection(collection: CollectionId)
        -> Result<(), SerializableLibraryError>;
    /// add a random song to the queue.
    /// (if the queue is empty, it will start playing the song.)
    async fn queue_add_rand_song() -> Result<(), SerializableLibraryError>;
    /// add a random album to the queue.
    /// (if the queue is empty, it will start playing the album.)
    async fn queue_add_rand_album() -> Result<(), SerializableLibraryError>;
    /// add a random artist to the queue.
    /// (if the queue is empty, it will start playing the artist.)
    async fn queue_add_rand_artist() -> Result<(), SerializableLibraryError>;
    /// set the current song to a queue index.
    /// if the index is out of bounds, it will be clamped to the nearest valid index.
    async fn queue_set_index(index: usize) -> ();
    /// remove a range of songs from the queue.
    /// if the range is out of bounds, it will be clamped to the nearest valid range.
    async fn queue_remove_range(range: Range<usize>) -> ();

    // Playlists.
    /// Returns brief information about the users playlists.
    async fn playlist_list() -> Box<[PlaylistBrief]>;
    /// create a new playlist.
    async fn playlist_new(name: String) -> PlaylistId;
    /// remove a playlist.
    async fn playlist_remove(name: String) -> bool;
    /// clone a playlist.
    /// (creates a new playlist with the same name (append "copy") and contents as the given playlist.)
    async fn playlist_clone(name: String) -> ();
    /// get the id of a playlist.
    async fn playlist_get_id(name: String) -> Option<PlaylistId>;
    /// remove a song from a playlist.
    /// if the song is not in the playlist, this will do nothing.
    async fn playlist_remove_song(playlist: PlaylistId, song: SongId) -> ();
    /// Add an artist to a playlist.
    async fn playlist_add_artist(playlist: PlaylistId, artist: ArtistId) -> ();
    /// Add an album to a playlist.
    async fn playlist_add_album(playlist: PlaylistId, album: AlbumId) -> ();
    /// Add a song to a playlist.
    async fn playlist_add_song(playlist: PlaylistId, song: SongId) -> ();
    /// Get a playlist by its ID.
    async fn playlist_get(id: PlaylistId) -> Option<Playlist>;

    // Auto Curration commands.
    // (collections, radios, smart playlists, etc.)
    /// Collections: Return brief information about the users auto curration collections.
    async fn collection_list() -> Box<[CollectionBrief]>;
    /// Collections: Recluster the users library, creating new collections.
    async fn collection_recluster() -> ();
    /// Collections: get a collection by its ID.
    async fn collection_get(id: CollectionId) -> Option<Collection>;
    /// Collections: freeze a collection (convert it to a playlist).
    async fn collection_freeze(id: CollectionId, name: String) -> PlaylistId;
    /// Radio: get the `n` most similar songs to the given song.
    async fn radio_get_similar_songs(song: SongId, n: usize) -> Box<[SongId]>;
    /// Radio: get the `n` most similar artists to the given artist.
    async fn radio_get_similar_artist(artist: ArtistId, n: usize) -> Box<[ArtistId]>;
    /// Radio: get the `n` most similar albums to the given album.
    async fn radio_get_similar_album(album: AlbumId, n: usize) -> Box<[AlbumId]>;
}
