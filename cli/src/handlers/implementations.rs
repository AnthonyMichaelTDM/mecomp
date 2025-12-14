use std::time::Duration;

use crate::handlers::{
    printing,
    utils::{self, StdIn},
};

use super::{
    Command, CommandHandler, CurrentTarget, LibraryCommand, LibraryGetCommand, LibraryListTarget,
    PlaylistGetMethod, QueueCommand, RandTarget, SearchTarget, SeekCommand, VolumeCommand,
};

use anyhow::bail;
use mecomp_core::state::SeekType;
use mecomp_prost::{
    CollectionFreezeRequest, DynamicPlaylist, DynamicPlaylistChangeSet,
    DynamicPlaylistCreateRequest, DynamicPlaylistUpdateRequest, LibraryAnalyzeRequest,
    PlaybackRepeatRequest, PlaybackSeekRequest, PlaybackSkipRequest, PlaybackVolumeAdjustRequest,
    PlaybackVolumeSetRequest, PlaylistAddListRequest, PlaylistExportRequest, PlaylistImportRequest,
    PlaylistName, PlaylistRemoveSongsRequest, PlaylistRenameRequest, QueueRemoveRangeRequest,
    QueueSetIndexRequest, RadioSimilarRequest, RecordId, RecordIdList, SearchRequest, SearchResult,
    Ulid,
};
use mecomp_storage::db::schemas::dynamic::query::Compile;
use tonic::{Code, Response};

impl CommandHandler for Command {
    type Output = anyhow::Result<()>;

    #[allow(clippy::too_many_lines)]
    async fn handle<
        W1: std::fmt::Write + Send,
        W2: std::fmt::Write + Send,
        R: utils::StdIn + Send,
    >(
        &self,
        mut client: mecomp_prost::MusicPlayerClient,
        stdout: &mut W1,
        stderr: &mut W2,
        stdin: &R,
    ) -> Self::Output {
        match self {
            Self::Ping => {
                let resp: String = client.ping(()).await?.into_inner().message;
                writeln!(stdout, "Daemon response:\n{resp}")?;
                Ok(())
            }
            Self::Stop => {
                client.daemon_shutdown(()).await?;
                writeln!(
                    stdout,
                    "Daemon stopping, check the daemon logs for more information"
                )?;
                Ok(())
            }
            Self::Library { command } => command.handle(client, stdout, stderr, stdin).await,
            Self::Status { command } => command.handle(client, stdout, stderr, stdin).await,
            Self::State => {
                if let Some(state) = client.state_audio(()).await?.into_inner().state {
                    Ok(writeln!(
                        stdout,
                        "{}",
                        printing::audio_state(&state.into())?
                    )?)
                } else {
                    Ok(writeln!(
                        stdout,
                        "Daemon response:\nNo audio state available"
                    )?)
                }
            }

            Self::Current {
                target: CurrentTarget::Artist,
            } => Ok(writeln!(
                stdout,
                "Daemon response:\n{:#?}",
                client.current_artists(()).await?.into_inner().artists
            )?),
            Self::Current {
                target: CurrentTarget::Album,
            } => Ok(writeln!(
                stdout,
                "Daemon response:\n{:#?}",
                client.current_album(()).await?.into_inner().album
            )?),
            Self::Current {
                target: CurrentTarget::Song,
            } => Ok(writeln!(
                stdout,
                "Daemon response:\n{:#?}",
                client.current_song(()).await?.into_inner().song
            )?),

            Self::Rand {
                target: RandTarget::Artist,
            } => Ok(writeln!(
                stdout,
                "Daemon response:\n{:#?}",
                client.rand_artist(()).await?.into_inner().artist
            )?),
            Self::Rand {
                target: RandTarget::Album,
            } => Ok(writeln!(
                stdout,
                "Daemon response:\n{:#?}",
                client.rand_album(()).await?.into_inner().album
            )?),
            Self::Rand {
                target: RandTarget::Song,
            } => Ok(writeln!(
                stdout,
                "Daemon response:\n{:#?}",
                client.rand_song(()).await?.into_inner().song
            )?),

            Self::Search {
                quiet,
                target: SearchTarget::All,
                query,
                limit,
            } => {
                let SearchResult {
                    songs,
                    albums,
                    artists,
                } = client
                    .search(SearchRequest::new(query, *limit))
                    .await?
                    .into_inner();
                writeln!(
                    stdout,
                    "Daemon response:\n{}\n{}\n{}",
                    printing::song_list("Songs", songs, *quiet, false)?,
                    printing::album_list("Albums", albums, *quiet, false)?,
                    printing::artist_list("Artists", artists, *quiet, false)?
                )?;
                Ok(())
            }
            Self::Search {
                quiet,
                target: SearchTarget::Artist,
                query,
                limit,
            } => Ok(writeln!(
                stdout,
                "Daemon response:\n{}",
                printing::artist_list(
                    "Artists",
                    client
                        .search_artist(SearchRequest::new(query, *limit))
                        .await?
                        .into_inner()
                        .artists,
                    *quiet,
                    false
                )?
            )?),
            Self::Search {
                quiet,
                target: SearchTarget::Album,
                query,
                limit,
            } => Ok(writeln!(
                stdout,
                "Daemon response:\n{}",
                printing::album_list(
                    "Albums",
                    client
                        .search_album(SearchRequest::new(query, *limit))
                        .await?
                        .into_inner()
                        .albums,
                    *quiet,
                    false
                )?
            )?),
            Self::Search {
                quiet,
                target: SearchTarget::Song,
                query,
                limit,
            } => Ok(writeln!(
                stdout,
                "Daemon response:\n{}",
                printing::song_list(
                    "Songs",
                    client
                        .search_song(SearchRequest::new(query, *limit))
                        .await?
                        .into_inner()
                        .songs,
                    *quiet,
                    false
                )?
            )?),

            Self::Playback { command } => command.handle(client, stdout, stderr, stdin).await,
            Self::Queue { command } => command.handle(client, stdout, stderr, stdin).await,
            Self::Playlist { command } => command.handle(client, stdout, stderr, stdin).await,
            Self::Dynamic { command } => command.handle(client, stdout, stderr, stdin).await,
            Self::Collection { command } => command.handle(client, stdout, stderr, stdin).await,
            Self::Radio(command) => command.handle(client, stdout, stderr, stdin).await,
        }
    }
}

