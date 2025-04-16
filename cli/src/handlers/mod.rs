pub mod implementations;
pub mod printing;
pub mod utils;

#[cfg(test)]
mod smoke_tests;

use std::str::FromStr;

use clap::{Subcommand, ValueEnum};
use mecomp_storage::db::schemas::dynamic::query::Query;

pub trait CommandHandler {
    type Output;

    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send>(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
        stdout: &mut W1,
        stderr: &mut W2,
    ) -> Self::Output;
}

#[derive(Debug, Subcommand, PartialEq)]
pub enum Command {
    /// Ping the daemon
    Ping,
    /// Stop the daemon
    #[clap(alias = "exit")]
    Stop,
    /// Library commands
    Library {
        #[clap(subcommand)]
        command: LibraryCommand,
    },
    /// Status commands (get the status of a running rescan, analysis, or reclustering)
    Status {
        #[clap(subcommand)]
        command: StatusCommand,
    },
    /// State commands
    State,
    /// Current (audio state)
    Current {
        /// Get the current <TARGET> from the library
        #[clap(value_enum)]
        target: CurrentTarget,
    },
    /// Rand (audio state)
    Rand {
        /// Get a random <TARGET> from the library
        #[clap(value_enum)]
        target: RandTarget,
    },
    /// Search (fuzzy keys)
    Search {
        /// Show only the ids of the items
        #[clap(long, short, action = clap::ArgAction::SetTrue)]
        quiet: bool,

        #[clap(value_enum)]
        /// Search for <TARGET>s in the library matching the query
        target: SearchTarget,

        /// The search query
        #[clap(value_hint = clap::ValueHint::Other)]
        query: String,

        /// The maximum number of results to return
        #[clap(default_value = "10", value_hint = clap::ValueHint::Other)]
        limit: u32,
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
    /// Dynamic playlist control
    Dynamic {
        #[clap(subcommand)]
        command: DynamicCommand,
    },
    /// Collection control
    Collection {
        #[clap(subcommand)]
        command: CollectionCommand,
    },
    /// Radio control
    Radio {
        #[clap(subcommand)]
        command: RadioCommand,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum LibraryCommand {
    /// Rescan the library
    Rescan,
    /// Analyze the library
    Analyze {
        /// Whether to overwrite existing analysis
        #[clap(long)]
        overwrite: bool,
    },
    /// Recluster the library
    Recluster,
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
        #[clap(value_enum)]
        target: LibraryListTarget,
    },
    /// Get a db item by its id
    Get {
        /// What to get (artist, album, song, playlist)
        #[clap(value_enum)]
        target: LibraryGetTarget,
        /// The id of the item
        #[clap(value_hint = clap::ValueHint::Other)]
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
    All,
    Artist,
    Album,
    Song,
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum StatusCommand {
    /// Get the status of a rescan
    Rescan,
    /// Get the status of an analysis
    Analyze,
    /// Get the status of a recluster
    Recluster,
}

#[derive(Debug, Subcommand, PartialEq)]
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
        #[clap(value_enum)]
        mode: RepeatMode,
    },
    /// Shuffle the queue
    Shuffle,
}

#[derive(Debug, Subcommand, PartialEq)]
pub enum SeekCommand {
    /// Seek forwards by a given amount (in seconds)
    #[clap(alias = "f", visible_alias = "+", alias = "ahead")]
    Forward {
        /// The amount to seek by, in seconds
        #[clap(default_value = "5.0", value_hint = clap::ValueHint::Other)]
        amount: f32,
    },
    /// Seek backwards by a given amount
    #[clap(alias = "b", visible_alias = "-", alias = "back")]
    Backward {
        /// The amount to seek by, in seconds
        #[clap(default_value = "5.0", value_hint = clap::ValueHint::Other)]
        amount: f32,
    },
    /// Seek to a given position
    #[clap(alias = "a", visible_alias = "=", alias = "to")]
    Absolute {
        /// The position to seek to, in seconds
        #[clap(value_hint = clap::ValueHint::Other)]
        position: f32,
    },
}

fn float_value_parser(s: &str) -> Result<f32, String> {
    let volume = s.parse::<f32>().map_err(|_| "Invalid volume".to_string())?;
    if !(0.0..=100.0).contains(&volume) {
        return Err("Volume must be between 0 and 100".to_string());
    }
    Ok(volume)
}

#[derive(Debug, Subcommand, PartialEq)]
pub enum VolumeCommand {
    /// Set the volume
    #[clap(visible_alias = "=")]
    Set {
        /// The volume to set to (0 is mute, 100 is max)
        #[clap(value_hint = clap::ValueHint::Other, value_parser = float_value_parser)]
        volume: f32,
    },
    /// Increase the volume
    #[clap(alias = "up", visible_alias = "+")]
    Increase {
        /// The amount to increase the volume by (0-100)
        #[clap(value_hint = clap::ValueHint::Other, value_parser = float_value_parser)]
        amount: f32,
    },
    /// Decrease the volume
    #[clap(alias = "down", visible_alias = "-")]
    Decrease {
        /// The amount to decrease the volume by (0-100)
        #[clap(value_hint = clap::ValueHint::Other, value_parser = float_value_parser)]
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
    One,
    All,
}

impl From<RepeatMode> for mecomp_core::state::RepeatMode {
    fn from(mode: RepeatMode) -> Self {
        match mode {
            RepeatMode::None => Self::None,
            RepeatMode::One => Self::One,
            RepeatMode::All => Self::All,
        }
    }
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum QueueCommand {
    /// Clear the queue
    Clear,
    /// List the queue
    List,
    /// Add to the queue
    Add {
        /// What to add (artist, album, song, playlist)
        #[clap(value_enum)]
        target: QueueAddTarget,
        /// The id of the item
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
    },
    /// Remove a range of items from the queue
    Remove {
        /// The start index of the range to remove
        #[clap(value_hint = clap::ValueHint::Other)]
        start: usize,
        /// The end index of the range to remove
        #[clap(value_hint = clap::ValueHint::Other)]
        end: usize,
    },
    /// set the current song to the given index
    Set {
        /// The index to set the current song to
        #[clap(value_hint = clap::ValueHint::Other)]
        index: usize,
    },
    /// Add a list of items to the queue (from a pipe)
    ///
    /// ex:
    /// ```sh, ignore
    /// mecomp-cli search all "the beatles" -q | mecomp-cli queue pipe
    /// ```
    /// This will add all the results of the search to the queue
    Pipe,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, ValueEnum)]
pub enum QueueAddTarget {
    Artist,
    Album,
    Song,
    Playlist,
    Collection,
    Dynamic,
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum PlaylistCommand {
    /// List playlists
    List,
    /// Get a playlist by its id or name
    Get {
        /// What to get by (id, name)
        #[clap(value_enum)]
        method: PlaylistGetMethod,
        /// The id or name of the playlist
        #[clap(value_hint = clap::ValueHint::Other)]
        target: String,
    },
    /// Create a playlist
    Create {
        /// The name of the playlist
        #[clap(value_hint = clap::ValueHint::Other)]
        name: String,
    },
    /// Rename a playlist
    Update {
        /// The id of the playlist
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
        /// The new name of the playlist
        #[clap(short, long, value_hint = clap::ValueHint::Other)]
        name: String,
    },
    /// Get the songs in a playlist
    Songs {
        /// The id of the playlist
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
    },
    /// Delete a playlist
    Delete {
        /// The id of the playlist
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
    },
    /// Add to a playlist
    Add {
        #[clap(subcommand)]
        command: PlaylistAddCommand,
    },
    /// Remove from a playlist
    Remove {
        /// The id of the playlist
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
        /// The id of the songs(s) to remove
        #[clap(value_hint = clap::ValueHint::Other)]
        item_ids: Vec<String>,
    },
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, ValueEnum)]
pub enum PlaylistGetMethod {
    Id,
    Name,
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum PlaylistAddCommand {
    /// Add an artist to a playlist
    Artist {
        /// The id of the playlist
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
        /// The id of the artist
        #[clap(value_hint = clap::ValueHint::Other)]
        artist_id: String,
    },
    /// Add an album to a playlist
    Album {
        /// The id of the playlist
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
        /// The id of the album
        #[clap(value_hint = clap::ValueHint::Other)]
        album_id: String,
    },
    /// Add a song to a playlist
    Song {
        /// The id of the playlist
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
        /// The ids of the song(s) to add
        #[clap(value_hint = clap::ValueHint::Other)]
        song_ids: Vec<String>,
    },
    /// Add a list of items to the playlist (from a pipe)
    ///
    /// ex:
    /// ```sh, ignore
    /// mecomp-cli search all "the beatles" -q | mecomp-cli playlist add pipe
    /// ```
    /// This will add all the results of the search to the playlist
    Pipe {
        /// The id of the playlist
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum DynamicCommand {
    /// List dynamic playlists
    List,
    /// Get a dynamic playlist by its id
    Get {
        /// The id of the dynamic playlist
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
    },
    /// Get the songs in a dynamic playlist
    Songs {
        /// The id of the dynamic playlist
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
    },
    /// Create a dynamic playlist
    Create {
        /// The name of the dynamic playlist
        #[clap(value_hint = clap::ValueHint::Other)]
        name: String,
        /// The query to use to generate the playlist
        #[clap(value_parser = Query::from_str)]
        query: Query,
    },
    /// Delete a dynamic playlist
    Delete {
        /// The id of the dynamic playlist
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
    },
    /// Update a dynamic playlist
    Update {
        /// The id of the dynamic playlist
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,

        #[clap(flatten)]
        update: DynamicUpdate,
    },
    /// Get the BNF Grammar for queries
    ShowBNF,
}

#[derive(Debug, clap::Args, PartialEq, Eq)]
#[group(required = true)]
pub struct DynamicUpdate {
    /// The new name of the dynamic playlist
    /// (if None, the name will not be updated)
    #[clap(short, long, value_hint = clap::ValueHint::Other)]
    pub name: Option<String>,
    /// The new query of the dynamic playlist
    /// (if None, the query will not be updated)
    #[clap(short, long, value_parser = Query::from_str, value_hint = clap::ValueHint::Other)]
    pub query: Option<Query>,
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum CollectionCommand {
    /// List collections
    List,
    /// Get a collection by its id
    Get {
        /// The id of the collection
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
    },
    /// Get the songs in a collection
    Songs {
        /// The id of the collection
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
    },
    /// Recluster collections
    Recluster,
    /// Freeze a collection
    Freeze {
        /// The id of the collection
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
        /// The new name of the collection
        #[clap(value_hint = clap::ValueHint::Other)]
        name: String,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum RadioCommand {
    /// get the 'n' most similar songs to the given song
    Song {
        /// The id of the song
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
        /// The number of songs to get
        #[clap(value_hint = clap::ValueHint::Other)]
        n: u32,
    },
    /// get the 'n' most similar songs to the given artist
    Artist {
        /// The id of the artist
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
        /// The number of songs to get
        #[clap(value_hint = clap::ValueHint::Other)]
        n: u32,
    },
    /// get the 'n' most similar songs to the given album
    Album {
        /// The id of the album
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
        /// The number of songs to get
        #[clap(value_hint = clap::ValueHint::Other)]
        n: u32,
    },
    /// get the 'n' most similar songs to the given playlist
    Playlist {
        /// The id of the playlist
        #[clap(value_hint = clap::ValueHint::Other)]
        id: String,
        /// The number of songs to get
        #[clap(value_hint = clap::ValueHint::Other)]
        n: u32,
    },
    /// Add a list of items to the radio (from a pipe)
    ///
    /// ex:
    /// ```sh, ignore
    /// mecomp-cli search all "the beatles" -q | mecomp-cli radio pipe
    /// ```
    /// This will add all the results of the search to the radio
    Pipe {
        /// The number of songs to get
        #[clap(value_hint = clap::ValueHint::Other)]
        n: u32,
    },
}
