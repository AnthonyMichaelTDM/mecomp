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
                if current_index < self.songs.len() - 1 {
                    current_index += 1;
                }
            }
            RepeatMode::Once => {
                if current_index < self.songs.len() - 1 {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{arb_repeat_mode, RepeatMode};
    use crate::test_utils::{
        arb_song_case, arb_vec, bar_sc, baz_sc, create_song, foo_sc, init, SongCase,
    };

    use pretty_assertions::assert_eq;
    use proptest::prelude::*;
    use rstest::*;

    #[test]
    fn test_new_queue() {
        let mut queue = Queue::new();
        assert_eq!(queue.len(), 0);
        assert_eq!(queue.current_index, None);
        assert_eq!(queue.repeat_mode, RepeatMode::None);

        assert_eq!(queue.current_song(), None);
        assert_eq!(queue.next_song(), None);
        assert_eq!(queue.current_index, None);
        assert_eq!(queue.previous_song(), None);
        assert_eq!(queue.current_index, None);
    }

    #[rstest]
    #[case(foo_sc())]
    #[case(bar_sc())]
    #[case(baz_sc())]
    #[tokio::test]
    async fn test_add_song(#[case] song: SongCase) -> anyhow::Result<()> {
        init().await?;
        let mut queue = Queue::new();
        let song = create_song(song).await?;
        queue.add_song(song.clone());
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.songs[0], song);
        assert_eq!(queue.current_song(), None);

        Ok(())
    }

    #[rstest]
    #[case(foo_sc())]
    #[case(bar_sc())]
    #[case(baz_sc())]
    #[tokio::test]
    async fn test_remove_song(#[case] song: SongCase) -> anyhow::Result<()> {
        init().await?;
        let mut queue = Queue::new();
        let song = create_song(song).await?;
        queue.add_song(song.clone());
        queue.remove_song(0);
        assert_eq!(queue.songs.len(), 0);

        Ok(())
    }

    #[rstest]
    #[case(foo_sc(), bar_sc())]
    #[case(bar_sc(), baz_sc())]
    #[case(baz_sc(), foo_sc())]
    #[tokio::test]
    async fn test_next_previous_basic(
        #[case] song1: SongCase,
        #[case] song2: SongCase,
    ) -> anyhow::Result<()> {
        init().await?;
        let mut queue = Queue::new();
        let song1 = create_song(song1).await?;
        let song2 = create_song(song2).await?;
        queue.add_song(song1.clone());
        queue.add_song(song2.clone());
        assert_eq!(queue.next_song(), Some(&song1));
        assert_eq!(queue.next_song(), Some(&song2));
        assert_eq!(queue.previous_song(), Some(&song1));
        assert_eq!(queue.previous_song(), None);

        Ok(())
    }

    proptest! {
        #[test]
        fn test_next_song_rp_none(
            songs in arb_vec(arb_song_case(), 2usize..5),
            skip in 1usize..10,
        )  {
            tokio::runtime::Runtime::new().unwrap().block_on(async move {
                init().await.map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
                let mut queue =  Queue::new();
                let len = songs.len();
                for sc in songs.into_iter() {
                    queue.add_song(create_song(sc).await.map_err(|e| TestCaseError::fail(format!("{e:?}")))?);
                }
                queue.set_repeat_mode(RepeatMode::None);

                for _ in 0..skip {
                    let _ = queue.next_song();
                }

                if skip < len {
                    prop_assert_eq!(queue.current_song(), queue.get(skip));
                } else {
                    prop_assert_eq!(queue.current_song(), queue.songs.last());
                }

                Ok(())
            })?;
        }

        #[test]
        fn test_next_song_rp_once(
            songs in arb_vec(arb_song_case(), 2usize..5),
            skip in 1usize..10,
        )  {
            tokio::runtime::Runtime::new().unwrap().block_on(async move {
                init().await.map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
                let mut queue =  Queue::new();
                let len = songs.len();
                for sc in songs.into_iter() {
                    queue.add_song(create_song(sc).await.map_err(|e| TestCaseError::fail(format!("{e:?}")))?);
                }
                queue.set_repeat_mode(RepeatMode::Once);

                for _ in 0..skip {
                    let _ = queue.next_song();
                }

                if skip / len <= 1 {
                    prop_assert_eq!(queue.current_song(), queue.get(skip % len));
                } else {
                    prop_assert_eq!(queue.current_song(), queue.songs.last());
                }

                Ok(())
            })?;
        }

        #[test]
        fn test_next_song_rp_continuous(
            songs in arb_vec(arb_song_case(), 2usize..5),
            skip in 1usize..15,
        )  {
            tokio::runtime::Runtime::new().unwrap().block_on(async move {
                init().await.map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
                let mut queue =  Queue::new();
                let len = songs.len();
                for sc in songs.into_iter() {
                    queue.add_song(create_song(sc).await.map_err(|e| TestCaseError::fail(format!("{e:?}")))?);
                }
                queue.set_repeat_mode(RepeatMode::Continuous);

                for _ in 0..skip {
                    let _ = queue.next_song();
                }

                prop_assert_eq!(queue.current_song(), queue.get(skip % len));

                Ok(())
            })?;
        }


        #[test]
        fn test_set_repeat_mode(repeat_mode in arb_repeat_mode()) {
            let mut queue = Queue::new();
            queue.set_repeat_mode(repeat_mode);
            prop_assert_eq!(queue.repeat_mode, repeat_mode);
        }
    }
}
