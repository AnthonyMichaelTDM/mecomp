//----------------------------------------------------------------------------------------- std lib
use std::{net::SocketAddr, ops::Range};
//--------------------------------------------------------------------------------- other libraries
use ::tarpc::context::Context;
use log::{info, warn};
use rand::seq::SliceRandom;
use tap::TapFallible;
//-------------------------------------------------------------------------------- MECOMP libraries
use mecomp_core::{
    audio::queue::Queue,
    errors::LibraryError,
    rpc::MusicPlayer,
    search::SearchResult,
    state::{
        library::{LibraryBrief, LibraryFull, LibraryHealth},
        Percent, RepeatMode, SeekType, StateAudio, StateRuntime,
    },
};
use mecomp_storage::db::schemas::{
    album::{Album, AlbumBrief, AlbumId},
    artist::{Artist, ArtistBrief, ArtistId},
    collection::{Collection, CollectionBrief, CollectionId},
    playlist::{Playlist, PlaylistBrief, PlaylistId},
    song::{Song, SongBrief, SongId},
};

use crate::{config::SETTINGS, services};

#[derive(Clone)]
pub struct MusicPlayerServer(pub SocketAddr);

impl MusicPlayer for MusicPlayerServer {
    async fn ping(self, _context: Context) -> String {
        "pong".to_string()
    }

    /// Rescans the music library.
    async fn library_rescan(self, _context: Context) -> Result<(), LibraryError> {
        info!("Rescanning library");
        Ok(services::library::rescan(
            &SETTINGS.library_paths,
            SETTINGS.artist_separator.as_deref(),
            SETTINGS.genre_separator.as_deref(),
            SETTINGS.conflict_resolution,
        )
        .await
        .tap_err(|e| warn!("Error in library_rescan: {e}"))?)
    }
    /// Returns brief information about the music library.
    async fn library_brief(self, _context: Context) -> Result<LibraryBrief, LibraryError> {
        info!("Creating library brief");
        Ok(services::library::brief()
            .await
            .tap_err(|e| warn!("Error in library_brief: {e}"))?)
    }
    /// Returns full information about the music library. (all songs, artists, albums, etc.)
    async fn library_full(self, _context: Context) -> Result<LibraryFull, LibraryError> {
        info!("Creating library full");
        Ok(services::library::full()
            .await
            .tap_err(|e| warn!("Error in library_full: {e}"))?)
    }
    /// Returns brief information about the music library's artists.
    async fn library_artists_brief(
        self,
        _context: Context,
    ) -> Result<Box<[ArtistBrief]>, LibraryError> {
        info!("Creating library artists brief");
        Ok(Artist::read_all()
            .await
            .tap_err(|e| warn!("Error in library_artists_brief: {e}"))?
            .iter()
            .map(std::convert::Into::into)
            .collect())
    }
    /// Returns full information about the music library's artists.
    async fn library_artists_full(self, _context: Context) -> Result<Box<[Artist]>, LibraryError> {
        info!("Creating library artists full");
        Ok(Artist::read_all()
            .await
            .map(std::vec::Vec::into_boxed_slice)
            .tap_err(|e| warn!("Error in library_artists_brief: {e}"))?)
    }
    /// Returns brief information about the music library's albums.
    async fn library_albums_brief(
        self,
        _context: Context,
    ) -> Result<Box<[AlbumBrief]>, LibraryError> {
        info!("Creating library albums brief");
        Ok(Album::read_all()
            .await
            .tap_err(|e| warn!("Error in library_albums_brief: {e}"))?
            .iter()
            .map(std::convert::Into::into)
            .collect())
    }
    /// Returns full information about the music library's albums.
    async fn library_albums_full(self, _context: Context) -> Result<Box<[Album]>, LibraryError> {
        info!("Creating library albums full");
        Ok(Album::read_all()
            .await
            .map(std::vec::Vec::into_boxed_slice)
            .tap_err(|e| warn!("Error in library_albums_full: {e}"))?)
    }
    /// Returns brief information about the music library's songs.
    async fn library_songs_brief(
        self,
        _context: Context,
    ) -> Result<Box<[SongBrief]>, LibraryError> {
        info!("Creating library songs brief");
        Ok(Song::read_all()
            .await
            .tap_err(|e| warn!("Error in library_songs_brief: {e}"))?
            .iter()
            .map(std::convert::Into::into)
            .collect())
    }
    /// Returns full information about the music library's songs.
    async fn library_songs_full(self, _context: Context) -> Result<Box<[Song]>, LibraryError> {
        info!("Creating library songs full");
        Ok(Song::read_all()
            .await
            .map(std::vec::Vec::into_boxed_slice)
            .tap_err(|e| warn!("Error in library_songs_full: {e}"))?)
    }
    /// Returns information about the health of the music library (are there any missing files, etc.)
    async fn library_health(self, _context: Context) -> LibraryHealth {
        info!("Creating library health");
        todo!()
    }
    /// Get a song by its ID.
    async fn library_song_get(self, _context: Context, id: SongId) -> Option<Song> {
        info!("Getting song by ID: {}", id);
        Song::read(id)
            .await
            .tap_err(|e| warn!("Error in library_song_get: {e}"))
            .ok()
            .flatten()
    }
    /// Get an album by its ID.
    async fn library_album_get(self, _context: Context, id: AlbumId) -> Option<Album> {
        info!("Getting album by ID: {}", id);
        Album::read(id)
            .await
            .tap_err(|e| warn!("Error in library_album_get: {e}"))
            .ok()
            .flatten()
    }
    /// Get an artist by its ID.
    async fn library_artist_get(self, _context: Context, id: ArtistId) -> Option<Artist> {
        info!("Getting artist by ID: {}", id);
        Artist::read(id)
            .await
            .tap_err(|e| warn!("Error in library_artist_get: {e}"))
            .ok()
            .flatten()
    }
    /// Get a playlist by its ID.
    async fn library_playlist_get(self, _context: Context, id: PlaylistId) -> Option<Playlist> {
        info!("Getting playlist by ID: {}", id);
        Playlist::read(id)
            .await
            .tap_err(|e| warn!("Error in library_playlist_get: {e}"))
            .ok()
            .flatten()
    }

