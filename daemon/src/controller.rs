//----------------------------------------------------------------------------------------- std lib
use std::{net::SocketAddr, ops::Range, sync::Arc};
use surrealdb::{engine::local::Db, Surreal};
//--------------------------------------------------------------------------------- other libraries
use ::tarpc::context::Context;
use log::{info, warn};
use rand::seq::SliceRandom;
use tap::TapFallible;
//-------------------------------------------------------------------------------- MECOMP libraries
use mecomp_core::{
    audio::{AudioCommand, VolumeCommand, AUDIO_KERNEL},
    errors::SerializableLibraryError,
    rpc::MusicPlayer,
    search::SearchResult,
    state::{
        library::{LibraryBrief, LibraryFull, LibraryHealth},
        RepeatMode, SeekType, StateAudio, StateRuntime,
    },
};
use mecomp_storage::{
    db::schemas::{
        album::{Album, AlbumBrief, AlbumId},
        artist::{Artist, ArtistBrief, ArtistId},
        collection::{Collection, CollectionBrief, CollectionId},
        playlist::{Playlist, PlaylistBrief, PlaylistId},
        song::{Song, SongBrief, SongId},
    },
    errors::Error::{self, NotFound},
    util::OneOrMany,
};
use tracing::{instrument, Instrument};

use crate::{config::SETTINGS, services};

#[derive(Clone, Debug)]
pub struct MusicPlayerServer {
    pub addr: SocketAddr,
    db: Arc<Surreal<Db>>,
}

impl MusicPlayerServer {
    #[must_use]
    pub fn new(addr: SocketAddr, db: Arc<Surreal<Db>>) -> Self {
        Self { addr, db }
    }
}

impl MusicPlayer for MusicPlayerServer {
    #[instrument]
    async fn ping(self, _context: Context) -> String {
        "pong".to_string()
    }

