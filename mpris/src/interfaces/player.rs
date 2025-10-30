//! Implements the player interface of the MPRIS specification.
//!
//! [org.mpris.MediaPlayer2.Player](https://specifications.freedesktop.org/mpris-spec/latest/Player_Interface.html)

use std::{path::PathBuf, str::FromStr, time::Duration};

use mecomp_core::state::{RepeatMode, SeekType, Status};
use mpris_server::{
    LoopStatus, Metadata, PlaybackRate, PlaybackStatus, PlayerInterface, Time, TrackId, Volume,
    zbus::{Error as ZbusError, fdo},
};
use tarpc::context::Context;

use crate::{Mpris, interfaces::root::SUPPORTED_MIME_TYPES, metadata_from_opt_song};

impl PlayerInterface for Mpris {
    async fn next(&self) -> fdo::Result<()> {
        let context = Context::current();
        let daemon_read_lock = self.daemon().await;
        if let Some(daemon) = daemon_read_lock.as_ref() {
            daemon
                .playback_skip_forward(context, 1)
                .await
                .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        }
        drop(daemon_read_lock);
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        let context = Context::current();
        let daemon_read_lock = self.daemon().await;
        if let Some(daemon) = daemon_read_lock.as_ref() {
            daemon
                .playback_skip_backward(context, 1)
                .await
                .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        }
        drop(daemon_read_lock);
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        let context = Context::current();
        let daemon_read_lock = self.daemon().await;
        if let Some(daemon) = daemon_read_lock.as_ref() {
            daemon
                .playback_pause(context)
                .await
                .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        }
        drop(daemon_read_lock);
        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        let context = Context::current();
        let daemon_read_lock = self.daemon().await;
        if let Some(daemon) = daemon_read_lock.as_ref() {
            daemon
                .playback_toggle(context)
                .await
                .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        }
        drop(daemon_read_lock);
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        let daemon_read_lock = self.daemon().await;
        if let Some(daemon) = daemon_read_lock.as_ref() {
            let context = Context::current();
            daemon
                .playback_stop(context)
                .await
                .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        }
        drop(daemon_read_lock);

        Ok(())
    }

    async fn play(&self) -> fdo::Result<()> {
        let context = Context::current();
        let daemon_read_lock = self.daemon().await;
        if let Some(daemon) = daemon_read_lock.as_ref() {
            daemon
                .playback_play(context)
                .await
                .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        }
        drop(daemon_read_lock);
        Ok(())
    }