    /// tells the daemon to shutdown.
    async fn daemon_shutdown(self, _context: Context) {
        info!("Shutting down daemon");
        todo!()
    }

    /// returns full information about the current state of the audio player (queue, current song, etc.)
    async fn state_audio(self, _context: Context) -> StateAudio {
        info!("Getting state of audio player");
        todo!()
    }
    /// returns information about the current queue.
    async fn state_queue(self, _context: Context) -> Queue {
        info!("Getting state of queue");
        todo!()
    }
    /// is the player currently playing?
    async fn state_playing(self, _context: Context) -> bool {
        info!("Getting state of playing");
        todo!()
    }
    /// what repeat mode is the player in?
    async fn state_repeat(self, _context: Context) -> RepeatMode {
        info!("Getting state of repeat");
        todo!()
    }
    /// returns the current volume.
    async fn state_volume(self, _context: Context) -> Percent {
        info!("Getting state of volume");
        todo!()
    }
    /// returns the current volume mute state.
    async fn state_volume_muted(self, _context: Context) -> bool {
        info!("Getting state of volume muted");
        todo!()
    }
    /// returns information about the runtime of the current song (seek position and duration)
    async fn state_runtime(self, _context: Context) -> StateRuntime {
        info!("Getting state of runtime");
        todo!()
    }
    /// returns the current artist.
    async fn current_artist(self, _context: Context) -> Option<Artist> {
        info!("Getting current artist");
        todo!()
    }
    /// returns the current album.
    async fn current_album(self, _context: Context) -> Option<Album> {
        info!("Getting current album");
        todo!()
    }
    /// returns the current song.
    async fn current_song(self, _context: Context) -> Option<Song> {
        info!("Getting current song");
        todo!()
    }

