//----------------------------------------------------------------------------------------- std lib
use std::{fs::File, ops::Range, path::PathBuf, sync::Arc, time::Duration};
//--------------------------------------------------------------------------------- other libraries
use ::tarpc::context::Context;
use log::{debug, error, info, warn};
use surrealdb::{Surreal, engine::local::Db};
use tap::TapFallible;
use tokio::sync::{Mutex, RwLock};
use tracing::{Instrument, instrument};
//-------------------------------------------------------------------------------- MECOMP libraries
use mecomp_core::{
    audio::{
        AudioKernelSender,
        commands::{AudioCommand, QueueCommand, VolumeCommand},
    },
    config::Settings,
    errors::{BackupError, SerializableLibraryError},
    rpc::{
        AlbumId, ArtistId, CollectionId, DynamicPlaylistId, MusicPlayer, PlaylistId, SearchResult,
        SongId,
    },
    state::{
        RepeatMode, SeekType, StateAudio,
        library::{LibraryBrief, LibraryFull, LibraryHealth},
    },
    udp::{Event, Message, Sender},
};
use mecomp_storage::{
    db::schemas::{
        self,
        album::{Album, AlbumBrief},
        artist::{Artist, ArtistBrief},
        collection::{Collection, CollectionBrief},
        dynamic::{DynamicPlaylist, DynamicPlaylistChangeSet, query::Query},
        playlist::{Playlist, PlaylistBrief, PlaylistChangeSet},
        song::{Song, SongBrief},
    },
    errors::Error,
};
use one_or_many::OneOrMany;

use crate::{
    services::{
        self,
        backup::{
            export_dynamic_playlists, export_playlist, import_dynamic_playlists, import_playlist,
            validate_file_path,
        },
    },
    termination::{self, Terminator},
};

#[derive(Clone, Debug)]
pub struct MusicPlayerServer {
    db: Arc<Surreal<Db>>,
    settings: Arc<Settings>,
    audio_kernel: Arc<AudioKernelSender>,
    library_rescan_lock: Arc<Mutex<()>>,
    library_analyze_lock: Arc<Mutex<()>>,
    collection_recluster_lock: Arc<Mutex<()>>,
    publisher: Arc<RwLock<Sender<Message>>>,
    terminator: Arc<Mutex<Terminator>>,
    interrupt: Arc<termination::InterruptReceiver>,
}

impl MusicPlayerServer {
    #[must_use]
    #[inline]
    pub fn new(
        db: Arc<Surreal<Db>>,
        settings: Arc<Settings>,
        audio_kernel: Arc<AudioKernelSender>,
        event_publisher: Arc<RwLock<Sender<Message>>>,
        terminator: Terminator,
        interrupt: termination::InterruptReceiver,
    ) -> Self {
        Self {
            db,
            publisher: event_publisher,
            settings,
            audio_kernel,
            library_rescan_lock: Arc::new(Mutex::new(())),
            library_analyze_lock: Arc::new(Mutex::new(())),
            collection_recluster_lock: Arc::new(Mutex::new(())),
            terminator: Arc::new(Mutex::new(terminator)),
            interrupt: Arc::new(interrupt),
        }
    }

    /// Publish a message to all listeners.
    ///
    /// # Errors
    ///
    /// Returns an error if the message could not be sent or encoded.
    #[instrument]
    pub async fn publish(
        &self,
        message: impl Into<Message> + Send + Sync + std::fmt::Debug,
    ) -> Result<(), mecomp_core::errors::UdpError> {
        self.publisher.read().await.send(message).await
    }
}

#[allow(clippy::missing_inline_in_public_items)]
impl MusicPlayer for MusicPlayerServer {
    #[instrument]
    async fn register_listener(self, context: Context, listener_addr: std::net::SocketAddr) {
        info!("Registering listener: {listener_addr}");
        self.publisher.write().await.add_subscriber(listener_addr);
    }

