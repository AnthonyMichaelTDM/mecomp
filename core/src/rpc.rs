//! This module contains the service definitions.

#![allow(clippy::future_not_send)]

use std::{
    net::{IpAddr, Ipv4Addr},
    ops::Range,
    time::Duration,
};

use mecomp_storage::db::schemas::{
    album::{Album, AlbumBrief},
    artist::{Artist, ArtistBrief},
    collection::{Collection, CollectionBrief},
    playlist::{Playlist, PlaylistBrief},
    song::{Song, SongBrief},
    Thing,
};
use one_or_many::OneOrMany;
use serde::{Deserialize, Serialize};
use tarpc::{client, tokio_serde::formats::Json};

use crate::{
    errors::SerializableLibraryError,
    state::{
        library::{LibraryBrief, LibraryFull, LibraryHealth},
        RepeatMode, SeekType, StateAudio, StateRuntime,
    },
};

pub type SongId = Thing;
pub type ArtistId = Thing;
pub type AlbumId = Thing;
pub type CollectionId = Thing;
pub type PlaylistId = Thing;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SearchResult {
    pub songs: Box<[Song]>,
    pub albums: Box<[Album]>,
    pub artists: Box<[Artist]>,
}

impl SearchResult {
    #[must_use]
    pub const fn len(&self) -> usize {
        self.songs.len() + self.albums.len() + self.artists.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.songs.is_empty() && self.albums.is_empty() && self.artists.is_empty()
    }
}

// TODO: commands for reading songs by paths, artists by name, etc.

#[tarpc::service]
pub trait MusicPlayer {
    // misc
    async fn ping() -> String;

    // Music library.
    /// Rescans the music library, only error is if a rescan is already in progress.
    async fn library_rescan() -> Result<(), SerializableLibraryError>;
    /// Check if a rescan is in progress.
    async fn library_rescan_in_progress() -> bool;
    /// Analyze the music library, only error is if an analysis is already in progress.
    async fn library_analyze() -> Result<(), SerializableLibraryError>;
    /// Check if an analysis is in progress.
    async fn library_analyze_in_progress() -> bool;
    /// Recluster the music library, only error is if a recluster is already in progress.
    async fn library_recluster() -> Result<(), SerializableLibraryError>;
    /// Check if a recluster is in progress.
    async fn library_recluster_in_progress() -> bool;
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
    async fn library_health() -> Result<LibraryHealth, SerializableLibraryError>;

    // music library CRUD operations
    /// Get a song by its ID.
    async fn library_song_get(id: SongId) -> Option<Song>;
    /// Get the artists of a song.
    async fn library_song_get_artist(id: SongId) -> OneOrMany<Artist>;
    /// Get the album of a song.
    async fn library_song_get_album(id: SongId) -> Option<Album>;
    /// Get the Playlists a song is in.
    async fn library_song_get_playlists(id: SongId) -> Box<[Playlist]>;
    /// Get the Collections a song is in.
    async fn library_song_get_collections(id: SongId) -> Box<[Collection]>;
    /// Get an album by its ID.
    async fn library_album_get(id: AlbumId) -> Option<Album>;
    /// Get the artists of an album
    async fn library_album_get_artist(id: AlbumId) -> OneOrMany<Artist>;
    /// Get the songs of an album
    async fn library_album_get_songs(id: AlbumId) -> Option<Box<[Song]>>;
    /// Get an artist by its ID.
    async fn library_artist_get(id: ArtistId) -> Option<Artist>;
    /// Get the songs of an artist
    async fn library_artist_get_songs(id: ArtistId) -> Option<Box<[Song]>>;
    /// Get the albums of an artist
    async fn library_artist_get_albums(id: ArtistId) -> Option<Box<[Album]>>;

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
    async fn state_volume() -> Option<f32>;
    /// returns the current volume mute state.
    async fn state_volume_muted() -> bool;
    /// returns information about the runtime of the current song (seek position and duration)
    async fn state_runtime() -> Option<StateRuntime>;