    /// returns a random artist.
    async fn rand_artist(self, _context: Context) -> Option<Artist> {
        info!("Getting random artist");
        Artist::read_all()
            .await
            .tap_err(|e| warn!("Error in rand_artist: {e}"))
            .ok()
            .and_then(|artists| artists.choose(&mut rand::thread_rng()).cloned())
    }
    /// returns a random album.
    async fn rand_album(self, _context: Context) -> Option<Album> {
        info!("Getting random album");
        Album::read_all()
            .await
            .tap_err(|e| warn!("Error in rand_album: {e}"))
            .ok()
            .and_then(|albums| albums.choose(&mut rand::thread_rng()).cloned())
    }
    /// returns a random song.
    async fn rand_song(self, _context: Context) -> Option<Song> {
        info!("Getting random song");
        Song::read_all()
            .await
            .tap_err(|e| warn!("Error in rand_song: {e}"))
            .ok()
            .and_then(|songs| songs.choose(&mut rand::thread_rng()).cloned())
    }

    /// returns a list of artists, albums, and songs matching the given search query.
    async fn search(self, _context: Context, query: String) -> Box<[SearchResult]> {
        info!("Searching for: {}", query);
        todo!()
    }
    /// returns a list of artists matching the given search query.
    async fn search_artist(self, _context: Context, query: String) -> Box<[Artist]> {
        info!("Searching for artist: {}", query);
        todo!()
    }
    /// returns a list of albums matching the given search query.
    async fn search_album(self, _context: Context, query: String) -> Box<[Album]> {
        info!("Searching for album: {}", query);
        todo!()
    }
    /// returns a list of songs matching the given search query.
    async fn search_song(self, _context: Context, query: String) -> Box<[Song]> {
        info!("Searching for song: {}", query);
        todo!()
    }

    /// toggles playback (play/pause).
    async fn playback_toggle(self, _context: Context) {
        info!("Toggling playback");
        todo!()
    }
    /// start playback (unpause).
    async fn playback_play(self, _context: Context) {
        info!("Starting playback");
        todo!()
    }
    /// pause playback.
    async fn playback_pause(self, _context: Context) {
        info!("Pausing playback");
        todo!()
    }
    /// set the current song to be the next song in the queue.
    async fn playback_next(self, _context: Context) {
        info!("Playing next song");
        todo!()
    }
    /// skip forward by the given amount of songs
    async fn playback_skip(self, _context: Context, amount: usize) {
        info!("Skipping forward by {} songs", amount);
        todo!()
    }
    /// go back to the previous song.
    /// (if the current song is more than `threshold` seconds in, it will restart the current song instead)
    async fn playback_previous(self, _context: Context, _threshold: Option<usize>) {
        info!("Playing previous song");
        todo!()
    }
    /// go backwards by the given amount of songs.
    async fn playback_back(self, _context: Context, amount: usize) {
        info!("Going back by {} songs", amount);
        todo!()
    }
    /// stop playback.
    /// (clears the queue and stops playback)
    async fn playback_stop(self, _context: Context) {
        info!("Stopping playback");
        todo!()
    }
    /// clear the queue.
    async fn playback_clear(self, _context: Context) {
        info!("Clearing queue");
        todo!()
    }
    /// seek forwards, backwards, or to an absolute second in the current song.
    async fn playback_seek(self, _context: Context, seek: SeekType, seconds: u64) {
        info!("Seeking {} seconds ({})", seconds, seek);
        todo!()
    }
    /// set the repeat mode.
    async fn playback_repeat(self, _context: Context, mode: RepeatMode) {
        info!("Setting repeat mode to: {}", mode);
        todo!()
    }
    /// Shuffle the current queue, then start playing from the 1st Song in the queue.
    async fn playback_shuffle(self, _context: Context) {
        info!("Shuffling queue");
        todo!()
    }
    /// set the volume to the given value (0-100).
    /// (if the value is greater than 100, it will be clamped to 100.)
    async fn playback_volume(self, _context: Context, volume: u8) {
        info!("Setting volume to: {}", volume);
        todo!()
    }
    /// increase the volume by the given amount (0-100).
    /// (volume will be clamped to 100.)
    async fn playback_volume_up(self, _context: Context, amount: u8) {
        info!("Increasing volume by: {}", amount);
        todo!()
    }
    /// decrease the volume by the given amount (0-100).
    /// (volume will be clamped to 0.)
    async fn playback_volume_down(self, _context: Context, amount: u8) {
        info!("Decreasing volume by: {}", amount);
        todo!()
    }
    /// toggle the volume mute.
    async fn playback_volume_toggle_mute(self, _context: Context) {
        info!("Toggling volume mute");
        todo!()
    }
    /// mute the volume.
    async fn playback_mute(self, _context: Context) {
        info!("Muting volume");
        todo!()
    }
    /// unmute the volume.
    async fn playback_unmute(self, _context: Context) {
        info!("Unmuting volume");
        todo!()
    }

