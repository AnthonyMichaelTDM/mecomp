//----------------------------------------------------------------------------------------- std lib
use std::{ops::Range, sync::Arc, time::Duration};
//--------------------------------------------------------------------------------- other libraries
use ::tarpc::context::Context;
use log::{debug, error, info, warn};
use rand::seq::SliceRandom;
use surrealdb::{engine::local::Db, Surreal};
use tap::TapFallible;
use tracing::instrument;
//-------------------------------------------------------------------------------- MECOMP libraries
use mecomp_core::{
    audio::{
        commands::{AudioCommand, QueueCommand, VolumeCommand},
        AudioKernelSender,
    },
    errors::SerializableLibraryError,
    rpc::{AlbumId, ArtistId, CollectionId, MusicPlayer, PlaylistId, SearchResult, SongId},
    state::{
        library::{LibraryBrief, LibraryFull, LibraryHealth},
        RepeatMode, SeekType, StateAudio, StateRuntime,
    },
};
use mecomp_storage::{
    db::schemas::{
        self,
        album::{Album, AlbumBrief},
        artist::{Artist, ArtistBrief},
        collection::{Collection, CollectionBrief},
        playlist::{Playlist, PlaylistBrief},
        song::{Song, SongBrief},
    },
    errors::Error::{self, NotFound},
};
use one_or_many::OneOrMany;

use crate::{
    config::Settings,
    services::{self, get_songs_from_things},
};

mod locks {
    use tokio::sync::Mutex;

    pub static LIBRARY_RESCAN_LOCK: Mutex<()> = Mutex::const_new(());
    pub static LIBRARY_ANALYZE_LOCK: Mutex<()> = Mutex::const_new(());
    pub static COLLECTION_RECLUSTER_LOCK: Mutex<()> = Mutex::const_new(());
}

#[derive(Clone, Debug)]
pub struct MusicPlayerServer {
    db: Arc<Surreal<Db>>,
    settings: Arc<Settings>,
    audio_kernel: Arc<AudioKernelSender>,
}

impl MusicPlayerServer {
    #[must_use]
    pub fn new(
        db: Arc<Surreal<Db>>,
        settings: Arc<Settings>,
        audio_kernel: Arc<AudioKernelSender>,
    ) -> Self {
        Self {
            db,
            settings,
            audio_kernel,
        }
    }
}

impl MusicPlayer for MusicPlayerServer {
    #[instrument]
    async fn ping(self, context: Context) -> String {
        "pong".to_string()
    }

