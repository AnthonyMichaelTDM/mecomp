use rand::{prelude::SliceRandom, thread_rng};
use serde::{Deserialize, Serialize};

use crate::state::RepeatMode;
use mecomp_storage::db::schemas::song::Song;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Queue {
    songs: Vec<Song>,
    current_index: Option<usize>,
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
            current_index: None,
            repeat_mode: RepeatMode::None,
        }
    }

    pub fn add_song(&mut self, song: Song) {
        self.songs.push(song);
    }

    pub fn remove_song(&mut self, index: usize) {
        if index >= self.len() {
            return;
        }

        match self.current_index {
            Some(current_index)
                if current_index > index || (current_index == index && current_index > 0) =>
            {
                self.current_index = Some(current_index - 1);
            }
            Some(_) if self.len() <= 1 => {
                self.current_index = None;
            }
            _ => {}
        }

        self.songs.remove(index);
    }

    pub fn clear(&mut self) {
        // TODO: stop playback
        self.songs.clear();
        self.current_index = None;
    }

    #[must_use]
    pub fn current_song(&self) -> Option<&Song> {
        match self.current_index {
            Some(index) => self.songs.get(index),
            None => None,
        }
    }

    pub fn next_song(&mut self) -> Option<&Song> {
        // check if the queue is empty before incrementing the index
        if self.songs.is_empty() {
            return None;
        }
        if self.current_index.is_none() {
            self.current_index = Some(0);
            return self.songs.get(0);
        }

        let mut current_index = self.current_index.unwrap_or(0);

        // now, increment the index depending on the repeat mode
        match self.repeat_mode {
            RepeatMode::None => {
                if current_index + 1 < self.songs.len() {
                    current_index += 1;
                }
            }
            RepeatMode::Once => {
                if current_index + 1 < self.songs.len() {
                    current_index += 1;
                } else {
                    current_index = 0;
                    self.repeat_mode = RepeatMode::None;
                }
            }
            RepeatMode::Continuous => {
                current_index = (current_index + 1) % self.songs.len();
            }
        }

        self.current_index = Some(current_index);

        // return the current song
        self.songs.get(current_index)
    }

    /// Skip n songs
    pub fn skip_song(&mut self, n: usize) -> Option<&Song> {
        // if we can skip n songs without reaching the end of the queue
        match self.current_index {
            Some(current_index) if current_index + n < self.songs.len() => {
                self.current_index = Some(current_index + n);
                self.current_index.and_then(|index| self.songs.get(index))
            }
            _ => match self.repeat_mode {
                RepeatMode::None => {
                    if self.current_index == Some(self.songs.len() - 1) {
                        return None;
                    }
                    self.current_index = Some(self.songs.len() - 1);
                    self.current_index.and_then(|index| self.songs.get(index))
                }
                RepeatMode::Once => {
                    self.current_index = Some(0);
                    self.repeat_mode = RepeatMode::None;
                    self.current_index.and_then(|index| self.songs.get(index))
                }
                RepeatMode::Continuous => {
                    self.current_index = self
                        .current_index
                        .map(|index| (index + n) % self.songs.len());
                    self.current_index.and_then(|index| self.songs.get(index))
                }
            },
        }
    }

    pub fn previous_song(&mut self) -> Option<&Song> {
        match self.current_index {
            Some(current_index) if current_index > 0 => {
                self.current_index = Some(current_index - 1);
                self.songs.get(current_index - 1)
            }
            _ => None,
        }
    }

    pub fn set_repeat_mode(&mut self, repeat_mode: RepeatMode) {
        self.repeat_mode = repeat_mode;
    }

    pub fn shuffle(&mut self) {
        // shuffle
        self.songs.shuffle(&mut thread_rng());
        // swap current song to first
        match self.current_index {
            Some(current_index) if current_index != 0 => {
                self.songs.swap(0, current_index);
                self.current_index = Some(0);
            }
            _ => {}
        }
    }

    pub fn get(&self, index: usize) -> Option<&Song> {
        self.songs.get(index)
    }

    pub fn len(&self) -> usize {
        self.songs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.songs.is_empty()
        if self.current_index != 0 {
            self.songs.swap(0, self.current_index);
            self.current_index = 0;
        }
    }
}
