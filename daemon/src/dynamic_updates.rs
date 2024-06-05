//! Module for handling dynamic updates to the music library.
//!
//! This module is only available when the `dynamic_updates` feature is enabled.
//!
//! The `init_music_library_watcher`
use std::{path::PathBuf, sync::Arc, time::Duration};

use log::{debug, error, warn};
use mecomp_storage::db::schemas::song::{Song, SongChangeSet, SongMetadata};
use notify::{
    event::{CreateKind, MetadataKind, ModifyKind, RemoveKind, RenameMode},
    EventKind, INotifyWatcher, RecursiveMode, Watcher,
};
use notify_debouncer_full::{
    new_debouncer, DebounceEventHandler, DebounceEventResult, DebouncedEvent, Debouncer, FileIdMap,
};
use surrealdb::{engine::local::Db, Surreal};
use walkdir::{DirEntry, WalkDir};

const VALID_AUDIO_EXTENSIONS: [&str; 5] = ["flac", "mp3", "m4a", "ogg", "wav"];

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map_or(false, |s| s.starts_with('.'))
}

/// uses the notify crate to update
/// the internal music library (database) when changes to configured
/// music library directories are detected.
///
/// this watcher is terminated when the returned value is dropped.
///
/// # Arguments
///
/// * `library_paths` - The root paths of the music library.
/// * `db` - The database connection used to update the library.
///
/// # Returns
///
/// If the watchers were successfully started, it is returned.
/// it will stop when it is dropped.
///
/// # Errors
///
/// If the watcher could not be started, an error is returned.
pub fn init_music_library_watcher(
    db: Arc<Surreal<Db>>,
    library_paths: &[PathBuf],
    artist_name_separator: Option<String>,
    genre_separator: Option<String>,
) -> anyhow::Result<Debouncer<INotifyWatcher, FileIdMap>> {
    // Select recommended watcher for debouncer.
    // Using a callback here, could also be a channel.
    let mut debouncer: Debouncer<INotifyWatcher, FileIdMap> = new_debouncer(
        Duration::from_secs(2),
        None,
        MusicLibEventHandler::new(db, artist_name_separator, genre_separator),
    )?;

    // Add all library paths to the debouncer.
    for path in library_paths {
        // Add a path to be watched. All files and directories at that path and
        // below will be monitored for changes.
        debouncer.watcher().watch(path, RecursiveMode::Recursive)?;

        if path.is_dir() {
            // Add the same path to the file ID cache. The cache uses unique file IDs
            // provided by the file system and is used to stich together rename events
            // in case the notification back-end doesn't emit rename cookies.
            debouncer.cache().add_root(path, RecursiveMode::Recursive);
        }
    }

    Ok(debouncer)
}

/// Handles incoming file Events.
struct MusicLibEventHandler {
    db: Arc<Surreal<Db>>,
    artist_name_separator: Option<String>,
    genre_separator: Option<String>,
}

impl DebounceEventHandler for MusicLibEventHandler {
    fn handle_event(&mut self, result: DebounceEventResult) {
        match result {
            Ok(events) => {
                for event in events {
                    if let Err(e) = self.handle_event(event) {
                        error!("failed to handle event: {:?}", e);
                    }
                }
            }
            Err(errors) => {
                for error in errors {
                    error!("watch error: {error:?}");
                }
            }
        }
    }
}

impl MusicLibEventHandler {
    /// Creates a new `MusicLibEventHandler`.
    pub fn new(
        db: Arc<Surreal<Db>>,
        artist_name_separator: Option<String>,
        genre_separator: Option<String>,
    ) -> Self {
        Self {
            db,
            artist_name_separator,
            genre_separator,
        }
    }

    /// Handles incoming file Events.
    fn handle_event(&mut self, event: DebouncedEvent) -> anyhow::Result<()> {
        debug!("file event detected: {:?}", event);

        match event.kind {
            // remove events
            EventKind::Remove(kind) => {
                futures::executor::block_on(self.remove_event_handler(event, kind))?;
            }
            // create events
            EventKind::Create(kind) => {
                futures::executor::block_on(self.create_event_handler(event, kind))?;
            }
            // modify events
            EventKind::Modify(kind) => {
                futures::executor::block_on(self.modify_event_handler(event, kind))?;
            }
            // other events
            EventKind::Any => {
                warn!("unhandled event (Any): {:?}", event.paths);
            }
            EventKind::Other => {
                warn!("unhandled event (Other): {:?}", event.paths);
            }
            EventKind::Access(_) => {}
        }

        Ok(())
    }

    // handler for remove events
    async fn remove_event_handler(
        &self,
        event: DebouncedEvent,
        kind: RemoveKind,
    ) -> anyhow::Result<()> {
        match kind {
            RemoveKind::File => {
                if let Some(path) = event.paths.first() {
                    match path.extension().map(|ext| ext.to_str()) {
                        Some(Some(ext)) if VALID_AUDIO_EXTENSIONS.contains(&ext) => {
                            debug!("file removed: {:?}. removing from db", event.paths);
                            let song = Song::read_by_path(self.db.as_ref(), path.clone()).await?;
                            if let Some(song) = song {
                                Song::delete(self.db.as_ref(), song.id).await?;
                            }
                        }
                        _ => {
                            debug!(
                                "file removed: {:?}. not a song, no action needed",
                                event.paths
                            );
                        }
                    }
                }
            }
            RemoveKind::Folder => {
                match event.paths.first() {
                    Some(dir_path) if dir_path.is_dir() => {
                        debug!("folder removed: {:?}. removing entries in db", event.paths);
                        // NOTE: another way to do this could be to delete all songs with a path that starts with the dir_path,
                        // which is probably easier to test, but may have worse performance
                        for entry in WalkDir::new(dir_path)
                            .into_iter()
                            .filter_entry(|e: &DirEntry| !is_hidden(e))
                            .filter_map(|e| match e {
                                Ok(entry) if entry.file_type().is_file() => Some(entry),
                                Ok(_) => None,
                                Err(e) => {
                                    error!("failed to read entry: {:?}", e);
                                    None
                                }
                            })
                        {
                            let song =
                                Song::read_by_path(self.db.as_ref(), entry.path().to_owned())
                                    .await?;
                            if let Some(song) = song {
                                Song::delete(self.db.as_ref(), song.id).await?;
                            }
                        }
                    }
                    _ => {}
                }
            }
            RemoveKind::Any | RemoveKind::Other => {
                warn!(
                    "unhandled remove event: {:?}. rescan recommended",
                    event.paths
                );
            }
        }

        Ok(())
    }

