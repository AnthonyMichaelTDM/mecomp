//! This module contains the commands that can be sent to the audio kernel.
#![allow(clippy::module_name_repetitions)]

use std::{fmt::Display, ops::Range, time::Duration};

use mecomp_storage::db::schemas::song::Song;
use one_or_many::OneOrMany;

use crate::{
    format_duration,
    state::{RepeatMode, SeekType, StateAudio},
};

/// Commands that can be sent to the audio kernel
#[derive(Debug)]
pub enum AudioCommand {
    Play,
    Pause,
    TogglePlayback,
    RestartSong,
    /// only clear the player (i.e. stop playback)
    ClearPlayer,
    /// Queue Commands
    Queue(QueueCommand),
    /// Stop the audio kernel
    Exit,
    /// used to report information about the state of the audio kernel
    ReportStatus(tokio::sync::oneshot::Sender<StateAudio>),
    /// volume control commands
    Volume(VolumeCommand),
    /// seek commands
    Seek(SeekType, Duration),
}

impl PartialEq for AudioCommand {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Play, Self::Play)
            | (Self::Pause, Self::Pause)
            | (Self::TogglePlayback, Self::TogglePlayback)
            | (Self::ClearPlayer, Self::ClearPlayer)
            | (Self::RestartSong, Self::RestartSong)
            | (Self::Exit, Self::Exit)
            | (Self::ReportStatus(_), Self::ReportStatus(_)) => true,
            (Self::Queue(a), Self::Queue(b)) => a == b,
            (Self::Volume(a), Self::Volume(b)) => a == b,
            (Self::Seek(a, b), Self::Seek(c, d)) => a == c && b == d,
            #[cfg(not(tarpaulin_include))]
            _ => false,
        }
    }
}

impl Display for AudioCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Play => write!(f, "Play"),
            Self::Pause => write!(f, "Pause"),
            Self::TogglePlayback => write!(f, "Toggle Playback"),
            Self::RestartSong => write!(f, "Restart Song"),
            Self::ClearPlayer => write!(f, "Clear Player"),
            Self::Queue(command) => write!(f, "Queue: {command}"),
            Self::Exit => write!(f, "Exit"),
            Self::ReportStatus(_) => write!(f, "Report Status"),
            Self::Volume(command) => write!(f, "Volume: {command}"),
            Self::Seek(seek_type, duration) => {
                write!(
                    f,
                    "Seek: {seek_type} {} (HH:MM:SS)",
                    format_duration(duration)
                )
            }
        }
    }
}

/// Queue Commands
#[derive(Debug, Clone, PartialEq)]
pub enum QueueCommand {
    SkipForward(usize),
    SkipBackward(usize),
    SetPosition(usize),
    Shuffle,
    AddToQueue(Box<OneOrMany<Song>>),
    RemoveRange(Range<usize>),
    Clear,
    SetRepeatMode(RepeatMode),
}

impl Display for QueueCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SkipForward(n) => write!(f, "Skip Forward by {n}"),
            Self::SkipBackward(n) => write!(f, "Skip Backward by {n}"),
            Self::SetPosition(n) => write!(f, "Set Position to {n}"),
            Self::Shuffle => write!(f, "Shuffle"),
            Self::AddToQueue(song_box) => match &**song_box {
                OneOrMany::None => write!(f, "Add nothing"),
                OneOrMany::One(song) => {
                    write!(f, "Add \"{}\"", song.title)
                }
                OneOrMany::Many(songs) => {
                    write!(
                        f,
                        "Add {:?}",
                        songs
                            .iter()
                            .map(|song| song.title.to_string())
                            .collect::<Vec<_>>()
                    )
                }
            },
            Self::RemoveRange(range) => {
                write!(f, "Remove items {}..{}", range.start, range.end)
            }
            Self::Clear => write!(f, "Clear"),
            Self::SetRepeatMode(mode) => {
                write!(f, "Set Repeat Mode to {mode}")
            }
        }
    }
}

/// Volume commands
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum VolumeCommand {
    Up(f32),
    Down(f32),
    Set(f32),
    Mute,
    Unmute,
    ToggleMute,
}

