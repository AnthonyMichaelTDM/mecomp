//! This module contains functions for:
//! - importing/exporting specific playlists from/to .m3u files
//! - importing/exporting all your dynamic playlists from/to .csv files

use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use mecomp_core::errors::BackupError;
use mecomp_storage::db::schemas::{
    dynamic::{
        DynamicPlaylist,
        query::{Compile, Query},
    },
    song::Song,
};

use csv::{Reader, Writer};

/// Validate a file path
///
/// # Arguments
///
/// * `path` - The path to validate
/// * `extension` - The expected file extension
/// * `exists` - Whether the file should exist or not
///   * if true, the file must exist
///   * if false, the file may not exist but will be overwritten if it does
pub(crate) fn validate_file_path(
    path: &Path,
    extension: &str,
    exists: bool,
) -> Result<(), BackupError> {
    if path.is_dir() {
        log::warn!("Path is a directory: {}", path.display());
        Err(BackupError::PathIsDirectory(path.to_path_buf()))
    } else if path.extension().is_none() || path.extension().unwrap() != extension {
        log::warn!(
            "Path has the wrong extension (wanted {extension}): {}",
            path.display()
        );
        Err(BackupError::WrongExtension(
            path.to_path_buf(),
            extension.to_string(),
        ))
    } else if exists && !path.exists() {
        log::warn!("Path does not exist: {}", path.display());
        Err(BackupError::FileNotFound(path.to_path_buf()))
    } else {
        Ok(())
    }
}

/// Exports the given dynamic playlists with the given `csv::Writer`
pub(crate) fn export_dynamic_playlists<W: std::io::Write>(
    dynamic_playlists: &[DynamicPlaylist],
    mut writer: Writer<W>,
) -> Result<(), BackupError> {
    writer.write_record(["dynamic playlist name", "query"])?;
    for dp in dynamic_playlists {
        writer.write_record(&[dp.name.clone(), dp.query.compile_for_storage()])?;
    }
    writer.flush()?;

    Ok(())
}

/// Import dynamic playlists from the given writer
///
/// Does not actually write the `DynamicPlaylist`s to the database
pub(crate) fn import_dynamic_playlists<R: std::io::Read>(
    mut reader: Reader<R>,
) -> Result<Vec<DynamicPlaylist>, BackupError> {
    let mut dynamic_playlists = Vec::new();
    for (i, result) in reader.records().enumerate() {
        let record = result?;
        if record.len() != 2 {
            return Err(BackupError::InvalidDynamicPlaylistFormat);
        }
        let name = record[0].to_string();
        let query = record[1].to_string();
        let query = Query::from_str(&query)
            .map_err(|e| BackupError::InvalidDynamicPlaylistQuery(e.to_string(), i + 1))?;
        dynamic_playlists.push(DynamicPlaylist {
            name,
            query,
            id: DynamicPlaylist::generate_id(),
        });
    }
    Ok(dynamic_playlists)
}

/// Export the given playlist (name and songs) to the given buffer as a .m3u file
pub(crate) fn export_playlist<W: std::io::Write>(
    playlist_name: &str,
    songs: &[Song],
    mut writer: W,
) -> Result<(), BackupError> {
    writeln!(writer, "#EXTM3U\n")?;
    writeln!(writer, "#PLAYLIST:{playlist_name}\n")?;
    for song in songs {
        writeln!(
            writer,
            "#EXTINF:{},{} - {}",
            song.runtime.as_secs(),
            song.title,
            song.artist.as_slice().join("; "),
        )?;
        if !song.genre.is_none() {
            writeln!(writer, "#EXTGENRE:{}", song.genre.as_slice().join("; "))?;
        }
        if !song.album_artist.is_none() {
            writeln!(
                writer,
                "#EXTALB:{}",
                song.album_artist.as_slice().join("; ")
            )?;
        }
        writeln!(writer, "{}\n", song.path.display())?;
    }
    Ok(())
}