    /// Rescans the music library, only error is if a rescan is already in progress.
    #[instrument]
    async fn library_rescan(self, context: Context) -> Result<(), SerializableLibraryError> {
        info!("Rescanning library");

        if locks::LIBRARY_RESCAN_LOCK.try_lock().is_err() {
            warn!("Library rescan already in progress");
            return Err(SerializableLibraryError::RescanInProgress);
        }

        std::thread::Builder::new()
            .name(String::from("Library Rescan"))
            .spawn(move || {
                futures::executor::block_on(async {
                    let _guard = locks::LIBRARY_RESCAN_LOCK.lock().await;
                    match services::library::rescan(
                        &self.db,
                        &self.settings.daemon.library_paths,
                        &self.settings.daemon.artist_separator,
                        self.settings.daemon.genre_separator.as_deref(),
                        self.settings.daemon.conflict_resolution,
                    )
                    .await
                    {
                        Ok(()) => info!("Library rescan complete"),
                        Err(e) => error!("Error in library_rescan: {e}"),
                    }
                });
            })?;

        Ok(())
    }
    /// Check if a rescan is in progress.
    #[instrument]
    async fn library_rescan_in_progress(self, context: Context) -> bool {
        locks::LIBRARY_RESCAN_LOCK.try_lock().is_err()
    }
    /// Analyze the music library, only error is if an analysis is already in progress.
    #[instrument]
    async fn library_analyze(self, context: Context) -> Result<(), SerializableLibraryError> {
        #[cfg(not(feature = "analysis"))]
        {
            warn!("Analysis is not enabled");
            return Err(SerializableLibraryError::AnalysisNotEnabled);
        }

        #[cfg(feature = "analysis")]
        {
            info!("Analyzing library");

            if locks::LIBRARY_ANALYZE_LOCK.try_lock().is_err() {
                warn!("Library analysis already in progress");
                return Err(SerializableLibraryError::AnalysisInProgress);
            }

            std::thread::Builder::new()
                .name(String::from("Library Analysis"))
                .spawn(move || {
                    futures::executor::block_on(async {
                        let _guard = locks::LIBRARY_ANALYZE_LOCK.lock().await;
                        match services::library::analyze(&self.db).await {
                            Ok(()) => info!("Library analysis complete"),
                            Err(e) => error!("Error in library_analyze: {e}"),
                        }
                    });
                })?;

            Ok(())
        }
    }
    /// Check if an analysis is in progress.
    #[instrument]
    async fn library_analyze_in_progress(self, context: Context) -> bool {
        locks::LIBRARY_ANALYZE_LOCK.try_lock().is_err()
    }
    /// Recluster the music library, only error is if a recluster is already in progress.
    #[instrument]
    async fn library_recluster(self, context: Context) -> Result<(), SerializableLibraryError> {
        #[cfg(not(feature = "analysis"))]
        {
            warn!("Analysis is not enabled");
            return Err(SerializableLibraryError::AnalysisNotEnabled);
        }

        #[cfg(feature = "analysis")]
        {
            info!("Reclustering collections");

            if locks::COLLECTION_RECLUSTER_LOCK.try_lock().is_err() {
                warn!("Collection reclustering already in progress");
                return Err(SerializableLibraryError::ReclusterInProgress);
            }

            std::thread::Builder::new()
                .name(String::from("Collection Recluster"))
                .spawn(move || {
                    futures::executor::block_on(async {
                        let _guard = locks::COLLECTION_RECLUSTER_LOCK.lock().await;
                        match services::library::recluster(&self.db, &self.settings.reclustering)
                            .await
                        {
                            Ok(()) => info!("Collection reclustering complete"),
                            Err(e) => error!("Error in collection_recluster: {e}"),
                        }
                    });
                })?;

            Ok(())
        }
    }
    /// Check if a recluster is in progress.
    #[instrument]
    async fn library_recluster_in_progress(self, context: Context) -> bool {
        locks::COLLECTION_RECLUSTER_LOCK.try_lock().is_err()
    }
    /// Returns brief information about the music library.
    #[instrument]
    async fn library_brief(
        self,
        context: Context,
    ) -> Result<LibraryBrief, SerializableLibraryError> {
        info!("Creating library brief");
        Ok(services::library::brief(&self.db)
            .await
            .tap_err(|e| warn!("Error in library_brief: {e}"))?)
    }
    /// Returns full information about the music library. (all songs, artists, albums, etc.)
    #[instrument]
    async fn library_full(self, context: Context) -> Result<LibraryFull, SerializableLibraryError> {
        info!("Creating library full");
        Ok(services::library::full(&self.db)
            .await
            .tap_err(|e| warn!("Error in library_full: {e}"))?)
    }
    /// Returns brief information about the music library's artists.
    #[instrument]
    async fn library_artists_brief(
        self,
        context: Context,
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
        context: Context,
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
        context: Context,
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
        context: Context,
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
        context: Context,
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
        context: Context,
    ) -> Result<Box<[Song]>, SerializableLibraryError> {
        info!("Creating library songs full");
        Ok(Song::read_all(&self.db)
            .await
            .map(std::vec::Vec::into_boxed_slice)
            .tap_err(|e| warn!("Error in library_songs_full: {e}"))?)
    }
    /// Returns information about the health of the music library (are there any missing files, etc.)
    #[instrument]
    async fn library_health(
        self,
        context: Context,
    ) -> Result<LibraryHealth, SerializableLibraryError> {
        info!("Creating library health");
        Ok(services::library::health(&self.db)
            .await
            .tap_err(|e| warn!("Error in library_health: {e}"))?)
    }
    /// Get a song by its ID.
    #[instrument]
    async fn library_song_get(self, context: Context, id: SongId) -> Option<Song> {
        let id = id.into();
        info!("Getting song by ID: {id}");
        Song::read(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in library_song_get: {e}"))
            .ok()
            .flatten()
    }
    /// Get the artists of a song.
    #[instrument]
    async fn library_song_get_artist(self, context: Context, id: SongId) -> OneOrMany<Artist> {
        let id = id.into();
        info!("Getting artist of: {id}");
        Song::read_artist(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in library_song_get_artist: {e}"))
            .ok()
            .into()
    }
    /// Get the album of a song.
    #[instrument]
    async fn library_song_get_album(self, context: Context, id: SongId) -> Option<Album> {
        let id = id.into();
        info!("Getting album of: {id}");
        Song::read_album(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in library_song_get_album: {e}"))
            .ok()
            .flatten()
    }
    /// Get an album by its ID.
    #[instrument]
    async fn library_album_get(self, context: Context, id: AlbumId) -> Option<Album> {
        let id = id.into();
        info!("Getting album by ID: {id}");
        Album::read(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in library_album_get: {e}"))
            .ok()
            .flatten()
    }
    /// Get the artists of an album
    #[instrument]
    async fn library_album_get_artist(self, context: Context, id: AlbumId) -> OneOrMany<Artist> {
        let id = id.into();
        info!("Getting artists of: {id}");
        Album::read_artist(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in library_album_get_artist: {e}"))
            .ok()
            .into()
    }
    /// Get the songs of an album
    #[instrument]
    async fn library_album_get_songs(self, context: Context, id: AlbumId) -> Option<Box<[Song]>> {
        let id = id.into();
        info!("Getting songs of: {id}");
        Album::read_songs(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in library_album_get_songs: {e}"))
            .ok()
            .map(Into::into)
    }
    /// Get an artist by its ID.
    #[instrument]
    async fn library_artist_get(self, context: Context, id: ArtistId) -> Option<Artist> {
        let id = id.into();
        info!("Getting artist by ID: {id}");
        Artist::read(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in library_artist_get: {e}"))
            .ok()
            .flatten()
    }
    /// Get the songs of an artist
    #[instrument]
    async fn library_artist_get_songs(self, context: Context, id: ArtistId) -> Option<Box<[Song]>> {
        let id = id.into();
        info!("Getting songs of: {id}");
        Artist::read_songs(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in library_artist_get_songs: {e}"))
            .ok()
            .map(Into::into)
    }
    /// Get the albums of an artist
    #[instrument]
    async fn library_artist_get_albums(
        self,
        context: Context,
        id: ArtistId,
    ) -> Option<Box<[Album]>> {
        let id = id.into();
        info!("Getting albums of: {id}");
        Artist::read_albums(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in library_artist_get_albums: {e}"))
            .ok()
            .map(Into::into)
    }

    /// tells the daemon to shutdown.
    #[instrument]
    async fn daemon_shutdown(self, context: Context) {
        let audio_kernel = self.audio_kernel.clone();
        std::thread::Builder::new()
            .name(String::from("Daemon Shutdown"))
            .spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(1));
                audio_kernel.send(AudioCommand::Exit);
                std::process::exit(0);
            })
            .unwrap();
        info!("Shutting down daemon in 1 second");
    }

    /// returns full information about the current state of the audio player (queue, current song, etc.)
    #[instrument]
    async fn state_audio(self, context: Context) -> Option<StateAudio> {
        debug!("Getting state of audio player");
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.audio_kernel.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in state_audio: {e}"))
            .ok()
    }
    /// returns information about the current queue.
    #[instrument]
    async fn state_queue(self, context: Context) -> Option<Box<[Song]>> {
        info!("Getting state of queue");
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.audio_kernel.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in state_queue: {e}"))
            .ok()
            .map(|state| state.queue)
    }
    /// returns the current queue position.
    #[instrument]
    async fn state_queue_position(self, context: Context) -> Option<usize> {
        info!("Getting state of queue position");
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.audio_kernel.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in state_queue_position: {e}"))
            .ok()
            .and_then(|state| state.queue_position)
    }
    /// is the player currently paused?
    #[instrument]
    async fn state_paused(self, context: Context) -> bool {
        info!("Getting state of playing");
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.audio_kernel.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in state_playing: {e}"))
            .ok()
            .is_some_and(|state| state.paused)
    }
    /// what repeat mode is the player in?
    #[instrument]
    async fn state_repeat(self, context: Context) -> Option<RepeatMode> {
        info!("Getting state of repeat");
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.audio_kernel.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in state_repeat: {e}"))
            .ok()
            .map(|state| state.repeat_mode)
    }
    /// returns the current volume.
    #[instrument]
    async fn state_volume(self, context: Context) -> Option<f32> {
        info!("Getting state of volume");
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.audio_kernel.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in state_volume: {e}"))
            .ok()
            .map(|state| state.volume)
    }
    /// returns the current volume mute state.
    #[instrument]
    async fn state_volume_muted(self, context: Context) -> bool {
        info!("Getting state of volume muted");
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.audio_kernel.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in state_volume_muted: {e}"))
            .ok()
            .is_some_and(|state| state.muted)
    }
    /// returns information about the runtime of the current song (seek position and duration)
    #[instrument]
    async fn state_runtime(self, context: Context) -> Option<StateRuntime> {
        info!("Getting state of runtime");
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.audio_kernel.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in state_runtime: {e}"))
            .ok()
            .and_then(|state| state.runtime)
    }

    /// returns the current artist.
    #[instrument]
    async fn current_artist(self, context: Context) -> OneOrMany<Artist> {
        info!("Getting current artist");
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.audio_kernel.send(AudioCommand::ReportStatus(tx));

        if let Some(song) = rx
            .await
            .tap_err(|e| warn!("Error in current_artist: {e}"))
            .ok()
            .and_then(|state| state.current_song)
        {
            Song::read_artist(&self.db, song.id)
                .await
                .tap_err(|e| warn!("Error in current_album: {e}"))
                .ok()
                .into()
        } else {
            OneOrMany::None
        }
    }
    /// returns the current album.
    #[instrument]
    async fn current_album(self, context: Context) -> Option<Album> {
        info!("Getting current album");
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.audio_kernel.send(AudioCommand::ReportStatus(tx));

        if let Some(song) = rx
            .await
            .tap_err(|e| warn!("Error in current_album: {e}"))
            .ok()
            .and_then(|state| state.current_song)
        {
            Song::read_album(&self.db, song.id)
                .await
                .tap_err(|e| warn!("Error in current_album: {e}"))
                .ok()
                .flatten()
        } else {
            None
        }
    }
    /// returns the current song.
    #[instrument]
    async fn current_song(self, context: Context) -> Option<Song> {
        info!("Getting current song");
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.audio_kernel.send(AudioCommand::ReportStatus(tx));

        rx.await
            .tap_err(|e| warn!("Error in current_song: {e}"))
            .ok()
            .and_then(|state| state.current_song)
    }

    /// returns a random artist.
    #[instrument]
    async fn rand_artist(self, context: Context) -> Option<Artist> {
        info!("Getting random artist");
        Artist::read_all(&self.db)
            .await
            .tap_err(|e| warn!("Error in rand_artist: {e}"))
            .ok()
            .and_then(|artists| artists.choose(&mut rand::thread_rng()).cloned())
    }
    /// returns a random album.
    #[instrument]
    async fn rand_album(self, context: Context) -> Option<Album> {
        info!("Getting random album");
        Album::read_all(&self.db)
            .await
            .tap_err(|e| warn!("Error in rand_album: {e}"))
            .ok()
            .and_then(|albums| albums.choose(&mut rand::thread_rng()).cloned())
    }
    /// returns a random song.
    #[instrument]
    async fn rand_song(self, context: Context) -> Option<Song> {
        info!("Getting random song");
        Song::read_all(&self.db)
            .await
            .tap_err(|e| warn!("Error in rand_song: {e}"))
            .ok()
            .and_then(|songs| songs.choose(&mut rand::thread_rng()).cloned())
    }

    /// returns a list of artists, albums, and songs matching the given search query.
    #[instrument]
    #[allow(clippy::type_complexity)]
    async fn search(self, context: Context, query: String, limit: u32) -> SearchResult {
        info!("Searching for: {query}");
        // basic idea:
        // 1. search for songs
        // 2. search for albums
        // 3. search for artists
        // 4. return the results
        let songs = Song::search(&self.db, &query, i64::from(limit))
            .await
            .tap_err(|e| warn!("Error in search: {e}"))
            .unwrap_or_default()
            .into();

        let albums = Album::search(&self.db, &query, i64::from(limit))
            .await
            .tap_err(|e| warn!("Error in search: {e}"))
            .unwrap_or_default()
            .into();

        let artists = Artist::search(&self.db, &query, i64::from(limit))
            .await
            .tap_err(|e| warn!("Error in search: {e}"))
            .unwrap_or_default()
            .into();
        SearchResult {
            songs,
            albums,
            artists,
        }
    }
    /// returns a list of artists matching the given search query.
    #[instrument]
    async fn search_artist(self, context: Context, query: String, limit: u32) -> Box<[Artist]> {
        info!("Searching for artist: {query}");
        Artist::search(&self.db, &query, i64::from(limit))
            .await
            .tap_err(|e| {
                warn!("Error in search_artist: {e}");
            })
            .unwrap_or_default()
            .into()
    }
    /// returns a list of albums matching the given search query.
    #[instrument]
    async fn search_album(self, context: Context, query: String, limit: u32) -> Box<[Album]> {
        info!("Searching for album: {query}");
        Album::search(&self.db, &query, i64::from(limit))
            .await
            .tap_err(|e| {
                warn!("Error in search_album: {e}");
            })
            .unwrap_or_default()
            .into()
    }
    /// returns a list of songs matching the given search query.
    #[instrument]
    async fn search_song(self, context: Context, query: String, limit: u32) -> Box<[Song]> {
        info!("Searching for song: {query}");
        Song::search(&self.db, &query, i64::from(limit))
            .await
            .tap_err(|e| {
                warn!("Error in search_song: {e}");
            })
            .unwrap_or_default()
            .into()
    }

    /// toggles playback (play/pause).
    #[instrument]
    async fn playback_toggle(self, context: Context) {
        info!("Toggling playback");
        self.audio_kernel.send(AudioCommand::TogglePlayback);
    }
    /// start playback (unpause).
    #[instrument]
    async fn playback_play(self, context: Context) {
        info!("Starting playback");
        self.audio_kernel.send(AudioCommand::Play);
    }
    /// pause playback.
    #[instrument]
    async fn playback_pause(self, context: Context) {
        info!("Pausing playback");
        self.audio_kernel.send(AudioCommand::Pause);
    }
    /// set the current song to be the next song in the queue.
    #[instrument]
    async fn playback_next(self, context: Context) {
        info!("Playing next song");
        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::SkipForward(1)));
    }
    /// restart the current song.
    #[instrument]
    async fn playback_restart(self, context: Context) {
        info!("Restarting current song");
        self.audio_kernel.send(AudioCommand::RestartSong);
    }
    /// skip forward by the given amount of songs
    #[instrument]
    async fn playback_skip_forward(self, context: Context, amount: usize) {
        info!("Skipping forward by {amount} songs");
        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::SkipForward(amount)));
    }
    /// go backwards by the given amount of songs.
    #[instrument]
    async fn playback_skip_backward(self, context: Context, amount: usize) {
        info!("Going back by {amount} songs");
        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::SkipBackward(amount)));
    }
    /// stop playback.
    /// (clears the queue and stops playback)
    #[instrument]
    async fn playback_clear_player(self, context: Context) {
        info!("Stopping playback");
        self.audio_kernel.send(AudioCommand::ClearPlayer);
    }
    /// clear the queue.
    #[instrument]
    async fn playback_clear(self, context: Context) {
        info!("Clearing queue and stopping playback");
        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::Clear));
    }
    /// seek forwards, backwards, or to an absolute second in the current song.
    #[instrument]
    async fn playback_seek(self, context: Context, seek: SeekType, duration: Duration) {
        info!("Seeking {seek} by {:.2}s", duration.as_secs_f32());
        self.audio_kernel.send(AudioCommand::Seek(seek, duration));
    }
    /// set the repeat mode.
    #[instrument]
    async fn playback_repeat(self, context: Context, mode: RepeatMode) {
        info!("Setting repeat mode to: {}", mode);
        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::SetRepeatMode(mode)));
    }
    /// Shuffle the current queue, then start playing from the 1st Song in the queue.
    #[instrument]
    async fn playback_shuffle(self, context: Context) {
        info!("Shuffling queue");
        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::Shuffle));
    }
    /// set the volume to the given value
    /// The value `1.0` is the "normal" volume (unfiltered input). Any value other than `1.0` will multiply each sample by this value.
    #[instrument]
    async fn playback_volume(self, context: Context, volume: f32) {
        info!("Setting volume to: {volume}",);
        self.audio_kernel
            .send(AudioCommand::Volume(VolumeCommand::Set(volume)));
    }
    /// increase the volume by the given amount
    #[instrument]
    async fn playback_volume_up(self, context: Context, amount: f32) {
        info!("Increasing volume by: {amount}",);
        self.audio_kernel
            .send(AudioCommand::Volume(VolumeCommand::Up(amount)));
    }
    /// decrease the volume by the given amount
    #[instrument]
    async fn playback_volume_down(self, context: Context, amount: f32) {
        info!("Decreasing volume by: {amount}",);
        self.audio_kernel
            .send(AudioCommand::Volume(VolumeCommand::Down(amount)));
    }
    /// toggle the volume mute.
    #[instrument]
    async fn playback_volume_toggle_mute(self, context: Context) {
        info!("Toggling volume mute");
        self.audio_kernel
            .send(AudioCommand::Volume(VolumeCommand::ToggleMute));
    }
    /// mute the volume.
    #[instrument]
    async fn playback_mute(self, context: Context) {
        info!("Muting volume");
        self.audio_kernel
            .send(AudioCommand::Volume(VolumeCommand::Mute));
    }
    /// unmute the volume.
    #[instrument]
    async fn playback_unmute(self, context: Context) {
        info!("Unmuting volume");
        self.audio_kernel
            .send(AudioCommand::Volume(VolumeCommand::Unmute));
    }

    /// add a song to the queue.
    /// (if the queue is empty, it will start playing the song.)
    #[instrument]
    async fn queue_add_song(
        self,
        context: Context,
        song: SongId,
    ) -> Result<(), SerializableLibraryError> {
        let song = song.into();
        info!("Adding song to queue: {song}",);
        let Some(song) = Song::read(&self.db, song).await? else {
            return Err(Error::NotFound.into());
        };

        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(
                OneOrMany::One(song),
            ))));

        Ok(())
    }
    /// add a list of things to the queue.
    /// (if the queue is empty, it will start playing the first thing in the list.)
    #[instrument]
    async fn queue_add_list(
        self,
        context: Context,
        list: Vec<mecomp_storage::db::schemas::Thing>,
    ) -> Result<(), SerializableLibraryError> {
        info!(
            "Adding list to queue: ({})",
            list.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );

        // go through the list, and get songs for each thing (depending on what it is)
        let songs: OneOrMany<Song> = get_songs_from_things(&self.db, list).await?;

        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(
                songs,
            ))));

        Ok(())
    }
    /// add an album to the queue.
    /// (if the queue is empty, it will start playing the album.)
    #[instrument]
    async fn queue_add_album(
        self,
        context: Context,
        album: AlbumId,
    ) -> Result<(), SerializableLibraryError> {
        let album = album.into();
        info!("Adding album to queue: {album}");

        let songs = Album::read_songs(&self.db, album).await?;

        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(
                songs.into(),
            ))));

        Ok(())
    }
    /// add an artist to the queue.
    /// (if the queue is empty, it will start playing the artist.)
    #[instrument]
    async fn queue_add_artist(
        self,
        context: Context,
        artist: ArtistId,
    ) -> Result<(), SerializableLibraryError> {
        let artist = artist.into();
        info!("Adding artist to queue: {artist}");

        let songs = Artist::read_songs(&self.db, artist).await?;

        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(
                songs.into(),
            ))));

        Ok(())
    }
    /// add a playlist to the queue.
    /// (if the queue is empty, it will start playing the playlist.)
    #[instrument]
    async fn queue_add_playlist(
        self,
        context: Context,
        playlist: PlaylistId,
    ) -> Result<(), SerializableLibraryError> {
        let playlist = playlist.into();
        info!("Adding playlist to queue: {playlist}");

        let songs = Playlist::read_songs(&self.db, playlist).await?;

        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(
                songs.into(),
            ))));

        Ok(())
    }
    /// add a collection to the queue.
    /// (if the queue is empty, it will start playing the collection.)
    #[instrument]
    async fn queue_add_collection(
        self,
        context: Context,
        collection: CollectionId,
    ) -> Result<(), SerializableLibraryError> {
        let collection = collection.into();
        info!("Adding collection to queue: {collection}");

        let songs = Collection::read_songs(&self.db, collection).await?;

        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(
                songs.into(),
            ))));

        Ok(())
    }
    /// add a random song to the queue.
    /// (if the queue is empty, it will start playing the song.)
    #[instrument]
    async fn queue_add_rand_song(self, context: Context) -> Result<(), SerializableLibraryError> {
        info!("Adding random song to queue");
        let song = Song::read_all(&self.db)
            .await
            .tap_err(|e| warn!("Error in rand_song: {e}"))
            .ok()
            .and_then(|songs| songs.choose(&mut rand::thread_rng()).cloned())
            .ok_or(NotFound)?;

        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(
                OneOrMany::One(song),
            ))));

        Ok(())
    }
    /// add a random album to the queue.
    /// (if the queue is empty, it will start playing the album.)
    #[instrument]
    async fn queue_add_rand_album(self, context: Context) -> Result<(), SerializableLibraryError> {
        info!("Adding random album to queue");

        let songs = Album::read_songs(
            &self.db,
            Album::read_all(&self.db)
                .await
                .tap_err(|e| warn!("Error in rand_album (reading albums): {e}"))
                .ok()
                .and_then(|albums| albums.choose(&mut rand::thread_rng()).cloned())
                .ok_or(NotFound)?
                .id,
        )
        .await
        .tap_err(|e| warn!("Error in rand_album (reading songs): {e}"))
        .map_err(|_| NotFound)?;

        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(
                OneOrMany::Many(songs),
            ))));

        Ok(())
    }
    /// add a random artist to the queue.
    /// (if the queue is empty, it will start playing the artist.)
    #[instrument]
    async fn queue_add_rand_artist(self, context: Context) -> Result<(), SerializableLibraryError> {
        info!("Adding random artist to queue");

        let songs = Artist::read_songs(
            &self.db,
            Artist::read_all(&self.db)
                .await
                .tap_err(|e| warn!("Error in rand_artist (reading artists): {e}"))
                .ok()
                .and_then(|artists| artists.choose(&mut rand::thread_rng()).cloned())
                .ok_or(NotFound)?
                .id,
        )
        .await
        .tap_err(|e| warn!("Error in rand_artist (reading songs): {e}"))
        .map_err(|_| NotFound)?;

        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(
                OneOrMany::Many(songs),
            ))));

        Ok(())
    }
    /// set the current song to a queue index.
    /// if the index is out of bounds, it will be clamped to the nearest valid index.
    #[instrument]
    async fn queue_set_index(self, context: Context, index: usize) {
        info!("Setting queue index to: {index}");

        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::SetPosition(index)));
    }
    /// remove a range of songs from the queue.
    /// if the range is out of bounds, it will be clamped to the nearest valid range.
    #[instrument]
    async fn queue_remove_range(self, context: Context, range: Range<usize>) {
        info!("Removing queue range: {range:?}");

        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::RemoveRange(range)));
    }

    /// Returns brief information about the users playlists.
    #[instrument]
    async fn playlist_list(self, context: Context) -> Box<[PlaylistBrief]> {
        info!("Listing playlists");
        Playlist::read_all(&self.db)
            .await
            .tap_err(|e| warn!("Error in playlist_list: {e}"))
            .ok()
            .map(|playlists| playlists.iter().map(std::convert::Into::into).collect())
            .unwrap_or_default()
    }
    /// create a new playlist.
    /// if a playlist with the same name already exists, this will return that playlist's id in the error variant
    #[instrument]
    async fn playlist_new(
        self,
        context: Context,
        name: String,
    ) -> Result<Result<PlaylistId, PlaylistId>, SerializableLibraryError> {
        info!("Creating new playlist: {name}");

        // see if a playlist with that name already exists
        if let Some(playlist) = Playlist::read_by_name(&self.db, name.clone())
            .await
            .tap_err(|e| warn!("Error in playlist_new: {e}"))
            .ok()
            .flatten()
        {
            Ok(Ok(playlist.id.into()))
        } else {
            Ok(Err(Playlist::create(
                &self.db,
                Playlist {
                    id: Playlist::generate_id(),
                    name: name.into(),
                    runtime: Duration::from_secs(0),
                    song_count: 0,
                },
            )
            .await?
            .map(|playlist| playlist.id.into())
            .ok_or(NotFound)?))
        }
    }
    /// remove a playlist.
    #[instrument]
    async fn playlist_remove(
        self,
        context: Context,
        id: PlaylistId,
    ) -> Result<(), SerializableLibraryError> {
        let id = id.into();
        info!("Removing playlist with id: {id}");

        Playlist::delete(&self.db, id).await?.ok_or(NotFound)?;

        Ok(())
    }
    /// clone a playlist.
    /// (creates a new playlist with the same name (append "copy") and contents as the given playlist.)
    /// returns the id of the new playlist
    #[instrument]
    async fn playlist_clone(
        self,
        context: Context,
        id: PlaylistId,
    ) -> Result<PlaylistId, SerializableLibraryError> {
        let id = id.into();
        info!("Cloning playlist with id: {id}");

        let new_playlist = Playlist::create_copy(&self.db, id).await?.ok_or(NotFound)?;

        Ok(new_playlist.id.into())
    }
    /// get the id of a playlist.
    /// returns none if the playlist does not exist.
    #[instrument]
    async fn playlist_get_id(self, context: Context, name: String) -> Option<PlaylistId> {
        info!("Getting playlist ID: {name}");

        Playlist::read_by_name(&self.db, name)
            .await
            .tap_err(|e| warn!("Error in playlist_get_id: {e}"))
            .ok()
            .flatten()
            .map(|playlist| playlist.id.into())
    }
    /// remove a list of songs from a playlist.
    /// if the songs are not in the playlist, this will do nothing.
    #[instrument]
    async fn playlist_remove_songs(
        self,
        context: Context,
        playlist: PlaylistId,
        songs: Vec<SongId>,
    ) -> Result<(), SerializableLibraryError> {
        let playlist = playlist.into();
        let songs = songs.into_iter().map(Into::into).collect::<Vec<_>>();
        info!("Removing song from playlist: {playlist} ({songs:?})");

        Ok(Playlist::remove_songs(&self.db, playlist, &songs).await?)
    }
    /// Add an artist to a playlist.
    #[instrument]
    async fn playlist_add_artist(
        self,
        context: Context,
        playlist: PlaylistId,
        artist: ArtistId,
    ) -> Result<(), SerializableLibraryError> {
        let playlist = playlist.into();
        let artist = artist.into();
        info!("Adding artist to playlist: {playlist} ({artist})");

        Ok(Playlist::add_songs(
            &self.db,
            playlist,
            &Artist::read_songs(&self.db, artist)
                .await?
                .iter()
                .map(|song| song.id.clone())
                .collect::<Vec<_>>(),
        )
        .await?)
    }
    /// Add an album to a playlist.
    #[instrument]
    async fn playlist_add_album(
        self,
        context: Context,
        playlist: PlaylistId,
        album: AlbumId,
    ) -> Result<(), SerializableLibraryError> {
        let playlist = playlist.into();
        let album = album.into();
        info!("Adding album to playlist: {playlist} ({album})");

        Ok(Playlist::add_songs(
            &self.db,
            playlist,
            &Album::read_songs(&self.db, album)
                .await?
                .iter()
                .map(|song| song.id.clone())
                .collect::<Vec<_>>(),
        )
        .await?)
    }
    /// Add songs to a playlist.
    #[instrument]
    async fn playlist_add_songs(
        self,
        context: Context,
        playlist: PlaylistId,
        songs: Vec<SongId>,
    ) -> Result<(), SerializableLibraryError> {
        let playlist = playlist.into();
        let songs = songs.into_iter().map(Into::into).collect::<Vec<_>>();
        info!("Adding songs to playlist: {playlist} ({songs:?})");

        Ok(Playlist::add_songs(&self.db, playlist, &songs).await?)
    }
    /// Add a list of things to a playlist.
    #[instrument]
    async fn playlist_add_list(
        self,
        context: Context,
        playlist: PlaylistId,
        list: Vec<mecomp_storage::db::schemas::Thing>,
    ) -> Result<(), SerializableLibraryError> {
        let playlist = playlist.into();
        info!(
            "Adding list to playlist: {playlist} ({})",
            list.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );

        // go through the list, and get songs for each thing (depending on what it is)
        let songs: OneOrMany<Song> = get_songs_from_things(&self.db, list).await?;

        Ok(Playlist::add_songs(
            &self.db,
            playlist,
            &songs.into_iter().map(|s| s.id).collect::<Vec<_>>(),
        )
        .await?)
    }
    /// Get a playlist by its ID.
    #[instrument]
    async fn playlist_get(self, context: Context, id: PlaylistId) -> Option<Playlist> {
        let id = id.into();
        info!("Getting playlist by ID: {}", id);

        Playlist::read(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in playlist_get: {e}"))
            .ok()
            .flatten()
    }
    /// Get the songs of a playlist
    #[instrument]
    async fn playlist_get_songs(self, context: Context, id: PlaylistId) -> Option<Box<[Song]>> {
        let id = id.into();
        info!("Getting songs in: {id}");
        Playlist::read_songs(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in playlist_get_songs: {e}"))
            .ok()
            .map(Into::into)
    }

    /// Collections: Return brief information about the users auto curration collections.
    #[instrument]
    async fn collection_list(self, context: Context) -> Box<[CollectionBrief]> {
        info!("Listing collections");
        Collection::read_all(&self.db)
            .await
            .tap_err(|e| warn!("Error in collection_list: {e}"))
            .ok()
            .map(|collections| collections.iter().map(std::convert::Into::into).collect())
            .unwrap_or_default()
    }
    /// Collections: get a collection by its ID.
    #[instrument]
    async fn collection_get(self, context: Context, id: CollectionId) -> Option<Collection> {
        info!("Getting collection by ID: {id:?}");
        Collection::read(&self.db, id.into())
            .await
            .tap_err(|e| warn!("Error in collection_get: {e}"))
            .ok()
            .flatten()
    }
    /// Collections: freeze a collection (convert it to a playlist).
    #[instrument]
    async fn collection_freeze(
        self,
        context: Context,
        id: CollectionId,
        name: String,
    ) -> Result<PlaylistId, SerializableLibraryError> {
        info!("Freezing collection: {id:?} ({name})");
        Ok(Collection::freeze(&self.db, id.into(), name.into())
            .await
            .map(|p| p.id.into())?)
    }
    /// Get the songs of a collection
    #[instrument]
    async fn collection_get_songs(self, context: Context, id: CollectionId) -> Option<Box<[Song]>> {
        let id = id.into();
        info!("Getting songs in: {id}");
        Collection::read_songs(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in collection_get_songs: {e}"))
            .ok()
            .map(Into::into)
    }

    /// Radio: get the `n` most similar songs to the given things.
    #[instrument]
    async fn radio_get_similar(
        self,
        context: Context,
        things: Vec<schemas::Thing>,
        n: u32,
    ) -> Result<Box<[Song]>, SerializableLibraryError> {
        #[cfg(not(feature = "analysis"))]
        {
            warn!("Analysis is not enabled");
            return Err(SerializableLibraryError::AnalysisNotEnabled);
        }

        #[cfg(feature = "analysis")]
        {
            info!("Getting the {n} most similar songs to: {things:?}");
            Ok(services::radio::get_similar(&self.db, things, n)
                .await
                .map(Vec::into_boxed_slice)
                .tap_err(|e| warn!("Error in radio_get_similar: {e}"))?)
        }
    }
    /// Radio: get the `n` most similar songs to the given song.
    #[instrument]
    async fn radio_get_similar_to_song(
        self,
        context: Context,
        song: SongId,
        n: u32,
    ) -> Result<Box<[SongId]>, SerializableLibraryError> {
        #[cfg(not(feature = "analysis"))]
        {
            warn!("Analysis is not enabled");
            return Err(SerializableLibraryError::AnalysisNotEnabled);
        }

        #[cfg(feature = "analysis")]
        {
            info!("Getting the {n} most similar songs to: {song:?}");
            Ok(
                services::radio::get_similar_to_song(&self.db, song.into(), n)
                    .await
                    .map(|songs| songs.into_iter().map(|song| song.id.into()).collect())
                    .tap_err(|e| warn!("Error in radio_get_similar_songs: {e}"))?,
            )
        }
    }

    /// Radio: get the `n` most similar songs to the given artist.
    #[instrument]
    async fn radio_get_similar_to_artist(
        self,
        context: Context,
        artist: ArtistId,
        n: u32,
    ) -> Result<Box<[SongId]>, SerializableLibraryError> {
        #[cfg(not(feature = "analysis"))]
        {
            warn!("Analysis is not enabled");
            return Err(SerializableLibraryError::AnalysisNotEnabled);
        }

        #[cfg(feature = "analysis")]
        {
            info!("Getting the {n} most similar songs to: {artist:?}");
            Ok(
                services::radio::get_similar_to_artist(&self.db, artist.into(), n)
                    .await
                    .map(|songs| songs.into_iter().map(|song| song.id.into()).collect())
                    .tap_err(|e| warn!("Error in radio_get_similar_to_artist: {e}"))?,
            )
        }
    }

    /// Radio: get the `n` most similar songs to the given album.
    #[instrument]
    async fn radio_get_similar_to_album(
        self,
        context: Context,
        album: AlbumId,
        n: u32,
    ) -> Result<Box<[SongId]>, SerializableLibraryError> {
        #[cfg(not(feature = "analysis"))]
        {
            warn!("Analysis is not enabled");
            return Err(SerializableLibraryError::AnalysisNotEnabled);
        }

        #[cfg(feature = "analysis")]
        {
            info!("Getting the {n} most similar songs to: {album:?}");
            Ok(
                services::radio::get_similar_to_album(&self.db, album.into(), n)
                    .await
                    .map(|songs| songs.into_iter().map(|song| song.id.into()).collect())
                    .tap_err(|e| warn!("Error in radio_get_similar_to_artist: {e}"))?,
            )
        }
    }

    /// Radio: get the `n` most similar songs to the given playlist.
    #[instrument]
    async fn radio_get_similar_to_playlist(
        self,
        context: Context,
        playlist: PlaylistId,
        n: u32,
    ) -> Result<Box<[SongId]>, SerializableLibraryError> {
        #[cfg(not(feature = "analysis"))]
        {
            warn!("Analysis is not enabled");
            return Err(SerializableLibraryError::AnalysisNotEnabled);
        }

        #[cfg(feature = "analysis")]
        {
            info!("Getting the {n} most similar songs to: {playlist:?}");
            Ok(
                services::radio::get_similar_to_playlist(&self.db, playlist.into(), n)
                    .await
                    .map(|songs| songs.into_iter().map(|song| song.id.into()).collect())
                    .tap_err(|e| warn!("Error in radio_get_similar_to_artist: {e}"))?,
            )
        }
    }
}
