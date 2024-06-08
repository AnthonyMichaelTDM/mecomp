pub mod implementations;
pub mod printing;

use clap::{Subcommand, ValueEnum};

pub trait CommandHandler {
    type Output;

    async fn handle(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
    ) -> Self::Output;
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Ping the daemon
    Ping,
    /// Stop the daemon
    Stop,
    /// Library commands
    Library {
        #[clap(subcommand)]
        command: LibraryCommand,
    },
    /// State commands
    State,
    /// Current (audio state)
    Current { target: CurrentTarget },
    /// Rand (audio state)
    Rand { target: RandTarget },
    /// Search (fuzzy keys)
    /// TODO: not implemented
    #[clap(hide = true)]
    Search {
        /// What we're searching for
        target: Option<SearchTarget>,

        /// The search query
        query: String,
    },
    /// Playback control
    Playback {
        #[clap(subcommand)]
        command: PlaybackCommand,
    },
    /// Queue control
    Queue {
        #[clap(subcommand)]
        command: QueueCommand,
    },
    /// Playlist control
    Playlist {
        #[clap(subcommand)]
        command: PlaylistCommand,
    },
    /// Collection control
    /// TODO: clustering not implemented yet
    #[clap(hide = true)]
    Collection {
        #[clap(subcommand)]
        command: CollectionCommand,
    },
    /// Radio control
    /// TODO: not implemented
    #[clap(hide = true)]
    Radio {
        #[clap(subcommand)]
        command: RadioCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum LibraryCommand {
    /// Rescan the library
    Rescan,
    /// Get brief library info
    Brief,
    /// Get detailed library info
    Full,
    /// Get library health info
    Health,
    /// List of stuff in the library
    List {
        /// List detailed info
        #[clap(long)]
        full: bool,
        /// What to list (artists, albums, songs)
        target: LibraryListTarget,
    },
    /// Get a db item by its id
    Get {
        /// What to get (artist, album, song, playlist)
        target: LibraryGetTarget,
        /// The id of the item
        id: String,
    },
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, ValueEnum)]
pub enum LibraryListTarget {
    Artists,
    Albums,
    Songs,
}
#[derive(Debug, PartialEq, Eq, Clone, Copy, ValueEnum)]
pub enum LibraryGetTarget {
    Artist,
    Album,
    Song,
    Playlist,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, ValueEnum)]
pub enum CurrentTarget {
    Artist,
    Album,
    Song,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, ValueEnum)]
pub enum RandTarget {
    Artist,
    Album,
    Song,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, ValueEnum)]
pub enum SearchTarget {
    Artist,
    Album,
    Song,
}

#[derive(Debug, Subcommand)]
pub enum PlaybackCommand {
    /// Toggle play/pause
    Toggle,
    /// Play
    Play,
    /// Pause
    Pause,
    /// Stop
    Stop,
    /// Restart
    Restart,
    /// Next
    Next,
    /// Previous
    Previous,
    /// Seek
    Seek {
        #[clap(subcommand)]
        command: SeekCommand,
    },
    /// Set volume
    Volume {
        #[clap(subcommand)]
        command: VolumeCommand,
    },
    /// Set repeat mode
    Repeat {
        /// The repeat mode to set to (none, once, continuous)
        mode: RepeatMode,
    },
    /// Shuffle the queue
    Shuffle,
}

#[derive(Debug, Subcommand)]
pub enum SeekCommand {
    /// Seek forwards by a given amount (in seconds)
    Forward {
        /// The amount to seek by
        amount: u64,
    },
    /// Seek backwards by a given amount
    Backward {
        /// The amount to seek by
        amount: u64,
    },
    /// Seek to a given position
    To {
        /// The position to seek to
        position: u64,
    },
}

#[derive(Debug, Subcommand)]
pub enum VolumeCommand {
    /// Set the volume
    Set {
        /// The volume to set to (0 is mute, 100 is max)
        volume: f32,
    },
    /// Increase the volume
    #[clap(alias = "up")]
    Increase {
        /// The amount to increase the volume by (0-100)
        amount: f32,
    },
    /// Decrease the volume
    #[clap(alias = "down")]
    Decrease {
        /// The amount to decrease the volume by (0-100)
        amount: f32,
    },
    /// Mute the volume
    Mute,
    /// Unmute the volume
    Unmute,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, ValueEnum)]
pub enum RepeatMode {
    None,
    Once,
    Continuous,
}

impl From<RepeatMode> for mecomp_core::state::RepeatMode {
    fn from(mode: RepeatMode) -> Self {
        match mode {
            RepeatMode::None => Self::None,
            RepeatMode::Once => Self::Once,
            RepeatMode::Continuous => Self::Continuous,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum QueueCommand {
    /// Clear the queue
    Clear,
    /// List the queue
    List,
    /// Add to the queue
    Add {
        /// What to add (artist, album, song, playlist)
        target: QueueAddTarget,
        /// The id of the item
        id: String,
    },
    /// Remove a range of items from the queue
    Remove {
        /// The start index of the range to remove
        start: usize,
        /// The end index of the range to remove
        end: usize,
    },
    /// set the current song to the given index
    Set {
        /// The index to set the current song to
        index: usize,
    },
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, ValueEnum)]
pub enum QueueAddTarget {
    Artist,
    Album,
    Song,
    Playlist,
    Collection,
}

#[derive(Debug, Subcommand)]
pub enum PlaylistCommand {
    /// List playlists
    List,
    /// Get a playlist by its id or name
    Get {
        /// What to get by (id, name)
        method: PlaylistGetMethod,
        /// The id or name of the playlist
        target: String,
    },
    /// Create a playlist
    Create {
        /// The name of the playlist
        name: String,
    },
    /// Delete a playlist
    Delete {
        /// The id of the playlist
        id: String,
    },
    /// Add to a playlist
    Add {
        /// The id of the playlist
        id: String,
        /// What to add (artist, album, song)
        target: PlaylistAddTarget,
        /// The id of the item
        item_id: String,
    },
    /// Remove from a playlist
    Remove {
        /// The id of the playlist
        id: String,
        /// The id of the songs(s) to remove
        item_ids: Vec<String>,
    },
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, ValueEnum)]
pub enum PlaylistGetMethod {
    Id,
    Name,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, ValueEnum)]
pub enum PlaylistAddTarget {
    Artist,
    Album,
    Song,
}

#[derive(Debug, Subcommand)]
pub enum CollectionCommand {
    /// List collections
    List,
    /// Get a collection by its id
    Get {
        /// The id of the collection
        id: String,
    },
    /// Recluster collections
    Recluster,
    /// Freeze a collection
    Freeze {
        /// The id of the collection
        id: String,
        /// The new name of the collection
        name: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum RadioCommand {
    /// get the 'n' most similar songs to the given song
    Songs {
        /// The id of the song
        id: String,
        /// The number of songs to get
        n: usize,
    },
    /// get the 'n' most similar songs to the given artist
    Artists {
        /// The id of the artist
        id: String,
        /// The number of songs to get
        n: usize,
    },
    /// get the 'n' most similar songs to the given album
    Albums {
        /// The id of the album
        id: String,
        /// The number of songs to get
        n: usize,
    },
}