impl CommandHandler for LibraryCommand {
    type Output = anyhow::Result<()>;

    #[allow(clippy::too_many_lines)]
    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send, R: StdIn>(
        &self,
        mut client: mecomp_prost::MusicPlayerClient,
        stdout: &mut W1,
        _: &mut W2,
        _: &R,
    ) -> Self::Output {
        match self {
            Self::Rescan => {
                let resp = client.library_rescan(()).await;
                if let Err(e) = resp {
                    writeln!(stdout, "Daemon response:\n{e}")?;
                } else {
                    writeln!(stdout, "Daemon response:\nLibrary rescan started")?;
                }
                Ok(())
            }
            Self::Analyze { overwrite } => {
                let resp = client
                    .library_analyze(LibraryAnalyzeRequest::new(*overwrite))
                    .await;
                if let Err(e) = resp {
                    writeln!(stdout, "Daemon response:\n{e}")?;
                } else {
                    writeln!(stdout, "Daemon response:\nLibrary analysis started")?;
                }
                Ok(())
            }
            Self::Recluster => {
                let resp = client.library_recluster(()).await;
                if let Err(e) = resp {
                    writeln!(stdout, "Daemon response:\n{e}")?;
                } else {
                    writeln!(stdout, "Daemon response:\nreclustering started")?;
                }
                Ok(())
            }
            Self::Brief => {
                let resp = client.library_brief(()).await?.into_inner();
                writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                Ok(())
            }
            Self::Full => {
                let resp = client.library_full(()).await?.into_inner();
                writeln!(stdout, "Daemon response:\n{resp:?}")?;
                Ok(())
            }
            Self::Health => {
                let resp = client.library_health(()).await?.into_inner();
                writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                Ok(())
            }
            Self::List { quiet, target } => {
                match target {
                    LibraryListTarget::Artists => {
                        let resp = client.library_artists(()).await?.into_inner().artists;
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::artist_list("Artists", resp, *quiet, true)?
                        )?;
                    }
                    LibraryListTarget::Albums => {
                        let resp = client.library_albums(()).await?.into_inner().albums;
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::album_list("Albums", resp, *quiet, true)?
                        )?;
                    }
                    LibraryListTarget::Songs => {
                        let resp = client.library_songs(()).await?.into_inner().songs;
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::song_list("Songs", resp, *quiet, true)?
                        )?;
                    }
                    LibraryListTarget::Playlists => {
                        let resp = client.library_playlists(()).await?.into_inner().playlists;
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::playlist_list("Playlists", resp)?
                        )?;
                    }
                    LibraryListTarget::DynamicPlaylists => {
                        let resp = client
                            .library_dynamic_playlists(())
                            .await?
                            .into_inner()
                            .playlists;
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::dynamic_playlist_list("Dynamic Playlists", resp)?
                        )?;
                    }
                    LibraryListTarget::Collections => {
                        let resp = client
                            .library_collections(())
                            .await?
                            .into_inner()
                            .collections;
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::collection_list("Collections", resp)?
                        )?;
                    }
                }
                Ok(())
            }
            Self::Get { command } => {
                match command {
                    LibraryGetCommand::Artist { id } => {
                        let resp = client
                            .library_artist_get(Ulid::new(id))
                            .await?
                            .into_inner()
                            .artist;
                        writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                    }
                    LibraryGetCommand::Album { id } => {
                        let resp = client
                            .library_album_get(Ulid::new(id))
                            .await?
                            .into_inner()
                            .album;
                        writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                    }
                    LibraryGetCommand::Song { id } => {
                        let resp = client
                            .library_song_get(Ulid::new(id))
                            .await?
                            .into_inner()
                            .song;
                        writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                    }
                    LibraryGetCommand::Playlist { id } => {
                        let resp = client
                            .library_playlist_get(Ulid::new(id))
                            .await?
                            .into_inner()
                            .playlist;
                        writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                    }
                    LibraryGetCommand::Dynamic { id } => {
                        let resp = client
                            .library_dynamic_playlist_get(Ulid::new(id))
                            .await?
                            .into_inner()
                            .playlist;
                        writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                    }
                    LibraryGetCommand::Collection { id } => {
                        let resp = client
                            .library_collection_get(Ulid::new(id))
                            .await?
                            .into_inner()
                            .collection;
                        writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                    }
                }
                Ok(())
            }
        }
    }
}