    /// add a song to the queue.
    /// (if the queue is empty, it will start playing the song.)
    async fn queue_add_song(self, _context: Context, song: SongId) {
        info!("Adding song to queue: {}", song);
        todo!()
    }
    /// add an album to the queue.
    /// (if the queue is empty, it will start playing the album.)
    async fn queue_add_album(self, _context: Context, album: AlbumId) {
        info!("Adding album to queue: {}", album);
        todo!()
    }
    /// add an artist to the queue.
    /// (if the queue is empty, it will start playing the artist.)
    async fn queue_add_artist(self, _context: Context, artist: ArtistId) {
        info!("Adding artist to queue: {}", artist);
        todo!()
    }
    /// add a playlist to the queue.
    /// (if the queue is empty, it will start playing the playlist.)
    async fn queue_add_playlist(self, _context: Context, playlist: PlaylistId) {
        info!("Adding playlist to queue: {}", playlist);
        todo!()
    }
    /// add a collection to the queue.
    /// (if the queue is empty, it will start playing the collection.)
    async fn queue_add_collection(self, _context: Context, collection: CollectionId) {
        info!("Adding collection to queue: {}", collection);
        todo!()
    }
    /// add a random song to the queue.
    /// (if the queue is empty, it will start playing the song.)
    async fn queue_add_rand_song(self, _context: Context) {
        info!("Adding random song to queue");
        todo!()
    }
    /// add a random album to the queue.
    /// (if the queue is empty, it will start playing the album.)
    async fn queue_add_rand_album(self, _context: Context) {
        info!("Adding random album to queue");
        todo!()
    }
    /// add a random artist to the queue.
    /// (if the queue is empty, it will start playing the artist.)
    async fn queue_add_rand_artist(self, _context: Context) {
        info!("Adding random artist to queue");
        todo!()
    }
    /// set the current song to a queue index.
    /// if the index is out of bounds, it will be clamped to the nearest valid index.
    async fn queue_set_index(self, _context: Context, index: usize) {
        info!("Setting queue index to: {}", index);
        todo!()
    }
    /// remove a range of songs from the queue.
    /// if the range is out of bounds, it will be clamped to the nearest valid range.
    async fn queue_remove_range(self, _context: Context, range: Range<usize>) {
        info!("Removing queue range: {:?}", range);
        todo!()
    }

