pub mod library;
use nutype::nutype;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString, EnumIter};

use crate::audio::queue::Queue;

#[derive(Clone, Copy, Debug, Display, PartialEq, Eq, Deserialize, Serialize, EnumIter, EnumString)]
pub enum SeekType {
    Absolute,
    Relative,
}

#[derive(Clone, Copy, Debug, Display, PartialEq, Eq, Deserialize, Serialize, EnumIter, EnumString)]
pub enum RepeatMode {
    None,
    Once,
    Continuous,
}

#[nutype(
    validate(predicate = |n| n.is_finite() && *n >= 0.0 && *n <= 100.0), 
    derive(Copy, Clone, Debug, PartialEq, Deserialize, Serialize)
)]
pub struct Percent(f32);


#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct StateRuntime {
    pub seek_position: f64,
    pub seek_percent: Percent,
    pub duration: f64,
    pub volume: Percent,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StateAudio {
    pub queue: Queue,
    pub repeat_mode: RepeatMode,
    pub shuffle: bool,
    pub runtime: StateRuntime,
    pub playing: bool,
    pub muted: bool,
    pub volume: Percent,
}