impl CommandHandler for super::StatusCommand {
    type Output = anyhow::Result<()>;

    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send, R: StdIn>(
        &self,
        mut client: mecomp_prost::MusicPlayerClient,
        stdout: &mut W1,
        _: &mut W2,
        _: &R,
    ) -> Self::Output {
        match self {
            Self::Rescan => {
                if client
                    .library_rescan_in_progress(())
                    .await?
                    .into_inner()
                    .in_progress
                {
                    writeln!(stdout, "Daemon response:\nthere is a rescan in progress")?;
                } else {
                    writeln!(
                        stdout,
                        "Daemon response:\nthere is not a rescan in progress"
                    )?;
                }
            }
            Self::Analyze => {
                if client
                    .library_analyze_in_progress(())
                    .await?
                    .into_inner()
                    .in_progress
                {
                    writeln!(stdout, "Daemon response:\nthere is an analysis in progress")?;
                } else {
                    writeln!(
                        stdout,
                        "Daemon response:\nthere is not an analysis in progress"
                    )?;
                }
            }
            Self::Recluster => {
                if client
                    .library_recluster_in_progress(())
                    .await?
                    .into_inner()
                    .in_progress
                {
                    writeln!(
                        stdout,
                        "Daemon response:\nthere is a reclustering in progress"
                    )?;
                } else {
                    writeln!(
                        stdout,
                        "Daemon response:\nthere is not a reclustering in progress"
                    )?;
                }
            }
        }
        Ok(())
    }
}

