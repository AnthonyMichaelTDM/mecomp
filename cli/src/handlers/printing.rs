//! Handles displaying the output of various commands in a human and machine readable format.

use std::{fmt::Write, time::Duration};

use mecomp_core::state::StateAudio;
use mecomp_prost::{AlbumBrief, ArtistBrief, Playlist, RecordId, SongBrief};

pub fn audio_state(state: &StateAudio) -> Result<String, std::fmt::Error> {
    let mut output = String::new();

    writeln!(output, "StateAudio:")?;

    // when printing the queue, print the previous 3 songs (index, name, id, and path), the current song, and the next 4 songs
    writeln!(output, "\tQueue: [")?;
    let mut queue_lines: Vec<String> = Vec::new();
    // print the previous 3 songs, if they exist
    let start_index = if state.queue_position.unwrap_or_default() > 3 {
        state.queue_position.unwrap_or_default() - 3
    } else {
        0
    };
    for i in start_index..state.queue_position.unwrap_or_default() {
        let song = &state.queue[i];
        queue_lines.push(format!("\t\t{}: \"{}\" (id: {}),", i, song.title, song.id));
    }
    // print the current song
    if let Some(current_index) = state.queue_position {
        let song = &state.queue[current_index];
        queue_lines.push(format!(
            "\t\t{}: \"{}\" (id: {}), <--- Current Song",
            current_index, song.title, song.id
        ));
    }
    // print the next 4 songs
    for i in state.queue_position.unwrap_or_default() + 1
        ..state
            .queue
            .len()
            .min(state.queue_position.unwrap_or_default() + 5)
    {
        let song = &state.queue[i];
        queue_lines.push(format!("\t\t{}: \"{}\" (id: {}),", i, song.title, song.id));
    }
    writeln!(output, "{}", queue_lines.join("\n"))?;
    writeln!(output, "\t],")?;

    writeln!(output, "\tQueue Position: {:?},", state.queue_position)?;

    if let Some(runtime) = state.runtime {
        writeln!(output, "\tRuntime: {runtime}")?;
    }

    writeln!(output, "\tRepeat Mode: {:?}", state.repeat_mode)?;

    writeln!(output, "\tStatus: {:?}", state.status)?;

    writeln!(output, "\tMuted: {:?}", state.muted)?;

    writeln!(output, "\tVolume: {:.2}%", state.volume * 100.0)?;

    Ok(output)
}

pub fn indexed_song_list(prefix: &str, songs: &[SongBrief]) -> Result<String, std::fmt::Error> {
    let mut output = String::new();

    writeln!(output, "{prefix}:")?;

    for (i, song) in songs.iter().enumerate() {
        writeln!(output, "\t{}:\t{}: \"{}\"", i, song.id, song.title)?;
    }

    Ok(output)
}

pub fn song_list(
    prefix: &str,
    songs: &[SongBrief],
    quiet: bool,
) -> Result<String, std::fmt::Error> {
    let mut output = String::new();

    writeln!(output, "{prefix}:")?;

    if quiet {
        for song in songs {
            writeln!(output, "\t{}", song.id)?;
        }
    } else {
        for song in songs {
            writeln!(
                output,
                "\t{}: \"{}\" (by: {:?}, album: {})",
                song.id, song.title, song.artists, song.album
            )?;
        }
    }

    Ok(output)
}

pub fn album_list(
    prefix: &str,
    albums: &[AlbumBrief],
    quiet: bool,
) -> Result<String, std::fmt::Error> {
    let mut output = String::new();

    writeln!(output, "{prefix}:")?;

    if quiet {
        for album in albums {
            writeln!(output, "\t{}", album.id)?;
        }
    } else {
        for album in albums {
            writeln!(
                output,
                "\t{}: \"{}\" (by: {:?})",
                album.id, album.title, album.artists
            )?;
        }
    }

    Ok(output)
}

pub fn artist_list(
    prefix: &str,
    artists: &[ArtistBrief],
    quiet: bool,
) -> Result<String, std::fmt::Error> {
    let mut output = String::new();

    writeln!(output, "{prefix}:")?;

    if quiet {
        for artist in artists {
            writeln!(output, "\t{}", artist.id)?;
        }
    } else {
        for artist in artists {
            writeln!(output, "\t{}: \"{}\"", artist.id, artist.name)?;
        }
    }

    Ok(output)
}

pub fn playlist_list(prefix: &str, playlists: &[Playlist]) -> Result<String, std::fmt::Error> {
    let mut output = String::new();

    writeln!(output, "{prefix}:")?;

    for playlist in playlists {
        writeln!(
            output,
            "\t{}: \"{}\" ({} songs, {:?})",
            playlist.id,
            playlist.name,
            playlist.song_count,
            Duration::try_from(playlist.runtime).unwrap_or_default()
        )?;
    }

    Ok(output)
}

pub fn dynamic_playlist_list(
    prefix: &str,
    playlists: &[mecomp_prost::DynamicPlaylist],
) -> Result<String, std::fmt::Error> {
    let mut output = String::new();

    writeln!(output, "{prefix}:")?;

    for playlist in playlists {
        writeln!(
            output,
            "\t{}: \"{}\" ({})",
            playlist.id, playlist.name, playlist.query
        )?;
    }

    Ok(output)
}

pub fn collection_list(
    prefix: &str,
    collections: &[mecomp_prost::Collection],
) -> Result<String, std::fmt::Error> {
    let mut output = String::new();

    writeln!(output, "{prefix}:")?;

    for collection in collections {
        writeln!(
            output,
            "\t{}: \"{}\" ({} songs, {:?})",
            collection.id,
            collection.name,
            collection.song_count,
            Duration::try_from(collection.runtime).unwrap_or_default()
        )?;
    }

    Ok(output)
}

pub fn thing_list(things: &[RecordId]) -> Result<String, std::fmt::Error> {
    let mut output = String::new();

    for thing in things {
        writeln!(output, "\t{thing}")?;
    }

    Ok(output)
}