    async fn ping(self, _: Context) -> String {
        "pong".to_string()
    }

    /// Rescans the music library, only error is if a rescan is already in progress.
    #[instrument]
    async fn library_rescan(self, context: Context) -> Result<(), SerializableLibraryError> {
        info!("Rescanning library");

        if self.library_rescan_lock.try_lock().is_err() {
            warn!("Library rescan already in progress");
            return Err(SerializableLibraryError::RescanInProgress);
        }

        let span = tracing::Span::current();

        tokio::task::spawn(
            async move {
                let _guard = self.library_rescan_lock.lock().await;
                match services::library::rescan(
                    &self.db,
                    &self.settings.daemon.library_paths,
                    &self.settings.daemon.artist_separator,
                    &self.settings.daemon.protected_artist_names,
                    self.settings.daemon.genre_separator.as_deref(),
                    self.settings.daemon.conflict_resolution,
                )
                .await
                {
                    Ok(()) => info!("Library rescan complete"),
                    Err(e) => error!("Error in library_rescan: {e}"),
                }

                let result = self.publish(Event::LibraryRescanFinished).await;
                if let Err(e) = result {
                    error!("Error notifying clients that library_rescan_finished: {e}");
                }
            }
            .instrument(span),
        );

        Ok(())
    }
    /// Check if a rescan is in progress.
    #[instrument]
    async fn library_rescan_in_progress(self, context: Context) -> bool {
        self.library_rescan_lock.try_lock().is_err()
    }
    /// Analyze the music library, only error is if an analysis is already in progress.
    #[instrument]
    async fn library_analyze(
        self,
        context: Context,
        overwrite: bool,
    ) -> Result<(), SerializableLibraryError> {
        info!("Analyzing library");

        if self.library_analyze_lock.try_lock().is_err() {
            warn!("Library analysis already in progress");
            return Err(SerializableLibraryError::AnalysisInProgress);
        }
        let span = tracing::Span::current();

        tokio::task::spawn(
            async move {
                let _guard = self.library_analyze_lock.lock().await;
                match services::library::analyze(&self.db, self.interrupt.resubscribe(), overwrite)
                    .await
                {
                    Ok(()) => info!("Library analysis complete"),
                    Err(e) => error!("Error in library_analyze: {e}"),
                }

                let result = self.publish(Event::LibraryAnalysisFinished).await;
                if let Err(e) = result {
                    error!("Error notifying clients that library_analysis_finished: {e}");
                }
            }
            .instrument(span),
        );

        Ok(())
    }
    /// Check if an analysis is in progress.
    #[instrument]
    async fn library_analyze_in_progress(self, context: Context) -> bool {
        self.library_analyze_lock.try_lock().is_err()
    }
    /// Recluster the music library, only error is if a recluster is already in progress.
    #[instrument]
    async fn library_recluster(self, context: Context) -> Result<(), SerializableLibraryError> {
        info!("Reclustering collections");

        if self.collection_recluster_lock.try_lock().is_err() {
            warn!("Collection reclustering already in progress");
            return Err(SerializableLibraryError::ReclusterInProgress);
        }

        let span = tracing::Span::current();

        tokio::task::spawn(
            async move {
                let _guard = self.collection_recluster_lock.lock().await;
                match services::library::recluster(
                    &self.db,
                    self.settings.reclustering,
                    self.interrupt.resubscribe(),
                )
                .await
                {
                    Ok(()) => info!("Collection reclustering complete"),
                    Err(e) => error!("Error in library_recluster: {e}"),
                }

                let result = self.publish(Event::LibraryReclusterFinished).await;
                if let Err(e) = result {
                    error!("Error notifying clients that library_recluster_finished: {e}");
                }
            }
            .instrument(span),
        );

        Ok(())
    }
    /// Check if a recluster is in progress.
    #[instrument]
    async fn library_recluster_in_progress(self, context: Context) -> bool {
        self.collection_recluster_lock.try_lock().is_err()
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
        Ok(Artist::read_all_brief(&self.db)
            .await
            .tap_err(|e| warn!("Error in library_artists_brief: {e}"))?
            .into_boxed_slice())
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
            .tap_err(|e| warn!("Error in library_artists_brief: {e}"))?
            .into_boxed_slice())
    }
    /// Returns brief information about the music library's albums.
    #[instrument]
    async fn library_albums_brief(
        self,
        context: Context,
    ) -> Result<Box<[AlbumBrief]>, SerializableLibraryError> {
        info!("Creating library albums brief");
        Ok(Album::read_all_brief(&self.db)
            .await
            .tap_err(|e| warn!("Error in library_albums_brief: {e}"))?
            .into_boxed_slice())
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
        Ok(Song::read_all_brief(&self.db)
            .await
            .tap_err(|e| warn!("Error in library_songs_brief: {e}"))?
            .into_boxed_slice())
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
            .tap_err(|e| warn!("Error in library_songs_full: {e}"))?
            .into_boxed_slice())
    }
    /// Returns brief information about the users playlists.
    #[instrument]
    async fn library_playlists_brief(
        self,
        context: Context,
    ) -> Result<Box<[PlaylistBrief]>, SerializableLibraryError> {
        info!("Creating library playlists brief");
        Ok(Playlist::read_all_brief(&self.db)
            .await
            .tap_err(|e| warn!("Error in library_playlists_brief: {e}"))?
            .into_boxed_slice())
    }
    /// Returns full information about the users playlists.
    #[instrument]
    async fn library_playlists_full(
        self,
        context: Context,
    ) -> Result<Box<[Playlist]>, SerializableLibraryError> {
        info!("Creating library playlists full");
        Ok(Playlist::read_all(&self.db)
            .await
            .tap_err(|e| warn!("Error in library_playlists_full: {e}"))?
            .into_boxed_slice())
    }
    /// Return brief information about the users collections.
    #[instrument]
    async fn library_collections_brief(
        self,
        context: Context,
    ) -> Result<Box<[CollectionBrief]>, SerializableLibraryError> {
        info!("Creating library collections brief");
        Ok(Collection::read_all_brief(&self.db)
            .await
            .tap_err(|e| warn!("Error in library_collections_brief: {e}"))?
            .into_boxed_slice())
    }
    /// Return full information about the users collections.
    #[instrument]
    async fn library_collections_full(
        self,
        context: Context,
    ) -> Result<Box<[Collection]>, SerializableLibraryError> {
        info!("Creating library collections full");
        Ok(Collection::read_all(&self.db)
            .await
            .tap_err(|e| warn!("Error in library_collections_full: {e}"))?
            .into_boxed_slice())
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
            .unwrap_or_default()
    }
    /// Get a song by its file path.
    #[instrument]
    async fn library_song_get_by_path(self, context: Context, path: PathBuf) -> Option<Song> {
        info!("Getting song by path: {}", path.display());
        Song::read_by_path(&self.db, path)
            .await
            .tap_err(|e| warn!("Error in library_song_get_by_path: {e}"))
            .unwrap_or_default()
    }
    /// Get the artists of a song.
    #[instrument]
    async fn library_song_get_artist(self, context: Context, id: SongId) -> OneOrMany<Artist> {
        let id = id.into();
        info!("Getting artist of: {id}");
        Song::read_artist(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in library_song_get_artist: {e}"))
            .unwrap_or_default()
    }
    /// Get the album of a song.
    #[instrument]
    async fn library_song_get_album(self, context: Context, id: SongId) -> Option<Album> {
        let id = id.into();
        info!("Getting album of: {id}");
        Song::read_album(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in library_song_get_album: {e}"))
            .unwrap_or_default()
    }
    /// Get the Playlists a song is in.
    #[instrument]
    async fn library_song_get_playlists(self, context: Context, id: SongId) -> Box<[Playlist]> {
        let id = id.into();
        info!("Getting playlists of: {id}");
        Song::read_playlists(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in library_song_get_playlists: {e}"))
            .ok()
            .unwrap_or_default()
            .into()
    }
    /// Get the Collections a song is in.
    #[instrument]
    async fn library_song_get_collections(self, context: Context, id: SongId) -> Box<[Collection]> {
        let id = id.into();
        info!("Getting collections of: {id}");
        Song::read_collections(&self.db, id)
            .await
            .tap_err(|e| warn!("Error in library_song_get_collections: {e}"))
            .ok()
            .unwrap_or_default()
            .into()
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
            .map(Vec::into_boxed_slice)
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
            .map(Vec::into_boxed_slice)
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
            .map(Vec::into_boxed_slice)
    }

    /// tells the daemon to shutdown.
    #[instrument]
    async fn daemon_shutdown(self, context: Context) {
        let terminator = self.terminator.clone();
        std::thread::Builder::new()
            .name(String::from("Daemon Shutdown"))
            .spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let terminate_result = terminator
                    .blocking_lock()
                    .terminate(termination::Interrupted::UserInt);
                if let Err(e) = terminate_result {
                    error!("Error terminating daemon, panicking instead: {e}");
                    panic!("Error terminating daemon: {e}");
                }
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
                .unwrap_or_default()
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
                .unwrap_or_default()
        } else {
            None
        }
    }
    /// returns the current song.
    #[instrument]
    async fn current_song(self, context: Context) -> Option<SongBrief> {
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
    async fn rand_artist(self, context: Context) -> Option<ArtistBrief> {
        info!("Getting random artist");
        Artist::read_rand(&self.db, 1)
            .await
            .tap_err(|e| warn!("Error in rand_artist: {e}"))
            .ok()
            .and_then(|results| results.first().cloned())
    }
    /// returns a random album.
    #[instrument]
    async fn rand_album(self, context: Context) -> Option<AlbumBrief> {
        info!("Getting random album");
        Album::read_rand(&self.db, 1)
            .await
            .tap_err(|e| warn!("Error in rand_album: {e}"))
            .ok()
            .and_then(|results| results.first().cloned())
    }
    /// returns a random song.
    #[instrument]
    async fn rand_song(self, context: Context) -> Option<SongBrief> {
        info!("Getting random song");
        Song::read_rand(&self.db, 1)
            .await
            .tap_err(|e| warn!("Error in rand_song: {e}"))
            .ok()
            .and_then(|results| results.first().cloned())
    }

    /// returns a list of artists, albums, and songs matching the given search query.
    #[instrument]
    async fn search(self, context: Context, query: String, limit: usize) -> SearchResult {
        info!("Searching for: {query}");
        // basic idea:
        // 1. search for songs
        // 2. search for albums
        // 3. search for artists
        // 4. return the results
        let songs = Song::search(&self.db, &query, limit)
            .await
            .tap_err(|e| warn!("Error in search: {e}"))
            .unwrap_or_default()
            .into();

        let albums = Album::search(&self.db, &query, limit)
            .await
            .tap_err(|e| warn!("Error in search: {e}"))
            .unwrap_or_default()
            .into();

        let artists = Artist::search(&self.db, &query, limit)
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
    async fn search_artist(
        self,
        context: Context,
        query: String,
        limit: usize,
    ) -> Box<[ArtistBrief]> {
        info!("Searching for artist: {query}");
        Artist::search(&self.db, &query, limit)
            .await
            .tap_err(|e| {
                warn!("Error in search_artist: {e}");
            })
            .unwrap_or_default()
            .into()
    }
    /// returns a list of albums matching the given search query.
    #[instrument]
    async fn search_album(
        self,
        context: Context,
        query: String,
        limit: usize,
    ) -> Box<[AlbumBrief]> {
        info!("Searching for album: {query}");
        Album::search(&self.db, &query, limit)
            .await
            .tap_err(|e| {
                warn!("Error in search_album: {e}");
            })
            .unwrap_or_default()
            .into()
    }
    /// returns a list of songs matching the given search query.
    #[instrument]
    async fn search_song(self, context: Context, query: String, limit: usize) -> Box<[SongBrief]> {
        info!("Searching for song: {query}");
        Song::search(&self.db, &query, limit)
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
    /// stop playback.
    #[instrument]
    async fn playback_stop(self, context: Context) {
        info!("Stopping playback");
        self.audio_kernel.send(AudioCommand::Stop);
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
        info!("Setting repeat mode to: {mode}");
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
    async fn queue_add(
        self,
        context: Context,
        thing: schemas::RecordId,
    ) -> Result<(), SerializableLibraryError> {
        info!("Adding thing to queue: {thing}");

        let songs = services::get_songs_from_things(&self.db, &[thing]).await?;

        if songs.is_empty() {
            return Err(Error::NotFound.into());
        }

        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::AddToQueue(
                songs.into_iter().map(Into::into).collect(),
            )));

        Ok(())
    }
    /// add a list of things to the queue.
    /// (if the queue is empty, it will start playing the first thing in the list.)
    #[instrument]
    async fn queue_add_list(
        self,
        context: Context,
        list: Vec<schemas::RecordId>,
    ) -> Result<(), SerializableLibraryError> {
        info!(
            "Adding list to queue: ({})",
            list.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );

        // go through the list, and get songs for each thing (depending on what it is)
        let songs: OneOrMany<Song> = services::get_songs_from_things(&self.db, &list).await?;

        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::AddToQueue(
                songs.into_iter().map(Into::into).collect(),
            )));

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

    /// create a new playlist.
    /// if a playlist with the same name already exists, this will return that playlist's id in the error variant
    #[instrument]
    async fn playlist_get_or_create(
        self,
        context: Context,
        name: String,
    ) -> Result<PlaylistId, SerializableLibraryError> {
        info!("Creating new playlist: {name}");

        // see if a playlist with that name already exists
        match Playlist::read_by_name(&self.db, name.clone()).await {
            Ok(Some(playlist)) => return Ok(playlist.id.into()),
            Err(e) => warn!("Error in playlist_new (looking for existing playlist): {e}"),
            _ => {}
        }
        // if it doesn't, create a new playlist with that name
        match Playlist::create(
            &self.db,
            Playlist {
                id: Playlist::generate_id(),
                name,
                runtime: Duration::from_secs(0),
                song_count: 0,
            },
        )
        .await
        .tap_err(|e| warn!("Error in playlist_new (creating new playlist): {e}"))?
        {
            Some(playlist) => Ok(playlist.id.into()),
            None => Err(Error::NotCreated.into()),
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

        Playlist::delete(&self.db, id)
            .await?
            .ok_or(Error::NotFound)?;

        Ok(())
    }
    /// clone a playlist.
    /// (creates a new playlist with the same name (append " (copy)") and contents as the given playlist.)
    /// returns the id of the new playlist
    #[instrument]
    async fn playlist_clone(
        self,
        context: Context,
        id: PlaylistId,
    ) -> Result<PlaylistId, SerializableLibraryError> {
        let id = id.into();
        info!("Cloning playlist with id: {id}");

        let new_playlist = Playlist::create_copy(&self.db, id)
            .await?
            .ok_or(Error::NotFound)?;

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

        Ok(Playlist::remove_songs(&self.db, playlist, songs).await?)
    }
    /// Add a thing to a playlist.
    /// If the thing is something that has songs (an album, artist, etc.), it will add all the songs.
    #[instrument]
    async fn playlist_add(
        self,
        context: Context,
        playlist: PlaylistId,
        thing: schemas::RecordId,
    ) -> Result<(), SerializableLibraryError> {
        let playlist = playlist.into();
        info!("Adding thing to playlist: {playlist} ({thing})");

        // get songs for the thing
        let songs: OneOrMany<Song> = services::get_songs_from_things(&self.db, &[thing]).await?;

        Ok(Playlist::add_songs(
            &self.db,
            playlist,
            songs.into_iter().map(|s| s.id).collect::<Vec<_>>(),
        )
        .await?)
    }
    /// Add a list of things to a playlist.
    /// If the things are something that have songs (an album, artist, etc.), it will add all the songs.
    #[instrument]
    async fn playlist_add_list(
        self,
        context: Context,
        playlist: PlaylistId,
        list: Vec<schemas::RecordId>,
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
        let songs: OneOrMany<Song> = services::get_songs_from_things(&self.db, &list).await?;

        Ok(Playlist::add_songs(
            &self.db,
            playlist,
            songs.into_iter().map(|s| s.id).collect::<Vec<_>>(),
        )
        .await?)
    }
    /// Get a playlist by its ID.
    #[instrument]
    async fn playlist_get(self, context: Context, id: PlaylistId) -> Option<Playlist> {
        let id = id.into();
        info!("Getting playlist by ID: {id}");

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
    /// Rename a playlist.
    #[instrument]
    async fn playlist_rename(
        self,
        context: Context,
        id: PlaylistId,
        name: String,
    ) -> Result<Playlist, SerializableLibraryError> {
        let id = id.into();
        info!("Renaming playlist: {id} ({name})");
        Playlist::update(&self.db, id, PlaylistChangeSet::new().name(name))
            .await?
            .ok_or(Error::NotFound.into())
    }
    /// Export a playlist to a .m3u file
    #[instrument]
    async fn playlist_export(
        self,
        context: Context,
        id: PlaylistId,
        path: PathBuf,
    ) -> Result<(), SerializableLibraryError> {
        info!("Exporting playlist to: {}", path.display());

        // validate the path
        validate_file_path(&path, "m3u", false)?;

        // read the playlist
        let playlist = Playlist::read(&self.db, id.into())
            .await
            .tap_err(|e| warn!("Error in playlist_export: {e}"))
            .ok()
            .flatten()
            .ok_or(Error::NotFound)?;
        // get the songs in the playlist
        let songs = Playlist::read_songs(&self.db, playlist.id)
            .await
            .tap_err(|e| warn!("Error in playlist_export: {e}"))
            .ok()
            .unwrap_or_default();

        // create the file
        let file = File::create(&path).tap_err(|e| warn!("Error in playlist_export: {e}"))?;
        // write the playlist to the file
        export_playlist(&playlist.name, &songs, file)
            .tap_err(|e| warn!("Error in playlist_export: {e}"))?;
        info!("Exported playlist to: {path:?}");
        Ok(())
    }
    /// Import a playlist from a .m3u file
    #[instrument]
    async fn playlist_import(
        self,
        context: Context,
        path: PathBuf,
        name: Option<String>,
    ) -> Result<PlaylistId, SerializableLibraryError> {
        info!("Importing playlist from: {}", path.display());

        // validate the path
        validate_file_path(&path, "m3u", true)?;

        // read file
        let file = File::open(&path).tap_err(|e| warn!("Error in playlist_import: {e}"))?;
        let (parsed_name, song_paths) =
            import_playlist(file).tap_err(|e| warn!("Error in playlist_import: {e}"))?;

        log::debug!("Parsed playlist name: {parsed_name:?}");
        log::debug!("Parsed song paths: {song_paths:?}");

        let name = match (name, parsed_name) {
            (Some(name), _) | (None, Some(name)) => name,
            (None, None) => "Imported Playlist".to_owned(),
        };

        // check if the playlist already exists
        if let Ok(Some(playlist)) = Playlist::read_by_name(&self.db, name.clone()).await {
            // if it does, return the id
            info!("Playlist \"{name}\" already exists, will not import");
            return Ok(playlist.id.into());
        }

        // create the playlist
        let playlist = Playlist::create(
            &self.db,
            Playlist {
                id: Playlist::generate_id(),
                name,
                runtime: Duration::from_secs(0),
                song_count: 0,
            },
        )
        .await
        .tap_err(|e| warn!("Error in playlist_import: {e}"))?
        .ok_or(Error::NotCreated)?;

        // lookup all the songs
        let mut songs = Vec::new();
        for path in &song_paths {
            let Some(song) = Song::read_by_path(&self.db, path.clone())
                .await
                .tap_err(|e| warn!("Error in playlist_import: {e}"))?
            else {
                warn!("Song at {} not found in the library", path.display());
                continue;
            };

            songs.push(song.id);
        }

        if songs.is_empty() {
            return Err(BackupError::NoValidSongs(song_paths.len()).into());
        }

        // add the songs to the playlist
        Playlist::add_songs(&self.db, playlist.id.clone(), songs)
            .await
            .tap_err(|e| {
                warn!("Error in playlist_import: {e}");
            })?;

        // return the playlist id
        Ok(playlist.id.into())
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
        Ok(Collection::freeze(&self.db, id.into(), name)
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
        things: Vec<schemas::RecordId>,
        n: u32,
    ) -> Result<Box<[Song]>, SerializableLibraryError> {
        info!("Getting the {n} most similar songs to: {things:?}");
        Ok(services::radio::get_similar(&self.db, things, n)
            .await
            .map(Vec::into_boxed_slice)
            .tap_err(|e| warn!("Error in radio_get_similar: {e}"))?)
    }
    /// Radio: get the ids of the `n` most similar songs to the given things.
    #[instrument]
    async fn radio_get_similar_ids(
        self,
        context: Context,
        things: Vec<schemas::RecordId>,
        n: u32,
    ) -> Result<Box<[SongId]>, SerializableLibraryError> {
        info!("Getting the {n} most similar songs to: {things:?}");
        Ok(services::radio::get_similar(&self.db, things, n)
            .await
            .map(|songs| songs.into_iter().map(|song| song.id.into()).collect())
            .tap_err(|e| warn!("Error in radio_get_similar_songs: {e}"))?)
    }

    // Dynamic playlist commands
    /// Dynamic Playlists: create a new DP with the given name and query
    #[instrument]
    async fn dynamic_playlist_create(
        self,
        context: Context,
        name: String,
        query: Query,
    ) -> Result<DynamicPlaylistId, SerializableLibraryError> {
        let id = DynamicPlaylist::generate_id();
        info!("Creating new DP: {id:?} ({name})");

        match DynamicPlaylist::create(&self.db, DynamicPlaylist { id, name, query })
            .await
            .tap_err(|e| warn!("Error in dynamic_playlist_create: {e}"))?
        {
            Some(dp) => Ok(dp.id.into()),
            None => Err(Error::NotCreated.into()),
        }
    }
    /// Dynamic Playlists: list all DPs
    #[instrument]
    async fn dynamic_playlist_list(self, context: Context) -> Box<[DynamicPlaylist]> {
        info!("Listing DPs");
        DynamicPlaylist::read_all(&self.db)
            .await
            .tap_err(|e| warn!("Error in dynamic_playlist_list: {e}"))
            .ok()
            .map(Into::into)
            .unwrap_or_default()
    }
    /// Dynamic Playlists: update a DP
    #[instrument]
    async fn dynamic_playlist_update(
        self,
        context: Context,
        id: DynamicPlaylistId,
        changes: DynamicPlaylistChangeSet,
    ) -> Result<DynamicPlaylist, SerializableLibraryError> {
        info!("Updating DP: {id:?}, {changes:?}");
        DynamicPlaylist::update(&self.db, id.into(), changes)
            .await
            .tap_err(|e| warn!("Error in dynamic_playlist_update: {e}"))?
            .ok_or(Error::NotFound.into())
    }
    /// Dynamic Playlists: remove a DP
    #[instrument]
    async fn dynamic_playlist_remove(
        self,
        context: Context,
        id: DynamicPlaylistId,
    ) -> Result<(), SerializableLibraryError> {
        info!("Removing DP with id: {id:?}");
        DynamicPlaylist::delete(&self.db, id.into())
            .await?
            .ok_or(Error::NotFound)?;
        Ok(())
    }
    /// Dynamic Playlists: get a DP by its ID
    #[instrument]
    async fn dynamic_playlist_get(
        self,
        context: Context,
        id: DynamicPlaylistId,
    ) -> Option<DynamicPlaylist> {
        info!("Getting DP by ID: {id:?}");
        DynamicPlaylist::read(&self.db, id.into())
            .await
            .tap_err(|e| warn!("Error in dynamic_playlist_get: {e}"))
            .ok()
            .flatten()
    }
    /// Dynamic Playlists: get the songs of a DP
    #[instrument]
    async fn dynamic_playlist_get_songs(
        self,
        context: Context,
        id: DynamicPlaylistId,
    ) -> Option<Box<[Song]>> {
        info!("Getting songs in DP: {id:?}");
        DynamicPlaylist::run_query_by_id(&self.db, id.into())
            .await
            .tap_err(|e| warn!("Error in dynamic_playlist_get_songs: {e}"))
            .ok()
            .flatten()
            .map(Into::into)
    }
    /// Dynamic Playlists: export dynamic playlists to a csv file
    #[instrument]
    async fn dynamic_playlist_export(
        self,
        context: Context,
        path: PathBuf,
    ) -> Result<(), SerializableLibraryError> {
        info!("Exporting dynamic playlists to: {path:?}");

        // validate the path
        validate_file_path(&path, "csv", false)?;

        // read the playlists
        let playlists = DynamicPlaylist::read_all(&self.db)
            .await
            .tap_err(|e| warn!("Error in dynamic_playlist_export: {e}"))?;

        // create the file
        let file =
            File::create(&path).tap_err(|e| warn!("Error in dynamic_playlist_export: {e}"))?;
        let writer = csv::Writer::from_writer(std::io::BufWriter::new(file));
        // write the playlists to the file
        export_dynamic_playlists(&playlists, writer)
            .tap_err(|e| warn!("Error in dynamic_playlist_export: {e}"))?;
        info!("Exported dynamic playlists to: {path:?}");
        Ok(())
    }
    /// Dynamic Playlists: import dynamic playlists from a csv file
    #[instrument]
    async fn dynamic_playlist_import(
        self,
        context: Context,
        path: PathBuf,
    ) -> Result<Vec<DynamicPlaylist>, SerializableLibraryError> {
        info!("Importing dynamic playlists from: {path:?}");

        // validate the path
        validate_file_path(&path, "csv", true)?;

        // read file
        let file = File::open(&path).tap_err(|e| warn!("Error in dynamic_playlist_import: {e}"))?;
        let reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(std::io::BufReader::new(file));

        // read the playlists from the file
        let playlists = import_dynamic_playlists(reader)
            .tap_err(|e| warn!("Error in dynamic_playlist_import: {e}"))?;

        if playlists.is_empty() {
            return Err(BackupError::NoValidPlaylists.into());
        }

        // create the playlists
        let mut ids = Vec::new();
        for playlist in playlists {
            // if a playlist with the same name already exists, skip this one
            if let Ok(Some(existing_playlist)) =
                DynamicPlaylist::read_by_name(&self.db, playlist.name.clone()).await
            {
                info!(
                    "Dynamic Playlist \"{}\" already exists, will not import",
                    existing_playlist.name
                );
                continue;
            }

            ids.push(
                DynamicPlaylist::create(&self.db, playlist)
                    .await
                    .tap_err(|e| warn!("Error in dynamic_playlist_import: {e}"))?
                    .ok_or(Error::NotCreated)?,
            );
        }

        Ok(ids)
    }
}