impl CommandHandler for super::PlaybackCommand {
    type Output = anyhow::Result<()>;

    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send, R: StdIn>(
        &self,
        mut client: mecomp_prost::MusicPlayerClient,
        stdout: &mut W1,
        stderr: &mut W2,
        stdin: &R,
    ) -> Self::Output {
        match self {
            Self::Toggle => {
                client.playback_toggle(()).await?;
                writeln!(stdout, "Daemon response:\nplayback toggled")?;
                Ok(())
            }
            Self::Play => {
                client.playback_play(()).await?;
                writeln!(stdout, "Daemon response:\nplayback started")?;
                Ok(())
            }
            Self::Pause => {
                client.playback_pause(()).await?;
                writeln!(stdout, "Daemon response:\nplayback paused")?;
                Ok(())
            }
            Self::Stop => {
                client.playback_clear_player(()).await?;
                writeln!(stdout, "Daemon response:\nplayback stopped")?;
                Ok(())
            }
            Self::Restart => {
                client.playback_restart(()).await?;
                writeln!(stdout, "Daemon response:\nplayback restarted")?;
                Ok(())
            }
            Self::Next => {
                client
                    .playback_skip_forward(PlaybackSkipRequest::new(1))
                    .await?;
                writeln!(stdout, "Daemon response:\nnext track started")?;
                Ok(())
            }
            Self::Previous => {
                client
                    .playback_skip_backward(PlaybackSkipRequest::new(1))
                    .await?;
                writeln!(stdout, "Daemon response:\nprevious track started")?;
                Ok(())
            }
            Self::Seek { command } => command.handle(client, stdout, stderr, stdin).await,
            Self::Volume { command } => command.handle(client, stdout, stderr, stdin).await,
            Self::Repeat { mode } => {
                let mode: mecomp_core::state::RepeatMode = (*mode).into();
                client
                    .playback_repeat(PlaybackRepeatRequest::new(mode))
                    .await?;
                writeln!(stdout, "Daemon response:\nrepeat mode set to {mode}")?;
                Ok(())
            }
            Self::Shuffle => {
                client.playback_shuffle(()).await?;
                writeln!(stdout, "Daemon response:\nqueue shuffled")?;
                Ok(())
            }
        }
    }
}

impl CommandHandler for SeekCommand {
    type Output = anyhow::Result<()>;

    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send, R: StdIn>(
        &self,
        mut client: mecomp_prost::MusicPlayerClient,
        stdout: &mut W1,
        _: &mut W2,
        _: &R,
    ) -> Self::Output {
        match self {
            Self::Forward { amount } => {
                client
                    .playback_seek(PlaybackSeekRequest::new(
                        SeekType::RelativeForwards,
                        Duration::from_secs_f32(*amount),
                    ))
                    .await?;
                writeln!(stdout, "Daemon response:\nsought forward by {amount:.2}s")?;
            }
            Self::Backward { amount } => {
                client
                    .playback_seek(PlaybackSeekRequest::new(
                        SeekType::RelativeBackwards,
                        Duration::from_secs_f32(*amount),
                    ))
                    .await?;
                writeln!(stdout, "Daemon response:\nsought backward by {amount:.2}s")?;
            }
            Self::Absolute { position } => {
                client
                    .playback_seek(PlaybackSeekRequest::new(
                        SeekType::Absolute,
                        Duration::from_secs_f32(*position),
                    ))
                    .await?;
                writeln!(
                    stdout,
                    "Daemon response:\nsought to position {position:.2}s"
                )?;
            }
        }
        Ok(())
    }
}

impl CommandHandler for VolumeCommand {
    type Output = anyhow::Result<()>;

    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send, R: StdIn>(
        &self,
        mut client: mecomp_prost::MusicPlayerClient,
        stdout: &mut W1,
        _: &mut W2,
        _: &R,
    ) -> Self::Output {
        match self {
            Self::Set { volume } => {
                client
                    .playback_volume(PlaybackVolumeSetRequest::new(*volume / 100.0))
                    .await?;
                writeln!(stdout, "Daemon response:\nvolume set to {volume}")?;
                Ok(())
            }
            Self::Increase { amount } => {
                client
                    .playback_volume_up(PlaybackVolumeAdjustRequest::new(*amount / 100.0))
                    .await?;
                writeln!(stdout, "Daemon response:\nvolume increased by {amount}")?;
                Ok(())
            }
            Self::Decrease { amount } => {
                client
                    .playback_volume_down(PlaybackVolumeAdjustRequest::new(*amount / 100.0))
                    .await?;
                writeln!(stdout, "Daemon response:\nvolume decreased by {amount}")?;
                Ok(())
            }
            Self::Mute => {
                client.playback_mute(()).await?;
                writeln!(stdout, "Daemon response:\nvolume muted")?;
                Ok(())
            }
            Self::Unmute => {
                client.playback_unmute(()).await?;
                writeln!(stdout, "Daemon response:\nvolume unmuted")?;
                Ok(())
            }
        }
    }
}