    // Current (audio state)
    /// returns the current artist.
    async fn current_artist() -> OneOrMany<Artist>;
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
    async fn search(query: String, limit: u32) -> SearchResult;
    /// returns a list of artists matching the given search query.
    async fn search_artist(query: String, limit: u32) -> Box<[Artist]>;
    /// returns a list of albums matching the given search query.
    async fn search_album(query: String, limit: u32) -> Box<[Album]>;
    /// returns a list of songs matching the given search query.
    async fn search_song(query: String, limit: u32) -> Box<[Song]>;

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
    async fn playback_seek(seek: SeekType, duration: Duration) -> ();
    /// set the repeat mode.
    async fn playback_repeat(mode: RepeatMode) -> ();
    /// Shuffle the current queue, then start playing from the 1st Song in the queue.
    async fn playback_shuffle() -> ();
    /// set the volume to the given value
    /// The value `1.0` is the "normal" volume (unfiltered input). Any value other than `1.0` will multiply each sample by this value.
    async fn playback_volume(volume: f32) -> ();
    /// increase the volume by the given amount
    async fn playback_volume_up(amount: f32) -> ();
    /// decrease the volume by the given amount
    async fn playback_volume_down(amount: f32) -> ();
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
    /// add a list of things to the queue.
    /// (if the queue is empty, it will start playing the first thing in the list.)
    async fn queue_add_list(list: Vec<Thing>) -> Result<(), SerializableLibraryError>;
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
    /// if a playlist with the same name already exists, this will return that playlist's id in the error variant
    async fn playlist_new(
        name: String,
    ) -> Result<Result<PlaylistId, PlaylistId>, SerializableLibraryError>;
    /// remove a playlist.
    async fn playlist_remove(id: PlaylistId) -> Result<(), SerializableLibraryError>;
    /// clone a playlist.
    /// (creates a new playlist with the same name (append "copy") and contents as the given playlist.)
    /// returns the id of the new playlist.
    async fn playlist_clone(id: PlaylistId) -> Result<PlaylistId, SerializableLibraryError>;
    /// get the id of a playlist.
    /// returns none if the playlist does not exist.
    async fn playlist_get_id(name: String) -> Option<PlaylistId>;
    /// remove a list of songs from a playlist.
    /// if the songs are not in the playlist, this will do nothing.
    async fn playlist_remove_songs(
        playlist: PlaylistId,
        songs: Vec<SongId>,
    ) -> Result<(), SerializableLibraryError>;
    /// Add an artist to a playlist.
    async fn playlist_add_artist(
        playlist: PlaylistId,
        artist: ArtistId,
    ) -> Result<(), SerializableLibraryError>;
    /// Add an album to a playlist.
    async fn playlist_add_album(
        playlist: PlaylistId,
        album: AlbumId,
    ) -> Result<(), SerializableLibraryError>;
    /// Add songs to a playlist.
    async fn playlist_add_songs(
        playlist: PlaylistId,
        songs: Vec<SongId>,
    ) -> Result<(), SerializableLibraryError>;
    /// Add a list of things to a playlist.
    async fn playlist_add_list(
        playlist: PlaylistId,
        list: Vec<Thing>,
    ) -> Result<(), SerializableLibraryError>;
    /// Get a playlist by its ID.
    async fn playlist_get(id: PlaylistId) -> Option<Playlist>;
    /// Get the songs of a playlist
    async fn playlist_get_songs(id: PlaylistId) -> Option<Box<[Song]>>;

    // Auto Curration commands.
    // (collections, radios, smart playlists, etc.)
    /// Collections: Return brief information about the users auto curration collections.
    async fn collection_list() -> Box<[CollectionBrief]>;
    /// Collections: get a collection by its ID.
    async fn collection_get(id: CollectionId) -> Option<Collection>;
    /// Collections: freeze a collection (convert it to a playlist).
    async fn collection_freeze(
        id: CollectionId,
        name: String,
    ) -> Result<PlaylistId, SerializableLibraryError>;
    /// Get the songs of a collection
    async fn collection_get_songs(id: CollectionId) -> Option<Box<[Song]>>;

    // Radio commands.
    /// Radio: get the `n` most similar songs to the given things.
    async fn radio_get_similar(
        things: Vec<Thing>,
        n: u32,
    ) -> Result<Box<[Song]>, SerializableLibraryError>;
    /// Radio: get the `n` most similar songs to the given song.
    async fn radio_get_similar_to_song(
        song: SongId,
        n: u32,
    ) -> Result<Box<[SongId]>, SerializableLibraryError>;
    /// Radio: get the `n` most similar songs to the given artist.
    async fn radio_get_similar_to_artist(
        artist: ArtistId,
        n: u32,
    ) -> Result<Box<[SongId]>, SerializableLibraryError>;
    /// Radio: get the `n` most similar songs to the given album.
    async fn radio_get_similar_to_album(
        album: AlbumId,
        n: u32,
    ) -> Result<Box<[SongId]>, SerializableLibraryError>;
    /// Radio: get the `n` most similar songs to the given playlist.
    async fn radio_get_similar_to_playlist(
        playlist: PlaylistId,
        n: u32,
    ) -> Result<Box<[SongId]>, SerializableLibraryError>;
}

/// Initialize the client
///
/// # Errors
///
/// If the client cannot be initialized, an error is returned.
pub async fn init_client(rpc_port: u16) -> Result<MusicPlayerClient, std::io::Error> {
    let server_addr = (IpAddr::V4(Ipv4Addr::LOCALHOST), rpc_port);

    let mut transport = tarpc::serde_transport::tcp::connect(server_addr, Json::default);
    transport.config_mut().max_frame_length(usize::MAX);

    // MusicPlayerClient is generated by the service attribute. It has a constructor `new` that takes a
    // config and any Transport as input.
    Ok(MusicPlayerClient::new(client::Config::default(), transport.await?).spawn())
}