    async fn open_uri(&self, uri: String) -> fdo::Result<()> {
        // the uri should be in the format:
        // file:///path/to/file
        // TODO: Support loading a playlist file when we've implemented importing/exporting external playlists
        log::info!("Opening URI: {uri}");

        // ensure the URI is a supported URI
        if !uri.starts_with("file://") {
            return Err(fdo::Error::InvalidArgs(
                "Only file:// URIs are supported".to_string(),
            ));
        }

        // extract the path from the URI, and ensure it's a valid file path
        let path = uri.strip_prefix("file://").unwrap();
        let mut path = PathBuf::from_str(path)
            .map_err(|_| fdo::Error::InvalidArgs("Invalid file path".to_string()))?;

        // parse out escaped characters (e.g. %20 for space)
        path = percent_encoding::percent_decode_str(&path.to_string_lossy())
            .decode_utf8()
            .map_err(|_| fdo::Error::InvalidArgs("Invalid file path".to_string()))?
            .into_owned()
            .into();

        // ensure the file type is supported
        if !SUPPORTED_MIME_TYPES
            .iter()
            .filter_map(|s| s.split('/').next_back())
            .any(|ext| path.extension().is_some_and(|e| e == ext))
        {
            return Err(fdo::Error::InvalidArgs(
                "File type not supported".to_string(),
            ));
        }

        // expand the tilde if present
        if path.starts_with("~") {
            path = shellexpand::tilde(&path.to_string_lossy())
                .into_owned()
                .into();
        }

        log::debug!("Locating file: {}", path.display());

        // ensure the path exists
        if !path.exists() {
            return Err(fdo::Error::InvalidArgs("File does not exist".to_string()));
        }

        // ensure the path is a file
        if !path.is_file() {
            return Err(fdo::Error::InvalidArgs("Path is not a file".to_string()));
        }

        // canonicalize the path
        path = path.canonicalize().unwrap_or(path);

        // add the song to the queue
        let daemon_read_lock = self.daemon().await;
        if let Some(daemon) = daemon_read_lock.as_ref() {
            let context = Context::current();
            if let Some(song) = daemon
                .library_song_get_by_path(context, path)
                .await
                .map_err(|e| fdo::Error::Failed(e.to_string()))?
            {
                let context = Context::current();
                daemon
                    .queue_add(context, song.id.clone().into())
                    .await
                    .map_err(|e| fdo::Error::Failed(e.to_string()))?
                    .map_err(|e| fdo::Error::Failed(e.to_string()))?;
            } else {
                return Err(fdo::Error::Failed(
                    "Failed to find song in database".to_string(),
                ));
            }
        }
        drop(daemon_read_lock);

        Ok(())
    }

    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        let status = self.state.read().await.status;
        match status {
            Status::Stopped => Ok(PlaybackStatus::Stopped),
            Status::Paused => Ok(PlaybackStatus::Paused),
            Status::Playing => Ok(PlaybackStatus::Playing),
        }
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        let repeat_mode = self.state.read().await.repeat_mode;
        match repeat_mode {
            RepeatMode::None => Ok(LoopStatus::None),
            RepeatMode::One => Ok(LoopStatus::Track),
            RepeatMode::All => Ok(LoopStatus::Playlist),
        }
    }

    async fn set_loop_status(&self, loop_status: LoopStatus) -> Result<(), ZbusError> {
        let repeat_mode = match loop_status {
            LoopStatus::None => RepeatMode::None,
            LoopStatus::Track => RepeatMode::One,
            LoopStatus::Playlist => RepeatMode::All,
        };

        let context = Context::current();

        let daemon_read_lock = self.daemon().await;
        if let Some(daemon) = daemon_read_lock.as_ref() {
            daemon
                .playback_repeat(context, repeat_mode)
                .await
                .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        }
        drop(daemon_read_lock);

        Ok(())
    }

    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn set_rate(&self, _: PlaybackRate) -> Result<(), ZbusError> {
        Ok(())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        // Mecomp has no distinction between shuffle and non-shuffle playback
        // shuffling is done by actually shuffling the queue and is not reversible
        // therefore, we always return true
        Ok(true)
    }

    async fn set_shuffle(&self, shuffle: bool) -> Result<(), ZbusError> {
        // if called with false, does nothing, if called with true, shuffles the queue
        if shuffle {
            let context = Context::current();
            let daemon_read_lock = self.daemon().await;
            if let Some(daemon) = daemon_read_lock.as_ref() {
                daemon
                    .playback_shuffle(context)
                    .await
                    .map_err(|e| fdo::Error::Failed(e.to_string()))?;
            }
            drop(daemon_read_lock);
        }

        Ok(())
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        let state = self.state.read().await;

        Ok(metadata_from_opt_song(state.current_song.as_ref()))
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        let state = self.state.read().await;
        if state.muted {
            Ok(0.0)
        } else {
            Ok(f64::from(state.volume))
        }
    }

    async fn set_volume(&self, volume: Volume) -> Result<(), ZbusError> {
        let context = Context::current();
        let daemon_read_lock = self.daemon().await;
        if let Some(daemon) = daemon_read_lock.as_ref() {
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
            daemon
                .playback_volume(context, volume as f32)
                .await
                .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        }
        drop(daemon_read_lock);
        Ok(())
    }

    async fn position(&self) -> fdo::Result<Time> {
        self.state.read().await.runtime.map_or_else(
            || Ok(Time::from_micros(0)),
            |runtime| {
                Ok(Time::from_micros(
                    i64::try_from(runtime.seek_position.as_micros()).unwrap_or(i64::MAX),
                ))
            },
        )
    }

    async fn seek(&self, offset: Time) -> fdo::Result<()> {
        //TODO: if the value passed in would mean seeking beyond the end of the track, act like a call to Next
        let context = Context::current();
        let daemon_read_lock = self.daemon().await;
        if let Some(daemon) = daemon_read_lock.as_ref() {
            let seek_type = if offset.as_micros() > 0 {
                SeekType::RelativeForwards
            } else {
                SeekType::RelativeBackwards
            };

            let offset = Duration::from_micros(offset.as_micros().unsigned_abs());

            daemon
                .playback_seek(context, seek_type, offset)
                .await
                .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        }
        drop(daemon_read_lock);
        Ok(())
    }

    async fn set_position(&self, track_id: TrackId, position: Time) -> fdo::Result<()> {
        // "track_id - The currently playing track's identifier. If this does not match the id of the currently-playing track, the call is ignored as 'stale'"
        if Some(track_id) != self.metadata().await?.trackid() {
            return Ok(());
        }

        let daemon_read_lock = self.daemon().await;
        if let Some(daemon) = daemon_read_lock.as_ref() {
            let maybe_state = self.state.read().await;
            if let Some(song) = maybe_state.current_song.as_ref() {
                // if the position not in the range of the song, ignore the call
                let position = position.as_micros();
                if position < 0 || u128::from(position.unsigned_abs()) > song.runtime.as_micros() {
                    return Ok(());
                }

                let context = Context::current();

                daemon
                    .playback_seek(
                        context,
                        SeekType::Absolute,
                        Duration::from_micros(u64::try_from(position).unwrap_or_default()),
                    )
                    .await
                    .map_err(|e| fdo::Error::Failed(e.to_string()))?;
            }
            drop(maybe_state);
        }
        drop(daemon_read_lock);

        Ok(())
    }

    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}

// NOTE: these tests do not run the event `Subscriber` main loop so events from the audio kernel are not
// actually going to be applied to the state, so we have to manually set the state when testing methods
// that simply report the state.
#[cfg(test)]
mod tests {
    use std::sync::{Arc, mpsc::Receiver};

    use mecomp_core::{
        audio::{
            AudioKernelSender,
            commands::{AudioCommand, QueueCommand},
        },
        test_utils::init,
        udp::StateChange,
    };

    use mecomp_storage::db::schemas::song::SongBrief;
    use pretty_assertions::{assert_eq, assert_ne};
    use rstest::rstest;
    use tempfile::TempDir;

    use super::*;
    use crate::test_utils::fixtures;