impl CommandHandler for QueueCommand {
    type Output = anyhow::Result<()>;

    #[allow(clippy::too_many_lines)]
    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send, R: StdIn>(
        &self,
        mut client: mecomp_prost::MusicPlayerClient,
        stdout: &mut W1,
        stderr: &mut W2,
        stdin: &R,
    ) -> Self::Output {
        match self {
            Self::Clear => {
                client.playback_clear(()).await?;
                writeln!(stdout, "Daemon response:\nqueue cleared")?;
            }
            Self::List { quiet: false } => {
                let resp = client
                    .state_audio(())
                    .await?
                    .into_inner()
                    .state
                    .map(|s| s.queue);
                if let Some(songs) = resp {
                    writeln!(
                        stdout,
                        "Daemon response:\n{}",
                        printing::indexed_song_list("Queue", &songs)?
                    )?;
                } else {
                    writeln!(stdout, "Daemon response:\nNo queue available")?;
                }
            }
            Self::List { quiet: true } => {
                let resp = client
                    .state_audio(())
                    .await?
                    .into_inner()
                    .state
                    .map(|s| s.queue);
                if let Some(songs) = resp {
                    writeln!(
                        stdout,
                        "Daemon response:\n{}",
                        printing::song_list("Queue", songs, true, false)?
                    )?;
                } else {
                    writeln!(stdout, "Daemon response:\nNo queue available")?;
                }
            }
            Self::Add { items } => {
                let Ok(ids) = items
                    .iter()
                    .map(|id| id.parse())
                    .collect::<Result<Vec<RecordId>, _>>()
                else {
                    bail!("One or more provided IDs are invalid");
                };

                // are we in a pipe?
                let list = if stdin.is_terminal() && ids.is_empty() {
                    bail!("No input provided, nothing to add to queue");
                } else if !stdin.is_terminal() {
                    let from_pipe: Vec<RecordId> =
                        utils::parse_from_lines(stdin.lines().filter_map(|l| match l {
                            Ok(line) => Some(line),
                            Err(e) => {
                                writeln!(stderr, "Error reading from stdin: {e}").ok();
                                None
                            }
                        }));
                    let mut all_ids = ids;
                    all_ids.extend(from_pipe);
                    if all_ids.is_empty() {
                        bail!("No input provided, nothing to add to queue");
                    }
                    all_ids
                } else {
                    ids
                };

                client.queue_add_list(RecordIdList { ids: list }).await?;
                writeln!(stdout, "Daemon response:\nitems added to queue")?;
            }
            Self::Remove { start, end } => {
                client
                    .queue_remove_range(QueueRemoveRangeRequest::new(*start, *end))
                    .await?;
                writeln!(stdout, "Daemon response:\nitems removed from queue")?;
            }
            Self::Set { index } => {
                client
                    .queue_set_index(QueueSetIndexRequest::new(*index))
                    .await?;
                writeln!(
                    stdout,
                    "Daemon response:\ncurrent song set to index {index}"
                )?;
            }
        }
        Ok(())
    }
}

impl CommandHandler for super::PlaylistCommand {
    type Output = anyhow::Result<()>;

