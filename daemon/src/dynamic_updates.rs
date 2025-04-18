//! Module for handling dynamic updates to the music library.
//!
//! This module is only available when the `dynamic_updates` feature is enabled.
//!
//! The `init_music_library_watcher`
use std::{path::PathBuf, sync::Arc, time::Duration};

use futures::FutureExt;
use futures::StreamExt;
use log::{debug, error, info, trace, warn};
use mecomp_storage::db::schemas::song::{Song, SongChangeSet, SongMetadata};
#[cfg(target_os = "macos")]
use notify::FsEventWatcher;
#[cfg(target_os = "linux")]
use notify::INotifyWatcher;
#[cfg(target_os = "windows")]
use notify::ReadDirectoryChangesWatcher;
use notify::{
    EventKind, RecursiveMode,
    event::{CreateKind, MetadataKind, ModifyKind, RemoveKind, RenameMode},
};
use notify_debouncer_full::RecommendedCache;
use notify_debouncer_full::{DebouncedEvent, Debouncer, new_debouncer};
use one_or_many::OneOrMany;
use surrealdb::{Surreal, engine::local::Db};

#[cfg(target_os = "linux")]
type WatcherType = INotifyWatcher;
#[cfg(target_os = "macos")]
type WatcherType = FsEventWatcher;
#[cfg(target_os = "windows")]
type WatcherType = ReadDirectoryChangesWatcher;

const VALID_AUDIO_EXTENSIONS: [&str; 4] = ["mp3", "wav", "ogg", "flac"];

