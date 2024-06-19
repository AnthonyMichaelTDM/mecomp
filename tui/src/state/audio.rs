//! This module contains the implementation of audio state store.
//! which is updated every tick and used by views to render the audio playback and queue state.
//!
//! The audio state store is responsible for maintaining the audio state, and for handling audio related actions.

use std::{sync::Arc, time::Duration};

use tokio::sync::{
    broadcast,
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};

use mecomp_core::rpc::MusicPlayerClient;
use mecomp_core::state::StateAudio;

use crate::termination::Interrupted;

use super::action::{AudioAction, PlaybackAction, QueueAction, VolumeAction};

pub const TICK_RATE: Duration = Duration::from_millis(100);

/// The audio state store.
#[derive(Debug, Clone)]
pub struct AudioState {
    state_tx: UnboundedSender<StateAudio>,
}

impl AudioState {
    /// create a new audio state store, and return the receiver for listening to state updates.
    pub fn new() -> (Self, UnboundedReceiver<StateAudio>) {
        let (state_tx, state_rx) = unbounded_channel::<StateAudio>();

        (Self { state_tx }, state_rx)
    }

    /// a loop that updates the audio state every tick.
    pub async fn main_loop(
        &self,
        daemon: Arc<MusicPlayerClient>,
        mut action_rx: UnboundedReceiver<AudioAction>,
        mut interrupt_rx: broadcast::Receiver<Interrupted>,
    ) -> anyhow::Result<Interrupted> {
        let mut state = get_state(daemon.clone()).await?;

        // the initial state once
        self.state_tx.send(state.clone())?;

        // the ticker
        let mut ticker = tokio::time::interval(TICK_RATE);

        let result = loop {
            tokio::select! {
                // Handle the actions coming from the UI
                // and process them to do async operations
                Some(action) = action_rx.recv() => {
                    self.handle_action(daemon.clone(), action).await?;
                },
                // Tick to terminate the select every N milliseconds
                _ = ticker.tick() => {},
                // Catch and handle interrupt signal to gracefully shutdown
                Ok(interrupted) = interrupt_rx.recv() => {
                    break interrupted;
                }
            }

            state = get_state(daemon.clone()).await?;
            self.state_tx.send(state.clone())?;
        };

        Ok(result)
    }

    async fn handle_action(
        &self,
        daemon: Arc<MusicPlayerClient>,
        action: AudioAction,
    ) -> anyhow::Result<()> {
        match action {
            AudioAction::Playback(action) => handle_playback(daemon, action).await?,
            AudioAction::Queue(action) => handle_queue(daemon, action).await?,
        }

        Ok(())
    }
}

/// get the audio state from the daemon.
async fn get_state(daemon: Arc<MusicPlayerClient>) -> anyhow::Result<StateAudio> {
    let ctx = tarpc::context::current();
    Ok(daemon.state_audio(ctx).await?.unwrap_or_default())
}

/// handle a playback action
async fn handle_playback(
    daemon: Arc<MusicPlayerClient>,
    action: PlaybackAction,
) -> anyhow::Result<()> {
    let ctx = tarpc::context::current();

    match action {
        PlaybackAction::Toggle => daemon.playback_toggle(ctx).await?,
        PlaybackAction::Next => daemon.playback_skip_forward(ctx, 1).await?,
        PlaybackAction::Previous => daemon.playback_skip_backward(ctx, 1).await?,
        PlaybackAction::Seek(seek_type, duration) => {
            daemon.playback_seek(ctx, seek_type, duration).await?
        }
        PlaybackAction::Volume(VolumeAction::Increase(amount)) => {
            daemon.playback_volume_up(ctx, amount).await?
        }
        PlaybackAction::Volume(VolumeAction::Decrease(amount)) => {
            daemon.playback_volume_down(ctx, amount).await?
        }
        PlaybackAction::ToggleMute => daemon.playback_volume_toggle_mute(ctx).await?,
    }

    Ok(())
}

/// handle a queue action
async fn handle_queue(daemon: Arc<MusicPlayerClient>, action: QueueAction) -> anyhow::Result<()> {
    let ctx = tarpc::context::current();

    match action {
        QueueAction::Add(ids) => daemon.queue_add_list(ctx, ids).await??,
        QueueAction::Remove(index) => {
            daemon.queue_remove_range(ctx, index..index + 1).await?;
        }
        QueueAction::SetPosition(index) => daemon.queue_set_index(ctx, index).await?,
        QueueAction::Shuffle => daemon.playback_shuffle(ctx).await?,
        QueueAction::Clear => daemon.playback_clear(ctx).await?,
        QueueAction::SetRepeatMode(mode) => daemon.playback_repeat(ctx, mode).await?,
    }

    Ok(())
}