    #[allow(clippy::too_many_lines)]
    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send, R: StdIn>(
        &self,
        mut client: mecomp_prost::MusicPlayerClient,
        stdout: &mut W1,
        stderr: &mut W2,
        stdin: &R,
    ) -> Self::Output {
        match self {
            Self::List => {
                let resp = client.library_playlists(()).await?.into_inner().playlists;
                writeln!(
                    stdout,
                    "Daemon response:\n{}",
                    printing::playlist_list("Playlists", resp)?
                )?;
                Ok(())
            }
            Self::Get { method, target } => {
                let resp = match method {
                    PlaylistGetMethod::Id => {
                        client
                            .library_playlist_get(Ulid::new(target))
                            .await?
                            .into_inner()
                            .playlist
                    }
                    PlaylistGetMethod::Name => {
                        client
                            .library_playlist_get_by_name(PlaylistName::new(target))
                            .await?
                            .into_inner()
                            .playlist
                    }
                };

                writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                Ok(())
            }
            Self::Create { name } => {
                let resp: RecordId = client
                    .playlist_get_or_create(PlaylistName::new(name))
                    .await?
                    .into_inner();
                writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                Ok(())
            }
            Self::Update { id, name } => {
                match client
                    .playlist_rename(PlaylistRenameRequest::new(id, name))
                    .await
                    .map(Response::into_inner)
                {
                    Ok(playlist) => {
                        writeln!(
                            stdout,
                            "Daemon response:\nplaylist renamed to \"{}\"",
                            playlist.name
                        )?;
                    }
                    Err(e) if e.code() == Code::NotFound => {
                        writeln!(stdout, "Daemon response:\nplaylist not found")?;
                    }
                    Err(e) => bail!(e),
                }
                Ok(())
            }
            Self::Songs { id } => {
                match client
                    .library_playlist_get_songs(Ulid::new(id))
                    .await
                    .map(|r| r.into_inner().songs)
                {
                    Ok(songs) => {
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::song_list("Songs", songs, false, true)?
                        )?;
                    }
                    Err(e) if e.code() == Code::NotFound => {
                        writeln!(stdout, "Daemon response:\nplaylist not found")?;
                    }
                    Err(e) => bail!(e),
                }
                Ok(())
            }

            Self::Delete { id } => {
                client.playlist_remove(Ulid::new(id)).await?;
                writeln!(stdout, "Daemon response:\nplaylist deleted")?;
                Ok(())
            }
            Self::Add(command) => command.handle(client, stdout, stderr, stdin).await,
            Self::Remove { id, item_ids } => {
                client
                    .playlist_remove_songs(PlaylistRemoveSongsRequest {
                        playlist_id: Ulid::new(id),
                        song_ids: item_ids.iter().map(Ulid::new).collect(),
                    })
                    .await?;
                writeln!(stdout, "Daemon response:\nsongs removed from playlist")?;

                Ok(())
            }
            Self::Export { id, path } => {
                client
                    .playlist_export(PlaylistExportRequest {
                        playlist_id: Ulid::new(id),
                        path: format!("{}", path.display()),
                    })
                    .await?;
                writeln!(
                    stdout,
                    "Daemon response:\nplaylist exported to {}",
                    path.display()
                )?;
                Ok(())
            }
            Self::Import { path, name } => {
                let resp: RecordId = client
                    .playlist_import(PlaylistImportRequest {
                        path: format!("{}", path.display()),
                        name: name.clone(),
                    })
                    .await?
                    .into_inner();
                writeln!(
                    stdout,
                    "Daemon response:\nplaylist imported from {}\n\t{}",
                    path.display(),
                    resp.id
                )?;
                Ok(())
            }
        }
    }
}

impl CommandHandler for super::PlaylistAddCommand {
    type Output = anyhow::Result<()>;

    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send, R: StdIn>(
        &self,
        mut client: mecomp_prost::MusicPlayerClient,
        stdout: &mut W1,
        stderr: &mut W2,
        stdin: &R,
    ) -> Self::Output {
        let Ok(ids) = self
            .items
            .iter()
            .map(|id| id.parse())
            .collect::<Result<Vec<RecordId>, _>>()
        else {
            bail!("One or more provided IDs are invalid");
        };

        // are we in a pipe?
        let list = if stdin.is_terminal() && ids.is_empty() {
            bail!("No input provided, can't add nothing to the playlist");
        } else if !stdin.is_terminal() {
            let from_pipe: Vec<RecordId> =
                utils::parse_from_lines(stdin.lines().filter_map(|l| match l {
                    Ok(line) => Some(line),
                    Err(e) => {
                        writeln!(stderr, "Error reading from stdin: {e}").ok();
                        None
                    }
                }));
            let mut all_ids = ids;
            all_ids.extend(from_pipe);
            if all_ids.is_empty() {
                bail!("No input provided, can't add nothing to the playlist");
            }
            all_ids
        } else {
            ids
        };

        client
            .playlist_add_list(PlaylistAddListRequest::new(self.id.clone(), list))
            .await?;

        writeln!(stdout, "Daemon response:\nitems added to playlist")?;
        Ok(())
    }
}

