//! This module contains the implementation of the daemon's queue persistence mechanism.
//!
//! On shutdown, the daemon will save the current audio state (`StateAudio`) to disk.
//! This includes:
//! - The current position in the queue (`usize`)
//! - The queues repeat mode (`RepeatMode`)
//! - The current seek position in the current song (`Duration`)
//! - The songs in the queue
//! - etc.
//!
//! On startup, the daemon will load that data from disk and use it to restore the queue by:
//! 1. restoring the repeat mode
//! 2. setting the volume and mute
//! 3. loading the songs into the queue
//! 4. pausing playback
//! 5. skipping to the correct position in the queue
//! 6. skipping to the correct position in the current song
//!
//! > we always restore the queue as paused, even if the last state was playing.
//!
//! Both of these tasks should be atomic, that it, if any step fails the process should be aborted.
//! - on startup, this means just logging the error and not restoring the queue
//! - on shutdown, this means logging the error and exiting as normal

use anyhow::{Context, Result};
use mecomp_core::audio::AudioKernelSender;
use mecomp_core::audio::commands::{AudioCommand, QueueCommand, VolumeCommand};
use mecomp_storage::db::schemas::song::SongBrief;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use std::{
    fs::File,
    io::{BufReader, BufWriter},
};

use mecomp_core::state::{RepeatMode, SeekType, StateAudio};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct QueueState {
    pub repeat_mode: RepeatMode,
    pub muted: bool,
    pub volume: f32,
    pub queue: Box<[SongBrief]>,
    pub queue_position: Option<usize>,
    pub seek_position: Option<Duration>,
}

impl From<StateAudio> for QueueState {
    #[inline]
    fn from(state: StateAudio) -> Self {
        // I unpack `state` this way so that any change to the `StateAudio` struct
        // will cause an error here so I know to update the `QueueState` struct
        // as well
        let StateAudio {
            queue,
            queue_position,
            current_song: _,
            repeat_mode,
            runtime,
            status: _,
            muted,
            volume,
        } = state;

        let seek_position = runtime.map(|r| r.seek_position);

        Self {
            repeat_mode,
            muted,
            volume,
            queue,
            queue_position,
            seek_position,
        }
    }
}

