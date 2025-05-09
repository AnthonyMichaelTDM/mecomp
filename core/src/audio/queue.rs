use rand::{prelude::SliceRandom, thread_rng};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::state::RepeatMode;
use mecomp_storage::db::schemas::song::SongBrief;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Queue {
    songs: Vec<SongBrief>,
    current_index: Option<usize>,
    repeat_mode: RepeatMode,
}

impl Default for Queue {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Queue {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            songs: Vec::new(),
            current_index: None,
            repeat_mode: RepeatMode::None,
        }
    }

    #[instrument]
    pub fn add_song(&mut self, song: SongBrief) {
        self.songs.push(song);
    }

    #[instrument]
    pub fn add_songs(&mut self, songs: Vec<SongBrief>) {
        self.songs.extend(songs);
    }

    #[instrument]
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

    #[instrument]
    pub fn clear(&mut self) {
        self.songs.clear();
        self.current_index = None;
    }

    #[must_use]
    #[instrument]
    pub fn current_song(&self) -> Option<&SongBrief> {
        self.current_index.and_then(|index| self.songs.get(index))
    }

    #[instrument]
    pub fn next_song(&mut self) -> Option<&SongBrief> {
        if self.repeat_mode == RepeatMode::One && self.current_index.is_some() {
            self.current_song()
        } else {
            self.skip_forward(1)
        }
    }

    /// Skip forward n songs in the queue.
    ///
    /// progresses the current index by n, following the repeat mode rules.
    #[instrument]
    pub fn skip_forward(&mut self, n: usize) -> Option<&SongBrief> {
        match self.current_index {
            Some(current_index) if current_index + n < self.songs.len() => {
                self.current_index = Some(current_index + n);
                self.current_index.and_then(|index| self.songs.get(index))
            }
            Some(current_index) => {
                match self.repeat_mode {
                    RepeatMode::None | RepeatMode::One => {
                        // if we reach this point, then skipping would put us at the end of the queue,
                        // so let's just stop playback
                        self.current_index = None;
                        self.songs.clear();
                        None
                    }
                    RepeatMode::All => {
                        // if we reach this point, then skipping would put us past the end of the queue,
                        // so let's emulate looping over the songs as many times as needed, then skipping the remaining songs
                        self.current_index = Some((current_index + n) % self.songs.len());
                        self.current_index.and_then(|index| self.songs.get(index))
                    }
                }
            }
            None => {
                if self.songs.is_empty() || n == 0 {
                    return None;
                }

                self.current_index = Some(0);
                self.skip_forward(n - 1)
            }
        }
    }

    #[instrument]
    pub fn previous_song(&mut self) -> Option<&SongBrief> {
        self.skip_backward(1)
    }

    #[instrument]
    pub fn skip_backward(&mut self, n: usize) -> Option<&SongBrief> {
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

    #[instrument]
    pub fn set_repeat_mode(&mut self, repeat_mode: RepeatMode) {
        self.repeat_mode = repeat_mode;
    }

    #[must_use]
    #[inline]
    pub const fn get_repeat_mode(&self) -> RepeatMode {
        self.repeat_mode
    }

    #[instrument]
    pub fn shuffle(&mut self) {
        // swap current song to first
        match self.current_index {
            Some(current_index) if current_index != 0 && !self.is_empty() => {
                self.songs.swap(0, current_index);
                self.current_index = Some(0);
            }
            _ => {}
        }
        if self.len() <= 1 {
            return;
        }
        // shuffle the slice from [1..]
        self.songs[1..].shuffle(&mut thread_rng());
    }

    #[must_use]
    #[instrument]
    pub fn get(&self, index: usize) -> Option<&SongBrief> {
        self.songs.get(index)
    }

    #[must_use]
    #[instrument]
    pub fn len(&self) -> usize {
        self.songs.len()
    }

    #[must_use]
    #[instrument]
    pub fn is_empty(&self) -> bool {
        self.songs.is_empty()
    }

    #[must_use]
    #[inline]
    pub const fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    #[must_use]
    #[instrument]
    pub fn queued_songs(&self) -> Box<[SongBrief]> {
        self.songs.clone().into_boxed_slice()
    }

    /// Sets the current index, clamped to the nearest valid index.
    #[instrument]
    pub fn set_current_index(&mut self, index: usize) {
        if self.songs.is_empty() {
            self.current_index = None;
        } else {
            self.current_index = Some(index.min(self.songs.len() - 1));
        }
    }

    /// Removes a range of songs from the queue.
    /// If the current index is within the range, it will be set to the next valid index (or the
    /// previous valid index if the range included the end of the queue).
    #[instrument]
    pub fn remove_range(&mut self, range: std::ops::Range<usize>) {
        if range.is_empty() || self.is_empty() {
            return;
        }
        let current_index = self.current_index.unwrap_or_default();
        let range_end = range.end.min(self.songs.len());
        let range_start = range.start.min(range_end);

        self.songs.drain(range_start..range_end);

        if current_index >= range_start && current_index < range_end {
            // current index is within the range
            self.current_index = Some(range_start);
        } else if current_index >= range_end {
            // current index is after the range
            self.current_index = Some(current_index - (range_end - range_start));
        }

        // if the current index was put out of bounds, set it to None
        if self.current_index.unwrap_or_default() >= self.songs.len() || self.is_empty() {
            self.current_index = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::RepeatMode;
    use crate::test_utils::init;

    use mecomp_storage::db::schemas::song::SongChangeSet;
    use mecomp_storage::test_utils::{
        IndexMode, RangeEndMode, RangeIndexMode, RangeStartMode, SongCase, arb_song_case, arb_vec,
        arb_vec_and_index, arb_vec_and_range_and_index, create_song_with_overrides,
        init_test_database,
    };

    use pretty_assertions::assert_eq;
    use rstest::*;
    use rstest_reuse;
    use rstest_reuse::{apply, template};

    #[test]
    fn test_new_queue() {
        let mut queue = Queue::default();
        assert_eq!(queue.len(), 0);
        assert_eq!(queue.current_index(), None);
        assert_eq!(queue.get_repeat_mode(), RepeatMode::None);

        assert_eq!(queue.current_song(), None);
        assert_eq!(queue.next_song(), None);
        assert_eq!(queue.current_index, None);
        assert_eq!(queue.previous_song(), None);
        assert_eq!(queue.current_index, None);
    }

    #[rstest]
    #[case(arb_song_case()())]
    #[case(arb_song_case()())]
    #[case(arb_song_case()())]
    #[tokio::test]
    async fn test_add_song(#[case] song: SongCase) -> anyhow::Result<()> {
        init();

        let db = init_test_database().await.unwrap();

        let mut queue = Queue::new();
        let song = create_song_with_overrides(&db, song, SongChangeSet::default()).await?;
        let song: SongBrief = song.into();
        queue.add_song(song.clone());
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.songs[0], song);
        assert_eq!(queue.current_song(), None);

        Ok(())
    }

    #[tokio::test]
    async fn test_add_songs() {
        init();
        let db = init_test_database().await.unwrap();

        let songs = vec![
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default())
                .await
                .unwrap()
                .into(),
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default())
                .await
                .unwrap()
                .into(),
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default())
                .await
                .unwrap()
                .into(),
        ];
        let mut queue = Queue::new();
        queue.add_songs(songs.clone());
        assert_eq!(queue.len(), 3);
        assert_eq!(queue.queued_songs(), songs.into_boxed_slice());
        assert_eq!(queue.current_song(), None);
    }

    #[rstest]
    #[case::index_oob(vec![arb_song_case()(), arb_song_case()(), arb_song_case()()], 1, 4, Some(1))]
    #[case::index_before_current(vec![arb_song_case()(), arb_song_case()(), arb_song_case()()], 2, 1, Some(1))]
    #[case::index_after_current(vec![arb_song_case()(),arb_song_case()(),arb_song_case()()], 1, 2, Some(1))]
    #[case::index_at_current(vec![arb_song_case()(), arb_song_case()(), arb_song_case()()],  1, 1, Some(0))]
    #[case::index_at_current_zero(vec![arb_song_case()(), arb_song_case()(), arb_song_case()()],  0, 0, Some(0))]
    #[case::remove_only_song(vec![arb_song_case()()], 0, 0, None )]
    #[tokio::test]
    async fn test_remove_song(
        #[case] songs: Vec<SongCase>,
        #[case] current_index_before: usize,
        #[case] index_to_remove: usize,
        #[case] expected_current_index_after: Option<usize>,
    ) {
        init();
        let db = init_test_database().await.unwrap();
        let mut queue = Queue::new();

        // add songs and set index
        for sc in songs {
            queue.add_song(
                create_song_with_overrides(&db, sc, SongChangeSet::default())
                    .await
                    .unwrap()
                    .into(),
            );
        }
        queue.set_current_index(current_index_before);

        // remove specified song
        queue.remove_song(index_to_remove);

        // assert current index is as expected
        assert_eq!(queue.current_index(), expected_current_index_after);
    }

    #[rstest]
    #[case::one_song(arb_vec_and_index( &arb_song_case(), 1..=1, IndexMode::InBounds)())]
    #[case::many_songs(arb_vec_and_index( &arb_song_case(), 2..=10, IndexMode::InBounds)())]
    #[case::many_songs_guaranteed_nonzero_index((arb_vec( &arb_song_case(), 2..=10)(), 1))]
    #[tokio::test]
    async fn test_shuffle(#[case] params: (Vec<SongCase>, usize)) {
        init();
        let (songs, index) = params;
        let db = init_test_database().await.unwrap();
        let mut queue = Queue::default();

        // add songs to queue and set index
        for sc in songs {
            queue.add_song(
                create_song_with_overrides(&db, sc, SongChangeSet::default())
                    .await
                    .unwrap()
                    .into(),
            );
        }
        queue.set_current_index(index);

        let current_song = queue.current_song().cloned();

        // shuffle queue
        queue.shuffle();

        // assert that the current song doesn't change and that current index is 0
        assert_eq!(queue.current_song().cloned(), current_song);
        assert_eq!(queue.current_index(), Some(0));
    }

    #[tokio::test]
    async fn test_next_previous_basic() -> anyhow::Result<()> {
        init();
        let db = init_test_database().await.unwrap();

        let mut queue = Queue::new();
        let song1: SongBrief =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default())
                .await?
                .into();
        let song2: SongBrief =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default())
                .await?
                .into();
        queue.add_song(song1.clone());
        queue.add_song(song2.clone());
        assert_eq!(queue.next_song(), Some(&song1));
        assert_eq!(queue.next_song(), Some(&song2));
        assert_eq!(queue.previous_song(), Some(&song1));
        assert_eq!(queue.previous_song(), None);

        queue.clear();
        assert_eq!(queue.next_song(), None);
        assert_eq!(queue.previous_song(), None);

        Ok(())
    }

    #[tokio::test]
    async fn test_next_song_with_rp_one() {
        init();
        let db = init_test_database().await.unwrap();

        let mut queue = Queue::new();
        queue.set_repeat_mode(RepeatMode::One);
        let song1: SongBrief =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default())
                .await
                .unwrap()
                .into();
        let song2: SongBrief =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default())
                .await
                .unwrap()
                .into();
        queue.add_song(song1.clone());
        queue.add_song(song2.clone());

        assert_eq!(queue.current_song(), None);
        assert_eq!(queue.next_song(), Some(&song1));
        assert_eq!(queue.current_song(), Some(&song1));
        assert_eq!(queue.next_song(), Some(&song1));
        queue.skip_forward(1);
        assert_eq!(queue.current_song(), Some(&song2));
        assert_eq!(queue.next_song(), Some(&song2));
        queue.skip_forward(1);
        assert_eq!(queue.current_song(), None);
        assert_eq!(queue.next_song(), None);
    }

    #[template]
    #[rstest]
    #[case::more_than_len( arb_vec(&arb_song_case(), 4..=5 )(), 7 )]
    #[case::way_more_than_len( arb_vec(&arb_song_case(), 3..=5 )(), 11 )]
    #[case::skip_len( arb_vec(&arb_song_case(), 5..=5 )(), 5 )]
    #[case::skip_len_twice( arb_vec(&arb_song_case(), 5..=5 )(), 10 )]
    #[case::less_than_len( arb_vec(&arb_song_case(), 4..=5 )(), 3 )]
    #[case::skip_one( arb_vec(&arb_song_case(), 2..=5 )(), 1 )]
    #[timeout(std::time::Duration::from_secs(30))]
    pub fn skip_song_test_template(#[case] songs: Vec<SongCase>, #[case] skip: usize) {}

    #[apply(skip_song_test_template)]
    #[tokio::test]
    async fn test_skip_song_rp_none(songs: Vec<SongCase>, skip: usize) -> anyhow::Result<()> {
        init();
        let db = init_test_database().await.unwrap();

        let mut queue = Queue::new();
        let len = songs.len();
        for sc in songs {
            queue.add_song(
                create_song_with_overrides(&db, sc, SongChangeSet::default())
                    .await?
                    .into(),
            );
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
    async fn test_skip_song_rp_one(songs: Vec<SongCase>, skip: usize) -> anyhow::Result<()> {
        init();
        let db = init_test_database().await.unwrap();

        let mut queue = Queue::new();
        let len = songs.len();
        for sc in songs {
            queue.add_song(
                create_song_with_overrides(&db, sc, SongChangeSet::default())
                    .await?
                    .into(),
            );
        }
        queue.set_repeat_mode(RepeatMode::One);

        queue.skip_forward(skip);

        if skip <= len {
            // if we haven't reached the end of the queue
            assert_eq!(
                queue.current_song(),
                queue.get(skip - 1),
                "len: {len}, skip: {skip}, current_index: {current_index}",
                current_index = queue.current_index.unwrap_or_default()
            );
        } else {
            // if we reached the end of the queue
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
    async fn test_next_song_rp_all(songs: Vec<SongCase>, skip: usize) -> anyhow::Result<()> {
        init();
        let db = init_test_database().await.unwrap();

        let mut queue = Queue::new();
        let len = songs.len();
        for sc in songs {
            queue.add_song(
                create_song_with_overrides(&db, sc, SongChangeSet::default())
                    .await?
                    .into(),
            );
        }
        queue.set_repeat_mode(RepeatMode::All);

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
    #[case(RepeatMode::One)]
    #[case(RepeatMode::All)]
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
        init();
        let db = init_test_database().await?;

        let mut queue = Queue::new();
        let len = songs.len();
        for sc in songs {
            queue.add_song(
                create_song_with_overrides(&db, sc, SongChangeSet::default())
                    .await?
                    .into(),
            );
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
        init();
        let (songs, range, index) = params;
        let len = songs.len();
        let db = init_test_database().await?;

        let mut queue = Queue::new();
        for sc in songs {
            queue.add_song(
                create_song_with_overrides(&db, sc, SongChangeSet::default())
                    .await?
                    .into(),
            );
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