    /// Returns brief information about the users playlists.
    async fn playlist_list(self, _context: Context) -> Box<[PlaylistBrief]> {
        info!("Listing playlists");
        Playlist::read_all()
            .await
            .tap_err(|e| warn!("Error in playlist_list: {e}"))
            .ok()
            .map(|playlists| playlists.iter().map(std::convert::Into::into).collect())
            .unwrap_or_default()
    }
    /// create a new playlist.
    async fn playlist_new(self, _context: Context, name: String) -> PlaylistId {
        info!("Creating new playlist: {}", name);
        todo!()
    }
    /// remove a playlist.
    async fn playlist_remove(self, _context: Context, name: String) -> bool {
        info!("Removing playlist: {}", name);
        todo!()
    }
    /// clone a playlist.
    /// (creates a new playlist with the same name (append "copy") and contents as the given playlist.)
    async fn playlist_clone(self, _context: Context, name: String) {
        info!("Cloning playlist: {}", name);
        todo!()
    }
    /// get the id of a playlist.
    async fn playlist_get_id(self, _context: Context, name: String) -> Option<PlaylistId> {
        info!("Getting playlist ID: {}", name);
        todo!()
    }
    /// remove a song from a playlist.
    /// if the song is not in the playlist, this will do nothing.
    async fn playlist_remove_song(self, _context: Context, playlist: PlaylistId, song: SongId) {
        info!("Removing song from playlist: {} ({})", playlist, song);
        todo!()
    }
    /// Add an artist to a playlist.
    async fn playlist_add_artist(self, _context: Context, playlist: PlaylistId, artist: ArtistId) {
        info!("Adding artist to playlist: {} ({})", playlist, artist);
        todo!()
    }
    /// Add an album to a playlist.
    async fn playlist_add_album(self, _context: Context, playlist: PlaylistId, album: AlbumId) {
        info!("Adding album to playlist: {} ({})", playlist, album);
        todo!()
    }
    /// Add a song to a playlist.
    async fn playlist_add_song(self, _context: Context, playlist: PlaylistId, song: SongId) {
        info!("Adding song to playlist: {} ({})", playlist, song);
        todo!()
    }
    /// Get a playlist by its ID.
    async fn playlist_get(self, _context: Context, id: PlaylistId) -> Option<Playlist> {
        info!("Getting playlist by ID: {}", id);
        todo!()
    }

    /// Collections: Return brief information about the users auto curration collections.
    async fn collection_list(self, _context: Context) -> Box<[CollectionBrief]> {
        info!("Listing collections");
        Collection::read_all()
            .await
            .tap_err(|e| warn!("Error in collection_list: {e}"))
            .ok()
            .map(|collections| collections.iter().map(std::convert::Into::into).collect())
            .unwrap_or_default()
    }
    /// Collections: Recluster the users library, creating new collections.
    async fn collection_recluster(self, _context: Context) {
        info!("Reclustering collections");
        todo!()
    }
    /// Collections: get a collection by its ID.
    async fn collection_get(self, _context: Context, id: CollectionId) -> Option<Collection> {
        info!("Getting collection by ID: {}", id);
        todo!()
    }
    /// Collections: freeze a collection (convert it to a playlist).
    async fn collection_freeze(
        self,
        _context: Context,
        id: CollectionId,
        name: String,
    ) -> PlaylistId {
        info!("Freezing collection: {} ({})", id, name);
        todo!()
    }

    /// Radio: get the `n` most similar songs to the given song.
    async fn radio_get_similar_songs(
        self,
        _context: Context,
        song: SongId,
        n: usize,
    ) -> Box<[SongId]> {
        info!("Getting the {} most similar songs to: {}", n, song);
        todo!()
    }
    /// Radio: get the `n` most similar artists to the given artist.
    async fn radio_get_similar_artist(
        self,
        _context: Context,
        artist: ArtistId,
        n: usize,
    ) -> Box<[ArtistId]> {
        info!("Getting the {} most similar artists to: {}", n, artist);
        todo!()
    }
    /// Radio: get the `n` most similar albums to the given album.
    async fn radio_get_similar_album(
        self,
        _context: Context,
        album: AlbumId,
        n: usize,
    ) -> Box<[AlbumId]> {
        info!("Getting the {} most similar albums to: {}", n, album);
        todo!()
    }
}