pub const MAX_DEBOUNCE_TIME: Duration = Duration::from_millis(500);

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
#[allow(clippy::missing_inline_in_public_items)]
pub fn init_music_library_watcher(
    db: Arc<Surreal<Db>>,
    library_paths: &[PathBuf],
    artist_name_separator: OneOrMany<String>,
    protected_artist_names: OneOrMany<String>,
    genre_separator: Option<String>,
) -> anyhow::Result<MusicLibEventHandlerGuard> {
    let (tx, rx) = futures::channel::mpsc::unbounded();
    // create a oneshot that can be used to stop the watcher
    let (stop_tx, stop_rx) = futures::channel::oneshot::channel();

    // spawn the event handler in a new thread
    std::thread::Builder::new()
        .name(String::from("Music Library Watcher"))
        .spawn(move || {
            let handler = MusicLibEventHandler::new(
                db,
                artist_name_separator,
                protected_artist_names,
                genre_separator,
            );
            futures::executor::block_on(async {
                let mut stop_rx = stop_rx.fuse();
                let mut rx = rx.fuse();

                loop {
                    futures::select! {
                        _ = stop_rx => {
                            debug!("stopping watcher");
                            break;
                        }
                        result = rx.select_next_some() => {
                            match result {
                                Ok(events) => {
                                    for event in events {
                                        if let Err(e) = handler.handle_event(event).await {
                                            error!("failed to handle event: {e:?}");
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
                }
            });
        })?;

    // Select recommended watcher for debouncer.
    // Using a callback here, could also be a channel.
    let mut debouncer: Debouncer<WatcherType, _> =
        new_debouncer(MAX_DEBOUNCE_TIME, None, move |event| {
            let _ = tx.unbounded_send(event);
        })?;

    // Add all library paths to the debouncer.
    for path in library_paths {
        log::debug!("watching path: {path:?}");
        // Add a path to be watched. All files and directories at that path and
        // below will be monitored for changes.
        debouncer.watch(path, RecursiveMode::Recursive)?;
    }

    Ok(MusicLibEventHandlerGuard { debouncer, stop_tx })
}

pub struct MusicLibEventHandlerGuard {
    debouncer: Debouncer<WatcherType, RecommendedCache>,
    stop_tx: futures::channel::oneshot::Sender<()>,
}

impl MusicLibEventHandlerGuard {
    #[inline]
    pub fn stop(self) {
        let Self { debouncer, stop_tx } = self;
        stop_tx.send(()).ok();
        debouncer.stop();
    }
}

/// Handles incoming file Events.
struct MusicLibEventHandler {
    db: Arc<Surreal<Db>>,
    artist_name_separator: OneOrMany<String>,
    protected_artist_names: OneOrMany<String>,
    genre_separator: Option<String>,
}

impl MusicLibEventHandler {
    /// Creates a new `MusicLibEventHandler`.
    pub const fn new(
        db: Arc<Surreal<Db>>,
        artist_name_separator: OneOrMany<String>,
        protected_artist_names: OneOrMany<String>,
        genre_separator: Option<String>,
    ) -> Self {
        Self {
            db,
            artist_name_separator,
            protected_artist_names,
            genre_separator,
        }
    }

    /// Handles incoming file Events.
    async fn handle_event(&self, event: DebouncedEvent) -> anyhow::Result<()> {
        trace!("file event detected: {event:?}");

        match event.kind {
            // remove events
            EventKind::Remove(kind) => {
                self.remove_event_handler(event, kind).await?;
            }
            // create events
            EventKind::Create(kind) => {
                self.create_event_handler(event, kind).await?;
            }
            // modify events
            EventKind::Modify(kind) => {
                self.modify_event_handler(event, kind).await?;
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
                            info!("file removed: {:?}. removing from db", event.paths);
                            let song = Song::read_by_path(&self.db, path.clone()).await?;
                            if let Some(song) = song {
                                Song::delete(&self.db, song.id).await?;
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
            RemoveKind::Folder => {} // if an empty folder is removed, no action needed
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
                            info!("file created: {:?}. adding to db", event.paths);

                            let metadata = SongMetadata::load_from_path(
                                path.to_owned(),
                                &self.artist_name_separator,
                                &self.protected_artist_names,
                                self.genre_separator.as_deref(),
                            )?;

                            Song::try_load_into_db(&self.db, metadata).await?;
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
            ModifyKind::Data(kind) => if let Some(path) = event.paths.first() {
                match path.extension().map(|ext| ext.to_str()) {
                    Some(Some(ext)) if VALID_AUDIO_EXTENSIONS.contains(&ext) => {
                        info!("file data modified ({kind:?}): {:?}. updating in db", event.paths);

                        // NOTE: if this fails, the song may just not've been added previously, may want to handle that in the future
                        let song = Song::read_by_path(&self.db, path.clone()).await?.ok_or(mecomp_storage::errors::Error::NotFound)?;

                        let new_metadata: SongMetadata = SongMetadata::load_from_path(
                            path.to_owned(),
                            &self.artist_name_separator,
                            &self.protected_artist_names,
                            self.genre_separator.as_deref(),
                        )?;

                        let changeset = new_metadata.merge_with_song(&song);

                        Song::update(&self.db, song.id, changeset).await?;
                    }
                    _ => {
                        debug!("file data modified ({kind:?}): {:?}.  not a song, no action needed", event.paths);
                    }
                }
            },
            // file name (path) modified
            ModifyKind::Name(RenameMode::Both) => {
                if let (Some(from_path),Some(to_path)) = (event.paths.first(), event.paths.get(1)) {
                     match (from_path.extension().map(|ext| ext.to_string_lossy()),to_path.extension().map(|ext| ext.to_string_lossy())) {
                        (Some(from_ext), Some(to_ext)) if VALID_AUDIO_EXTENSIONS.iter().any(|ext| *ext == from_ext) && VALID_AUDIO_EXTENSIONS.iter().any(|ext| *ext == to_ext) => {
                            info!("file name modified: {:?}. updating in db",
                            event.paths);

                            // NOTE: if this fails, the song may just not've been added previously, may want to handle that in the future
                            let song = Song::read_by_path(&self.db, from_path.clone()).await?.ok_or(mecomp_storage::errors::Error::NotFound)?;

                            Song::update(&self.db, song.id, SongChangeSet{
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
            ModifyKind::Metadata(
                MetadataKind::AccessTime
                | MetadataKind::WriteTime
                | MetadataKind::Ownership
                | MetadataKind::Permissions,
            ) => {}
            ModifyKind::Metadata(kind@(MetadataKind::Any | MetadataKind::Other | MetadataKind::Extended)) => {
                warn!(
                    "unhandled metadata modification ({kind:?}): {:?}. rescan recommended",
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

#[cfg(test)]
mod tests {
    //! Tests for the `dynamic_updates` module.
    //!
    //! These tests initialize the database, and create a temporary music library directory
    //!
    //! The tests then create a `MusicLibEventHandler` and test the event handlers
    //! by adding, modifying, and removing files in the temporary music library directory
    //!
    //! The way many of these tests work is by exploiting the timeout attribute.
    //! Rather than asserting that certain state changes have occurred, we wait for them to occur within a certain time frame.

    use crate::test_utils::init;

    use super::*;

    use lofty::file::AudioFile;
    use pretty_assertions::assert_eq;
    use rstest::{fixture, rstest};
    use tempfile::{TempDir, tempdir};

    use mecomp_storage::test_utils::{
        ARTIST_NAME_SEPARATOR, arb_song_case, create_song_metadata, init_test_database,
    };

    #[fixture]
    async fn setup() -> (TempDir, Arc<Surreal<Db>>, MusicLibEventHandlerGuard) {
        init();
        let music_lib = tempdir().expect("Failed to create temporary directory");
        let db = Arc::new(init_test_database().await.unwrap());
        let handler = init_music_library_watcher(
            db.clone(),
            &[music_lib.path().to_owned()],
            OneOrMany::One(ARTIST_NAME_SEPARATOR.into()),
            OneOrMany::None,
            Some(ARTIST_NAME_SEPARATOR.into()),
        )
        .expect("Failed to create music library watcher");

        (music_lib, db, handler)
    }

    #[rstest]
    #[timeout(Duration::from_secs(5))]
    #[tokio::test]
    async fn test_create_song(
        #[future] setup: (TempDir, Arc<Surreal<Db>>, MusicLibEventHandlerGuard),
    ) {
        let (music_lib, db, handler) = setup.await;

        // let's call create_song_metadata to create a new song in our temporary music library, and get the metadata of that song
        let metadata = create_song_metadata(&music_lib, arb_song_case()()).unwrap();

        // this should trigger the create event handler to add the song to the database, so let's see if it's there
        while Song::read_all(&db).await.unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let path = metadata.path.clone();
        let song = Song::read_by_path(&db, path).await.unwrap().unwrap();

        // let's assert that the song in the database is the same as the song we created
        assert_eq!(metadata, song.into());

        // let's stop the watcher
        handler.stop();
        music_lib.close().unwrap();
    }

    #[rstest]
    #[timeout(Duration::from_secs(5))]
    #[tokio::test]
    async fn test_rename_song(
        #[future] setup: (TempDir, Arc<Surreal<Db>>, MusicLibEventHandlerGuard),
    ) {
        let (music_lib, db, handler) = setup.await;

        // let's call create_song_metadata to create a new song in our temporary music library, and get the metadata of that song
        let metadata = create_song_metadata(&music_lib, arb_song_case()()).unwrap();

        // this should trigger the create event handler to add the song to the database, so let's see if it's there
        while Song::read_all(&db).await.unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let path = metadata.path.clone();
        let song = Song::read_by_path(&db, path.clone())
            .await
            .unwrap()
            .unwrap();

        // let's assert that the song in the database is the same as the song we created
        assert_eq!(metadata, song.clone().into());

        // let's rename the song
        let new_path = music_lib.path().join("new_song.mp3");
        std::fs::rename(&path, &new_path).unwrap();

        // this should trigger the modify event handler to update the song in the database, so let's see if it's there
        while Song::read_by_path(&db, new_path.clone())
            .await
            .unwrap()
            .is_none()
        {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let new_song = Song::read_by_path(&db, new_path.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(song.id, new_song.id);

        // let's stop the watcher
        handler.stop();
        music_lib.close().unwrap();
    }

    fn modify_song_metadata(path: &PathBuf, new_name: String) -> anyhow::Result<()> {
        use lofty::{file::TaggedFileExt, tag::Accessor};
        let mut tagged_file = lofty::probe::Probe::open(path)?.read()?;
        let tag = tagged_file
            .primary_tag_mut()
            .ok_or_else(|| anyhow::anyhow!("ERROR: No tags found"))?;
        tag.set_title(new_name);
        tagged_file.save_to_path(path, lofty::config::WriteOptions::default())?;
        Ok(())
    }

    #[rstest]
    #[timeout(Duration::from_secs(5))]
    #[tokio::test]
    async fn test_modify_song(
        #[future] setup: (TempDir, Arc<Surreal<Db>>, MusicLibEventHandlerGuard),
    ) {
        let (music_lib, db, handler) = setup.await;

        // let's call create_song_metadata to create a new song in our temporary music library, and get the metadata of that song
        let metadata = create_song_metadata(&music_lib, arb_song_case()()).unwrap();

        // this should trigger the create event handler to add the song to the database, so let's see if it's there
        while Song::read_all(&db).await.unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let path = metadata.path.clone();
        let song = Song::read_by_path(&db, path.clone())
            .await
            .unwrap()
            .unwrap();

        // let's assert that the song in the database is the same as the song we created
        assert_eq!(metadata, song.clone().into());

        // let's modify the song metadata in the file
        modify_song_metadata(&path, "new song name".to_string()).unwrap();

        // this should trigger the modify event handler to update the song in the database, so let's see if it's there
        while Song::read_by_path(&db, path.clone())
            .await
            .unwrap()
            .unwrap()
            .title
            != "new song name"
        {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // let's stop the watcher
        handler.stop();
        music_lib.close().unwrap();
    }

    #[rstest]
    #[timeout(Duration::from_secs(5))]
    #[tokio::test]
    async fn test_remove_song(
        #[future] setup: (TempDir, Arc<Surreal<Db>>, MusicLibEventHandlerGuard),
    ) {
        let (music_lib, db, handler) = setup.await;

        // let's call create_song_metadata to create a new song in our temporary music library, and get the metadata of that song
        let metadata = create_song_metadata(&music_lib, arb_song_case()()).unwrap();

        // this should trigger the create event handler to add the song to the database, so let's see if it's there
        while Song::read_all(&db).await.unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let path = metadata.path.clone();
        let song = Song::read_by_path(&db, path.clone())
            .await
            .unwrap()
            .unwrap();

        // let's assert that the song in the database is the same as the song we created
        assert_eq!(metadata, song.clone().into());

        // let's remove the song
        std::fs::remove_file(&path).unwrap();

        // this should trigger the remove event handler to remove the song from the database, so let's see if it's there
        while Song::read_by_path(&db, path.clone())
            .await
            .unwrap()
            .is_some()
        {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // let's stop the watcher
        handler.stop();
        music_lib.close().unwrap();
    }

    #[rstest]
    #[tokio::test]
    async fn test_remove_empty_folder(
        #[future] setup: (TempDir, Arc<Surreal<Db>>, MusicLibEventHandlerGuard),
    ) {
        let (music_lib, _, handler) = setup.await;

        // let's create an empty folder in our temporary music library
        let empty_folder = music_lib.path().join("empty_folder");
        std::fs::create_dir(&empty_folder).unwrap();

        // this should trigger the remove event handler, but no action is needed
        tokio::time::sleep(Duration::from_secs(1)).await;

        // let's stop the watcher
        handler.stop();
        music_lib.close().unwrap();
    }
}
