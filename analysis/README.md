# MECOMP Analysis

The `mecomp-analysis` crate contains implementations of the audio-analysis algorithms used by MECOMP to create song recommendations, cluster related songs, find similar songs, etc. etc.

It is *heavily* influenced by the [bliss-rs](https://github.com/Polochon-street/bliss-rs/tree/49cebf46cc5f974319f355bceb26861c6e24877a) project, and a lot of the code is taken from there with modifications.

The reason we don't just use bliss-rs is because it does a lot of extra stuff that we don't care about, all we want is the audio features so that's all that's been ported.
