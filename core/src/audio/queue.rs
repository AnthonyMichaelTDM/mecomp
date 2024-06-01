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

    pub fn add_songs(&mut self, songs: Vec<Song>) {
        self.songs.extend(songs);
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
        self.songs.clear();
        self.current_index = None;
    }

    #[must_use]
    pub fn current_song(&self) -> Option<&Song> {
        self.current_index.and_then(|index| self.songs.get(index))
    }

    #[inline]
    pub fn next_song(&mut self) -> Option<&Song> {
        self.skip_forward(1)
    }

    /// Skip forward n songs in the queue.
    ///
    /// progresses the current index by n, following the repeat mode rules.
    pub fn skip_forward(&mut self, n: usize) -> Option<&Song> {
        match self.current_index {
            Some(current_index) if current_index + n < self.songs.len() => {
                self.current_index = Some(current_index + n);
                self.current_index.and_then(|index| self.songs.get(index))
            }
            Some(current_index) => {
                match self.repeat_mode {
                    RepeatMode::None => {
                        // if we reach this point, then skipping would put us at the end of the queue,
                        // so let's just stop playback
                        self.current_index = None;
                        None
                    }
                    RepeatMode::Once => {
                        // if we reach this point, then skipping would put us past the end of the queue,
                        // so let's emutate looping back to the first song and then skipping n - len songs
                        self.current_index = Some(0);
                        self.repeat_mode = RepeatMode::None;
                        self.skip_forward((current_index + n) - self.songs.len())
                    }
                    RepeatMode::Continuous => {
                        // if we reach this point, then skipping would put us past the end of the queue,
                        // so let's emulate looping over the songs as many times as needed, then skipping the remaining songs
                        self.current_index = Some((current_index + n) % self.songs.len());
                        self.current_index.and_then(|index| self.songs.get(index))
                    }
                }
            }
            None => {
                if self.songs.is_empty() {
                    return None;
                }
                if n == 0 {
                    return None;
                }

                self.current_index = Some(0);
                self.skip_forward(n - 1)
            }
        }
    }

    pub fn previous_song(&mut self) -> Option<&Song> {
        self.skip_backward(1)
    }

    pub fn skip_backward(&mut self, n: usize) -> Option<&Song> {
        match self.current_index {
            Some(current_index) if current_index >= n => {
                self.current_index = Some(current_index - n);
                self.current_index.and_then(|index| self.songs.get(index))
            }
            _ => {
                self.current_index = None;
                None
            }
        }
    }

    pub fn set_repeat_mode(&mut self, repeat_mode: RepeatMode) {
        self.repeat_mode = repeat_mode;
    }

    #[must_use]
    pub const fn get_repeat_mode(&self) -> RepeatMode {
        self.repeat_mode
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

    #[must_use]
    pub fn get(&self, index: usize) -> Option<&Song> {
        self.songs.get(index)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.songs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.songs.is_empty()
    }

    #[must_use]
    pub const fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    #[must_use]
    pub fn queued_songs(&self) -> Box<[Song]> {
        self.songs.clone().into_boxed_slice()
    }

    /// Sets the current index, clamped to the nearest valid index.
    pub fn set_current_index(&mut self, index: usize) {
        if self.songs.is_empty() {
            self.current_index = None;
        } else {
            self.current_index = Some(index.min(self.songs.len().saturating_sub(1)));
        }
    }

    /// Removes a range of songs from the queue.
    /// If the current index is within the range, it will be set to the next valid index (or the
    /// previous valid index if the range included the end of the queue).
    pub fn remove_range(&mut self, range: std::ops::Range<usize>) {
        if range.is_empty() || self.is_empty() {
            return;
        }
        let current_index = self.current_index.unwrap_or_default();
        let range_end = range.end.min(self.songs.len());
        let range_start = range.start.min(range_end);

        self.songs.drain(range_start..range_end);

        if current_index >= range_start && current_index < range_end {
            self.current_index = Some(range_start + 1);
        } else if current_index >= range_end {
            self.current_index = Some(current_index - (range_end - range_start));
        }
        if self.current_index.unwrap_or_default() >= self.songs.len() || self.is_empty() {
            self.current_index = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::RepeatMode;
    use crate::test_utils::{
        arb_song_case, arb_vec, arb_vec_and_range_and_index, bar_sc, baz_sc, create_song, foo_sc,
        init, RangeEndMode, RangeIndexMode, RangeStartMode, SongCase, TIMEOUT,
    };

    use mecomp_storage::db::init_test_database;

    use pretty_assertions::assert_eq;
    use rstest::*;
    use rstest_reuse;
    use rstest_reuse::{apply, template};

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
        let db = init_test_database().await.unwrap();
        init().await?;
        let mut queue = Queue::new();
        let song = create_song(&db, song).await?;
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
        let db = init_test_database().await.unwrap();
        init().await?;
        let mut queue = Queue::new();
        let song = create_song(&db, song).await?;
        queue.add_song(song);
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
        let db = init_test_database().await.unwrap();
        init().await?;
        let mut queue = Queue::new();
        let song1 = create_song(&db, song1).await?;
        let song2 = create_song(&db, song2).await?;
        queue.add_song(song1.clone());
        queue.add_song(song2.clone());
        assert_eq!(queue.next_song(), Some(&song1));
        assert_eq!(queue.next_song(), Some(&song2));
        assert_eq!(queue.previous_song(), Some(&song1));
        assert_eq!(queue.previous_song(), None);

        Ok(())
    }

    #[template]
    #[rstest]
    #[case::more_than_len( arb_vec(&arb_song_case(), 4..=5 )(), 7 )]
    #[case::way_more_than_len( arb_vec(&arb_song_case(), 3..=5 )(), 11 )]
    #[case::skip_len( arb_vec(&arb_song_case(), 5..=5 )(), 5 )]
    #[case::skip_len_twice( arb_vec(&arb_song_case(), 5..=5 )(), 10 )]
    #[case::less_than_len( arb_vec(&arb_song_case(), 4..=5 )(), 3 )]
    #[case::skip_one( arb_vec(&arb_song_case(), 2..=5 )(), 1 )]
    #[timeout(TIMEOUT)]
    pub fn skip_song_test_template(#[case] songs: Vec<SongCase>, #[case] skip: usize) {}

    #[apply(skip_song_test_template)]
    #[tokio::test]
    async fn test_skip_song_rp_none(songs: Vec<SongCase>, skip: usize) -> anyhow::Result<()> {
        let db = init_test_database().await.unwrap();
        init().await?;
        let mut queue = Queue::new();
        let len = songs.len();
        for sc in songs {
            queue.add_song(create_song(&db, sc).await?);
        }
        queue.set_repeat_mode(RepeatMode::None);

        queue.skip_forward(skip);

        if skip <= len {
            assert_eq!(
                queue.current_song(),
                queue.get(skip - 1),
                "len: {len}, skip: {skip}, current_index: {current_index}",
                current_index = queue.current_index.unwrap_or_default()
            );
        } else {
            assert_eq!(
                queue.current_song(),
                None,
                "len: {len}, skip: {skip}, current_index: {current_index}",
                current_index = queue.current_index.unwrap_or_default()
            );
        }

        Ok(())
    }

    #[apply(skip_song_test_template)]
    #[tokio::test]
    async fn test_skip_song_rp_once(songs: Vec<SongCase>, skip: usize) -> anyhow::Result<()> {
        let db = init_test_database().await.unwrap();
        init().await?;
        let mut queue = Queue::new();
        let len = songs.len();
        for sc in songs {
            queue.add_song(create_song(&db, sc).await?);
        }
        queue.set_repeat_mode(RepeatMode::Once);

        queue.skip_forward(skip);

        if skip <= len {
            // if we haven't reached the end of the queue
            assert_eq!(
                queue.current_song(),
                queue.get(skip - 1),
                "len: {len}, skip: {skip}, current_index: {current_index}",
                current_index = queue.current_index.unwrap_or_default()
            );
        } else if skip <= 2 * len {
            // if we reached the end of the queue, looped back, and didn't reach the end again
            assert_eq!(
                queue.current_song(),
                queue.get(skip - 1 - len),
                "len: {len}, skip: {skip}, current_index: {current_index}",
                current_index = queue.current_index.unwrap_or_default()
            );
        } else {
            // if we reached the end of the queue, looped back, and reached the end again
            assert_eq!(
                queue.current_song(),
                None,
                "len: {len}, skip: {skip}, current_index: {current_index}",
                current_index = queue.current_index.unwrap_or_default()
            );
        }

        Ok(())
    }

    #[apply(skip_song_test_template)]
    #[tokio::test]
    async fn test_next_song_rp_continuous(songs: Vec<SongCase>, skip: usize) -> anyhow::Result<()> {
        let db = init_test_database().await.unwrap();
        init().await?;
        let mut queue = Queue::new();
        let len = songs.len();
        for sc in songs {
            queue.add_song(create_song(&db, sc).await?);
        }
        queue.set_repeat_mode(RepeatMode::Continuous);

        queue.skip_forward(skip);

        assert_eq!(
            queue.current_song(),
            queue.get((skip - 1) % len),
            "len: {len}, skip: {skip}, current_index: {current_index}",
            current_index = queue.current_index.unwrap_or_default()
        );

        Ok(())
    }

    #[rstest]
    #[case(RepeatMode::None)]
    #[case(RepeatMode::Once)]
    #[case(RepeatMode::Continuous)]
    #[test]
    fn test_set_repeat_mode(#[case] repeat_mode: RepeatMode) {
        let mut queue = Queue::new();
        queue.set_repeat_mode(repeat_mode);
        assert_eq!(queue.repeat_mode, repeat_mode);
    }

    #[rstest]
    #[case::within_range( arb_vec(&arb_song_case(), 5..=10 )(), 3 )]
    #[case::at_start( arb_vec(&arb_song_case(), 5..=10 )(), 0 )]
    #[case::at_end( arb_vec(&arb_song_case(), 10..=10 )(), 9 )]
    #[case::empty( arb_vec(&arb_song_case(),0..=0)(), 0)]
    #[case::out_of_range( arb_vec(&arb_song_case(), 5..=10 )(), 15 )]
    #[tokio::test]
    async fn test_set_current_index(
        #[case] songs: Vec<SongCase>,
        #[case] index: usize,
    ) -> anyhow::Result<()> {
        let db = init_test_database().await?;
        init().await?;
        let mut queue = Queue::new();
        let len = songs.len();
        for sc in songs {
            queue.add_song(create_song(&db, sc).await?);
        }

        queue.set_current_index(index);

        if len == 0 {
            assert_eq!(queue.current_index, None);
        } else if index >= len {
            assert_eq!(queue.current_index, Some(len - 1));
        } else {
            assert_eq!(queue.current_index, Some(index.min(len - 1)));
        }

        Ok(())
    }

    #[rstest]
    #[case( arb_vec_and_range_and_index(&arb_song_case(), 5..=10,RangeStartMode::Standard,RangeEndMode::Standard, RangeIndexMode::InRange )() )]
    #[case( arb_vec_and_range_and_index(&arb_song_case(), 5..=10,RangeStartMode::Standard,RangeEndMode::Standard, RangeIndexMode::BeforeRange )() )]
    #[case( arb_vec_and_range_and_index(&arb_song_case(), 5..=10,RangeStartMode::Standard,RangeEndMode::Standard, RangeIndexMode::AfterRangeInBounds )() )]
    #[case( arb_vec_and_range_and_index(&arb_song_case(), 5..=10,RangeStartMode::Standard,RangeEndMode::Standard, RangeIndexMode::OutOfBounds )() )]
    #[case( arb_vec_and_range_and_index(&arb_song_case(), 5..=10,RangeStartMode::Standard,RangeEndMode::Standard, RangeIndexMode::InBounds )() )]
    #[case( arb_vec_and_range_and_index(&arb_song_case(), 5..=10,RangeStartMode::OutOfBounds,RangeEndMode::Standard, RangeIndexMode::InRange )() )]
    #[case( arb_vec_and_range_and_index(&arb_song_case(), 0..=0,RangeStartMode::Zero,RangeEndMode::Start, RangeIndexMode::InBounds )() )]
    #[case( arb_vec_and_range_and_index(&arb_song_case(), 5..=10, RangeStartMode::Standard, RangeEndMode::Start, RangeIndexMode::InBounds)() )]
    #[case( arb_vec_and_range_and_index(&arb_song_case(), 5..=10,RangeStartMode::Standard,RangeEndMode::OutOfBounds, RangeIndexMode::InBounds )() )]
    #[case( arb_vec_and_range_and_index(&arb_song_case(), 5..=10,RangeStartMode::Standard,RangeEndMode::OutOfBounds, RangeIndexMode::InRange )() )]
    #[case( arb_vec_and_range_and_index(&arb_song_case(), 5..=10,RangeStartMode::Standard,RangeEndMode::OutOfBounds, RangeIndexMode::BeforeRange )() )]
    #[tokio::test]
    async fn test_remove_range(
        #[case] params: (Vec<SongCase>, std::ops::Range<usize>, Option<usize>),
    ) -> anyhow::Result<()> {
        let (songs, range, index) = params;
        let len = songs.len();
        let db = init_test_database().await?;
        init().await?;
        let mut queue = Queue::new();
        for sc in songs {
            queue.add_song(create_song(&db, sc).await?);
        }

        if let Some(index) = index {
            queue.set_current_index(index);
        }

        let unmodified_songs = queue.clone();

        queue.remove_range(range.clone());

        let start = range.start;
        let end = range.end.min(len);

        // our tests fall into 4 categories:
        // 1. nothing is removed (start==end or start>=len)
        // 2. everything is removed (start==0 and end>=len)
        // 3. only some songs are removed(end>start>0)

        if start >= len || start == end {
            assert_eq!(queue.len(), len);
        } else if start == 0 && end >= len {
            assert_eq!(queue.len(), 0);
            assert_eq!(queue.current_index, None);
        } else {
            assert_eq!(queue.len(), len - (end.min(len) - start));
            for i in 0..start {
                assert_eq!(queue.get(i), unmodified_songs.get(i));
            }
            for i in end..len {
                assert_eq!(queue.get(i - (end - start)), unmodified_songs.get(i));
            }
        }
        Ok(())
    }
}
