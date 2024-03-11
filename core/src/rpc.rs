//! This module contains the service definitions.

#![allow(clippy::future_not_send)]

use std::ops::Range;

use mecomp_storage::db::schemas::{
    album::{Album, AlbumBrief, AlbumId},
    artist::{Artist, ArtistBrief, ArtistId},
    collection::{Collection, CollectionBrief, CollectionId},
    playlist::{Playlist, PlaylistBrief, PlaylistId},
    song::{Song, SongBrief, SongId},
};

use crate::{
    errors::LibraryError,
    library::{LibraryBrief, LibraryFull, LibraryHealth},
    playback::{Percent, RepeatMode, SeekType, StateAudio, StateRuntime},
    queue::Queue,
    search::SearchResult,
};

#[tarpc::service]
pub trait MusicPlayer {
    // misc
    async fn ping() -> String;

    // Music library.
    /// Rescans the music library.
    async fn library_rescan() -> Result<(), LibraryError>;
    /// Returns brief information about the music library.
    async fn library_brief() -> LibraryBrief;
    /// Returns full information about the music library. (all songs, artists, albums, etc.)
    async fn library_full() -> LibraryFull;
    /// Returns brief information about the music library's artists.
    async fn library_artists_brief() -> Box<[ArtistBrief]>;
    /// Returns full information about the music library's artists.
    async fn library_artists_full() -> Box<[Artist]>;
    /// Returns brief information about the music library's albums.
    async fn library_albums_brief() -> Box<[AlbumBrief]>;
    /// Returns full information about the music library's albums.
    async fn library_albums_full() -> Box<[Album]>;
    /// Returns brief information about the music library's songs.
    async fn library_songs_brief() -> Box<[SongBrief]>;
    /// Returns full information about the music library's songs.
    async fn library_songs_full() -> Box<[Song]>;
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
    async fn state_audio() -> StateAudio;
    /// returns information about the current queue.
    async fn state_queue() -> Queue;
    /// is the player currently playing?
    async fn state_playing() -> bool;
    /// what repeat mode is the player in?
    async fn state_repeat() -> RepeatMode;
    /// returns the current volume.
    async fn state_volume() -> Percent;
    /// returns the current volume mute state.
    async fn state_volume_muted() -> bool;
    /// returns information about the runtime of the current song (seek position and duration)
    async fn state_runtime() -> StateRuntime;

    // Current (audio state)
    /// returns the current artist.
    async fn current_artist() -> Option<Artist>;
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
    /// skip forward by the given amount of songs
    async fn playback_skip(amount: usize) -> ();
    /// go back to the previous song.
    /// (if the current song is more than `threshold` seconds in, it will restart the current song instead)
    async fn playback_previous(threshold: Option<usize>) -> ();
    /// go backwards by the given amount of songs.
    async fn playback_back(amount: usize) -> ();
    /// stop playback.
    /// (clears the queue and stops playback)
    async fn playback_stop() -> ();
    /// clear the queue.
    async fn playback_clear() -> ();
    /// seek forwards, backwards, or to an absolute second in the current song.
    async fn playback_seek(seek: SeekType, seconds: u64) -> ();
    /// set the repeat mode.
    async fn playback_repeat(mode: RepeatMode) -> ();
    /// Shuffle the current queue, then start playing from the 1st Song in the queue.
    async fn playback_shuffle() -> ();
    /// set the volume to the given value (0-100).
    /// (if the value is greater than 100, it will be clamped to 100.)
    async fn playback_volume(volume: u8) -> ();
    /// increase the volume by the given amount (0-100).
    /// (volume will be clamped to 100.)
    async fn playback_volume_up(amount: u8) -> ();
    /// decrease the volume by the given amount (0-100).
    /// (volume will be clamped to 0.)
    async fn playback_volume_down(amount: u8) -> ();
    /// toggle the volume mute.
    async fn playback_volume_toggle_mute() -> ();
    /// mute the volume.
    async fn playback_mute() -> ();
    /// unmute the volume.
    async fn playback_unmute() -> ();

    // Queue control.
    /// add a song to the queue.
    /// (if the queue is empty, it will start playing the song.)
    async fn queue_add_song(song: SongId) -> ();
    /// add an album to the queue.
    /// (if the queue is empty, it will start playing the album.)
    async fn queue_add_album(album: AlbumId) -> ();
    /// add an artist to the queue.
    /// (if the queue is empty, it will start playing the artist.)
    async fn queue_add_artist(artist: ArtistId) -> ();
    /// add a playlist to the queue.
    /// (if the queue is empty, it will start playing the playlist.)
    async fn queue_add_playlist(playlist: PlaylistId) -> ();
    /// add a collection to the queue.
    /// (if the queue is empty, it will start playing the collection.)
    async fn queue_add_collection(collection: CollectionId) -> ();
    /// add a random song to the queue.
    /// (if the queue is empty, it will start playing the song.)
    async fn queue_add_rand_song() -> ();
    /// add a random album to the queue.
    /// (if the queue is empty, it will start playing the album.)
    async fn queue_add_rand_album() -> ();
    /// add a random artist to the queue.
    /// (if the queue is empty, it will start playing the artist.)
    async fn queue_add_rand_artist() -> ();
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
    /// (creates a new playlist with the same name and contents as the given playlist.)
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