static BNF_GRAMMAR: &str = r#"Dynamic playlists are playlists that are generated based on a query.

The syntax for queries is as follows:

```bnf
<query> ::= <clause>

<clause> ::= <compound> | <leaf>

<compound> ::= (<clause> (" OR " | " AND ") <clause>)

<leaf> ::= <value> <operator> <value>

<value> ::= <string> | <int> | <set> | <field>

<field> ::= "title" | "artist" | "album" | "album_artist" | "genre" | "release_year"

<operator> ::= "=" | "!=" | "?=" | "*=" | ">" | ">=" | "<" | "<=" | "~" | "!~" | "?~" | "*~" | "IN" | "NOT IN" | "CONTAINS" | "CONTAINSNOT" | "CONTAINSALL" | "CONTAINSANY" | "CONTAINSNONE"

<string> ::= <quote> {{ <char> }} <quote>

<set> ::= '[' <value> {{ ", " <value> }} ']' | '[' ']'

<quote> ::= '"' | "'"

<int> ::= <digit> {{ <digit> }}
```"#;

impl CommandHandler for super::DynamicCommand {
    type Output = anyhow::Result<()>;

    #[allow(clippy::too_many_lines)]
    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send, R: StdIn>(
        &self,
        mut client: mecomp_prost::MusicPlayerClient,
        stdout: &mut W1,
        _: &mut W2,
        _: &R,
    ) -> Self::Output {
        match self {
            Self::List => {
                let resp = client
                    .library_dynamic_playlists(())
                    .await?
                    .into_inner()
                    .playlists;
                writeln!(
                    stdout,
                    "Daemon response:\n{}",
                    printing::dynamic_playlist_list("Dynamic Playlists", resp)?
                )?;
                Ok(())
            }
            Self::Get { id } => {
                let resp: Option<DynamicPlaylist> = client
                    .library_dynamic_playlist_get(Ulid::new(id))
                    .await?
                    .into_inner()
                    .playlist;
                writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                Ok(())
            }
            Self::Songs { id } => {
                match client
                    .library_dynamic_playlist_get_songs(Ulid::new(id))
                    .await
                    .map(|r| r.into_inner().songs)
                {
                    Ok(songs) => {
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::song_list("Songs", songs, false, true)?
                        )?;
                    }
                    Err(e) if e.code() == Code::NotFound => {
                        writeln!(stdout, "Daemon response:\ndynamic playlist not found")?;
                    }
                    Err(e) => bail!(e),
                }
                Ok(())
            }
            Self::Create { name, query } => {
                let resp: RecordId = client
                    .dynamic_playlist_create(DynamicPlaylistCreateRequest::new(
                        name,
                        query.compile_for_storage(),
                    ))
                    .await?
                    .into_inner();
                writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                Ok(())
            }
            Self::Delete { id } => {
                client.dynamic_playlist_remove(Ulid::new(id)).await?;
                writeln!(stdout, "Daemon response:\nDynamic playlist deleted")?;
                Ok(())
            }
            Self::Update { id, update } => {
                let mut changes = DynamicPlaylistChangeSet::new();
                if let Some(name) = &update.name {
                    changes = changes.name(name.as_str());
                }
                if let Some(query) = &update.query {
                    changes = changes.query(query.compile_for_storage());
                }

                if let Ok(resp) = client
                    .dynamic_playlist_update(DynamicPlaylistUpdateRequest::new(id, changes))
                    .await
                    .map(Response::into_inner)
                {
                    writeln!(
                        stdout,
                        "Daemon response:\nDynamic Playlist updated\n{resp:?}"
                    )?;
                } else {
                    writeln!(stdout, "Daemon response:\ndynamic playlist not found")?;
                }
                Ok(())
            }
            Self::ShowBNF => {
                writeln!(stdout, "{BNF_GRAMMAR}")?;
                Ok(())
            }
            Self::Export { path } => {
                client
                    .dynamic_playlist_export(mecomp_prost::Path::new(path))
                    .await?;
                writeln!(
                    stdout,
                    "Daemon response:\nDynamic playlists exported to {}",
                    path.display()
                )?;
                Ok(())
            }
            Self::Import { path } => {
                let resp: Vec<DynamicPlaylist> = client
                    .dynamic_playlist_import(mecomp_prost::Path::new(path))
                    .await?
                    .into_inner()
                    .playlists;
                writeln!(
                    stdout,
                    "Daemon response:\n{}",
                    printing::dynamic_playlist_list("Dynamic Playlists", resp)?
                )?;
                Ok(())
            }
        }
    }
}