    /// """
    /// Skips to the next track in the tracklist.
    /// If there is no next track (and endless playback and track repeat are both off), stop playback.
    /// If playback is paused or stopped, it remains that way.
    /// If [CanGoNext] is false, attempting to call this method should have no effect.
    /// """
    ///
    /// Mecomp supports skipping to the next track in the queue.
    ///
    /// the last case is irrelevant here, as we always return true for [CanGoNext]
    #[rstest]
    #[timeout(Duration::from_secs(10))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 3)]
    async fn test_next(
        #[future] fixtures: (
            Mpris,
            Receiver<StateChange>,
            TempDir,
            Arc<AudioKernelSender>,
        ),
    ) {
        init();
        let (mpris, event_rx, tempdir, audio_kernel) = fixtures.await;

        assert_eq!(mpris.can_go_next().await.unwrap(), true);

        // setup
        let context = Context::current();
        let songs: Vec<SongBrief> = mpris
            .daemon
            .read()
            .await
            .as_ref()
            .unwrap()
            .library_songs_brief(context)
            .await
            .unwrap()
            .unwrap()
            .to_vec();
        assert_eq!(songs.len(), 4);
        // send all the songs to the audio kernel (adding them to the queue and starting playback)
        audio_kernel.send(AudioCommand::Queue(QueueCommand::AddToQueue(songs.into())));
        assert_eq!(event_rx.recv(), Ok(StateChange::QueueChanged));
        let Ok(StateChange::TrackChanged(Some(first_song))) = event_rx.recv() else {
            panic!("Expected a TrackChanged event, but got something else");
        };
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Playing))
        );

        // it skips to the next track //
        mpris.next().await.unwrap();

        let Ok(StateChange::TrackChanged(Some(second_song))) = event_rx.recv() else {
            panic!("Expected a TrackChanged event, but got something else");
        };

        // the current song should be different from the previous song
        assert_ne!(first_song, second_song);

        drop(tempdir);
    }

    #[rstest]
    #[timeout(Duration::from_secs(10))]
    #[tokio::test]
    async fn test_next_maintains_status(
        #[future] fixtures: (
            Mpris,
            Receiver<StateChange>,
            TempDir,
            Arc<AudioKernelSender>,
        ),
    ) {
        init();
        let (mpris, event_rx, tempdir, audio_kernel) = fixtures.await;

        assert_eq!(mpris.can_go_next().await.unwrap(), true);

        // setup
        let context = Context::current();
        let songs: Vec<SongBrief> = mpris
            .daemon
            .read()
            .await
            .as_ref()
            .unwrap()
            .library_songs_brief(context)
            .await
            .unwrap()
            .unwrap()
            .to_vec();
        assert_eq!(songs.len(), 4);
        // send all the songs to the audio kernel (adding them to the queue and starting playback)
        audio_kernel.send(AudioCommand::Queue(QueueCommand::AddToQueue(songs.into())));
        assert_eq!(event_rx.recv(), Ok(StateChange::QueueChanged));
        let Ok(StateChange::TrackChanged(Some(first_song))) = event_rx.recv() else {
            panic!("Expected a TrackChanged event, but got something else");
        };
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Playing))
        );

        // if playback is paused or stopped, it remains that way //

        // stop playback
        mpris.stop().await.unwrap();
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::Seeked(Duration::from_secs(0)))
        );
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Stopped))
        );

        // skip to the next track
        mpris.next().await.unwrap();
        let Ok(StateChange::TrackChanged(Some(second_song))) = event_rx.recv() else {
            panic!("Expected a TrackChanged event, but got something else");
        };
        // playback should remain stopped
        assert!(event_rx.try_recv().is_err());

        // the current song should be different from the previous song
        assert_ne!(first_song, second_song);

        // pause playback
        mpris.pause().await.unwrap();
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Paused))
        );

        // skip to the next track
        mpris.next().await.unwrap();
        let Ok(StateChange::TrackChanged(Some(third_song))) = event_rx.recv() else {
            panic!("Expected a TrackChanged event, but got something else");
        };
        // playback should remain paused
        assert!(event_rx.try_recv().is_err());

        // the current song should be different from the previous song
        assert_ne!(second_song, third_song);

        drop(tempdir);
    }

    #[rstest]
    #[timeout(Duration::from_secs(10))]
    #[tokio::test]
    async fn test_next_no_next_track(
        #[future] fixtures: (
            Mpris,
            Receiver<StateChange>,
            TempDir,
            Arc<AudioKernelSender>,
        ),
    ) {
        init();
        let (mpris, event_rx, tempdir, audio_kernel) = fixtures.await;

        assert_eq!(mpris.can_go_next().await.unwrap(), true);

        // setup
        let context = Context::current();
        let songs: Vec<SongBrief> = mpris
            .daemon
            .read()
            .await
            .as_ref()
            .unwrap()
            .library_songs_brief(context)
            .await
            .unwrap()
            .unwrap()
            .to_vec();
        assert_eq!(songs.len(), 4);
        // send one song to the audio kernel (adding them to the queue and starting playback)
        audio_kernel.send(AudioCommand::Queue(QueueCommand::AddToQueue(
            songs[0].clone().into(),
        )));
        let _ = event_rx.recv();
        let _ = event_rx.recv();
        let _ = event_rx.recv();

        // if there is no next track (and endless playback and track repeat are both off), stop playback. //
        // skip to the next track (which should be nothing)
        mpris.next().await.unwrap();
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::QueueChanged),
            "since it was the last song, the queue clears"
        );
        assert_eq!(event_rx.recv(), Ok(StateChange::TrackChanged(None)));
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Stopped))
        );
        drop(tempdir);
    }

    /// """
    /// Skips to the previous track in the tracklist.
    /// If there is no previous track (and endless playback and track repeat are both off), stop playback.
    /// If playback is paused or stopped, it remains that way.
    /// If [CanGoPrevious] is false, attempting to call this method should have no effect.
    /// """
    ///
    /// Mecomp supports skipping to the previous track in the queue.
    ///
    /// the last case is irrelevant here, as we always return true for [CanGoPrevious]
    #[rstest]
    #[timeout(Duration::from_secs(10))]
    #[tokio::test]
    async fn test_prev(
        #[future] fixtures: (
            Mpris,
            Receiver<StateChange>,
            TempDir,
            Arc<AudioKernelSender>,
        ),
    ) {
        init();
        let (mpris, event_rx, tempdir, audio_kernel) = fixtures.await;

        assert_eq!(mpris.can_go_previous().await.unwrap(), true);

        // setup
        let context = Context::current();
        let songs: Vec<SongBrief> = mpris
            .daemon
            .read()
            .await
            .as_ref()
            .unwrap()
            .library_songs_brief(context)
            .await
            .unwrap()
            .unwrap()
            .to_vec();
        assert_eq!(songs.len(), 4);
        let first_song: mecomp_storage::db::schemas::RecordId = songs[0].id.clone().into();
        let third_song: mecomp_storage::db::schemas::RecordId = songs[2].id.clone().into();
        let fourth_song: mecomp_storage::db::schemas::RecordId = songs[3].id.clone().into();

        // send all the songs to the audio kernel (adding them to the queue and starting playback)
        audio_kernel.send(AudioCommand::Queue(QueueCommand::AddToQueue(songs.into())));
        assert_eq!(event_rx.recv(), Ok(StateChange::QueueChanged));
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::TrackChanged(Some(first_song)))
        );
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Playing))
        );

        // skip to the last song in the queue
        audio_kernel.send(AudioCommand::Queue(QueueCommand::SetPosition(3)));
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::TrackChanged(Some(fourth_song.clone())))
        );

        // it skips to the previous track //
        mpris.previous().await.unwrap();
        // should go back to the fourth song
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::TrackChanged(Some(third_song))),
        );

        drop(tempdir);
    }
    #[rstest]
    #[timeout(Duration::from_secs(10))]
    #[tokio::test]
    async fn test_prev_maintains_state(
        #[future] fixtures: (
            Mpris,
            Receiver<StateChange>,
            TempDir,
            Arc<AudioKernelSender>,
        ),
    ) {
        init();
        let (mpris, event_rx, tempdir, audio_kernel) = fixtures.await;

        assert_eq!(mpris.can_go_previous().await.unwrap(), true);

        // setup
        let context = Context::current();
        let songs: Vec<SongBrief> = mpris
            .daemon
            .read()
            .await
            .as_ref()
            .unwrap()
            .library_songs_brief(context)
            .await
            .unwrap()
            .unwrap()
            .to_vec();
        assert_eq!(songs.len(), 4);
        let first_song: mecomp_storage::db::schemas::RecordId = songs[0].id.clone().into();
        let second_song: mecomp_storage::db::schemas::RecordId = songs[1].id.clone().into();
        let third_song: mecomp_storage::db::schemas::RecordId = songs[2].id.clone().into();
        let fourth_song: mecomp_storage::db::schemas::RecordId = songs[3].id.clone().into();

        // send all the songs to the audio kernel (adding them to the queue and starting playback)
        audio_kernel.send(AudioCommand::Queue(QueueCommand::AddToQueue(songs.into())));
        assert_eq!(event_rx.recv(), Ok(StateChange::QueueChanged));
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::TrackChanged(Some(first_song)))
        );
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Playing))
        );

        // skip to the last song in the queue
        audio_kernel.send(AudioCommand::Queue(QueueCommand::SetPosition(3)));
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::TrackChanged(Some(fourth_song.clone())))
        );

        // if playback is paused or stopped, it remains that way //

        // stop playback
        mpris.stop().await.unwrap();
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::Seeked(Duration::from_secs(0)))
        );
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Stopped))
        );
        // skip to the previous track
        mpris.previous().await.unwrap();
        // should go back to the third
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::TrackChanged(Some(third_song))),
        );
        // playback should remain stopped
        assert!(event_rx.try_recv().is_err());

        // pause playback
        mpris.pause().await.unwrap();
        assert!(matches!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Paused))
        ));

        // skip to the previous track
        mpris.previous().await.unwrap();
        // should go back to the second song
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::TrackChanged(Some(second_song))),
        );
        // playback should remain paused
        assert!(event_rx.try_recv().is_err());

        drop(tempdir);
    }

    #[rstest]
    #[timeout(Duration::from_secs(10))]
    #[tokio::test]
    async fn test_prev_no_prev_track(
        #[future] fixtures: (
            Mpris,
            Receiver<StateChange>,
            TempDir,
            Arc<AudioKernelSender>,
        ),
    ) {
        init();
        let (mpris, event_rx, tempdir, audio_kernel) = fixtures.await;

        assert_eq!(mpris.can_go_previous().await.unwrap(), true);

        // setup
        let context = Context::current();
        let songs: Vec<SongBrief> = mpris
            .daemon
            .read()
            .await
            .as_ref()
            .unwrap()
            .library_songs_brief(context)
            .await
            .unwrap()
            .unwrap()
            .to_vec();
        assert_eq!(songs.len(), 4);

        // send all the songs to the audio kernel (adding them to the queue and starting playback)
        audio_kernel.send(AudioCommand::Queue(QueueCommand::AddToQueue(
            songs[0].clone().into(),
        )));
        assert_eq!(event_rx.recv(), Ok(StateChange::QueueChanged));
        let _ = event_rx.recv();
        let _ = event_rx.recv();

        // if there is no previous track (and endless playback and track repeat are both off), stop playback. //
        // skip to the previous track
        mpris.previous().await.unwrap();
        // should go back to nothing
        assert_eq!(event_rx.recv(), Ok(StateChange::TrackChanged(None)),);
        // playback should be stopped
        assert!(matches!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Stopped))
        ));

        drop(tempdir);
    }

    /// """
    /// Pauses playback.
    /// If playback is already paused, this has no effect.
    /// Calling [Play] after this should cause playback to start again from the same position.
    /// If [CanPause] is false, attempting to call this method should have no effect.
    /// """
    ///
    /// Mecomp supports pausing playback.
    ///
    /// the last case is irrelevant here, as we always return true for [CanPause]
    ///
    /// """
    /// Starts or resumes playback.
    /// If already playing, this has no effect.
    /// If paused, playback resumes from the current position.
    /// If there is no track to play, this has no effect.
    /// If [CanPlay] is false, attempting to call this method should have no effect.
    /// """
    ///
    /// Mecomp supports starting or resuming playback.
    ///
    /// the last case is irrelevant here, as we always return true for [CanPlay]
    ///
    /// """
    /// Pauses playback.
    /// If playback is already paused, resumes playback.
    /// If playback is stopped, starts playback.
    /// If [CanPause] is false, attempting to call this method should have no effect and raise an error.
    /// """
    ///
    /// Mecomp supports toggling between playing and pausing playback.
    ///
    /// """
    /// Stops playback.
    /// If playback is already stopped, this has no effect.
    /// Calling Play after this should cause playback to start again from the beginning of the track.
    /// If [CanControl] is false, attempting to call this method should have no effect and raise an error.
    /// """
    ///
    /// Mecomp supports stopping playback.
    ///
    /// the last case is irrelevant here, as we always return true for [CanControl]
    #[rstest]
    #[timeout(Duration::from_secs(10))]
    #[tokio::test]
    async fn test_play_pause_stop(
        #[future] fixtures: (
            Mpris,
            Receiver<StateChange>,
            TempDir,
            Arc<AudioKernelSender>,
        ),
    ) {
        init();
        let (mpris, event_rx, tempdir, audio_kernel) = fixtures.await;

        assert_eq!(mpris.can_pause().await.unwrap(), true);
        assert_eq!(mpris.can_play().await.unwrap(), true);
        assert_eq!(mpris.can_control().await.unwrap(), true);

        // setup
        let context = Context::current();
        let songs: Vec<SongBrief> = mpris
            .daemon
            .read()
            .await
            .as_ref()
            .unwrap()
            .library_songs_brief(context)
            .await
            .unwrap()
            .unwrap()
            .to_vec();
        assert_eq!(songs.len(), 4);
        let first_song = songs[0].clone();

        // Play: if there is no track to play, this has no effect //
        mpris.play().await.unwrap();
        let event = event_rx.try_recv();
        assert!(
            event.is_err(),
            "Expected not to receive an event, but got {event:?}"
        );

        // send all the songs to the audio kernel (adding them to the queue and starting playback)
        audio_kernel.send(AudioCommand::Queue(QueueCommand::AddToQueue(
            first_song.clone().into(),
        )));
        assert_eq!(event_rx.recv(), Ok(StateChange::QueueChanged));
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::TrackChanged(Some(
                first_song.id.clone().into()
            )))
        );
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Playing))
        );

        // Pause: pauses playback //
        mpris.pause().await.unwrap();
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Paused))
        );

        // Pause: if playback is already paused, this has no effect //
        mpris.pause().await.unwrap();
        let event = event_rx.try_recv();
        assert!(
            event.is_err(),
            "Expected not to receive an event, but got {event:?}"
        );

        // Pause: calling [Play] after this should cause playback to start again from the same position. //
        // not easily testable

        // Play: Starts or resumes playback. //
        mpris.play().await.unwrap();
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Playing))
        );

        // Play if already playing, this has no effect //
        mpris.play().await.unwrap();
        let event = event_rx.try_recv();
        assert!(
            event.is_err(),
            "Expected not to receive an event, but got {event:?}"
        );

        // Play: If paused, playback resumes from the current position. //
        // not easily testable

        // Play: If there is no track to play, this has no effect. //
        // tested above before sending the songs to the audio kernel

        // Play-Pause: Pauses playback. //
        mpris.play_pause().await.unwrap();
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Paused))
        );

        // Play-Pause: If playback is already paused, resumes playback. //
        mpris.play_pause().await.unwrap();
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Playing))
        );

        // Play-Pause: If playback is stopped, starts playback. //
        mpris.stop().await.unwrap();
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::Seeked(Duration::from_secs(0)))
        );
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Stopped))
        );
        mpris.play_pause().await.unwrap();
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Playing))
        );

        // Stop: Stops playback. //
        mpris.stop().await.unwrap();
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::Seeked(Duration::from_secs(0)))
        );
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Stopped))
        );

        // Stop: If playback is already stopped, this has no effect. //
        mpris.stop().await.unwrap();
        let event = event_rx.try_recv();
        assert!(
            event.is_err(),
            "Expected not to receive an event, but got {event:?}"
        );

        // Stop: Calling Play after this should cause playback to start again from the beginning of the track. //
        // not easily testable

        drop(tempdir);
    }

    /// """
    /// Opens the uri given as an argument
    /// If the playback is stopped, starts playing
    /// If the uri scheme or the mime-type of the uri to open is not supported,
    ///  this method does nothing and may raise an error.
    ///  In particular, if the list of available uri schemes is empty,
    ///  this method may not be implemented.
    /// If the media player implements the [TrackList interface], then the opened track should be made part of the tracklist,
    ///  the [TrackAdded] or [TrackListReplaced] signal should be fired, as well as the
    ///  org.freedesktop.DBus.Properties.PropertiesChanged signal on the [TrackList interface]
    /// """
    ///
    /// Mecomp supports opening file URIs, and returns errors for unsupported or invalid URIs.
    ///
    /// Mecomp does not currently implement the [TrackList interface], so the last case is irrelevant here.
    #[rstest]
    #[timeout(Duration::from_secs(10))]
    #[tokio::test]
    async fn test_open_uri(
        #[future] fixtures: (
            Mpris,
            Receiver<StateChange>,
            TempDir,
            Arc<AudioKernelSender>,
        ),
    ) {
        init();
        let (mpris, event_rx, tempdir, _) = fixtures.await;

        // setup
        let context = Context::current();
        let songs: Vec<SongBrief> = mpris
            .daemon
            .read()
            .await
            .as_ref()
            .unwrap()
            .library_songs_brief(context)
            .await
            .unwrap()
            .unwrap()
            .to_vec();
        assert_eq!(songs.len(), 4);
        let first_song = songs[0].clone();

        // Opens the uri given as an argument //

        // open a valid file uri
        let file_uri = format!("file://{}", first_song.path.display());
        mpris.open_uri(file_uri).await.unwrap();
        assert_eq!(event_rx.recv(), Ok(StateChange::QueueChanged));
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::TrackChanged(Some(
                first_song.id.clone().into()
            )))
        );
        // If the playback is stopped, starts playing //
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::StatusChanged(Status::Playing))
        );

        // If the uri scheme or the mime-type of the uri to open is not supported, this method does nothing and may raise an error. //
        // In particular, if the list of available uri schemes is empty, this method may not be implemented. //

        // open a uri with an unsupported scheme
        let file_uri = "http://example.com/song.mp3".to_string();
        let result = mpris.open_uri(file_uri).await;
        assert!(result.is_err());
        // open a file uri with an invalid path (is not a file path)
        let file_uri = "file://".to_string();
        let result = mpris.open_uri(file_uri).await;
        assert!(result.is_err());
        // open a file uri with an invalid path (a directory)
        let file_uri = format!("file://{}", tempdir.path().display());
        let result = mpris.open_uri(file_uri).await;
        assert!(result.is_err());
        // open a file uri that doesn't exist
        let file_uri = "file:///nonexistent.mp3".to_string();
        let result = mpris.open_uri(file_uri).await;
        assert!(result.is_err());
        // open a file uri with an unsupported mime type
        std::fs::write(tempdir.path().join("unsupported.txt"), "unsupported")
            .expect("Failed to write file");
        let file_uri = format!(
            "file:///{}",
            tempdir.path().join("unsupported.txt").display()
        );
        let result = mpris.open_uri(file_uri).await;
        assert!(result.is_err());

        // If the media player implements the [TrackList interface], then the opened track should be made part of the tracklist, the [TrackAdded] or [TrackListReplaced] signal should be fired, as well as the org.freedesktop.DBus.Properties.PropertiesChanged signal on the [TrackList interface] //
        // TODO: test this when we implement the [TrackList interface]

        drop(tempdir);
    }

    /// """
    /// Returns the playback status.
    /// """
    /// Mecomp supports returning the playback status.
    #[rstest]
    #[timeout(Duration::from_secs(10))]
    #[tokio::test]
    async fn test_playback_status(
        #[future] fixtures: (
            Mpris,
            Receiver<StateChange>,
            TempDir,
            Arc<AudioKernelSender>,
        ),
    ) {
        init();
        let (mpris, event_rx, tempdir, audio_kernel) = fixtures.await;

        // setup
        let context = Context::current();
        let songs: Vec<SongBrief> = mpris
            .daemon
            .read()
            .await
            .as_ref()
            .unwrap()
            .library_songs_brief(context)
            .await
            .unwrap()
            .unwrap()
            .to_vec();
        assert_eq!(songs.len(), 4);
        let first_song: SongBrief = songs[0].clone();

        // Returns the playback status. //
        // playback is stopped
        assert_eq!(
            mpris.playback_status().await.unwrap(),
            PlaybackStatus::Stopped
        );

        // send all the songs to the audio kernel (adding them to the queue and starting playback)
        audio_kernel.send(AudioCommand::Queue(QueueCommand::AddToQueue(
            first_song.clone().into(),
        )));

        // pause playback
        mpris.pause().await.unwrap();

        // we expect there to be 4 events
        let mut events = [false; 4];
        for _ in 0..4 {
            let event = event_rx.recv().unwrap();

            match event {
                StateChange::QueueChanged => {
                    events[0] = true;
                }
                StateChange::TrackChanged(Some(_)) => {
                    mpris.state.write().await.current_song = Some(first_song.clone());
                    events[1] = true;
                }
                StateChange::StatusChanged(Status::Playing) => {
                    mpris.state.write().await.status = Status::Playing;
                    assert_eq!(
                        mpris.playback_status().await.unwrap(),
                        PlaybackStatus::Playing
                    );
                    events[2] = true;
                }
                StateChange::StatusChanged(Status::Paused) => {
                    mpris.state.write().await.status = Status::Paused;
                    assert_eq!(
                        mpris.playback_status().await.unwrap(),
                        PlaybackStatus::Paused
                    );
                    events[3] = true;
                }
                _ => panic!("Unexpected event: {event:?}"),
            }
        }

        assert!(events.iter().all(|&e| e));

        drop(tempdir);
    }

    /// """
    /// Returns the loop status.
    /// """
    ///
    /// Mecomp supports returning the loop status.
    ///
    /// """
    /// Sets the loop status.
    /// """
    ///
    /// Mecomp supports setting the loop status.
    #[rstest]
    #[timeout(Duration::from_secs(10))]
    #[tokio::test]
    async fn test_loop_status(
        #[future] fixtures: (
            Mpris,
            Receiver<StateChange>,
            TempDir,
            Arc<AudioKernelSender>,
        ),
    ) {
        init();
        let (mpris, event_rx, _, _) = fixtures.await;

        // Returns the loop status. //
        // loop status is none
        assert_eq!(mpris.loop_status().await.unwrap(), LoopStatus::None);

        // set loop status to track
        mpris.set_loop_status(LoopStatus::Track).await.unwrap();
        if event_rx.recv() == Ok(StateChange::RepeatModeChanged(RepeatMode::One)) {
            mpris.state.write().await.repeat_mode = RepeatMode::One;
        } else {
            panic!("Expected a RepeatModeChanged event, but got something else");
        }
        assert_eq!(mpris.loop_status().await.unwrap(), LoopStatus::Track);

        // set loop status to playlist
        mpris.set_loop_status(LoopStatus::Playlist).await.unwrap();
        if event_rx.recv() == Ok(StateChange::RepeatModeChanged(RepeatMode::All)) {
            mpris.state.write().await.repeat_mode = RepeatMode::All;
        } else {
            panic!("Expected a RepeatModeChanged event, but got something else");
        }
        assert_eq!(mpris.loop_status().await.unwrap(), LoopStatus::Playlist);

        // set loop status to none
        mpris.set_loop_status(LoopStatus::None).await.unwrap();
        if event_rx.recv() == Ok(StateChange::RepeatModeChanged(RepeatMode::None)) {
            mpris.state.write().await.repeat_mode = RepeatMode::None;
        } else {
            panic!("Expected a RepeatModeChanged event, but got something else");
        }
        assert_eq!(mpris.loop_status().await.unwrap(), LoopStatus::None);
    }

    /// """
    /// Returns the rate.
    /// """
    /// """
    /// The minimum value which the [Rate] property can take. Clients should not attempt to set the [Rate] property below this value.
    /// """
    /// """
    /// """
    /// The maximum value which the [Rate] property can take. Clients should not attempt to set the [Rate] property above this value.
    /// """
    /// """
    /// Sets the playback rate.
    /// """
    ///
    /// Mecomp supports returning the playback rate, but does not support changing it.
    #[rstest]
    #[timeout(Duration::from_secs(10))]
    #[tokio::test]
    async fn test_rate(
        #[future] fixtures: (
            Mpris,
            Receiver<StateChange>,
            TempDir,
            Arc<AudioKernelSender>,
        ),
    ) {
        init();
        let (mpris, event_rx, _, _) = fixtures.await;

        // Returns the playback rate. //
        let rate = mpris.rate().await.unwrap();
        assert!(f64::EPSILON > (rate - 1.0).abs(), "{rate} != 1.0");

        // The minimum value which the [Rate] property can take. Clients should not attempt to set the [Rate] property below this value. //
        let min_rate = mpris.minimum_rate().await.unwrap();
        assert!(f64::EPSILON > (min_rate - 1.0).abs(), "{min_rate} != 1.0");

        // The maximum value which the [Rate] property can take. Clients should not attempt to set the [Rate] property above this value. //
        let max_rate = mpris.maximum_rate().await.unwrap();
        assert!(f64::EPSILON > (max_rate - 1.0).abs(), "{max_rate} != 1.0");

        // Sets the playback rate. //
        // not supported, but the spec doesn't specify that an error should be reported so we just return Ok
        let result = mpris.set_rate(1.0).await;
        assert!(result.is_ok());
        assert!(event_rx.try_recv().is_err());
    }

    /// """
    /// Returns whether playback is shuffled.
    /// """
    ///
    /// Mecomp supports returning whether playback is shuffled.
    ///
    /// """
    /// Sets whether playback is shuffled.
    /// """
    ///
    /// Mecomp supports setting whether playback is shuffled.
    ///
    /// NOTE: Mecomp does not actually implement this properly,
    /// as setting shuffle to false will not restore the original order of the queue
    /// and is instead a no-op.
    #[rstest]
    #[timeout(Duration::from_secs(10))]
    #[tokio::test]
    async fn test_shuffle(
        #[future] fixtures: (
            Mpris,
            Receiver<StateChange>,
            TempDir,
            Arc<AudioKernelSender>,
        ),
    ) {
        init();
        let (mpris, event_rx, _, _) = fixtures.await;

        // Returns whether playback is shuffled. //
        assert_eq!(mpris.shuffle().await.unwrap(), true);

        // Sets whether playback is shuffled. //
        // set shuffle to true
        mpris.set_shuffle(true).await.unwrap();
        assert_eq!(mpris.shuffle().await.unwrap(), true);
        assert_eq!(event_rx.recv(), Ok(StateChange::QueueChanged));

        // set shuffle to false
        mpris.set_shuffle(false).await.unwrap();
        assert_eq!(mpris.shuffle().await.unwrap(), true);
        assert!(event_rx.recv_timeout(Duration::from_millis(100)).is_err());
    }

    /// """
    /// The metadata of the current element.
    /// When this property changes, the org.freedesktop.DBus.Properties.PropertiesChanged
    ///     signal via [properties_changed] must be emitted with the new value.
    /// If there is a current track, this must have a [mpris:trackid] entry at the very least,
    ///     which contains a D-Bus path that uniquely identifies this track.
    /// """
    #[rstest]
    #[timeout(Duration::from_secs(10))]
    #[tokio::test]
    async fn test_metadata(
        #[future] fixtures: (
            Mpris,
            Receiver<StateChange>,
            TempDir,
            Arc<AudioKernelSender>,
        ),
    ) {
        init();
        let (mpris, event_rx, tempdir, audio_kernel) = fixtures.await;

        // setup
        let context = Context::current();
        let songs: Vec<SongBrief> = mpris
            .daemon
            .read()
            .await
            .as_ref()
            .unwrap()
            .library_songs_brief(context)
            .await
            .unwrap()
            .unwrap()
            .to_vec();
        assert_eq!(songs.len(), 4);
        let first_song = songs[0].clone();

        // The metadata of the current element. //
        // when there is no current song
        assert_eq!(
            mpris.metadata().await.unwrap(),
            Metadata::builder().trackid(TrackId::NO_TRACK).build()
        );

        // send all the songs to the audio kernel (adding them to the queue and starting playback)
        audio_kernel.send(AudioCommand::Queue(QueueCommand::AddToQueue(
            first_song.clone().into(),
        )));

        assert_eq!(event_rx.recv(), Ok(StateChange::QueueChanged));
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::TrackChanged(Some(
                first_song.id.clone().into()
            )))
        );

        *mpris.state.write().await = mpris
            .daemon
            .read()
            .await
            .as_ref()
            .unwrap()
            .state_audio(Context::current())
            .await
            .unwrap()
            .unwrap();

        // when there is a current song
        let metadata = mpris.metadata().await.unwrap();
        assert_eq!(metadata, metadata_from_opt_song(Some(&first_song)));
        assert_ne!(
            metadata,
            Metadata::builder().trackid(TrackId::NO_TRACK).build()
        );

        drop(tempdir);
    }

    /// """
    /// The volume level.
    /// When setting, if a negative value is passed, the volume should be set to 0.0.
    /// """
    #[rstest]
    #[timeout(Duration::from_secs(10))]
    #[tokio::test]
    async fn test_volume(
        #[future] fixtures: (
            Mpris,
            Receiver<StateChange>,
            TempDir,
            Arc<AudioKernelSender>,
        ),
    ) {
        let (mpris, event_rx, _, _) = fixtures.await;

        // The volume level. //
        let volume = mpris.volume().await.unwrap();
        assert!(f64::EPSILON > (volume - 1.0).abs(), "{volume} != 1.0");

        // When setting, if a negative value is passed, the volume should be set to 0.0. //
        mpris.set_volume(-1.0).await.unwrap();
        if event_rx.recv() == Ok(StateChange::VolumeChanged(0.0)) {
            mpris.state.write().await.volume = 0.0;
            let volume = mpris.volume().await.unwrap();
            assert!(f64::EPSILON > volume.abs(), "{volume} != 0.0");
        } else {
            panic!("Expected a VolumeChanged event, but got something else");
        }

        // set the volume back to 1.0
        mpris.set_volume(1.0).await.unwrap();
        if event_rx.recv() == Ok(StateChange::VolumeChanged(1.0)) {
            mpris.state.write().await.volume = 1.0;
            let volume = mpris.volume().await.unwrap();
            assert!(f64::EPSILON > (volume - 1.0).abs(), "{volume} != 1.0");
        } else {
            panic!("Expected a VolumeChanged event, but got something else");
        }

        // set the volume to the same value
        mpris.set_volume(1.0).await.unwrap();
        assert!(event_rx.try_recv().is_err());
    }

    /// """
    /// The current track position, between 0 and the [mpris:length] metadata entry.
    /// If the media player allows it, the current playback position can be changed either the [SetPosition] method or the [Seek] method on this interface.
    /// If this is not the case, the [CanSeek] property is false, and setting this property has no effect and can raise an error.
    /// """
    ///
    /// for set_position:
    /// """
    /// If the Position argument is less than 0, do nothing.
    /// If the Position argument is greater than the track length, do nothing.
    /// If the given `track_id` this does not match the id of the currently-playing track, the call is ignored as "stale"
    /// """
    #[rstest]
    #[timeout(Duration::from_secs(10))]
    #[tokio::test]
    async fn test_position(
        #[future] fixtures: (
            Mpris,
            Receiver<StateChange>,
            TempDir,
            Arc<AudioKernelSender>,
        ),
    ) {
        init();
        let (mpris, event_rx, tempdir, audio_kernel) = fixtures.await;

        assert!(mpris.can_seek().await.unwrap());

        // setup
        let context = Context::current();
        let songs: Vec<SongBrief> = mpris
            .daemon
            .read()
            .await
            .as_ref()
            .unwrap()
            .library_songs_brief(context)
            .await
            .unwrap()
            .unwrap()
            .to_vec();
        assert_eq!(songs.len(), 4);
        let first_song = songs[0].clone();

        // The current track position, between 0 and the [mpris:length] metadata entry. //
        // when there is no current song
        assert_eq!(mpris.position().await.unwrap(), Time::from_micros(0));

        // send all the songs to the audio kernel (adding them to the queue and starting playback)
        audio_kernel.send(AudioCommand::Queue(QueueCommand::AddToQueue(
            first_song.clone().into(),
        )));
        audio_kernel.send(AudioCommand::Pause);
        let _ = event_rx.recv().unwrap();
        let _ = event_rx.recv().unwrap();
        let _ = event_rx.recv().unwrap();
        let _ = event_rx.recv().unwrap();
        // update internal state
        *mpris.state.write().await = mpris
            .daemon
            .read()
            .await
            .as_ref()
            .unwrap()
            .state_audio(Context::current())
            .await
            .unwrap()
            .unwrap();

        let first_song_track_id = mpris.metadata().await.unwrap().trackid().unwrap();

        // normal:
        mpris
            .set_position(first_song_track_id.clone(), Time::from_secs(2))
            .await
            .unwrap();
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::Seeked(Duration::from_secs(2)))
        );

        // If the Position argument is less than 0, do nothing. //
        mpris
            .set_position(first_song_track_id.clone(), Time::from_secs(-1))
            .await
            .unwrap();
        assert!(event_rx.try_recv().is_err());

        // If the Position argument is greater than the track length, do nothing. //
        mpris
            .set_position(first_song_track_id.clone(), Time::from_secs(100))
            .await
            .unwrap();
        assert!(event_rx.try_recv().is_err());

        // If the media player allows it, the current playback position can be changed either the [SetPosition] method or the [Seek] method on this interface. //
        // If this is not the case, the [CanSeek] property is false, and setting this property has no effect and can raise an error. //
        assert!(mpris.can_seek().await.unwrap());

        mpris.seek(Time::from_secs(1)).await.unwrap();
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::Seeked(Duration::from_secs(3)))
        );

        mpris.seek(Time::from_secs(-1)).await.unwrap();
        assert_eq!(
            event_rx.recv(),
            Ok(StateChange::Seeked(Duration::from_secs(2)))
        );

        drop(tempdir);
    }
}
