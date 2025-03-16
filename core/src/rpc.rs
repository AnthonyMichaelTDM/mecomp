//! This module contains the service definitions.

#![allow(clippy::future_not_send)]

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ops::Range,
    path::PathBuf,
    time::Duration,
};

use mecomp_storage::db::schemas::{
    album::{Album, AlbumBrief},
    artist::{Artist, ArtistBrief},
    collection::{Collection, CollectionBrief},
    dynamic::{query::Query, DynamicPlaylist, DynamicPlaylistChangeSet},
    playlist::{Playlist, PlaylistBrief},
    song::{Song, SongBrief},
    RecordId,
};
use one_or_many::OneOrMany;
use serde::{Deserialize, Serialize};
use tarpc::{client, tokio_serde::formats::Json};

use crate::{
    errors::SerializableLibraryError,
    state::{
        library::{LibraryBrief, LibraryFull, LibraryHealth},
        RepeatMode, SeekType, StateAudio,
    },
};

pub type SongId = RecordId;
pub type ArtistId = RecordId;
pub type AlbumId = RecordId;
pub type CollectionId = RecordId;
pub type PlaylistId = RecordId;
pub type DynamicPlaylistId = RecordId;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SearchResult {
    pub songs: Box<[Song]>,
    pub albums: Box<[Album]>,
    pub artists: Box<[Artist]>,
}

impl SearchResult {
    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.songs.len() + self.albums.len() + self.artists.len()
    }

    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.songs.is_empty() && self.albums.is_empty() && self.artists.is_empty()
    }
}

// TODO: commands for reading songs by paths, artists by name, etc.

/// The music player service, implemented by the music player daemon.
#[tarpc::service]
pub trait MusicPlayer {
    /// Register a UDP listener with the daemon.
    async fn register_listener(listener_addr: SocketAddr) -> ();

    // misc
    async fn ping() -> String;

    // Music library.
    /// Rescans the music library, only error is if a rescan is already in progress.
    async fn library_rescan() -> Result<(), SerializableLibraryError>;
    /// Check if a rescan is in progress.
    async fn library_rescan_in_progress() -> bool;
    /// Analyze the music library, only error is if an analysis is already in progress.
    async fn library_analyze(overwrite: bool) -> Result<(), SerializableLibraryError>;
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
    /// Get a song by its path.
    async fn library_song_get_by_path(path: PathBuf) -> Option<Song>;
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
    /// stop playback.
    async fn playback_stop() -> ();
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
    /// add a thing to the queue.
    /// (if the queue is empty, it will start playing the song.)
    async fn queue_add(thing: RecordId) -> Result<(), SerializableLibraryError>;
    /// add a list of things to the queue.
    /// (if the queue is empty, it will start playing the first thing in the list.)
    async fn queue_add_list(list: Vec<RecordId>) -> Result<(), SerializableLibraryError>;
    /// set the current song to a queue index.
    /// if the index is out of bounds, it will be clamped to the nearest valid index.
    async fn queue_set_index(index: usize) -> ();
    /// remove a range of songs from the queue.
    /// if the range is out of bounds, it will be clamped to the nearest valid range.
    async fn queue_remove_range(range: Range<usize>) -> ();

    // Playlists.
    /// Returns brief information about the users playlists.
    async fn playlist_list() -> Box<[PlaylistBrief]>;
    /// create a new playlist with the given name (if it does not already exist).
    async fn playlist_get_or_create(name: String) -> Result<PlaylistId, SerializableLibraryError>;
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
    /// Add a thing to a playlist.
    /// If the thing is something that has songs (an album, artist, etc.), it will add all the songs.
    async fn playlist_add(
        playlist: PlaylistId,
        thing: RecordId,
    ) -> Result<(), SerializableLibraryError>;
    /// Add a list of things to a playlist.
    /// If the things are something that have songs (an album, artist, etc.), it will add all the songs.
    async fn playlist_add_list(
        playlist: PlaylistId,
        list: Vec<RecordId>,
    ) -> Result<(), SerializableLibraryError>;
    /// Get a playlist by its ID.
    async fn playlist_get(id: PlaylistId) -> Option<Playlist>;
    /// Get the songs of a playlist
    async fn playlist_get_songs(id: PlaylistId) -> Option<Box<[Song]>>;
    /// Rename a playlist.
    async fn playlist_rename(
        id: PlaylistId,
        name: String,
    ) -> Result<Playlist, SerializableLibraryError>;

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
        things: Vec<RecordId>,
        n: u32,
    ) -> Result<Box<[Song]>, SerializableLibraryError>;
    /// Radio: get the ids of the `n` most similar songs to the given things.
    async fn radio_get_similar_ids(
        things: Vec<RecordId>,
        n: u32,
    ) -> Result<Box<[SongId]>, SerializableLibraryError>;

    // Dynamic playlist commands
    /// Dynamic Playlists: create a new DP with the given name and query
    async fn dynamic_playlist_create(
        name: String,
        query: Query,
    ) -> Result<DynamicPlaylistId, SerializableLibraryError>;
    /// Dynamic Playlists: list all DPs
    async fn dynamic_playlist_list() -> Box<[DynamicPlaylist]>;
    /// Dynamic Playlists: update a DP
    async fn dynamic_playlist_update(
        id: DynamicPlaylistId,
        changes: DynamicPlaylistChangeSet,
    ) -> Result<DynamicPlaylist, SerializableLibraryError>;
    /// Dynamic Playlists: remove a DP
    async fn dynamic_playlist_remove(id: DynamicPlaylistId)
        -> Result<(), SerializableLibraryError>;
    /// Dynamic Playlists: get a DP by its ID
    async fn dynamic_playlist_get(id: DynamicPlaylistId) -> Option<DynamicPlaylist>;
    /// Dynamic Playlists: get the songs of a DP
    async fn dynamic_playlist_get_songs(id: DynamicPlaylistId) -> Option<Box<[Song]>>;
}

/// Initialize the music player client
///
/// # Errors
///
/// If the client cannot be initialized, an error is returned.
#[allow(clippy::missing_inline_in_public_items)]
pub async fn init_client(rpc_port: u16) -> Result<MusicPlayerClient, std::io::Error> {
    let server_addr = (IpAddr::V4(Ipv4Addr::LOCALHOST), rpc_port);

    let mut transport = tarpc::serde_transport::tcp::connect(server_addr, Json::default);
    transport.config_mut().max_frame_length(usize::MAX);

    // MusicPlayerClient is generated by the service attribute. It has a constructor `new` that takes a
    // config and any Transport as input.
    Ok(MusicPlayerClient::new(client::Config::default(), transport.await?).spawn())
}
