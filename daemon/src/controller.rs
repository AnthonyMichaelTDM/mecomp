//----------------------------------------------------------------------------------------- std lib
use std::{fs::File, path::PathBuf, sync::Arc, time::Duration};
//--------------------------------------------------------------------------------- other libraries
use log::{debug, error, info, warn};
use surrealdb::{Surreal, engine::local::Db};
use tokio::sync::Mutex;
use tonic::{Code, Request, Response};
use tracing::{Instrument, instrument};
//-------------------------------------------------------------------------------- MECOMP libraries
use mecomp_core::{
    audio::{
        AudioKernelSender,
        commands::{AudioCommand, QueueCommand, VolumeCommand},
    },
    config::Settings,
    errors::BackupError,
    state::{RepeatMode, SeekType},
    udp::{Event, Message, Sender},
};
use mecomp_prost::{
    AlbumBriefList, AlbumBriefOption, AlbumOption, ArtistBriefList, ArtistBriefListOption,
    ArtistBriefOption, ArtistOption, CollectionFreezeRequest, CollectionList, CollectionOption,
    DynamicPlaylistCreateRequest, DynamicPlaylistList, DynamicPlaylistOption,
    DynamicPlaylistUpdateRequest, InProgressResponse, LibraryAnalyzeRequest, LibraryBrief,
    LibraryFull, LibraryHealth, Path, PingResponse, PlaybackRepeatRequest, PlaybackSeekRequest,
    PlaybackSkipRequest, PlaybackVolumeAdjustRequest, PlaybackVolumeSetRequest,
    PlaylistAddListRequest, PlaylistAddRequest, PlaylistBrief, PlaylistExportRequest,
    PlaylistImportRequest, PlaylistList, PlaylistName, PlaylistOption, PlaylistRemoveSongsRequest,
    PlaylistRenameRequest, QueueRemoveRangeRequest, QueueSetIndexRequest, RadioSimilarRequest,
    RecordId, RecordIdList, RegisterListenerRequest, SearchRequest, SearchResult, SongBriefList,
    SongBriefOption, SongOption, StateAudioResponse, Ulid, server::MusicPlayer as MusicPlayerTrait,
};
use mecomp_storage::db::schemas::{
    self,
    album::Album,
    artist::Artist,
    collection::Collection,
    dynamic::{DynamicPlaylist, DynamicPlaylistChangeSet, query::Query},
    playlist::{Playlist, PlaylistChangeSet},
    song::Song,
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
pub struct MusicPlayer {
    db: Arc<Surreal<Db>>,
    settings: Arc<Settings>,
    audio_kernel: Arc<AudioKernelSender>,
    library_rescan_lock: Arc<Mutex<()>>,
    library_analyze_lock: Arc<Mutex<()>>,
    collection_recluster_lock: Arc<Mutex<()>>,
    publisher: Arc<Sender<Message>>,
    terminator: Arc<Mutex<Terminator>>,
    interrupt: Arc<termination::InterruptReceiver>,
}

impl MusicPlayer {
    #[must_use]
    #[inline]
    pub fn new(
        db: Arc<Surreal<Db>>,
        settings: Arc<Settings>,
        audio_kernel: Arc<AudioKernelSender>,
        event_publisher: Arc<Sender<Message>>,
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
    #[inline]
    pub async fn publish(
        &self,
        message: impl Into<Message> + Send + Sync + std::fmt::Debug,
    ) -> Result<(), mecomp_core::errors::UdpError> {
        self.publisher.send(message).await
    }
}

type TonicResult<T> = std::result::Result<Response<T>, tonic::Status>;

#[allow(clippy::missing_inline_in_public_items)]
#[tonic::async_trait]
impl MusicPlayerTrait for MusicPlayer {
    #[instrument]
    async fn register_listener(
        self: Arc<Self>,
        request: Request<RegisterListenerRequest>,
    ) -> TonicResult<()> {
        let RegisterListenerRequest { host, port } = request.into_inner();
        let listener_addr = format!("{host}:{port}").parse().map_err(|e| {
            tonic::Status::invalid_argument(format!("Invalid listener address: {e}"))
        })?;
        info!("Registering listener: {listener_addr}");
        self.publisher.add_subscriber(listener_addr).await;
        Ok(Response::new(()))
    }
    async fn ping(self: Arc<Self>, _: Request<()>) -> TonicResult<PingResponse> {
        Ok(Response::new(PingResponse {
            message: "pong".to_string(),
        }))
    }
    async fn daemon_shutdown(self: Arc<Self>, _: Request<()>) -> TonicResult<()> {
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
        Ok(Response::new(()))
    }

    /// rescans the music library, only error is if a rescan is already in progress.
    #[instrument]
    async fn library_rescan(self: Arc<Self>, _: Request<()>) -> TonicResult<()> {
        info!("Rescanning library");

        if self.library_rescan_lock.try_lock().is_err() {
            warn!("Library rescan already in progress");
            return Err(tonic::Status::aborted("Library rescan already in progress"));
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

        Ok(Response::new(()))
    }
    /// Check if a rescan is in progress.
    #[instrument]
    async fn library_rescan_in_progress(
        self: Arc<Self>,
        _: Request<()>,
    ) -> TonicResult<InProgressResponse> {
        let in_progress = self.library_rescan_lock.try_lock().is_err();
        Ok(Response::new(InProgressResponse { in_progress }))
    }
    /// Analyze the music library, only error is if an analysis is already in progress.
    #[instrument]
    async fn library_analyze(
        self: Arc<Self>,
        request: Request<LibraryAnalyzeRequest>,
    ) -> TonicResult<()> {
        let overwrite = request.get_ref().overwrite;
        info!("Analyzing library");

        if self.library_analyze_lock.try_lock().is_err() {
            warn!("Library analysis already in progress");
            return Err(tonic::Status::aborted(
                "Library analysis already in progress",
            ));
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

        Ok(Response::new(()))
    }
    /// Check if an analysis is in progress.
    #[instrument]
    async fn library_analyze_in_progress(
        self: Arc<Self>,
        _: Request<()>,
    ) -> TonicResult<InProgressResponse> {
        let in_progress = self.library_analyze_lock.try_lock().is_err();
        Ok(Response::new(InProgressResponse { in_progress }))
    }
    /// Recluster the music library, only error is if a recluster is already in progress.
    #[instrument]
    async fn library_recluster(self: Arc<Self>, _: Request<()>) -> TonicResult<()> {
        info!("Reclustering collections");

        if self.collection_recluster_lock.try_lock().is_err() {
            warn!("Collection reclustering already in progress");
            return Err(tonic::Status::aborted(
                "Collection reclustering already in progress",
            ));
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

        Ok(Response::new(()))
    }
    /// Check if a recluster is in progress.
    #[instrument]
    async fn library_recluster_in_progress(
        self: Arc<Self>,
        _: Request<()>,
    ) -> TonicResult<InProgressResponse> {
        let in_progress = self.collection_recluster_lock.try_lock().is_err();
        Ok(Response::new(InProgressResponse { in_progress }))
    }

    /// Returns brief information about the music library.
    #[instrument]
    async fn library_brief(self: Arc<Self>, _: Request<()>) -> TonicResult<LibraryBrief> {
        info!("Creating library brief");
        let brief = services::library::brief(&self.db)
            .await
            .map_err(|e| tonic::Status::internal(format!("Error in library_brief: {e}")))?;
        Ok(Response::new(brief))
    }
    /// Returns full information about the music library. (all songs, artists, albums, etc.)
    #[instrument]
    async fn library_full(self: Arc<Self>, _: Request<()>) -> TonicResult<LibraryFull> {
        info!("Creating library full");
        Ok(Response::new(
            services::library::full(&self.db)
                .await
                .map_err(|e| tonic::Status::internal(format!("Error in library_full: {e}")))?,
        ))
    }
    /// Returns information about the health of the music library (are there any missing files, etc.)
    #[instrument]
    async fn library_health(self: Arc<Self>, _: Request<()>) -> TonicResult<LibraryHealth> {
        info!("Creating library health");
        Ok(Response::new(
            services::library::health(&self.db)
                .await
                .map_err(|e| tonic::Status::internal(format!("Error in library_health: {e}")))?,
        ))
    }

    #[instrument]
    async fn library_artists(self: Arc<Self>, _: Request<()>) -> TonicResult<ArtistBriefList> {
        info!("Creating library artists brief");
        let artists = Artist::read_all_brief(&self.db)
            .await
            .map_err(|e| tonic::Status::internal(format!("Error in library_artists_brief: {e}")))?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(ArtistBriefList { artists }))
    }
    #[instrument]
    async fn library_albums(self: Arc<Self>, _: Request<()>) -> TonicResult<AlbumBriefList> {
        info!("Creating library albums brief");
        let albums = Album::read_all_brief(&self.db)
            .await
            .map_err(|e| tonic::Status::internal(format!("Error in library_albums_brief: {e}")))?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(AlbumBriefList { albums }))
    }
    #[instrument]
    async fn library_songs(self: Arc<Self>, _: Request<()>) -> TonicResult<SongBriefList> {
        info!("Creating library songs brief");
        let songs = Song::read_all_brief(&self.db)
            .await
            .map_err(|e| tonic::Status::internal(format!("Error in library_songs_brief: {e}")))?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(SongBriefList { songs }))
    }
    #[instrument]
    async fn library_playlists(self: Arc<Self>, _: Request<()>) -> TonicResult<PlaylistList> {
        info!("Creating library playlists brief");
        let playlists = Playlist::read_all(&self.db)
            .await
            .map_err(|e| tonic::Status::internal(format!("Error in library_playlists_brief: {e}")))?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(PlaylistList { playlists }))
    }
    #[instrument]
    async fn library_collections(self: Arc<Self>, _: Request<()>) -> TonicResult<CollectionList> {
        info!("Creating library collections brief");
        let collections = Collection::read_all(&self.db)
            .await
            .map_err(|e| {
                tonic::Status::internal(format!("Error in library_collections_brief: {e}"))
            })?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(CollectionList { collections }))
    }
    #[instrument]
    async fn library_dynamic_playlists(
        self: Arc<Self>,
        _: Request<()>,
    ) -> TonicResult<DynamicPlaylistList> {
        info!("Creating library dynamic playlists full");
        let playlists = DynamicPlaylist::read_all(&self.db)
            .await
            .map_err(|e| {
                tonic::Status::internal(format!("Error in library_dynamic_playlists_full: {e}"))
            })?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(DynamicPlaylistList { playlists }))
    }

    /// Get a song by its ID.
    #[instrument]
    async fn library_song_get(self: Arc<Self>, request: Request<Ulid>) -> TonicResult<SongOption> {
        let id = (schemas::song::TABLE_NAME, request.into_inner().id).into();
        info!("Getting song by ID: {id}");
        let song = Song::read(&self.db, id)
            .await
            .map_err(|e| tonic::Status::internal(format!("Error in library_song_get: {e}")))?
            .map(Into::into);
        Ok(Response::new(SongOption { song }))
    }
    /// Get a song by its file path.
    #[instrument]
    async fn library_song_get_by_path(
        self: Arc<Self>,
        request: Request<Path>,
    ) -> TonicResult<SongOption> {
        let path = request.into_inner().path;
        let path = PathBuf::from(path)
            .canonicalize()
            .map_err(|e| tonic::Status::invalid_argument(format!("Invalid path provided: {e}")))?;
        info!("Getting song by path: {}", path.display());
        let song = Song::read_by_path(&self.db, path)
            .await
            .map_err(|e| {
                tonic::Status::internal(format!("Error in library_song_get_by_path: {e}"))
            })?
            .map(Into::into);
        Ok(Response::new(SongOption { song }))
    }
    /// Get the artists of a song.
    #[instrument]
    async fn library_song_get_artists(
        self: Arc<Self>,
        request: Request<Ulid>,
    ) -> TonicResult<ArtistBriefList> {
        let id = (schemas::song::TABLE_NAME, request.into_inner().id).into();
        info!("Getting artist of: {id}");
        let artists = Song::read_artist(&self.db, id)
            .await
            .map_err(|e| tonic::Status::internal(format!("Error in library_song_get_artist: {e}")))?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(ArtistBriefList { artists }))
    }
    #[instrument]
    async fn library_song_get_album(
        self: Arc<Self>,
        request: Request<Ulid>,
    ) -> TonicResult<AlbumBriefOption> {
        let song_id = (schemas::song::TABLE_NAME, request.into_inner().id).into();
        info!("Resolving album for song {song_id}");
        let album = Song::read_album(&self.db, song_id)
            .await
            .map_err(|e| tonic::Status::internal(format!("library_song_get_album failed: {e}")))?
            .map(Into::into);
        Ok(Response::new(AlbumBriefOption { album }))
    }
    #[instrument]
    async fn library_song_get_playlists(
        self: Arc<Self>,
        request: Request<Ulid>,
    ) -> TonicResult<PlaylistList> {
        let song_id = (schemas::song::TABLE_NAME, request.into_inner().id).into();
        info!("Collecting playlists containing {song_id}");
        let playlists = Song::read_playlists(&self.db, song_id)
            .await
            .map_err(|e| {
                tonic::Status::internal(format!("library_song_get_playlists failed: {e}"))
            })?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(PlaylistList { playlists }))
    }
    #[instrument]
    async fn library_song_get_collections(
        self: Arc<Self>,
        request: Request<Ulid>,
    ) -> TonicResult<CollectionList> {
        let song_id = (schemas::song::TABLE_NAME, request.into_inner().id).into();
        info!("Collecting collections containing {song_id}");
        let collections = Song::read_collections(&self.db, song_id)
            .await
            .map_err(|e| {
                tonic::Status::internal(format!("library_song_get_collections failed: {e}"))
            })?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(CollectionList { collections }))
    }
    #[instrument]
    async fn library_album_get(
        self: Arc<Self>,
        request: Request<Ulid>,
    ) -> TonicResult<AlbumOption> {
        let album_id = (schemas::album::TABLE_NAME, request.into_inner().id).into();
        info!("Fetching album {album_id}");
        let album = Album::read(&self.db, album_id)
            .await
            .map_err(|e| tonic::Status::internal(format!("library_album_get failed: {e}")))?
            .map(Into::into);
        Ok(Response::new(AlbumOption { album }))
    }
    #[instrument]
    async fn library_album_get_artists(
        self: Arc<Self>,
        request: Request<Ulid>,
    ) -> TonicResult<ArtistBriefList> {
        let album_id = (schemas::album::TABLE_NAME, request.into_inner().id).into();
        info!("Fetching contributors for album {album_id}");
        let artists = Album::read_artist(&self.db, album_id)
            .await
            .map_err(|e| tonic::Status::internal(format!("library_album_get_artist failed: {e}")))?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(ArtistBriefList { artists }))
    }
    #[instrument]
    async fn library_album_get_songs(
        self: Arc<Self>,
        request: Request<Ulid>,
    ) -> TonicResult<SongBriefList> {
        let album_id = (schemas::album::TABLE_NAME, request.into_inner().id).into();
        info!("Listing songs for album {album_id}");
        let songs = Album::read_songs(&self.db, album_id)
            .await
            .map_err(|e| tonic::Status::internal(format!("library_album_get_songs failed: {e}")))?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(SongBriefList { songs }))
    }
    #[instrument]
    async fn library_artist_get(
        self: Arc<Self>,
        request: Request<Ulid>,
    ) -> TonicResult<ArtistOption> {
        let artist_id = (schemas::artist::TABLE_NAME, request.into_inner().id).into();
        info!("Fetching artist {artist_id}");
        let artist = Artist::read(&self.db, artist_id)
            .await
            .map_err(|e| tonic::Status::internal(format!("library_artist_get failed: {e}")))?
            .map(Into::into);
        Ok(Response::new(ArtistOption { artist }))
    }
    #[instrument]
    async fn library_artist_get_songs(
        self: Arc<Self>,
        request: Request<Ulid>,
    ) -> TonicResult<SongBriefList> {
        let artist_id = (schemas::artist::TABLE_NAME, request.into_inner().id).into();
        info!("Listing songs for artist {artist_id}");
        let songs = Artist::read_songs(&self.db, artist_id)
            .await
            .map_err(|e| tonic::Status::internal(format!("library_artist_get_songs failed: {e}")))?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(SongBriefList { songs }))
    }
    #[instrument]
    async fn library_artist_get_albums(
        self: Arc<Self>,
        request: Request<Ulid>,
    ) -> TonicResult<AlbumBriefList> {
        let artist_id = (schemas::artist::TABLE_NAME, request.into_inner().id).into();
        info!("Listing albums for artist {artist_id}");
        let albums = Artist::read_albums(&self.db, artist_id)
            .await
            .map_err(|e| tonic::Status::internal(format!("library_artist_get_albums failed: {e}")))?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(AlbumBriefList { albums }))
    }
    #[instrument]
    async fn library_playlist_get(
        self: Arc<Self>,
        request: Request<Ulid>,
    ) -> TonicResult<PlaylistOption> {
        let playlist_id = (schemas::playlist::TABLE_NAME, request.into_inner().id).into();
        info!("Fetching playlist {playlist_id}");
        let playlist = Playlist::read(&self.db, playlist_id)
            .await
            .map_err(|e| tonic::Status::internal(format!("library_playlist_get failed: {e}")))?
            .map(Into::into);
        Ok(Response::new(PlaylistOption { playlist }))
    }
    #[instrument]
    async fn library_playlist_get_by_name(
        self: Arc<Self>,
        request: Request<PlaylistName>,
    ) -> TonicResult<PlaylistOption> {
        let name = request.into_inner().name;
        info!("Fetching playlist by name: {name}");
        let playlist = Playlist::read_by_name(&self.db, name)
            .await
            .map_err(|e| {
                tonic::Status::internal(format!("library_playlist_get_by_name failed: {e}"))
            })?
            .map(Into::into);
        Ok(Response::new(PlaylistOption { playlist }))
    }
    #[instrument]
    async fn library_playlist_get_songs(
        self: Arc<Self>,
        request: Request<Ulid>,
    ) -> TonicResult<SongBriefList> {
        let playlist_id = (schemas::playlist::TABLE_NAME, request.into_inner().id).into();
        info!("Listing songs for playlist {playlist_id}");
        let songs = Playlist::read_songs(&self.db, playlist_id)
            .await
            .map_err(|e| {
                tonic::Status::internal(format!("library_playlist_get_songs failed: {e}"))
            })?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(SongBriefList { songs }))
    }
    #[instrument]
    async fn library_collection_get(
        self: Arc<Self>,
        request: Request<Ulid>,
    ) -> TonicResult<CollectionOption> {
        let collection_id = (schemas::collection::TABLE_NAME, request.into_inner().id).into();
        info!("Fetching collection {collection_id}");
        let collection = Collection::read(&self.db, collection_id)
            .await
            .map_err(|e| tonic::Status::internal(format!("library_collection_get failed: {e}")))?
            .map(Into::into);
        Ok(Response::new(CollectionOption { collection }))
    }
    #[instrument]
    async fn library_collection_get_songs(
        self: Arc<Self>,
        request: Request<Ulid>,
    ) -> TonicResult<SongBriefList> {
        let collection_id = (schemas::collection::TABLE_NAME, request.into_inner().id).into();
        info!("Listing songs for collection {collection_id}");
        let songs = Collection::read_songs(&self.db, collection_id)
            .await
            .map_err(|e| {
                tonic::Status::internal(format!("library_collection_get_songs failed: {e}"))
            })?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(SongBriefList { songs }))
    }
    #[instrument]
    async fn library_dynamic_playlist_get(
        self: Arc<Self>,
        request: Request<Ulid>,
    ) -> TonicResult<DynamicPlaylistOption> {
        let dynamic_playlist_id = (schemas::dynamic::TABLE_NAME, request.into_inner().id).into();
        info!("Fetching dynamic playlist {dynamic_playlist_id}");
        let playlist = DynamicPlaylist::read(&self.db, dynamic_playlist_id)
            .await
            .map_err(|e| {
                tonic::Status::internal(format!("library_dynamic_playlist_get failed: {e}"))
            })?
            .map(Into::into);
        Ok(Response::new(DynamicPlaylistOption { playlist }))
    }
    #[instrument]
    async fn library_dynamic_playlist_get_songs(
        self: Arc<Self>,
        request: Request<Ulid>,
    ) -> TonicResult<SongBriefList> {
        let dynamic_playlist_id = (schemas::dynamic::TABLE_NAME, request.into_inner().id).into();
        info!("Listing songs for dynamic playlist {dynamic_playlist_id}");
        let songs = DynamicPlaylist::run_query_by_id(&self.db, dynamic_playlist_id)
            .await
            .map_err(|e| {
                tonic::Status::internal(format!("library_dynamic_playlist_get_songs failed: {e}"))
            })?
            .ok_or_else(|| tonic::Status::not_found("dynamic playlist not found"))?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(SongBriefList { songs }))
    }

    #[instrument]
    async fn state_audio(self: Arc<Self>, _: Request<()>) -> TonicResult<StateAudioResponse> {
        debug!("Getting state of audio player");
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.audio_kernel.send(AudioCommand::ReportStatus(tx));

        let state = rx
            .await
            .inspect_err(|e| warn!("Error in state_audio: {e}"))
            .ok()
            .map(Into::into);

        Ok(Response::new(StateAudioResponse { state }))
    }
    #[instrument]
    async fn current_artists(
        self: Arc<Self>,
        _: Request<()>,
    ) -> TonicResult<ArtistBriefListOption> {
        info!("Fetching current song artists");
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.audio_kernel.send(AudioCommand::ReportStatus(tx));

        if let Some(song) = rx
            .await
            .inspect_err(|e| warn!("Error in current_artists: {e}"))
            .ok()
            .and_then(|s| s.current_song)
        {
            let artists = Song::read_artist(&self.db, song.id)
                .await
                .map_err(|e| {
                    tonic::Status::not_found(format!("Error finding artists of current song: {e}"))
                })?
                .into_iter()
                .map(Into::into)
                .collect();
            let artists = ArtistBriefList { artists };
            Ok(Response::new(ArtistBriefListOption {
                artists: Some(artists),
            }))
        } else {
            Ok(Response::new(ArtistBriefListOption { artists: None }))
        }
    }
    #[instrument]
    async fn current_album(self: Arc<Self>, _: Request<()>) -> TonicResult<AlbumBriefOption> {
        info!("Fetching current song album");
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.audio_kernel.send(AudioCommand::ReportStatus(tx));

        if let Some(song) = rx
            .await
            .inspect_err(|e| warn!("Error in current_album: {e}"))
            .ok()
            .and_then(|s| s.current_song)
        {
            let album = Song::read_album(&self.db, song.id)
                .await
                .map_err(|e| {
                    tonic::Status::not_found(format!("Error finding album of current song: {e}"))
                })?
                .map(Into::into);
            Ok(Response::new(AlbumBriefOption { album }))
        } else {
            Ok(Response::new(AlbumBriefOption { album: None }))
        }
    }
    #[instrument]
    async fn current_song(self: Arc<Self>, _: Request<()>) -> TonicResult<SongBriefOption> {
        info!("Fetching current song");
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.audio_kernel.send(AudioCommand::ReportStatus(tx));
        let song = rx.await.ok().and_then(|s| s.current_song).map(Into::into);
        Ok(Response::new(SongBriefOption { song }))
    }

    /// Get a random artist
    #[instrument]
    async fn rand_artist(self: Arc<Self>, _: Request<()>) -> TonicResult<ArtistBriefOption> {
        info!("Getting random artist");
        let artist = Artist::read_rand(&self.db, 1)
            .await
            .map_err(|e| tonic::Status::internal(format!("rand_artist failed: {e}")))?
            .first()
            .cloned()
            .map(Into::into);
        Ok(Response::new(ArtistBriefOption { artist }))
    }
    /// Get a random album
    #[instrument]
    async fn rand_album(self: Arc<Self>, _: Request<()>) -> TonicResult<AlbumBriefOption> {
        info!("Getting random album");
        let album = Album::read_rand(&self.db, 1)
            .await
            .map_err(|e| tonic::Status::internal(format!("rand_album failed: {e}")))?
            .first()
            .cloned()
            .map(Into::into);
        Ok(Response::new(AlbumBriefOption { album }))
    }
    /// Get a random song
    #[instrument]
    async fn rand_song(self: Arc<Self>, _: Request<()>) -> TonicResult<SongBriefOption> {
        info!("Getting random song");
        let song = Song::read_rand(&self.db, 1)
            .await
            .map_err(|e| tonic::Status::internal(format!("rand_song failed: {e}")))?
            .first()
            .cloned()
            .map(Into::into);
        Ok(Response::new(SongBriefOption { song }))
    }

    /// returns a list of artists, albums, and songs matching the given search query.
    async fn search(self: Arc<Self>, request: Request<SearchRequest>) -> TonicResult<SearchResult> {
        let SearchRequest { query, limit } = request.into_inner();
        info!("Searching for: {query}");
        // basic idea:
        // 1. search for songs
        // 2. search for albums
        // 3. search for artists
        // 4. return the results
        let songs = Song::search(
            &self.db,
            &query,
            usize::try_from(limit).unwrap_or(usize::MAX),
        )
        .await
        .inspect_err(|e| warn!("Error in search: {e}"))
        .unwrap_or_default()
        .into_iter()
        .map(Into::into)
        .collect();

        let albums = Album::search(
            &self.db,
            &query,
            usize::try_from(limit).unwrap_or(usize::MAX),
        )
        .await
        .inspect_err(|e| warn!("Error in search: {e}"))
        .unwrap_or_default()
        .into_iter()
        .map(Into::into)
        .collect();

        let artists = Artist::search(
            &self.db,
            &query,
            usize::try_from(limit).unwrap_or(usize::MAX),
        )
        .await
        .inspect_err(|e| warn!("Error in search: {e}"))
        .unwrap_or_default()
        .into_iter()
        .map(Into::into)
        .collect();
        Ok(Response::new(SearchResult {
            songs,
            albums,
            artists,
        }))
    }
    /// returns a list of artists matching the given search query.
    #[instrument]
    async fn search_artist(
        self: Arc<Self>,
        request: Request<SearchRequest>,
    ) -> TonicResult<ArtistBriefList> {
        let SearchRequest { query, limit } = request.into_inner();
        info!("Searching for artist: {query}");
        let artists = Artist::search(
            &self.db,
            &query,
            usize::try_from(limit).unwrap_or(usize::MAX),
        )
        .await
        .inspect_err(|e| {
            warn!("Error in search_artist: {e}");
        })
        .unwrap_or_default()
        .into_iter()
        .map(Into::into)
        .collect();
        Ok(Response::new(ArtistBriefList { artists }))
    }
    /// returns a list of albums matching the given search query.
    #[instrument]
    async fn search_album(
        self: Arc<Self>,
        request: Request<SearchRequest>,
    ) -> TonicResult<AlbumBriefList> {
        let SearchRequest { query, limit } = request.into_inner();
        info!("Searching for album: {query}");
        let albums = Album::search(
            &self.db,
            &query,
            usize::try_from(limit).unwrap_or(usize::MAX),
        )
        .await
        .inspect_err(|e| {
            warn!("Error in search_album: {e}");
        })
        .unwrap_or_default()
        .into_iter()
        .map(Into::into)
        .collect();
        Ok(Response::new(AlbumBriefList { albums }))
    }
    /// returns a list of songs matching the given search query.
    #[instrument]
    async fn search_song(
        self: Arc<Self>,
        request: Request<SearchRequest>,
    ) -> TonicResult<SongBriefList> {
        let SearchRequest { query, limit } = request.into_inner();
        info!("Searching for song: {query}");
        let songs = Song::search(
            &self.db,
            &query,
            usize::try_from(limit).unwrap_or(usize::MAX),
        )
        .await
        .inspect_err(|e| {
            warn!("Error in search_song: {e}");
        })
        .unwrap_or_default()
        .into_iter()
        .map(Into::into)
        .collect();
        Ok(Response::new(SongBriefList { songs }))
    }

    /// toggles playback (play/pause)
    #[instrument]
    async fn playback_toggle(self: Arc<Self>, _: Request<()>) -> TonicResult<()> {
        info!("Toggling playback");
        self.audio_kernel.send(AudioCommand::TogglePlayback);
        Ok(Response::new(()))
    }
    /// starts playback (unpause).
    #[instrument]
    async fn playback_play(self: Arc<Self>, _: Request<()>) -> TonicResult<()> {
        info!("Playing");
        self.audio_kernel.send(AudioCommand::Play);
        Ok(Response::new(()))
    }
    /// pause playback.
    #[instrument]
    async fn playback_pause(self: Arc<Self>, _: Request<()>) -> TonicResult<()> {
        info!("Pausing playback");
        self.audio_kernel.send(AudioCommand::Pause);
        Ok(Response::new(()))
    }
    /// stop playback.
    #[instrument]
    async fn playback_stop(self: Arc<Self>, _: Request<()>) -> TonicResult<()> {
        info!("Stopping playback");
        self.audio_kernel.send(AudioCommand::Stop);
        Ok(Response::new(()))
    }
    /// restart the current song.
    #[instrument]
    async fn playback_restart(self: Arc<Self>, _: Request<()>) -> TonicResult<()> {
        info!("Restarting current song");
        self.audio_kernel.send(AudioCommand::RestartSong);
        Ok(Response::new(()))
    }
    /// skip forward by the given amount of songs
    #[instrument]
    async fn playback_skip_forward(
        self: Arc<Self>,
        request: Request<PlaybackSkipRequest>,
    ) -> TonicResult<()> {
        let PlaybackSkipRequest { amount } = request.into_inner();
        info!("Skipping forward by {amount} songs");
        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::SkipForward(
                usize::try_from(amount).unwrap_or(usize::MAX),
            )));
        Ok(Response::new(()))
    }
    /// go backwards by the given amount of songs.
    #[instrument]
    async fn playback_skip_backward(
        self: Arc<Self>,
        request: Request<PlaybackSkipRequest>,
    ) -> TonicResult<()> {
        let PlaybackSkipRequest { amount } = request.into_inner();
        info!("Going back by {amount} songs");
        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::SkipBackward(
                usize::try_from(amount).unwrap_or(usize::MAX),
            )));
        Ok(Response::new(()))
    }
    /// stop playback.
    /// (clears the queue and stops playback)
    #[instrument]
    async fn playback_clear_player(self: Arc<Self>, _: Request<()>) -> TonicResult<()> {
        info!("Stopping playback");
        self.audio_kernel.send(AudioCommand::ClearPlayer);
        Ok(Response::new(()))
    }
    /// clear the queue.
    #[instrument]
    async fn playback_clear(self: Arc<Self>, _: Request<()>) -> TonicResult<()> {
        info!("Clearing queue and stopping playback");
        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::Clear));
        Ok(Response::new(()))
    }
    /// seek forwards, backwards, or to an absolute second in the current song.
    #[instrument]
    async fn playback_seek(
        self: Arc<Self>,
        request: Request<PlaybackSeekRequest>,
    ) -> TonicResult<()> {
        let PlaybackSeekRequest { seek, duration } = request.into_inner();
        let duration: Duration = duration.normalized().try_into().map_err(|e| {
            tonic::Status::invalid_argument(format!("Invalid duration provided: {e}"))
        })?;
        let seek: SeekType = mecomp_prost::SeekType::try_from(seek)
            .map_err(|e| tonic::Status::invalid_argument(e.to_string()))?
            .into();
        info!("Seeking {seek} by {:.2}s", duration.as_secs_f32());
        self.audio_kernel.send(AudioCommand::Seek(seek, duration));
        Ok(Response::new(()))
    }
    /// set the repeat mode.
    #[instrument]
    async fn playback_repeat(
        self: Arc<Self>,
        request: Request<PlaybackRepeatRequest>,
    ) -> TonicResult<()> {
        let PlaybackRepeatRequest { mode } = request.into_inner();
        let mode: RepeatMode = mecomp_prost::RepeatMode::try_from(mode)
            .map_err(|e| tonic::Status::invalid_argument(e.to_string()))?
            .into();
        info!("Setting repeat mode to: {mode}");
        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::SetRepeatMode(mode)));
        Ok(Response::new(()))
    }
    /// Shuffle the current queue, then start playing from the 1st Song in the queue.
    #[instrument]
    async fn playback_shuffle(self: Arc<Self>, _: Request<()>) -> TonicResult<()> {
        info!("Shuffling queue");
        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::Shuffle));
        Ok(Response::new(()))
    }
    /// set the volume to the given value
    /// The value `1.0` is the "normal" volume (unfiltered input). Any value other than `1.0` will multiply each sample by this value.
    #[instrument]
    async fn playback_volume(
        self: Arc<Self>,
        request: Request<PlaybackVolumeSetRequest>,
    ) -> TonicResult<()> {
        let PlaybackVolumeSetRequest { volume } = request.into_inner();
        info!("Setting volume to: {volume}",);
        self.audio_kernel
            .send(AudioCommand::Volume(VolumeCommand::Set(volume)));
        Ok(Response::new(()))
    }
    /// increase the volume by the given amount
    #[instrument]
    async fn playback_volume_up(
        self: Arc<Self>,
        request: Request<PlaybackVolumeAdjustRequest>,
    ) -> TonicResult<()> {
        let PlaybackVolumeAdjustRequest { amount } = request.into_inner();
        info!("Increasing volume by: {amount}",);
        self.audio_kernel
            .send(AudioCommand::Volume(VolumeCommand::Up(amount)));
        Ok(Response::new(()))
    }
    /// decrease the volume by the given amount
    #[instrument]
    async fn playback_volume_down(
        self: Arc<Self>,
        request: Request<PlaybackVolumeAdjustRequest>,
    ) -> TonicResult<()> {
        let PlaybackVolumeAdjustRequest { amount } = request.into_inner();
        info!("Decreasing volume by: {amount}",);
        self.audio_kernel
            .send(AudioCommand::Volume(VolumeCommand::Down(amount)));
        Ok(Response::new(()))
    }
    /// toggle the volume mute.
    #[instrument]
    async fn playback_toggle_mute(self: Arc<Self>, _: Request<()>) -> TonicResult<()> {
        info!("Toggling volume mute");
        self.audio_kernel
            .send(AudioCommand::Volume(VolumeCommand::ToggleMute));
        Ok(Response::new(()))
    }
    /// mute the volume.
    #[instrument]
    async fn playback_mute(self: Arc<Self>, _: Request<()>) -> TonicResult<()> {
        info!("Muting volume");
        self.audio_kernel
            .send(AudioCommand::Volume(VolumeCommand::Mute));
        Ok(Response::new(()))
    }
    /// unmute the volume.
    #[instrument]
    async fn playback_unmute(self: Arc<Self>, _: Request<()>) -> TonicResult<()> {
        info!("Unmuting volume");
        self.audio_kernel
            .send(AudioCommand::Volume(VolumeCommand::Unmute));
        Ok(Response::new(()))
    }

    /// add a song to the queue.
    /// (if the queue is empty, it will start playing the song.)
    #[instrument]
    async fn queue_add(self: Arc<Self>, request: Request<RecordId>) -> TonicResult<()> {
        let thing = request.into_inner().into();
        info!("Adding thing to queue: {thing}");

        let songs = services::get_songs_from_things(&self.db, &[thing])
            .await
            .map_err(|e| {
                tonic::Status::internal(format!("Error getting songs from provided things: {e}"))
            })?;

        if songs.is_empty() {
            return Err(tonic::Status::not_found("No songs found"));
        }

        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::AddToQueue(
                songs.into_iter().map(Into::into).collect(),
            )));

        Ok(Response::new(()))
    }
    /// add a list of things to the queue.
    /// (if the queue is empty, it will start playing the first thing in the list.)
    #[instrument]
    async fn queue_add_list(self: Arc<Self>, request: Request<RecordIdList>) -> TonicResult<()> {
        let RecordIdList { ids } = request.into_inner();
        let list = ids.into_iter().map(Into::into).collect::<Vec<_>>();

        info!(
            "Adding list to queue: ({})",
            list.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );

        // go through the list, and get songs for each thing (depending on what it is)
        let songs: OneOrMany<Song> = services::get_songs_from_things(&self.db, &list)
            .await
            .map_err(|e| {
                tonic::Status::internal(format!("Error getting songs from provided things: {e}"))
            })?;

        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::AddToQueue(
                songs.into_iter().map(Into::into).collect(),
            )));

        Ok(Response::new(()))
    }
    /// set the current song to a queue index.
    /// if the index is out of bounds, it will be clamped to the nearest valid index.
    #[instrument]
    async fn queue_set_index(
        self: Arc<Self>,
        request: Request<QueueSetIndexRequest>,
    ) -> TonicResult<()> {
        let QueueSetIndexRequest { index } = request.into_inner();
        info!("Setting queue index to: {index}");

        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::SetPosition(
                usize::try_from(index).unwrap_or(usize::MAX),
            )));
        Ok(Response::new(()))
    }
    /// remove a range of songs from the queue.
    /// if the range is out of bounds, it will be clamped to the nearest valid range.
    #[instrument]
    async fn queue_remove_range(
        self: Arc<Self>,
        request: Request<QueueRemoveRangeRequest>,
    ) -> TonicResult<()> {
        let QueueRemoveRangeRequest { start, end } = request.into_inner();
        let start = usize::try_from(start).map_err(|e| {
            tonic::Status::invalid_argument(format!("Invalid start index for range: {e}"))
        })?;
        let end = usize::try_from(end).map_err(|e| {
            tonic::Status::invalid_argument(format!("Invalid end index for range: {e}"))
        })?;
        let range = start..end;
        info!("Removing queue range: {range:?}");

        self.audio_kernel
            .send(AudioCommand::Queue(QueueCommand::RemoveRange(range)));
        Ok(Response::new(()))
    }

    /// create a new playlist.
    /// if a playlist with the same name already exists, this will return that playlist's id in the error variant
    #[instrument]
    async fn playlist_get_or_create(
        self: Arc<Self>,
        request: Request<PlaylistName>,
    ) -> TonicResult<RecordId> {
        let PlaylistName { name } = request.into_inner();
        info!("Creating new playlist: {name}");

        // see if a playlist with that name already exists
        match Playlist::read_by_name(&self.db, name.clone()).await {
            Ok(Some(playlist)) => {
                return Ok(Response::new(RecordId::new(
                    playlist.id.table(),
                    playlist.id.key(),
                )));
            }
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
        .map_err(|e| {
            tonic::Status::internal(format!(
                "Error in playlist_new (creating new playlist): {e}"
            ))
        })? {
            Some(playlist) => Ok(Response::new(RecordId::new(
                playlist.id.table(),
                playlist.id.key(),
            ))),
            None => Err(tonic::Status::not_found("playlist was not created")),
        }
    }
    /// remove a playlist.
    #[instrument]
    async fn playlist_remove(self: Arc<Self>, request: Request<Ulid>) -> TonicResult<()> {
        let id = (schemas::playlist::TABLE_NAME, request.into_inner().id).into();
        info!("Removing playlist with id: {id}");
        Playlist::delete(&self.db, id)
            .await
            .map_err(|e| tonic::Status::internal(format!("failed to delete playlist, {e}")))?
            .ok_or_else(|| tonic::Status::not_found("playlist was not found"))?;
        Ok(Response::new(()))
    }
    /// clone a playlist.
    /// (creates a new playlist with the same name (append " (copy)") and contents as the given playlist.)
    /// returns the id of the new playlist
    #[instrument]
    async fn playlist_clone(self: Arc<Self>, request: Request<Ulid>) -> TonicResult<RecordId> {
        let id = (schemas::playlist::TABLE_NAME, request.into_inner().id).into();
        info!("Cloning playlist with id: {id}");

        let new = Playlist::create_copy(&self.db, id)
            .await
            .map_err(|e| tonic::Status::internal(format!("failed to clone playlist, {e}")))?
            .ok_or_else(|| tonic::Status::not_found("playlist was not found"))?;

        Ok(Response::new(RecordId::new(new.id.table(), new.id.key())))
    }
    /// remove a list of songs from a playlist.
    /// if the songs are not in the playlist, this will do nothing.
    #[instrument]
    async fn playlist_remove_songs(
        self: Arc<Self>,
        request: Request<PlaylistRemoveSongsRequest>,
    ) -> TonicResult<()> {
        let PlaylistRemoveSongsRequest {
            playlist_id,
            song_ids,
        } = request.into_inner();
        let playlist = (schemas::playlist::TABLE_NAME, playlist_id.id).into();
        let songs = song_ids
            .into_iter()
            .map(|id| (schemas::song::TABLE_NAME, id.id).into())
            .collect::<Vec<_>>();
        info!("Removing song from playlist: {playlist} ({songs:?})");

        Playlist::remove_songs(&self.db, playlist, songs)
            .await
            .map_err(|e| {
                tonic::Status::internal(format!("failed to remove songs from playlist, {e}"))
            })?;
        Ok(Response::new(()))
    }
    /// Add a thing to a playlist.
    /// If the thing is something that has songs (an album, artist, etc.), it will add all the songs.
    #[instrument]
    async fn playlist_add(
        self: Arc<Self>,
        request: Request<PlaylistAddRequest>,
    ) -> TonicResult<()> {
        let PlaylistAddRequest {
            playlist_id,
            record_id,
        } = request.into_inner();
        let playlist = (schemas::playlist::TABLE_NAME, playlist_id.id).into();
        let thing = record_id.into();
        info!("Adding thing to playlist: {playlist} ({thing})");

        // get songs for the thing
        let songs: OneOrMany<Song> = services::get_songs_from_things(&self.db, &[thing])
            .await
            .map_err(|e| {
                tonic::Status::internal(format!("failed to get songs from things, {e}"))
            })?;

        Playlist::add_songs(
            &self.db,
            playlist,
            songs.into_iter().map(|s| s.id).collect::<Vec<_>>(),
        )
        .await
        .map_err(|e| tonic::Status::internal(format!("Error adding things to playlist: {e}")))?;
        Ok(Response::new(()))
    }
    /// Add a list of things to a playlist.
    /// If the things are something that have songs (an album, artist, etc.), it will add all the songs.
    #[instrument]
    async fn playlist_add_list(
        self: Arc<Self>,
        request: Request<PlaylistAddListRequest>,
    ) -> TonicResult<()> {
        let PlaylistAddListRequest {
            playlist_id,
            record_ids,
        } = request.into_inner();
        let playlist = (schemas::playlist::TABLE_NAME, playlist_id.id).into();
        let list = record_ids.into_iter().map(Into::into).collect::<Vec<_>>();
        info!(
            "Adding list to playlist: {playlist} ({})",
            list.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );

        // go through the list, and get songs for each thing (depending on what it is)
        let songs: OneOrMany<Song> = services::get_songs_from_things(&self.db, &list)
            .await
            .map_err(|e| {
                tonic::Status::internal(format!("failed to get songs from things, {e}"))
            })?;

        Playlist::add_songs(
            &self.db,
            playlist,
            songs.into_iter().map(|s| s.id).collect::<Vec<_>>(),
        )
        .await
        .map_err(|e| tonic::Status::internal(format!("failed to add songs to playlist, {e}")))?;
        Ok(Response::new(()))
    }
    /// Rename a playlist.
    #[instrument]
    async fn playlist_rename(
        self: Arc<Self>,
        request: Request<PlaylistRenameRequest>,
    ) -> TonicResult<PlaylistBrief> {
        let PlaylistRenameRequest { playlist_id, name } = request.into_inner();
        let id = (schemas::playlist::TABLE_NAME, playlist_id.id).into();
        info!("Renaming playlist: {id} ({name})");
        let updated = Playlist::update(&self.db, id, PlaylistChangeSet::new().name(name))
            .await
            .map_err(|e| tonic::Status::internal(format!("failed to rename playlist, {e}")))?
            .ok_or_else(|| tonic::Status::not_found("playlist not found"))?;
        Ok(Response::new(updated.into()))
    }
    /// Export a playlist to a .m3u file
    #[instrument]
    async fn playlist_export(
        self: Arc<Self>,
        request: Request<PlaylistExportRequest>,
    ) -> TonicResult<()> {
        let PlaylistExportRequest { playlist_id, path } = request.into_inner();
        let id = (schemas::playlist::TABLE_NAME, playlist_id.id).into();
        info!("Exporting playlist to: {path}");

        // validate the path
        validate_file_path(&path, "m3u", false)
            .map_err(|e| tonic::Status::invalid_argument(format!("invalid file path: {e}")))?;

        // read the playlist
        let playlist = Playlist::read(&self.db, id)
            .await
            .inspect_err(|e| warn!("Error in playlist_export: {e}"))
            .ok()
            .flatten()
            .ok_or_else(|| tonic::Status::not_found("playlist not found"))?;
        // get the songs in the playlist
        let songs = Playlist::read_songs(&self.db, playlist.id)
            .await
            .inspect_err(|e| warn!("Error in playlist_export: {e}"))
            .ok()
            .unwrap_or_default();

        // create the file
        let file = File::create(&path).inspect_err(|e| warn!("Error in playlist_export: {e}"))?;
        // write the playlist to the file
        export_playlist(&playlist.name, &songs, file)
            .inspect_err(|e| warn!("Error in playlist_export: {e}"))
            .map_err(|e| tonic::Status::internal(e.to_string()))?;
        info!("Exported playlist to: {path}");
        Ok(Response::new(()))
    }
    /// Import a playlist from a .m3u file
    #[instrument]
    async fn playlist_import(
        self: Arc<Self>,
        request: Request<PlaylistImportRequest>,
    ) -> TonicResult<RecordId> {
        let PlaylistImportRequest { path, name } = request.into_inner();

        info!("Importing playlist from: {path}");

        // validate the path
        validate_file_path(&path, "m3u", true)
            .map_err(|e| tonic::Status::invalid_argument(format!("invalid file path: {e}")))?;

        // read file
        let file = File::open(&path).inspect_err(|e| warn!("Error in playlist_import: {e}"))?;
        let (parsed_name, song_paths) = import_playlist(file)
            .inspect_err(|e| warn!("Error in playlist_import: {e}"))
            .map_err(|e| tonic::Status::internal(e.to_string()))?;

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
            return Ok(Response::new(RecordId::new(
                playlist.id.table(),
                playlist.id.key(),
            )));
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
        .inspect_err(|e| warn!("Error in playlist_import: {e}"))
        .map_err(|e| tonic::Status::internal(e.to_string()))?
        .ok_or_else(|| tonic::Status::not_found("failed to create playlist"))?;

        // lookup all the songs
        let mut songs = Vec::new();
        for path in &song_paths {
            let Some(song) = Song::read_by_path(&self.db, path.clone())
                .await
                .inspect_err(|e| warn!("Error in playlist_import: {e}"))
                .map_err(|e| tonic::Status::internal(e.to_string()))?
            else {
                warn!("Song at {} not found in the library", path.display());
                continue;
            };

            songs.push(song.id);
        }

        if songs.is_empty() {
            return Err(tonic::Status::new(
                Code::InvalidArgument,
                BackupError::NoValidSongs(song_paths.len()).to_string(),
            ));
        }

        // add the songs to the playlist
        Playlist::add_songs(&self.db, playlist.id.clone(), songs)
            .await
            .inspect_err(|e| {
                warn!("Error in playlist_import: {e}");
            })
            .map_err(|e| tonic::Status::internal(e.to_string()))?;

        // return the playlist id
        Ok(Response::new(RecordId::new(
            playlist.id.table(),
            playlist.id.key(),
        )))
    }

    /// Collections: freeze a collection (convert it to a playlist).
    #[instrument]
    async fn collection_freeze(
        self: Arc<Self>,
        request: Request<CollectionFreezeRequest>,
    ) -> TonicResult<RecordId> {
        let CollectionFreezeRequest { id, name } = request.into_inner();
        let id = (schemas::collection::TABLE_NAME, id.id).into();
        info!("Freezing collection: {id:?} ({name})");
        let playlist = Collection::freeze(&self.db, id, name)
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;
        Ok(Response::new(RecordId::new(
            playlist.id.table(),
            playlist.id.key(),
        )))
    }

    /// Radio: get the `n` most similar songs to the given things.
    #[instrument]
    async fn radio_get_similar(
        self: Arc<Self>,
        request: Request<RadioSimilarRequest>,
    ) -> TonicResult<SongBriefList> {
        let RadioSimilarRequest { record_ids, limit } = request.into_inner();
        let things = record_ids.into_iter().map(Into::into).collect();
        info!("Getting the {limit} most similar songs to: {things:?}");
        let songs = services::radio::get_similar(&self.db, things, limit, &self.settings.analysis)
            .await
            .inspect_err(|e| warn!("Error in radio_get_similar: {e}"))
            .map_err(|e| tonic::Status::internal(e.to_string()))?
            .into_iter()
            .map(|s| s.brief().into())
            .collect();
        Ok(Response::new(SongBriefList { songs }))
    }
    /// Radio: get the ids of the `n` most similar songs to the given things.
    #[instrument]
    async fn radio_get_similar_ids(
        self: Arc<Self>,
        request: Request<RadioSimilarRequest>,
    ) -> TonicResult<RecordIdList> {
        let RadioSimilarRequest { record_ids, limit } = request.into_inner();
        let things = record_ids.into_iter().map(Into::into).collect();
        info!("Getting the {limit} most similar songs to: {things:?}");
        let ids = services::radio::get_similar(&self.db, things, limit, &self.settings.analysis)
            .await
            .inspect_err(|e| warn!("Error in radio_get_similar_songs: {e}"))
            .map_err(|e| tonic::Status::internal(e.to_string()))?
            .into_iter()
            .map(|song| RecordId::new(song.id.table(), song.id.key()))
            .collect();
        Ok(Response::new(RecordIdList::new(ids)))
    }

    /// Dynamic Playlists: create a new DP with the given name and query
    #[instrument]
    async fn dynamic_playlist_create(
        self: Arc<Self>,
        request: Request<DynamicPlaylistCreateRequest>,
    ) -> TonicResult<RecordId> {
        let DynamicPlaylistCreateRequest { name, query } = request.into_inner();
        let query = query
            .parse::<Query>()
            .map_err(|e| tonic::Status::invalid_argument(e.to_string()))?;
        let id = DynamicPlaylist::generate_id();
        info!("Creating new DP: {id:?} ({name})");

        match DynamicPlaylist::create(&self.db, DynamicPlaylist { id, name, query })
            .await
            .inspect_err(|e| warn!("Error in dynamic_playlist_create: {e}"))
            .map_err(|e| tonic::Status::internal(e.to_string()))?
        {
            Some(dp) => Ok(Response::new(RecordId::new(dp.id.table(), dp.id.key()))),
            None => Err(tonic::Status::not_found(
                "failed to create dynamic playlist",
            )),
        }
    }
    /// Dynamic Playlists: update a DP
    #[instrument]
    async fn dynamic_playlist_update(
        self: Arc<Self>,
        request: Request<DynamicPlaylistUpdateRequest>,
    ) -> TonicResult<mecomp_prost::DynamicPlaylist> {
        let DynamicPlaylistUpdateRequest { id, changes } = request.into_inner();
        let query = if let Some(new_query) = changes.new_query {
            Some(
                new_query
                    .parse::<Query>()
                    .map_err(|e| tonic::Status::invalid_argument(e.to_string()))?,
            )
        } else {
            None
        };
        let id = (schemas::dynamic::TABLE_NAME, id.id).into();
        let changes = DynamicPlaylistChangeSet {
            name: changes.new_name,
            query,
        };
        info!("Updating DP: {id:?}, {changes:?}");
        let updated = DynamicPlaylist::update(&self.db, id, changes)
            .await
            .inspect_err(|e| warn!("Error in dynamic_playlist_update: {e}"))
            .map_err(|e| tonic::Status::internal(e.to_string()))?
            .ok_or_else(|| tonic::Status::not_found("Dynamic Playlist not found"))?
            .into();
        Ok(Response::new(updated))
    }
    /// Dynamic Playlists: remove a DP
    #[instrument]
    async fn dynamic_playlist_remove(self: Arc<Self>, request: Request<Ulid>) -> TonicResult<()> {
        let id = (schemas::dynamic::TABLE_NAME, request.into_inner().id).into();
        info!("Removing DP with id: {id:?}");
        DynamicPlaylist::delete(&self.db, id)
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?
            .ok_or_else(|| tonic::Status::not_found("Dynamic Playlist not found"))?;
        Ok(Response::new(()))
    }
    /// Dynamic Playlists: export dynamic playlists to a csv file
    #[instrument]
    async fn dynamic_playlist_export(
        self: Arc<Self>,
        request: Request<mecomp_prost::Path>,
    ) -> TonicResult<()> {
        let path = request.into_inner().path;
        info!("Exporting dynamic playlists to: {path}");

        // validate the path
        validate_file_path(&path, "csv", false)
            .map_err(|e| tonic::Status::invalid_argument(format!("Backup Error: {e}")))?;

        // read the playlists
        let playlists = DynamicPlaylist::read_all(&self.db)
            .await
            .inspect_err(|e| warn!("Error in dynamic_playlist_export: {e}"))
            .map_err(|e| tonic::Status::internal(e.to_string()))?;

        // create the file
        let file =
            File::create(&path).inspect_err(|e| warn!("Error in dynamic_playlist_export: {e}"))?;
        let writer = csv::Writer::from_writer(std::io::BufWriter::new(file));
        // write the playlists to the file
        export_dynamic_playlists(&playlists, writer)
            .inspect_err(|e| warn!("Error in dynamic_playlist_export: {e}"))
            .map_err(|e| tonic::Status::internal(e.to_string()))?;
        info!("Exported dynamic playlists to: {path}");
        Ok(Response::new(()))
    }
    /// Dynamic Playlists: import dynamic playlists from a csv file
    #[instrument]
    async fn dynamic_playlist_import(
        self: Arc<Self>,
        request: Request<mecomp_prost::Path>,
    ) -> TonicResult<DynamicPlaylistList> {
        let path = request.into_inner().path;
        info!("Importing dynamic playlists from: {path}");

        // validate the path
        validate_file_path(&path, "csv", true)
            .map_err(|e| tonic::Status::invalid_argument(format!("Backup Error: {e}")))?;

        // read file
        let file =
            File::open(&path).inspect_err(|e| warn!("Error in dynamic_playlist_import: {e}"))?;
        let reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(std::io::BufReader::new(file));

        // read the playlists from the file
        let playlists = import_dynamic_playlists(reader)
            .inspect_err(|e| warn!("Error in dynamic_playlist_import: {e}"))
            .map_err(|e| tonic::Status::internal(format!("Backup Error: {e}")))?;

        if playlists.is_empty() {
            return Err(tonic::Status::new(
                Code::InvalidArgument,
                format!("Backup Error: {}", BackupError::NoValidPlaylists),
            ));
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
                    .inspect_err(|e| warn!("Error in dynamic_playlist_import: {e}"))
                    .map_err(|e| tonic::Status::internal(e.to_string()))?
                    .ok_or_else(|| tonic::Status::internal("Failed to create Dynamic Playlist"))?
                    .into(),
            );
        }

        Ok(Response::new(DynamicPlaylistList { playlists: ids }))
    }
}