/// Import a playlist from the given reader
///
/// Returns the playlist name a list of paths to the songs in the playlist
pub(crate) fn import_playlist<R: std::io::Read>(
    mut reader: R,
) -> Result<(Option<String>, Vec<PathBuf>), BackupError> {
    let mut playlist_name = None;
    let mut songs = Vec::new();
    let mut buffer = String::new();
    reader.read_to_string(&mut buffer)?;
    for (i, record) in buffer.lines().enumerate() {
        if let Some(name) = record.strip_prefix("#PLAYLIST:") {
            if name.is_empty() || playlist_name.is_some() {
                // Playlist name is empty or already set
                return Err(BackupError::PlaylistNameInvalidOrAlreadySet(i + 1));
            }
            playlist_name = Some(name.to_string());
            continue;
        }
        if record.is_empty() || record.starts_with('#') {
            continue;
        }

        songs.push(PathBuf::from(record));
    }
    Ok((playlist_name, songs))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use mecomp_storage::db::schemas::dynamic::query::Query;
    use one_or_many::OneOrMany;

    use pretty_assertions::{assert_eq, assert_str_eq};
    use rstest::rstest;

    #[test]
    fn test_export_import() {
        let dynamic_playlists = vec![
            DynamicPlaylist {
                name: "test".into(),
                query: Query::from_str("title = \"a song\"").unwrap(),
                id: DynamicPlaylist::generate_id(),
            },
            DynamicPlaylist {
                name: "test2".into(),
                query: Query::from_str("artist CONTAINS \"an artist\"").unwrap(),
                id: DynamicPlaylist::generate_id(),
            },
        ];

        let mut buffer = Vec::new();
        let writer = Writer::from_writer(&mut buffer);
        export_dynamic_playlists(&dynamic_playlists, writer).unwrap();

        let reader = Reader::from_reader(buffer.as_slice());
        let imported_dynamic_playlists = import_dynamic_playlists(reader).unwrap();

        assert_eq!(imported_dynamic_playlists.len(), 2);
        assert_eq!(imported_dynamic_playlists[0].name, "test");
        assert_eq!(
            imported_dynamic_playlists[0].query.compile_for_storage(),
            "title = \"a song\""
        );
        assert_eq!(imported_dynamic_playlists[1].name, "test2");
        assert_eq!(
            imported_dynamic_playlists[1].query.compile_for_storage(),
            "artist CONTAINS \"an artist\""
        );
    }

    #[test]
    fn test_import_invalid() {
        let buffer = r#"dynamic playlist name,query
valid,title = "test"
invalid"#
            .as_bytes()
            .to_vec();

        let reader = Reader::from_reader(buffer.as_slice());
        let result = import_dynamic_playlists(reader);
        assert!(result.is_err());
        assert_str_eq!(
            result.unwrap_err().to_string(),
            "CSV error: CSV error: record 2 (line: 3, byte: 49): found record with 1 fields, but the previous record has 2 fields"
        );
    }

    #[test]
    fn test_import_invalid_query() {
        let buffer = r#"dynamic playlist name,query
valid,title = "test"
invalid,invalid query
"#
        .as_bytes()
        .to_vec();

        let reader = Reader::from_reader(buffer.as_slice());
        let result = import_dynamic_playlists(reader);
        assert!(result.is_err());
        assert_str_eq!(
            result.unwrap_err().to_string(),
            "Error parsing dynamic playlist query in record 2: failed to parse field at 0, (inner: Mismatch at 0: seq [114, 101, 108, 101, 97, 115, 101, 95, 121, 101, 97, 114] expect: 114, found: 105)"
        );
    }

    #[test]
    fn test_export_playlist() {
        let songs = vec![
            Song {
                id: Song::generate_id(),
                title: "A Song".into(),
                artist: OneOrMany::Many(vec!["Artist1".into(), "Artist2".into()]),
                album_artist: "Album Artist".to_string().into(),
                album: "Album1".into(),
                genre: OneOrMany::Many(vec!["Genre1".into(), "Genre2".into()]),
                runtime: Duration::from_secs(10),
                track: None,
                disc: None,
                release_year: None,
                extension: "mp3".into(),
                path: PathBuf::from("foo/bar.mp3"),
            },
            Song {
                id: Song::generate_id(),
                title: "B Song".into(),
                artist: "Artist1".to_string().into(),
                album_artist: "Album Artist".to_string().into(),
                album: "Album2".into(),
                genre: "Genre1".to_string().into(),
                runtime: Duration::from_secs(20),
                track: None,
                disc: None,
                release_year: None,
                extension: "mp3".into(),
                path: PathBuf::from("foo/bar2.mp3"),
            },
            Song {
                id: Song::generate_id(),
                title: "C Song".into(),
                artist: "Artist1".to_string().into(),
                album_artist: "Album Artist".to_string().into(),
                album: "Album3".into(),
                genre: "Genre1".to_string().into(),
                runtime: Duration::from_secs(30),
                track: None,
                disc: None,
                release_year: None,
                extension: "mp3".into(),
                path: PathBuf::from("foo/bar3.mp3"),
            },
        ];

        let mut buffer = Vec::new();
        export_playlist("Test Playlist", &songs, &mut buffer).unwrap();
        let result = String::from_utf8(buffer).unwrap();
        let expected = r"#EXTM3U

#PLAYLIST:Test Playlist

#EXTINF:10,A Song - Artist1; Artist2
#EXTGENRE:Genre1; Genre2
#EXTALB:Album Artist
foo/bar.mp3

#EXTINF:20,B Song - Artist1
#EXTGENRE:Genre1
#EXTALB:Album Artist
foo/bar2.mp3

#EXTINF:30,C Song - Artist1
#EXTGENRE:Genre1
#EXTALB:Album Artist
foo/bar3.mp3

";
        assert_str_eq!(result, expected);

        let (playlist_name, songs) = import_playlist(result.as_bytes()).unwrap();
        assert_eq!(playlist_name, Some("Test Playlist".to_string()));
        assert_eq!(songs.len(), 3);
        assert_eq!(songs[0], PathBuf::from("foo/bar.mp3"));
        assert_eq!(songs[1], PathBuf::from("foo/bar2.mp3"));
        assert_eq!(songs[2], PathBuf::from("foo/bar3.mp3"));
    }

    #[rstest]
    #[case(
        Some("Test Playlist".to_string()),
        r"#EXTM3U
#PLAYLIST:Test Playlist
#EXTINF:10,A Song - Artist1; Artist2
#EXTGENRE:Genre1; Genre2
#EXTALB:Album Artist
foo/bar.mp3
#EXTINF:20,B Song - Artist1
#EXTGENRE:Genre1
#EXTALB:Album Artist
foo/bar2.mp3
#EXTINF:30,C Song - Artist1
#EXTGENRE:Genre1
#EXTALB:Album Artist
foo/bar3.mp3
"
    )]
    #[case::no_name(
        None,
        r"#EXTM3U
