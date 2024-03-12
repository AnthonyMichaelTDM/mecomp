use rand::{prelude::SliceRandom, thread_rng};
use serde::{Deserialize, Serialize};

use crate::playback::RepeatMode;
use mecomp_storage::db::schemas::song::Song;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Queue {
    songs: Vec<Song>,
    current_index: usize,
    repeat_mode: RepeatMode,
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

impl Queue {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            songs: Vec::new(),
            current_index: 0,
            repeat_mode: RepeatMode::None,
        }
    }

    pub fn add_song(&mut self, song: Song) {
        self.songs.push(song);
    }

    pub fn remove_song(&mut self, index: usize) {
        // TODO: if index is current_index, update current_index
        if index < self.current_index || (index == self.current_index && self.current_index != 0) {
            self.current_index -= 1;
        }
        self.songs.remove(index);
    }

    pub fn clear(&mut self) {
        // TODO: stop playback
        self.songs.clear();
        self.current_index = 0;
    }

    #[must_use]
    pub fn current_song(&self) -> Option<&Song> {
        self.songs.get(self.current_index)
    }

    pub fn next_song(&mut self) -> Option<&Song> {
        match self.repeat_mode {
            RepeatMode::None => {
                if self.current_index + 1 < self.songs.len() {
                    self.current_index += 1;
                    self.songs.get(self.current_index)
                } else {
                    None
                }
            }
            RepeatMode::Once => {
                if self.current_index + 1 < self.songs.len() {
                    self.current_index += 1;
                } else {
                    self.current_index = 0;
                    self.repeat_mode = RepeatMode::None;
                }
                self.songs.get(self.current_index)
            }
            RepeatMode::Continuous => {
                self.current_index = (self.current_index + 1) % self.songs.len();
                self.songs.get(self.current_index)
            }
        }
    }

    pub fn previous_song(&mut self) -> Option<&Song> {
        if self.current_index > 0 {
            self.current_index -= 1;
            self.songs.get(self.current_index)
        } else {
            None
        }
    }

    pub fn set_repeat_mode(&mut self, repeat_mode: RepeatMode) {
        self.repeat_mode = repeat_mode;
    }

    pub fn shuffle(&mut self) {
        // shuffle
        self.songs.shuffle(&mut thread_rng());
        // swap current song to first
        if self.current_index != 0 {
            self.songs.swap(0, self.current_index);
            self.current_index = 0;
        }
    }
}