impl QueueState {
    /// Get the state from the audio kernel
    ///
    /// # Errors
    ///
    /// Returns an error if the status cannot be retrieved from the audio kernel
    #[inline]
    pub async fn retrieve(audio_kernel: &AudioKernelSender) -> Result<Self> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        audio_kernel.send(AudioCommand::ReportStatus(tx));
        Ok(rx.await?.into())
    }

    #[doc(hidden)]
    #[inline]
    pub fn retrieve_blocking(audio_kernel: &AudioKernelSender) -> Result<Self> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        audio_kernel.send(AudioCommand::ReportStatus(tx));
        Ok(rx.blocking_recv()?.into())
    }

    /// Restore the state into the audio kernel
    #[inline]
    pub fn restore_to(&self, audio_kernel: &AudioKernelSender) {
        // restore the repeat mode
        audio_kernel.send(AudioCommand::Queue(QueueCommand::SetRepeatMode(
            self.repeat_mode,
        )));
        // volume state
        let mute_command = if self.muted {
            AudioCommand::Volume(VolumeCommand::Mute)
        } else {
            AudioCommand::Volume(VolumeCommand::Unmute)
        };
        audio_kernel.send(mute_command);
        audio_kernel.send(AudioCommand::Volume(VolumeCommand::Set(self.volume)));
        // load songs into queue
        audio_kernel.send(AudioCommand::Queue(QueueCommand::AddToQueue(
            self.queue.as_ref().into(),
        )));
        // pause playback
        audio_kernel.send(AudioCommand::Pause);
        // skip to correct song
        if let Some(position) = self.queue_position {
            audio_kernel.send(AudioCommand::Queue(QueueCommand::SetPosition(position)));
        }
        // seek to the correct position
        if let Some(seek) = self.seek_position {
            audio_kernel.send(AudioCommand::Seek(SeekType::Absolute, seek));
        }
    }

    /// Save the state to a file
    /// This will overwrite the file if it exists
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created or written to
    /// or if the state cannot be serialized.
    #[inline]
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let writer = BufWriter::new(File::create(path).context(format!(
            "Queue Persistence: Failed to create/open {}",
            path.display()
        ))?);

        serde_json::to_writer_pretty(writer, self)
            .context("Queue Persistence: Failed to serialize state")?;

        Ok(())
    }

    /// Load the state from a file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened or read
    /// or if the state cannot be deserialized.
    #[inline]
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let reader = BufReader::new(File::open(path).context(format!(
            "Queue Persistence: Failed to read {}",
            path.display()
        ))?);

        serde_json::from_reader(reader).context("Queue Persistence: Failed to deserialize state")
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::{Arc, mpsc};

    use super::*;
    use mecomp_core::udp::StateChange;
    use mecomp_storage::db::schemas::song::Song;
    use mecomp_storage::test_utils::{
        IndexMode, SongCase, arb_song_case, arb_vec, arb_vec_and_index, create_song_metadata,
        init_test_database,
    };
    use pretty_assertions::assert_eq;
    use rstest::{fixture, rstest};
    use tempfile::tempdir;

    #[fixture]
    fn kernel() -> (Arc<AudioKernelSender>, mpsc::Receiver<StateChange>) {
        let (event_tx, event_rx) = mpsc::channel();
        (AudioKernelSender::start(event_tx), event_rx)
    }

    #[rstest]
    fn test_retrieve_plain(kernel: (Arc<AudioKernelSender>, mpsc::Receiver<StateChange>)) {
        let (audio_kernel, _event_rx) = kernel;
        let state = QueueState::retrieve_blocking(&audio_kernel).unwrap();
        assert_eq!(state, StateAudio::default().into());
    }

    #[rstest]
    #[case::one_song(arb_vec_and_index( &arb_song_case(), 1..=1, IndexMode::InBounds)())]
    #[case::many_songs(arb_vec_and_index( &arb_song_case(), 2..=10, IndexMode::InBounds)())]
    #[case::many_songs_guaranteed_nonzero_index((arb_vec( &arb_song_case(), 2..=10)(), 1))]
    #[tokio::test]
    async fn test_restore_retrieve_e2e(
        kernel: (Arc<AudioKernelSender>, mpsc::Receiver<StateChange>),
        #[case] (song_cases, index): (Vec<SongCase>, usize),
        #[values(true, false)] is_muted: bool,
        #[values(0.0, 1.0)] volume: f32,
        #[values(RepeatMode::None, RepeatMode::All)] repeat_mode: RepeatMode,
    ) {
        let temp_dir = tempdir().unwrap();
        // load songs into the database
        let db = init_test_database().await.unwrap();
        let mut songs = Vec::new();
        for sc in song_cases {
            let metadata = create_song_metadata(&temp_dir, sc).unwrap();
            let song = Song::try_load_into_db(&db, metadata).await.unwrap();
            songs.push(song.into());
        }

        // create the queue state
        let expected_queue_state = QueueState {
            repeat_mode,
            volume,
            muted: is_muted,
            queue: songs.into_boxed_slice(),
            queue_position: Some(index),
            seek_position: Some(Duration::from_secs(5)),
        };

        // load the queue state into the audio kernel
        let (audio_kernel, event_rx) = kernel;

        expected_queue_state.restore_to(&audio_kernel);

        // event breakdown:
        // 1. repeat mode change (1 event)
        // 2. volume state (2 events) (mute/unmute + set volume)
        // 3. loading songs into queue (2 event) (song change + unpause)
        // 4. pause playback (1 event) (pause)
        // 5. skip to correct song (1 event) (song change)
        // 6. seek to correct position (1 event) (seek)

        let mut expected_number_of_events = 8;
        if volume == 1.0 {
            expected_number_of_events -= 1; // no volume change event
        }
        if index == 0 {
            expected_number_of_events -= 1; // no 2nd song change event
        }
        let mut event_count = 0;
        while event_count < expected_number_of_events {
            match event_rx.recv_timeout(std::time::Duration::from_millis(500)) {
                Ok(event) => {
                    dbg!(event);
                    event_count += 1;
                }
                Err(_) => break,
            }
        }

        // ensure that no more than the expected number of events were received
        assert_eq!(event_count, expected_number_of_events);
        // ensure that no more events remain
        assert!(event_rx.try_recv().is_err());

        // retrieve the state from the audio kernel
        let retrieved_state = QueueState::retrieve(&audio_kernel).await.unwrap();

        // check that the retrieved state is the same as the original state
        assert_eq!(retrieved_state, expected_queue_state);
    }

    #[rstest]
    #[case::one_song(arb_vec_and_index( &arb_song_case(), 1..=1, IndexMode::InBounds)())]
    #[case::many_songs(arb_vec_and_index( &arb_song_case(), 2..=10, IndexMode::InBounds)())]
    #[case::many_songs_guaranteed_nonzero_index((arb_vec( &arb_song_case(), 2..=10)(), 1))]
    #[tokio::test]
    async fn test_save_load_e2e(
        #[case] (song_cases, index): (Vec<SongCase>, usize),
        #[values(true, false)] is_muted: bool,
        #[values(0.0, 1.0)] volume: f32,
        #[values(RepeatMode::None, RepeatMode::One)] repeat_mode: RepeatMode,
    ) {
        let temp_dir = tempdir().unwrap();
        // load songs into the database
        let db = init_test_database().await.unwrap();
        let mut songs = Vec::new();
        for sc in song_cases {
            let metadata = create_song_metadata(&temp_dir, sc).unwrap();
            let song = Song::try_load_into_db(&db, metadata).await.unwrap();
            songs.push(song.into());
        }

        // create the queue state
        let queue_state = QueueState {
            repeat_mode,
            volume,
            muted: is_muted,
            queue: songs.into_boxed_slice(),
            queue_position: Some(index),
            seek_position: Some(Duration::from_secs(10)),
        };

        // save the queue state to a file
        let path = temp_dir.path().join("test_queue_state.json");
        queue_state.save_to_file(&path).unwrap();

        // load the queue state from the file
        let loaded_queue_state = QueueState::load_from_file(&path).unwrap();

        // check that the loaded state is the same as the original state
        assert_eq!(loaded_queue_state, queue_state);

        // clean up
        fs::remove_file(path).unwrap();
    }
}
