#![allow(clippy::module_name_repetitions)]
pub mod library;
use std::{fmt::Display, time::Duration};

use mecomp_storage::db::schemas::song::Song;
use nutype::nutype;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

#[derive(
    Clone, Copy, Debug, Display, PartialEq, Eq, Deserialize, Serialize, EnumIter, EnumString,
)]
pub enum SeekType {
    Absolute,
    Relative,
}

#[derive(
    Clone, Copy, Debug, Display, PartialEq, Eq, Deserialize, Serialize, EnumIter, EnumString,
)]
pub enum RepeatMode {
    /// No repeat: after the queue is finished the player stops
    None,
    /// Repeat Once: after going through the queue once, the player goes back to `RepeatMode::None` and continues
    Once,
    /// Repeat Continuously: after going through the queue, the player goes back to the beginning and continues
    Continuous,
}

impl RepeatMode {
    #[must_use]
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    #[must_use]
    pub const fn is_once(&self) -> bool {
        matches!(self, Self::Once)
    }

    #[must_use]
    pub const fn is_continuous(&self) -> bool {
        matches!(self, Self::Continuous)
    }
}

#[nutype(
    sanitize(with = | n | if n.is_finite() { n.clamp(0.0, 100.0) } else { 0.0 }),
    derive(Copy, Clone, Debug, PartialEq, Deserialize, Serialize)
)]
pub struct Percent(f32);

impl Display for Percent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2}%", self.into_inner())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct StateRuntime {
    pub seek_position: f64,
    pub seek_percent: Percent,
    pub duration: Duration,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StateAudio {
    pub queue: Box<[Song]>,
    pub queue_position: Option<usize>,
    pub current_song: Option<Song>,
    pub repeat_mode: RepeatMode,
    pub runtime: Option<StateRuntime>,
    pub paused: bool,
    pub muted: bool,
    pub volume: f32,
}