impl Display for VolumeCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Up(percent) => write!(f, "+{percent:.0}%", percent = percent * 100.0),
            Self::Down(percent) => write!(f, "-{percent:.0}%", percent = percent * 100.0),
            Self::Set(percent) => write!(f, "={percent:.0}%", percent = percent * 100.0),
            Self::Mute => write!(f, "Mute"),
            Self::Unmute => write!(f, "Unmute"),
            Self::ToggleMute => write!(f, "Toggle Mute"),
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_str_eq;
    use rstest::rstest;
    use std::time::Duration;

    use super::*;

    #[rstest]
    #[case(AudioCommand::Play, AudioCommand::Play, true)]
    #[case(AudioCommand::Play, AudioCommand::Pause, false)]
    #[case(AudioCommand::Pause, AudioCommand::Pause, true)]
    #[case(AudioCommand::TogglePlayback, AudioCommand::TogglePlayback, true)]
    #[case(AudioCommand::RestartSong, AudioCommand::RestartSong, true)]
    #[case(
        AudioCommand::Queue(QueueCommand::Clear),
        AudioCommand::Queue(QueueCommand::Clear),
        true
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::Clear),
        AudioCommand::Queue(QueueCommand::Shuffle),
        false
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SkipForward(1)),
        AudioCommand::Queue(QueueCommand::SkipForward(1)),
        true
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SkipForward(1)),
        AudioCommand::Queue(QueueCommand::SkipForward(2)),
        false
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SkipBackward(1)),
        AudioCommand::Queue(QueueCommand::SkipBackward(1)),
        true
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SkipBackward(1)),
        AudioCommand::Queue(QueueCommand::SkipBackward(2)),
        false
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SetPosition(1)),
        AudioCommand::Queue(QueueCommand::SetPosition(1)),
        true
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SetPosition(1)),
        AudioCommand::Queue(QueueCommand::SetPosition(2)),
        false
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::Shuffle),
        AudioCommand::Queue(QueueCommand::Shuffle),
        true
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::Shuffle),
        AudioCommand::Queue(QueueCommand::Clear),
        false
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Up(0.1)),
        AudioCommand::Volume(VolumeCommand::Up(0.1)),
        true
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Up(0.1)),
        AudioCommand::Volume(VolumeCommand::Up(0.2)),
        false
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Down(0.1)),
        AudioCommand::Volume(VolumeCommand::Down(0.1)),
        true
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Down(0.1)),
        AudioCommand::Volume(VolumeCommand::Down(0.2)),
        false
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Set(0.1)),
        AudioCommand::Volume(VolumeCommand::Set(0.1)),
        true
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Set(0.1)),
        AudioCommand::Volume(VolumeCommand::Set(0.2)),
        false
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Mute),
        AudioCommand::Volume(VolumeCommand::Mute),
        true
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Mute),
        AudioCommand::Volume(VolumeCommand::Unmute),
        false
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Unmute),
        AudioCommand::Volume(VolumeCommand::Unmute),
        true
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::Unmute),
        AudioCommand::Volume(VolumeCommand::Mute),
        false
    )]
    #[case(
        AudioCommand::Volume(VolumeCommand::ToggleMute),
        AudioCommand::Volume(VolumeCommand::ToggleMute),
        true
    )]
    #[case(
        AudioCommand::Seek(SeekType::Absolute, Duration::from_secs(10)),
        AudioCommand::Seek(SeekType::Absolute, Duration::from_secs(10)),
        true
    )]
    #[case(
        AudioCommand::Seek(SeekType::Absolute, Duration::from_secs(10)),
        AudioCommand::Seek(SeekType::Absolute, Duration::from_secs(20)),
        false
    )]
    #[case(
        AudioCommand::Seek(SeekType::RelativeForwards, Duration::from_secs(10)),
        AudioCommand::Seek(SeekType::RelativeForwards, Duration::from_secs(10)),
        true
    )]
    #[case(
        AudioCommand::Seek(SeekType::RelativeForwards, Duration::from_secs(10)),
        AudioCommand::Seek(SeekType::RelativeForwards, Duration::from_secs(20)),
        false
    )]
    #[case(
        AudioCommand::Seek(SeekType::RelativeBackwards, Duration::from_secs(10)),
        AudioCommand::Seek(SeekType::RelativeBackwards, Duration::from_secs(10)),
        true
    )]
    #[case(
        AudioCommand::Seek(SeekType::RelativeBackwards, Duration::from_secs(10)),
        AudioCommand::Seek(SeekType::RelativeBackwards, Duration::from_secs(20)),
        false
    )]
    #[case(
        AudioCommand::Seek(SeekType::Absolute, Duration::from_secs(10)),
        AudioCommand::Seek(SeekType::RelativeBackwards, Duration::from_secs(10)),
        false
    )]
    #[case(
        AudioCommand::Seek(SeekType::Absolute, Duration::from_secs(10)),
        AudioCommand::Seek(SeekType::RelativeForwards, Duration::from_secs(10)),
        false
    )]
    #[case(
        AudioCommand::Seek(SeekType::RelativeForwards, Duration::from_secs(10)),
        AudioCommand::Seek(SeekType::RelativeBackwards, Duration::from_secs(10)),
        false
    )]
    fn test_audio_command_equality(
        #[case] lhs: AudioCommand,
        #[case] rhs: AudioCommand,
        #[case] expected: bool,
    ) {
        assert_eq!(lhs == rhs, expected);
        assert_eq!(rhs == lhs, expected);
    }

    // dummy song used for display tests, makes the tests more readable
    fn dummy_song() -> Song {
        Song {
            id: Song::generate_id(),
            title: "Song 1".into(),
            artist: OneOrMany::None,
            album_artist: OneOrMany::None,
            album: "album".into(),
            genre: OneOrMany::None,
            runtime: Duration::from_secs(100),
            track: None,
            disc: None,
            release_year: None,
            extension: "mp3".into(),
            path: "foo/bar.mp3".into(),
            analysis: [0.; 20],
        }
    }

    #[rstest]
    #[case(AudioCommand::Play, "Play")]
    #[case(AudioCommand::Pause, "Pause")]
    #[case(AudioCommand::TogglePlayback, "Toggle Playback")]
    #[case(AudioCommand::ClearPlayer, "Clear Player")]
    #[case(AudioCommand::RestartSong, "Restart Song")]
    #[case(AudioCommand::Queue(QueueCommand::Clear), "Queue: Clear")]
    #[case(AudioCommand::Queue(QueueCommand::Shuffle), "Queue: Shuffle")]
    #[case(
        AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(OneOrMany::None))),
        "Queue: Add nothing"
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(OneOrMany::One(dummy_song())))),
        "Queue: Add \"Song 1\""
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::AddToQueue(Box::new(OneOrMany::Many(vec![dummy_song()])))),
        "Queue: Add [\"Song 1\"]"
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::RemoveRange(0..1)),
        "Queue: Remove items 0..1"
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SetRepeatMode(RepeatMode::None)),
        "Queue: Set Repeat Mode to None"
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SkipForward(1)),
        "Queue: Skip Forward by 1"
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SkipBackward(1)),
        "Queue: Skip Backward by 1"
    )]
    #[case(
        AudioCommand::Queue(QueueCommand::SetPosition(1)),
        "Queue: Set Position to 1"
    )]
    #[case(AudioCommand::Volume(VolumeCommand::Up(0.1)), "Volume: +10%")]
    #[case(AudioCommand::Volume(VolumeCommand::Down(0.1)), "Volume: -10%")]
    #[case(AudioCommand::Volume(VolumeCommand::Set(0.1)), "Volume: =10%")]
    #[case(AudioCommand::Volume(VolumeCommand::Mute), "Volume: Mute")]
    #[case(AudioCommand::Volume(VolumeCommand::Unmute), "Volume: Unmute")]
    #[case(AudioCommand::Volume(VolumeCommand::ToggleMute), "Volume: Toggle Mute")]
    #[case(AudioCommand::Exit, "Exit")]
    #[case(AudioCommand::ReportStatus(tokio::sync::oneshot::channel().0), "Report Status")]
    #[case(
        AudioCommand::Seek(SeekType::Absolute, Duration::from_secs(10)),
        "Seek: Absolute 00:00:10.00 (HH:MM:SS)"
    )]
    #[case(
        AudioCommand::Seek(SeekType::RelativeForwards, Duration::from_secs(10)),
        "Seek: Forwards 00:00:10.00 (HH:MM:SS)"
    )]
    #[case(
        AudioCommand::Seek(SeekType::RelativeBackwards, Duration::from_secs(10)),
        "Seek: Backwards 00:00:10.00 (HH:MM:SS)"
    )]
    #[case(
        AudioCommand::Seek(SeekType::Absolute, Duration::from_secs(3600 + 120 + 1)),
        "Seek: Absolute 01:02:01.00 (HH:MM:SS)"
    )]
    fn test_audio_command_display(#[case] command: AudioCommand, #[case] expected: &str) {
        assert_str_eq!(command.to_string(), expected);
    }
}