    /// Rescans the music library.
    #[instrument]
    async fn library_rescan(self, _context: Context) -> Result<(), SerializableLibraryError> {
        info!("Rescanning library");
        Ok(services::library::rescan(
            &self.db,
            &SETTINGS.library_paths,
            SETTINGS.artist_separator.as_deref(),
            SETTINGS.genre_separator.as_deref(),
            SETTINGS.conflict_resolution,
        )
        .await
        .tap_err(|e| warn!("Error in library_rescan: {e}"))?)
    }
    /// Returns brief information about the music library.
    #[instrument]
    async fn library_brief(
        self,
        _context: Context,
    ) -> Result<LibraryBrief, SerializableLibraryError> {
        info!("Creating library brief");
        Ok(services::library::brief(&self.db)
            .await
            .tap_err(|e| warn!("Error in library_brief: {e}"))?)
    }
    /// Returns full information about the music library. (all songs, artists, albums, etc.)
    #[instrument]
    async fn library_full(
        self,
        _context: Context,
    ) -> Result<LibraryFull, SerializableLibraryError> {
        info!("Creating library full");
        Ok(services::library::full(&self.db)
            .await
            .tap_err(|e| warn!("Error in library_full: {e}"))?)
    }
    /// Returns brief information about the music library's artists.
    #[instrument]
    async fn library_artists_brief(
        self,
        _context: Context,
    ) -> Result<Box<[ArtistBrief]>, SerializableLibraryError> {
        info!("Creating library artists brief");
        Ok(Artist::read_all(&self.db)
            .await
            .tap_err(|e| warn!("Error in library_artists_brief: {e}"))?
            .iter()
            .map(std::convert::Into::into)
            .collect())
    }
    /// Returns full information about the music library's artists.
    #[instrument]
    async fn library_artists_full(
        self,
        _context: Context,
    ) -> Result<Box<[Artist]>, SerializableLibraryError> {
        info!("Creating library artists full");
        Ok(Artist::read_all(&self.db)
            .await
            .map(std::vec::Vec::into_boxed_slice)
            .tap_err(|e| warn!("Error in library_artists_brief: {e}"))?)
    }
    /// Returns brief information about the music library's albums.
    #[instrument]
    async fn library_albums_brief(
        self,
        _context: Context,
    ) -> Result<Box<[AlbumBrief]>, SerializableLibraryError> {
        info!("Creating library albums brief");
        Ok(Album::read_all(&self.db)
            .await
            .tap_err(|e| warn!("Error in library_albums_brief: {e}"))?
            .iter()
            .map(std::convert::Into::into)
            .collect())
    }
    /// Returns full information about the music library's albums.
    #[instrument]
    async fn library_albums_full(
        self,
        _context: Context,
    ) -> Result<Box<[Album]>, SerializableLibraryError> {
        info!("Creating library albums full");
        Ok(Album::read_all(&self.db)
            .await
            .map(std::vec::Vec::into_boxed_slice)
            .tap_err(|e| warn!("Error in library_albums_full: {e}"))?)
    }
    /// Returns brief information about the music library's songs.
    #[instrument]
    async fn library_songs_brief(
        self,
        _context: Context,
    ) -> Result<Box<[SongBrief]>, SerializableLibraryError> {
        info!("Creating library songs brief");
        Ok(Song::read_all(&self.db)
            .await
            .tap_err(|e| warn!("Error in library_songs_brief: {e}"))?
            .iter()
            .map(std::convert::Into::into)
            .collect())
    }
    /// Returns full information about the music library's songs.
    #[instrument]
    async fn library_songs_full(
        self,
        _context: Context,
    ) -> Result<Box<[Song]>, SerializableLibraryError> {
        info!("Creating library songs full");
        Ok(Song::read_all(&self.db)
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
    #[instrument]
    async fn library_song_get(self, _context: Context, id: SongId) -> Option<Song> {
        info!("Getting song by ID: {}", id);
        Song::read(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in library_song_get: {e}"))
            .ok()
            .flatten()
    }
    /// Get an album by its ID.
    #[instrument]
    async fn library_album_get(self, _context: Context, id: AlbumId) -> Option<Album> {
        info!("Getting album by ID: {}", id);
        Album::read(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in library_album_get: {e}"))
            .ok()
            .flatten()
    }
    /// Get an artist by its ID.
    #[instrument]
    async fn library_artist_get(self, _context: Context, id: ArtistId) -> Option<Artist> {
        info!("Getting artist by ID: {}", id);
        Artist::read(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in library_artist_get: {e}"))
            .ok()
            .flatten()
    }
    /// Get a playlist by its ID.
    #[instrument]
    async fn library_playlist_get(self, _context: Context, id: PlaylistId) -> Option<Playlist> {
        info!("Getting playlist by ID: {}", id);
        Playlist::read(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in library_playlist_get: {e}"))
            .ok()
            .flatten()
    }

    /// tells the daemon to shutdown.
    #[instrument]
    async fn daemon_shutdown(self, _context: Context) {
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(1));
            AUDIO_KERNEL.send(AudioCommand::Exit);
            std::process::exit(0);
        });
        info!("Shutting down daemon in 1 second");
    }

    /// returns full information about the current state of the audio player (queue, current song, etc.)
    #[instrument]
    async fn state_audio(self, _context: Context) -> Option<StateAudio> {
        info!("Getting state of audio player");
        let (tx, rx) = tokio::sync::oneshot::channel();

        AUDIO_KERNEL.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in state_audio: {e}"))
            .ok()
    }
    /// returns information about the current queue.
    #[instrument]
    async fn state_queue(self, _context: Context) -> Option<Box<[Song]>> {
        info!("Getting state of queue");
        let (tx, rx) = tokio::sync::oneshot::channel();

        AUDIO_KERNEL.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in state_queue: {e}"))
            .ok()
            .map(|state| state.queue)
    }
    /// returns the current queue position.
    #[instrument]
    async fn state_queue_position(self, _context: Context) -> Option<usize> {
        info!("Getting state of queue position");
        let (tx, rx) = tokio::sync::oneshot::channel();

        AUDIO_KERNEL.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in state_queue_position: {e}"))
            .ok()
            .and_then(|state| state.queue_position)
    }
    /// is the player currently paused?
    #[instrument]
    async fn state_paused(self, _context: Context) -> bool {
        info!("Getting state of playing");
        let (tx, rx) = tokio::sync::oneshot::channel();

        AUDIO_KERNEL.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in state_playing: {e}"))
            .ok()
            .is_some_and(|state| state.paused)
    }
    /// what repeat mode is the player in?
    #[instrument]
    async fn state_repeat(self, _context: Context) -> Option<RepeatMode> {
        info!("Getting state of repeat");
        let (tx, rx) = tokio::sync::oneshot::channel();

        AUDIO_KERNEL.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in state_repeat: {e}"))
            .ok()
            .map(|state| state.repeat_mode)
    }
    /// returns the current volume.
    #[instrument]
    async fn state_volume(self, _context: Context) -> Option<f32> {
        info!("Getting state of volume");
        let (tx, rx) = tokio::sync::oneshot::channel();

        AUDIO_KERNEL.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in state_volume: {e}"))
            .ok()
            .map(|state| state.volume)
    }
    /// returns the current volume mute state.
    #[instrument]
    async fn state_volume_muted(self, _context: Context) -> bool {
        info!("Getting state of volume muted");
        let (tx, rx) = tokio::sync::oneshot::channel();

        AUDIO_KERNEL.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in state_volume_muted: {e}"))
            .ok()
            .is_some_and(|state| state.muted)
    }
    /// returns information about the runtime of the current song (seek position and duration)
    #[instrument]
    async fn state_runtime(self, _context: Context) -> Option<StateRuntime> {
        info!("Getting state of runtime");
        let (tx, rx) = tokio::sync::oneshot::channel();

        AUDIO_KERNEL.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in state_runtime: {e}"))
            .ok()
            .and_then(|state| state.runtime)
    }

    /// returns the current artist.
    #[instrument]
    async fn current_artist(self, _context: Context) -> Option<OneOrMany<Artist>> {
        info!("Getting current artist");
        let (tx, rx) = tokio::sync::oneshot::channel();

        AUDIO_KERNEL.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in current_artist: {e}"))
            .ok()
            .map(|state| state.current_artist)
    }
    /// returns the current album.
    #[instrument]
    async fn current_album(self, _context: Context) -> Option<Album> {
        info!("Getting current album");
        let (tx, rx) = tokio::sync::oneshot::channel();

        AUDIO_KERNEL.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in current_album: {e}"))
            .ok()
            .and_then(|state| state.current_album)
    }
    /// returns the current song.
    #[instrument]
    async fn current_song(self, _context: Context) -> Option<Song> {
        info!("Getting current song");
        let (tx, rx) = tokio::sync::oneshot::channel();

        AUDIO_KERNEL.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in current_song: {e}"))
            .ok()
            .and_then(|state| state.current_song)
    }

    /// returns a random artist.
    #[instrument]
    async fn rand_artist(self, _context: Context) -> Option<Artist> {
        info!("Getting random artist");
        Artist::read_all(&self.db)
            .await
            .tap_err(|e| warn!("Error in rand_artist: {e}"))
            .ok()
            .and_then(|artists| artists.choose(&mut rand::thread_rng()).cloned())
    }
    /// returns a random album.
    #[instrument]
    async fn rand_album(self, _context: Context) -> Option<Album> {
        info!("Getting random album");
        Album::read_all(&self.db)
            .await
            .tap_err(|e| warn!("Error in rand_album: {e}"))
            .ok()
            .and_then(|albums| albums.choose(&mut rand::thread_rng()).cloned())
    }
    /// returns a random song.
    #[instrument]
    async fn rand_song(self, _context: Context) -> Option<Song> {
        info!("Getting random song");
        Song::read_all(&self.db)
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
    #[instrument]
    async fn playback_toggle(self, _context: Context) {
        info!("Toggling playback");
        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::TogglePlayback);
            }
            .in_current_span(),
        )
        .await
        .unwrap();
    }
    /// start playback (unpause).
    #[instrument]
    async fn playback_play(self, _context: Context) {
        info!("Starting playback");
        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::Play);
            }
            .in_current_span(),
        )
        .await
        .unwrap();
    }
    /// pause playback.
    #[instrument]
    async fn playback_pause(self, _context: Context) {
        info!("Pausing playback");
        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::Pause);
            }
            .in_current_span(),
        )
        .await
        .unwrap();
    }
    /// set the current song to be the next song in the queue.
    #[instrument]
    async fn playback_next(self, _context: Context) {
        info!("Playing next song");
        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::SkipForward(1));
            }
            .in_current_span(),
        )
        .await
        .unwrap();
    }
    /// restart the current song.
    #[instrument]
    async fn playback_restart(self, _context: Context) {
        info!("Restarting current song");
        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::RestartSong);
            }
            .in_current_span(),
        )
        .await
        .unwrap();
    }
    /// skip forward by the given amount of songs
    #[instrument]
    async fn playback_skip_forward(self, _context: Context, amount: usize) {
        info!("Skipping forward by {} songs", amount);
        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::SkipForward(amount));
            }
            .in_current_span(),
        )
        .await
        .unwrap();
    }
    /// go backwards by the given amount of songs.
    async fn playback_skip_backward(self, _context: Context, amount: usize) {
        info!("Going back by {} songs", amount);
        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::SkipBackward(amount));
            }
            .in_current_span(),
        )
        .await
        .unwrap();
    }
    /// stop playback.
    /// (clears the queue and stops playback)
    async fn playback_clear_player(self, _context: Context) {
        info!("Stopping playback");
        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::ClearPlayer);
            }
            .in_current_span(),
        )
        .await
        .unwrap();
    }
    /// clear the queue.
    async fn playback_clear(self, _context: Context) {
        info!("Clearing queue and stopping playback");
        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::Clear);
            }
            .in_current_span(),
        )
        .await
        .unwrap();
    }
    /// seek forwards, backwards, or to an absolute second in the current song.
    async fn playback_seek(self, _context: Context, seek: SeekType, seconds: u64) {
        info!("Seeking {} seconds ({})", seconds, seek);
        todo!()
    }
    /// set the repeat mode.
    #[instrument]
    async fn playback_repeat(self, _context: Context, mode: RepeatMode) {
        info!("Setting repeat mode to: {}", mode);
        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::SetRepeatMode(mode));
            }
            .in_current_span(),
        )
        .await
        .unwrap();
    }
    /// Shuffle the current queue, then start playing from the 1st Song in the queue.
    async fn playback_shuffle(self, _context: Context) {
        info!("Shuffling queue");
        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::ShuffleQueue);
            }
            .in_current_span(),
        )
        .await
        .unwrap();
    }
    /// set the volume to the given value
    /// The value `1.0` is the "normal" volume (unfiltered input). Any value other than `1.0` will multiply each sample by this value.
    async fn playback_volume(self, _context: Context, volume: f32) {
        info!("Setting volume to: {}", volume);
        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::Volume(VolumeCommand::Set(volume)));
            }
            .in_current_span(),
        )
        .await
        .unwrap();
    }
    /// increase the volume by the given amount
    async fn playback_volume_up(self, _context: Context, amount: f32) {
        info!("Increasing volume by: {}", amount);
        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::Volume(VolumeCommand::Up(amount)));
            }
            .in_current_span(),
        )
        .await
        .unwrap();
    }
    /// decrease the volume by the given amount
    async fn playback_volume_down(self, _context: Context, amount: f32) {
        info!("Decreasing volume by: {}", amount);
        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::Volume(VolumeCommand::Down(amount)));
            }
            .in_current_span(),
        )
        .await
        .unwrap();
    }
    /// toggle the volume mute.
    async fn playback_volume_toggle_mute(self, _context: Context) {
        info!("Toggling volume mute");
        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::Volume(VolumeCommand::ToggleMute));
            }
            .in_current_span(),
        )
        .await
        .unwrap();
    }
    /// mute the volume.
    async fn playback_mute(self, _context: Context) {
        info!("Muting volume");
        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::Volume(VolumeCommand::Mute));
            }
            .in_current_span(),
        )
        .await
        .unwrap();
    }
    /// unmute the volume.
    async fn playback_unmute(self, _context: Context) {
        info!("Unmuting volume");
        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::Volume(VolumeCommand::Unmute));
            }
            .in_current_span(),
        )
        .await
        .unwrap();
    }

    /// add a song to the queue.
    /// (if the queue is empty, it will start playing the song.)
    #[instrument]
    async fn queue_add_song(
        self,
        _context: Context,
        song: SongId,
    ) -> Result<(), SerializableLibraryError> {
        info!("Adding song to queue: {}", song);
        let Some(song) = Song::read(&self.db, song).await? else {
            return Err(Error::NotFound.into());
        };

        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::AddToQueue(OneOrMany::One(song)));
            }
            .in_current_span(),
        )
        .await
        .unwrap();

        Ok(())
    }
    /// add an album to the queue.
    /// (if the queue is empty, it will start playing the album.)
    async fn queue_add_album(
        self,
        _context: Context,
        album: AlbumId,
    ) -> Result<(), SerializableLibraryError> {
        info!("Adding album to queue: {}", album);

        let songs = Album::read_songs(&self.db, album).await?;

        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::AddToQueue(songs.into()));
            }
            .in_current_span(),
        )
        .await
        .unwrap();

        Ok(())
    }
    /// add an artist to the queue.
    /// (if the queue is empty, it will start playing the artist.)
    async fn queue_add_artist(
        self,
        _context: Context,
        artist: ArtistId,
    ) -> Result<(), SerializableLibraryError> {
        info!("Adding artist to queue: {}", artist);

        let songs = Artist::read_songs(&self.db, artist).await?;

        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::AddToQueue(songs.into()));
            }
            .in_current_span(),
        )
        .await
        .unwrap();

        Ok(())
    }
    /// add a playlist to the queue.
    /// (if the queue is empty, it will start playing the playlist.)
    async fn queue_add_playlist(
        self,
        _context: Context,
        playlist: PlaylistId,
    ) -> Result<(), SerializableLibraryError> {
        info!("Adding playlist to queue: {}", playlist);

        let songs = Playlist::read_songs(&self.db, playlist).await?;

        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::AddToQueue(songs.into()));
            }
            .in_current_span(),
        )
        .await
        .unwrap();

        Ok(())
    }
    /// add a collection to the queue.
    /// (if the queue is empty, it will start playing the collection.)
    async fn queue_add_collection(
        self,
        _context: Context,
        collection: CollectionId,
    ) -> Result<(), SerializableLibraryError> {
        info!("Adding collection to queue: {}", collection);

        let songs = Collection::read_songs(&self.db, collection).await?;

        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::AddToQueue(songs.into()));
            }
            .in_current_span(),
        )
        .await
        .unwrap();

        Ok(())
    }
    /// add a random song to the queue.
    /// (if the queue is empty, it will start playing the song.)
    #[instrument]
    async fn queue_add_rand_song(self, _context: Context) -> Result<(), SerializableLibraryError> {
        info!("Adding random song to queue");
        let song = Song::read_all(&self.db)
            .await
            .tap_err(|e| warn!("Error in rand_song: {e}"))
            .ok()
            .and_then(|songs| songs.choose(&mut rand::thread_rng()).cloned())
            .ok_or(NotFound)?;

        tokio::spawn(
            async move {
                AUDIO_KERNEL.send(AudioCommand::AddToQueue(OneOrMany::One(song)));
            }
            .in_current_span(),
        )
        .await
        .unwrap();

        Ok(())
    }
    /// add a random album to the queue.
    /// (if the queue is empty, it will start playing the album.)
    async fn queue_add_rand_album(self, _context: Context) -> Result<(), SerializableLibraryError> {
        info!("Adding random album to queue");
        todo!()
    }
    /// add a random artist to the queue.
    /// (if the queue is empty, it will start playing the artist.)
    async fn queue_add_rand_artist(
        self,
        _context: Context,
    ) -> Result<(), SerializableLibraryError> {
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
    #[instrument]
    async fn playlist_list(self, _context: Context) -> Box<[PlaylistBrief]> {
        info!("Listing playlists");
        Playlist::read_all(&self.db)
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
    #[instrument]
    async fn collection_list(self, _context: Context) -> Box<[CollectionBrief]> {
        info!("Listing collections");
        Collection::read_all(&self.db)
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
