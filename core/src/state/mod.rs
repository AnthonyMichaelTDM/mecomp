pub mod library;
use std::{fmt::Display, time::Duration};

use mecomp_storage::{
    db::schemas::{album::Album, artist::Artist, song::Song},
    util::OneOrMany,
};
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
    /// Repeat Once: after going through the queue once, the player goes back to RepeatMode::None and continues
    Once,
    /// Repeat Continuously: after going through the queue, the player goes back to the beginning and continues
    Continuous,
}

impl RepeatMode {
    pub const fn is_none(&self) -> bool {
        matches!(self, RepeatMode::None)
    }

    pub const fn is_once(&self) -> bool {
        matches!(self, RepeatMode::Once)
    }

    pub const fn is_continuous(&self) -> bool {
        matches!(self, RepeatMode::Continuous)
    }
}

#[nutype(
    validate(predicate = |n| n.is_finite() && *n >= 0.0 && *n <= 100.0),
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
    pub current_album: Option<Album>,
    pub current_artist: OneOrMany<Artist>,
    pub repeat_mode: RepeatMode,
    pub runtime: Option<StateRuntime>,
    pub paused: bool,
    pub muted: bool,
    pub volume: f32,
}