    // handler for create events
    async fn create_event_handler(
        &self,
        event: DebouncedEvent,
        kind: CreateKind,
    ) -> anyhow::Result<()> {
        match kind {
            CreateKind::File => {
                if let Some(path) = event.paths.first() {
                    match path.extension().map(|ext| ext.to_str()) {
                        Some(Some(ext)) if VALID_AUDIO_EXTENSIONS.contains(&ext) => {
                            debug!("file created: {:?}. adding to db", event.paths);

                            let metadata = SongMetadata::load_from_path(
                                path.to_owned(),
                                self.artist_name_separator.as_deref(),
                                self.genre_separator.as_deref(),
                            )?;

                            Song::try_load_into_db(self.db.as_ref(), metadata).await?;
                        }
                        _ => {
                            debug!(
                                "file created: {:?}. not a song, no action needed",
                                event.paths
                            );
                        }
                    }
                }
            }
            CreateKind::Folder => {
                debug!("folder created: {:?}. no action needed", event.paths);
            }
            CreateKind::Any | CreateKind::Other => {
                warn!(
                    "unhandled create event: {:?}. rescan recommended",
                    event.paths
                );
            }
        }
        Ok(())
    }

    // handler for modify events
    async fn modify_event_handler(
        &self,
        event: DebouncedEvent,
        kind: ModifyKind,
    ) -> anyhow::Result<()> {
        match kind {
            // file data modified
            ModifyKind::Data(_) => {
                debug!("file data modified: {:?}. no action needed", event.paths);
            }
            // file name (path) modified
            ModifyKind::Name(RenameMode::Both) => {
                if let (Some(from_path),Some(to_path)) = (event.paths.first(), event.paths.get(1)) {
                     match (from_path.extension().map(|ext| ext.to_string_lossy()),to_path.extension().map(|ext| ext.to_string_lossy())) {
                        (Some(from_ext), Some(to_ext)) if VALID_AUDIO_EXTENSIONS.iter().any(|ext| *ext == from_ext) && VALID_AUDIO_EXTENSIONS.iter().any(|ext| *ext == to_ext) => {
                            debug!("file name modified: {:?}. updating in db",
                            event.paths);

                            // NOTE: if this fails, the song may just not've been added previously, may want to handle that in the future
                            let song = Song::read_by_path(self.db.as_ref(), from_path.clone()).await?.ok_or(mecomp_storage::errors::Error::NotFound)?;

                            Song::update(self.db.as_ref(), song.id, SongChangeSet{
                                path: Some(to_path.clone()),
                                ..Default::default()
                            }).await?;

                        }
                        _ => {
                            debug!(
                                "file name modified: {:?}. not a song, no action needed",
                                event.paths
                            );
                        }
                    }
                }

            }
            ModifyKind::Name(
                kind @ (
                    RenameMode::From // likely a Remove event
                |  RenameMode::To // likely a Create event
            )) => {
                warn!(
                    "file name modified ({kind:?}): {:?}. not enough info to handle properly, rescan recommended",
                    event.paths
                );
            }
            ModifyKind::Name(RenameMode::Other | RenameMode::Any) => {
                warn!(
                    "unhandled file name modification: {:?}. rescan recommended",
                    event.paths
                );
            }
            // file attributes modified
            ModifyKind::Metadata(MetadataKind::Extended) => if let Some(path) = event.paths.first() {
                match path.extension().map(|ext| ext.to_str()) {
                    Some(Some(ext)) if VALID_AUDIO_EXTENSIONS.contains(&ext) => {
                        debug!("file metadata modified: {:?}. updating in db", event.paths);

                        // NOTE: if this fails, the song may just not've been added previously, may want to handle that in the future
                        let song = Song::read_by_path(self.db.as_ref(), path.clone()).await?.ok_or(mecomp_storage::errors::Error::NotFound)?;

                        let new_metadata: SongMetadata = SongMetadata::load_from_path(
                            path.to_owned(),
                            self.artist_name_separator.as_deref(),
                            self.genre_separator.as_deref(),
                        )?;

                        let changeset = new_metadata.merge_with_song(&song);

                        Song::update(self.db.as_ref(), song.id, changeset).await?;
                    }
                    _ => {
                        debug!("file metadata modified: {:?}.  not a song, no action needed", event.paths);
                    }
                }
            },
            ModifyKind::Metadata(
                MetadataKind::AccessTime
                | MetadataKind::WriteTime
                | MetadataKind::Ownership
                | MetadataKind::Permissions,
            ) => {}
            ModifyKind::Metadata(MetadataKind::Any | MetadataKind::Other) => {
                warn!(
                    "unhandled metadata modification: {:?}. rescan recommended",
                    event.paths
                );
            }
            // other modification event
            ModifyKind::Any | ModifyKind::Other => {
                warn!(
                    "unhandled modify event: {:?}. rescan recommended",
                    event.paths
                );
            }
        }
        Ok(())
    }
}
