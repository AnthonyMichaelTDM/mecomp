pub mod library;
use std::{ fmt::Display, time::Duration};

use mecomp_storage::{db::schemas::{album::Album, artist::Artist, song::Song}, util::OneOrMany};
use nutype::nutype;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString, EnumIter};

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

impl Display for Percent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2}%", self)
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
    pub volume: Percent,
}