impl CommandHandler for super::CollectionCommand {
    type Output = anyhow::Result<()>;

    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send, R: StdIn>(
        &self,
        mut client: mecomp_prost::MusicPlayerClient,
        stdout: &mut W1,
        _: &mut W2,
        _: &R,
    ) -> Self::Output {
        match self {
            Self::List => {
                let resp = client
                    .library_collections(())
                    .await?
                    .into_inner()
                    .collections;
                writeln!(
                    stdout,
                    "Daemon response:\n{}",
                    printing::collection_list("Collections", resp)?
                )?;
                Ok(())
            }
            Self::Get { id } => {
                let resp = client
                    .library_collection_get(Ulid::new(id))
                    .await?
                    .into_inner()
                    .collection;
                writeln!(stdout, "Daemon response:\n{resp:?}")?;
                Ok(())
            }
            Self::Songs { id } => {
                match client
                    .library_collection_get_songs(Ulid::new(id))
                    .await
                    .map(|r| r.into_inner().songs)
                {
                    Ok(songs) => {
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::song_list("Songs", songs, false, true)?
                        )?;
                    }
                    Err(e) if e.code() == Code::NotFound => {
                        writeln!(stdout, "Daemon response:\ncollection not found")?;
                    }
                    Err(e) => bail!(e),
                }
                Ok(())
            }
            Self::Recluster => {
                let resp = client
                    .library_recluster(())
                    .await?
                    .map(|()| "reclustering started")
                    .into_inner();
                writeln!(stdout, "Daemon response:\n{resp:?}")?;
                Ok(())
            }
            Self::Freeze { id, name } => {
                let resp: RecordId = client
                    .collection_freeze(CollectionFreezeRequest::new(id, name))
                    .await?
                    .into_inner();
                writeln!(stdout, "Daemon response:\n{resp}")?;
                Ok(())
            }
        }
    }
}

impl CommandHandler for super::RadioCommand {
    type Output = anyhow::Result<()>;

    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send, R: StdIn>(
        &self,
        mut client: mecomp_prost::MusicPlayerClient,
        stdout: &mut W1,
        stderr: &mut W2,
        stdin: &R,
    ) -> Self::Output {
        let Ok(ids) = self
            .items
            .iter()
            .map(|id| id.parse())
            .collect::<Result<Vec<RecordId>, _>>()
        else {
            bail!("One or more provided IDs are invalid");
        };

        // are we in a pipe?
        let list = if stdin.is_terminal() && ids.is_empty() {
            bail!("No input provided, can't create a radio from nothing");
        } else if !stdin.is_terminal() {
            let from_pipe: Vec<RecordId> =
                utils::parse_from_lines(stdin.lines().filter_map(|l| match l {
                    Ok(line) => Some(line),
                    Err(e) => {
                        writeln!(stderr, "Error reading from stdin: {e}").ok();
                        None
                    }
                }));
            let mut all_ids = ids;
            all_ids.extend(from_pipe);
            if all_ids.is_empty() {
                bail!("No input provided, can't create a radio from nothing");
            }
            all_ids
        } else {
            ids
        };

        let resp = client
            .radio_get_similar_ids(RadioSimilarRequest::new(list, self.n))
            .await?
            .into_inner()
            .ids;
        writeln!(stdout, "Daemon response:\n{}", printing::thing_list(&resp)?)?;
        Ok(())
    }
}