#EXTINF:10,A Song - Artist1; Artist2
#EXTGENRE:Genre1; Genre2
#EXTALB:Album Artist
foo/bar.mp3
#EXTINF:20,B Song - Artist1
#EXTGENRE:Genre1
#EXTALB:Album Artist
foo/bar2.mp3
#EXTINF:30,C Song - Artist1
#EXTGENRE:Genre1
#EXTALB:Album Artist
foo/bar3.mp3
"
    )]
    #[case::no_metadata(
        Some("Test Playlist".to_string()),
        r"#EXTM3U
#PLAYLIST:Test Playlist
foo/bar.mp3
foo/bar2.mp3
foo/bar3.mp3
"
    )]
    #[case::no_name_no_metadata(
        None,
        r"#EXTM3U
foo/bar.mp3
foo/bar2.mp3
foo/bar3.mp3
"
    )]
    fn test_import_playlist(#[case] expected_name: Option<String>, #[case] playlist: &str) {
        let (playlist_name, songs) = import_playlist(playlist.as_bytes()).unwrap();
        assert_eq!(playlist_name, expected_name);
        assert_eq!(songs.len(), 3);
        assert_eq!(songs[0], PathBuf::from("foo/bar.mp3"));
        assert_eq!(songs[1], PathBuf::from("foo/bar2.mp3"));
        assert_eq!(songs[2], PathBuf::from("foo/bar3.mp3"));
    }
}
