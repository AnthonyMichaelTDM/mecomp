use std::{
    io::{BufRead, IsTerminal},
    time::Duration,
};

use crate::handlers::{printing, utils};

use super::{
    Command, CommandHandler, CurrentTarget, LibraryCommand, LibraryGetCommand, LibraryListTarget,
    PlaylistGetMethod, QueueAddTarget, QueueCommand, RandTarget, SearchTarget, SeekCommand,
    VolumeCommand,
};

use anyhow::bail;
use mecomp_core::{
    rpc::SearchResult,
    state::{
        SeekType,
        library::{LibraryBrief, LibraryFull, LibraryHealth},
    },
};
use mecomp_storage::db::schemas::{
    Id, RecordId,
    album::{self, Album, AlbumBrief},
    artist::{self, Artist, ArtistBrief},
    collection::{self, Collection},
    dynamic::{self, DynamicPlaylist, DynamicPlaylistChangeSet},
    playlist::{self, Playlist},
    song::{self, Song, SongBrief},
};
use one_or_many::OneOrMany;

impl CommandHandler for Command {
    type Output = anyhow::Result<()>;

    #[allow(clippy::too_many_lines)]
    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send>(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
        stdout: &mut W1,
        stderr: &mut W2,
    ) -> Self::Output {
        match self {
            Self::Ping => {
                let resp: String = client.ping(ctx).await?;
                writeln!(stdout, "Daemon response:\n{resp}")?;
                Ok(())
            }
            Self::Stop => {
                client.daemon_shutdown(ctx).await?;
                writeln!(
                    stdout,
                    "Daemon stopping, check the daemon logs for more information"
                )?;
                Ok(())
            }
            Self::Library { command } => command.handle(ctx, client, stdout, stderr).await,
            Self::Status { command } => command.handle(ctx, client, stdout, stderr).await,
            Self::State => {
                if let Some(state) = client.state_audio(ctx).await? {
                    writeln!(stdout, "{}", printing::audio_state(&state)?)?;
                } else {
                    writeln!(stdout, "Daemon response:\nNo audio state available")?;
                }
                Ok(())
            }
            Self::Current { target } => {
                match target {
                    CurrentTarget::Artist => {
                        let resp: OneOrMany<Artist> = client.current_artist(ctx).await?;
                        writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                    }
                    CurrentTarget::Album => {
                        let resp: Option<Album> = client.current_album(ctx).await?;
                        writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                    }
                    CurrentTarget::Song => {
                        let resp: Option<SongBrief> = client.current_song(ctx).await?;
                        writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                    }
                }
                Ok(())
            }
            Self::Rand { target } => {
                match target {
                    RandTarget::Artist => {
                        let resp: Option<ArtistBrief> = client.rand_artist(ctx).await?;
                        writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                    }
                    RandTarget::Album => {
                        let resp: Option<AlbumBrief> = client.rand_album(ctx).await?;
                        writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                    }
                    RandTarget::Song => {
                        let resp: Option<SongBrief> = client.rand_song(ctx).await?;
                        writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                    }
                }
                Ok(())
            }
            Self::Search {
                quiet,
                target,
                query,
                limit,
            } => {
                match target {
                    SearchTarget::All => {
                        let SearchResult {
                            songs,
                            albums,
                            artists,
                        } = client.search(ctx, query.clone(), *limit).await?;
                        writeln!(
                            stdout,
                            "Daemon response:\n{}\n{}\n{}",
                            printing::song_list("Songs", &songs, *quiet)?,
                            printing::album_list("Albums", &albums, *quiet)?,
                            printing::artist_list("Artists", &artists, *quiet)?
                        )?;
                    }
                    SearchTarget::Artist => {
                        let resp: Box<[ArtistBrief]> =
                            client.search_artist(ctx, query.clone(), *limit).await?;
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::artist_list("Artists", &resp, *quiet)?
                        )?;
                    }
                    SearchTarget::Album => {
                        let resp: Box<[AlbumBrief]> =
                            client.search_album(ctx, query.clone(), *limit).await?;
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::album_list("Albums", &resp, *quiet)?
                        )?;
                    }
                    SearchTarget::Song => {
                        let resp: Box<[SongBrief]> =
                            client.search_song(ctx, query.clone(), *limit).await?;
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::song_list("Songs", &resp, *quiet)?
                        )?;
                    }
                }
                Ok(())
            }
            Self::Playback { command } => command.handle(ctx, client, stdout, stderr).await,
            Self::Queue { command } => command.handle(ctx, client, stdout, stderr).await,
            Self::Playlist { command } => command.handle(ctx, client, stdout, stderr).await,
            Self::Dynamic { command } => command.handle(ctx, client, stdout, stderr).await,
            Self::Collection { command } => command.handle(ctx, client, stdout, stderr).await,
            Self::Radio { command } => command.handle(ctx, client, stdout, stderr).await,
        }
    }
}

impl CommandHandler for LibraryCommand {
    type Output = anyhow::Result<()>;

    #[allow(clippy::too_many_lines)]
    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send>(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
        stdout: &mut W1,
        _: &mut W2,
    ) -> Self::Output {
        match self {
            Self::Rescan => {
                let resp: Result<(), _> = client.library_rescan(ctx).await?;
                if let Err(e) = resp {
                    writeln!(stdout, "Daemon response:\n{e}")?;
                } else {
                    writeln!(stdout, "Daemon response:\nLibrary rescan started")?;
                }
                Ok(())
            }
            Self::Analyze { overwrite } => {
                let resp: Result<(), _> = client.library_analyze(ctx, *overwrite).await?;
                if let Err(e) = resp {
                    writeln!(stdout, "Daemon response:\n{e}")?;
                } else {
                    writeln!(stdout, "Daemon response:\nLibrary analysis started")?;
                }
                Ok(())
            }
            Self::Recluster => {
                let resp: Result<(), _> = client.library_recluster(ctx).await?;
                if let Err(e) = resp {
                    writeln!(stdout, "Daemon response:\n{e}")?;
                } else {
                    writeln!(stdout, "Daemon response:\nreclustering started")?;
                }
                Ok(())
            }
            Self::Brief => {
                let resp: Result<LibraryBrief, _> = client.library_brief(ctx).await?;
                writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                Ok(())
            }
            Self::Full => {
                let resp: Result<LibraryFull, _> = client.library_full(ctx).await?;
                writeln!(stdout, "Daemon response:\n{resp:?}")?;
                Ok(())
            }
            Self::Health => {
                let resp: Result<LibraryHealth, _> = client.library_health(ctx).await?;
                writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                Ok(())
            }
            Self::List { quiet, target } => {
                match target {
                    LibraryListTarget::Artists => {
                        let resp: Box<[ArtistBrief]> = client.library_artists_brief(ctx).await??;
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::artist_list("Artists", &resp, *quiet)?
                        )?;
                    }
                    LibraryListTarget::Albums => {
                        let resp: Box<[AlbumBrief]> = client.library_albums_brief(ctx).await??;
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::album_list("Albums", &resp, *quiet)?
                        )?;
                    }
                    LibraryListTarget::Songs => {
                        let resp: Box<[SongBrief]> = client.library_songs_brief(ctx).await??;
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::song_list("Songs", &resp, false)?
                        )?;
                    }
                    LibraryListTarget::Playlists => {
                        let resp: Box<[Playlist]> = client.library_playlists_full(ctx).await??;
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::playlist_list("Playlists", &resp)?
                        )?;
                    }
                    LibraryListTarget::DynamicPlaylists => {
                        let resp: Box<[DynamicPlaylist]> =
                            client.dynamic_playlist_list(ctx).await?;
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::dynamic_playlist_list("Dynamic Playlists", &resp)?
                        )?;
                    }
                    LibraryListTarget::Collections => {
                        let resp: Box<[Collection]> =
                            client.library_collections_full(ctx).await??;
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::collection_list("Collections", &resp)?
                        )?;
                    }
                }
                Ok(())
            }
            Self::Get { command } => {
                match command {
                    LibraryGetCommand::Artist { id } => {
                        let resp: Option<Artist> = client
                            .library_artist_get(
                                ctx,
                                RecordId {
                                    tb: artist::TABLE_NAME.to_owned(),
                                    id: Id::String(id.to_owned()),
                                },
                            )
                            .await?;
                        writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                    }
                    LibraryGetCommand::Album { id } => {
                        let resp: Option<Album> = client
                            .library_album_get(
                                ctx,
                                RecordId {
                                    tb: album::TABLE_NAME.to_owned(),
                                    id: Id::String(id.to_owned()),
                                },
                            )
                            .await?;
                        writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                    }
                    LibraryGetCommand::Song { id } => {
                        let resp: Option<Song> = client
                            .library_song_get(
                                ctx,
                                RecordId {
                                    tb: song::TABLE_NAME.to_owned(),
                                    id: Id::String(id.to_owned()),
                                },
                            )
                            .await?;
                        writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                    }
                    LibraryGetCommand::Playlist { id } => {
                        let resp: Option<Playlist> = client
                            .playlist_get(
                                ctx,
                                RecordId {
                                    tb: playlist::TABLE_NAME.to_owned(),
                                    id: Id::String(id.to_owned()),
                                },
                            )
                            .await?;
                        writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                    }
                    LibraryGetCommand::Dynamic { id } => {
                        let resp: Option<DynamicPlaylist> = client
                            .dynamic_playlist_get(
                                ctx,
                                RecordId {
                                    tb: dynamic::TABLE_NAME.to_owned(),
                                    id: Id::String(id.to_owned()),
                                },
                            )
                            .await?;
                        writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                    }
                    LibraryGetCommand::Collection { id } => {
                        let resp: Option<Collection> = client
                            .collection_get(
                                ctx,
                                RecordId {
                                    tb: collection::TABLE_NAME.to_owned(),
                                    id: Id::String(id.to_owned()),
                                },
                            )
                            .await?;
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

    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send>(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
        stdout: &mut W1,
        _: &mut W2,
    ) -> Self::Output {
        match self {
            Self::Rescan => {
                if client.library_rescan_in_progress(ctx).await? {
                    writeln!(stdout, "Daemon response:\nthere is a rescan in progress")?;
                } else {
                    writeln!(
                        stdout,
                        "Daemon response:\nthere is not a rescan in progress"
                    )?;
                }
            }
            Self::Analyze => {
                if client.library_analyze_in_progress(ctx).await? {
                    writeln!(stdout, "Daemon response:\nthere is an analysis in progress")?;
                } else {
                    writeln!(
                        stdout,
                        "Daemon response:\nthere is not an analysis in progress"
                    )?;
                }
            }
            Self::Recluster => {
                if client.library_recluster_in_progress(ctx).await? {
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

    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send>(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
        stdout: &mut W1,
        stderr: &mut W2,
    ) -> Self::Output {
        match self {
            Self::Toggle => {
                client.playback_toggle(ctx).await?;
                writeln!(stdout, "Daemon response:\nplayback toggled")?;
                Ok(())
            }
            Self::Play => {
                client.playback_play(ctx).await?;
                writeln!(stdout, "Daemon response:\nplayback started")?;
                Ok(())
            }
            Self::Pause => {
                client.playback_pause(ctx).await?;
                writeln!(stdout, "Daemon response:\nplayback paused")?;
                Ok(())
            }
            Self::Stop => {
                client.playback_clear_player(ctx).await?;
                writeln!(stdout, "Daemon response:\nplayback stopped")?;
                Ok(())
            }
            Self::Restart => {
                client.playback_restart(ctx).await?;
                writeln!(stdout, "Daemon response:\nplayback restarted")?;
                Ok(())
            }
            Self::Next => {
                client.playback_skip_forward(ctx, 1).await?;
                writeln!(stdout, "Daemon response:\nnext track started")?;
                Ok(())
            }
            Self::Previous => {
                client.playback_skip_backward(ctx, 1).await?;
                writeln!(stdout, "Daemon response:\nprevious track started")?;
                Ok(())
            }
            Self::Seek { command } => command.handle(ctx, client, stdout, stderr).await,
            Self::Volume { command } => command.handle(ctx, client, stdout, stderr).await,
            Self::Repeat { mode } => {
                let mode: mecomp_core::state::RepeatMode = (*mode).into();
                client.playback_repeat(ctx, mode).await?;
                writeln!(stdout, "Daemon response:\nrepeat mode set to {mode}")?;
                Ok(())
            }
            Self::Shuffle => {
                client.playback_shuffle(ctx).await?;
                writeln!(stdout, "Daemon response:\nqueue shuffled")?;
                Ok(())
            }
        }
    }
}

impl CommandHandler for SeekCommand {
    type Output = anyhow::Result<()>;

    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send>(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
        stdout: &mut W1,
        _: &mut W2,
    ) -> Self::Output {
        match self {
            Self::Forward { amount } => {
                client
                    .playback_seek(
                        ctx,
                        SeekType::RelativeForwards,
                        Duration::from_secs_f32(*amount),
                    )
                    .await?;
                writeln!(stdout, "Daemon response:\nsought forward by {amount:.2}s")?;
            }
            Self::Backward { amount } => {
                client
                    .playback_seek(
                        ctx,
                        SeekType::RelativeBackwards,
                        Duration::from_secs_f32(*amount),
                    )
                    .await?;
                writeln!(stdout, "Daemon response:\nsought backward by {amount:.2}s")?;
            }
            Self::Absolute { position } => {
                client
                    .playback_seek(ctx, SeekType::Absolute, Duration::from_secs_f32(*position))
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

    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send>(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
        stdout: &mut W1,
        _: &mut W2,
    ) -> Self::Output {
        match self {
            Self::Set { volume } => {
                client.playback_volume(ctx, *volume / 100.0).await?;
                writeln!(stdout, "Daemon response:\nvolume set to {volume}")?;
                Ok(())
            }
            Self::Increase { amount } => {
                client.playback_volume_up(ctx, *amount / 100.0).await?;
                writeln!(stdout, "Daemon response:\nvolume increased by {amount}")?;
                Ok(())
            }
            Self::Decrease { amount } => {
                client.playback_volume_down(ctx, *amount / 100.0).await?;
                writeln!(stdout, "Daemon response:\nvolume decreased by {amount}")?;
                Ok(())
            }
            Self::Mute => {
                client.playback_mute(ctx).await?;
                writeln!(stdout, "Daemon response:\nvolume muted")?;
                Ok(())
            }
            Self::Unmute => {
                client.playback_unmute(ctx).await?;
                writeln!(stdout, "Daemon response:\nvolume unmuted")?;
                Ok(())
            }
        }
    }
}

impl CommandHandler for QueueCommand {
    type Output = anyhow::Result<()>;

    #[allow(clippy::too_many_lines)]
    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send>(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
        stdout: &mut W1,
        stderr: &mut W2,
    ) -> Self::Output {
        match self {
            Self::Clear => {
                client.playback_clear(ctx).await?;
                writeln!(stdout, "Daemon response:\nqueue cleared")?;
            }
            Self::List { quiet: false } => {
                let resp: Option<Box<[SongBrief]>> =
                    client.state_audio(ctx).await?.map(|s| s.queue);
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
                let resp: Option<Box<[SongBrief]>> =
                    client.state_audio(ctx).await?.map(|s| s.queue);
                if let Some(songs) = resp {
                    writeln!(
                        stdout,
                        "Daemon response:\n{}",
                        printing::song_list("Queue", &songs, true)?
                    )?;
                } else {
                    writeln!(stdout, "Daemon response:\nNo queue available")?;
                }
            }
            Self::Add { target, id } => {
                let message: &str = match target {
                    QueueAddTarget::Artist => client
                        .queue_add(
                            ctx,
                            RecordId {
                                tb: artist::TABLE_NAME.to_owned(),
                                id: Id::String(id.clone()),
                            },
                        )
                        .await?
                        .map(|()| "artist added to queue"),
                    QueueAddTarget::Album => client
                        .queue_add(
                            ctx,
                            RecordId {
                                tb: album::TABLE_NAME.to_owned(),
                                id: Id::String(id.clone()),
                            },
                        )
                        .await?
                        .map(|()| "album added to queue"),
                    QueueAddTarget::Song => client
                        .queue_add(
                            ctx,
                            RecordId {
                                tb: song::TABLE_NAME.to_owned(),
                                id: Id::String(id.clone()),
                            },
                        )
                        .await?
                        .map(|()| "song added to queue"),
                    QueueAddTarget::Playlist => client
                        .queue_add(
                            ctx,
                            RecordId {
                                tb: playlist::TABLE_NAME.to_owned(),
                                id: Id::String(id.clone()),
                            },
                        )
                        .await?
                        .map(|()| "playlist added to queue"),
                    QueueAddTarget::Collection => client
                        .queue_add(
                            ctx,
                            RecordId {
                                tb: collection::TABLE_NAME.to_owned(),
                                id: Id::String(id.clone()),
                            },
                        )
                        .await?
                        .map(|()| "collection added to queue"),
                    QueueAddTarget::Dynamic => client
                        .queue_add(
                            ctx,
                            RecordId {
                                tb: dynamic::TABLE_NAME.to_owned(),
                                id: Id::String(id.clone()),
                            },
                        )
                        .await?
                        .map(|()| "dynamic added to queue"),
                }?;

                writeln!(stdout, "Daemon response:\n{message}")?;
            }
            Self::Remove { start, end } => {
                client.queue_remove_range(ctx, *start..*end).await?;
                writeln!(stdout, "Daemon response:\nitems removed from queue")?;
            }
            Self::Set { index } => {
                client.queue_set_index(ctx, *index).await?;
                writeln!(
                    stdout,
                    "Daemon response:\ncurrent song set to index {index}"
                )?;
            }
            Self::Pipe => {
                let stdin = std::io::stdin();
                if stdin.is_terminal() {
                    writeln!(
                        stdout,
                        "No input provided, this command is meant to be used with a pipe"
                    )?;
                } else {
                    let list: Vec<RecordId> = utils::parse_things_from_lines(
                        stdin.lock().lines().filter_map(|l| match l {
                            Ok(line) => Some(line),
                            Err(e) => {
                                writeln!(stderr, "Error reading from stdin: {e}").ok();
                                None
                            }
                        }),
                    );

                    client.queue_add_list(ctx, list).await??;
                    writeln!(stdout, "Daemon response:\nitems added to queue")?;
                }
            }
        }
        Ok(())
    }
}

impl CommandHandler for super::PlaylistCommand {
    type Output = anyhow::Result<()>;

    #[allow(clippy::too_many_lines)]
    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send>(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
        stdout: &mut W1,
        stderr: &mut W2,
    ) -> Self::Output {
        match self {
            Self::List => {
                let resp: Box<[Playlist]> = client.library_playlists_full(ctx).await??;
                writeln!(
                    stdout,
                    "Daemon response:\n{}",
                    printing::playlist_list("Playlists", &resp)?
                )?;
                Ok(())
            }
            Self::Get { method, target } => {
                let resp: Option<Playlist> = match method {
                    PlaylistGetMethod::Id => {
                        client
                            .playlist_get(
                                ctx,
                                RecordId {
                                    tb: playlist::TABLE_NAME.to_owned(),
                                    id: Id::String(target.clone()),
                                },
                            )
                            .await?
                    }
                    PlaylistGetMethod::Name => {
                        if let Some(id) = client.playlist_get_id(ctx, target.clone()).await? {
                            client.playlist_get(ctx, id).await?
                        } else {
                            None
                        }
                    }
                };

                writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                Ok(())
            }
            Self::Create { name } => {
                let resp: RecordId = client.playlist_get_or_create(ctx, name.clone()).await??;
                writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                Ok(())
            }
            Self::Update { id, name } => {
                let resp: Playlist = client
                    .playlist_rename(
                        ctx,
                        RecordId {
                            tb: playlist::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                        name.clone(),
                    )
                    .await??;
                writeln!(
                    stdout,
                    "Daemon response:\nplaylist renamed to \"{}\"",
                    resp.name
                )?;
                Ok(())
            }
            Self::Songs { id } => {
                match client
                    .playlist_get_songs(
                        ctx,
                        RecordId {
                            tb: playlist::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                    )
                    .await?
                {
                    Some(songs) => {
                        let songs = songs.into_iter().map(Into::into).collect::<Box<_>>();
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::song_list("Songs", &songs, false)?
                        )?;
                    }
                    None => {
                        writeln!(stdout, "Daemon response:\nplaylist not found")?;
                    }
                }
                Ok(())
            }

            Self::Delete { id } => {
                client
                    .playlist_remove(
                        ctx,
                        RecordId {
                            tb: playlist::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                    )
                    .await??;
                writeln!(stdout, "Daemon response:\nplaylist deleted")?;
                Ok(())
            }
            Self::Add { command } => command.handle(ctx, client, stdout, stderr).await,
            Self::Remove { id, item_ids } => {
                client
                    .playlist_remove_songs(
                        ctx,
                        RecordId {
                            tb: playlist::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                        item_ids
                            .iter()
                            .map(|id| RecordId {
                                tb: song::TABLE_NAME.to_owned(),
                                id: Id::String(id.clone()),
                            })
                            .collect(),
                    )
                    .await??;
                writeln!(stdout, "Daemon response:\nsongs removed from playlist")?;

                Ok(())
            }
            Self::Export { id, path } => {
                client
                    .playlist_export(
                        ctx,
                        RecordId {
                            tb: playlist::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                        path.clone(),
                    )
                    .await??;
                writeln!(
                    stdout,
                    "Daemon response:\nplaylist exported to {}",
                    path.display()
                )?;
                Ok(())
            }
            Self::Import { path, name } => {
                let resp: RecordId = client
                    .playlist_import(ctx, path.clone(), name.clone())
                    .await??;
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

    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send>(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
        stdout: &mut W1,
        stderr: &mut W2,
    ) -> Self::Output {
        let resp = match self {
            Self::Artist { id, artist_id } => client
                .playlist_add(
                    ctx,
                    RecordId {
                        tb: artist::TABLE_NAME.to_owned(),
                        id: Id::String(artist_id.clone()),
                    },
                    RecordId {
                        tb: playlist::TABLE_NAME.to_owned(),
                        id: Id::String(id.clone()),
                    },
                )
                .await?
                .map(|()| "artist added to playlist"),
            Self::Album { id, album_id } => client
                .playlist_add(
                    ctx,
                    RecordId {
                        tb: album::TABLE_NAME.to_owned(),
                        id: Id::String(album_id.clone()),
                    },
                    RecordId {
                        tb: playlist::TABLE_NAME.to_owned(),
                        id: Id::String(id.clone()),
                    },
                )
                .await?
                .map(|()| "album added to playlist"),
            Self::Song { id, song_ids } => client
                .playlist_add_list(
                    ctx,
                    RecordId {
                        tb: playlist::TABLE_NAME.to_owned(),
                        id: Id::String(id.clone()),
                    },
                    song_ids
                        .iter()
                        .map(|id| RecordId {
                            tb: song::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        })
                        .collect(),
                )
                .await?
                .map(|()| "songs added to playlist"),
            Self::Pipe { id } => {
                let stdin = std::io::stdin();
                if stdin.is_terminal() {
                    bail!("No input provided, this command is meant to be used with a pipe");
                }
                let list: Vec<RecordId> =
                    utils::parse_things_from_lines(stdin.lock().lines().filter_map(|l| match l {
                        Ok(line) => Some(line),
                        Err(e) => {
                            writeln!(stderr, "Error reading from stdin: {e}").ok();
                            None
                        }
                    }));

                client
                    .playlist_add_list(
                        ctx,
                        RecordId {
                            tb: playlist::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                        list,
                    )
                    .await?
                    .map(|()| "items added to playlist")
            }
        };
        writeln!(stdout, "Daemon response:\n{resp:?}")?;
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
    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send>(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
        stdout: &mut W1,
        _: &mut W2,
    ) -> Self::Output {
        match self {
            Self::List => {
                let resp: Box<[DynamicPlaylist]> = client.dynamic_playlist_list(ctx).await?;
                writeln!(
                    stdout,
                    "Daemon response:\n{}",
                    printing::dynamic_playlist_list("Dynamic Playlists", &resp)?
                )?;
                Ok(())
            }
            Self::Get { id } => {
                let resp: Option<DynamicPlaylist> = client
                    .dynamic_playlist_get(
                        ctx,
                        RecordId {
                            tb: dynamic::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                    )
                    .await?;
                writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                Ok(())
            }
            Self::Songs { id } => {
                match client
                    .dynamic_playlist_get_songs(
                        ctx,
                        RecordId {
                            tb: dynamic::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                    )
                    .await?
                {
                    Some(songs) => {
                        let songs = songs.into_iter().map(Into::into).collect::<Box<_>>();
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::song_list("Songs", &songs, false)?
                        )?;
                    }
                    None => {
                        writeln!(stdout, "Daemon response:\ndynamic playlist not found")?;
                    }
                }
                Ok(())
            }
            Self::Create { name, query } => {
                let resp: RecordId = client
                    .dynamic_playlist_create(ctx, name.clone(), query.clone())
                    .await??;
                writeln!(stdout, "Daemon response:\n{resp:#?}")?;
                Ok(())
            }
            Self::Delete { id } => {
                client
                    .dynamic_playlist_remove(
                        ctx,
                        RecordId {
                            tb: dynamic::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                    )
                    .await??;
                writeln!(stdout, "Daemon response:\nDynamic playlist deleted")?;
                Ok(())
            }
            Self::Update { id, update } => {
                let mut changes = DynamicPlaylistChangeSet::new();
                if let Some(name) = &update.name {
                    changes = changes.name(name.as_str());
                }
                if let Some(query) = &update.query {
                    changes = changes.query(query.clone());
                }

                let resp: DynamicPlaylist = client
                    .dynamic_playlist_update(
                        ctx,
                        RecordId {
                            tb: dynamic::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                        changes,
                    )
                    .await??;
                writeln!(
                    stdout,
                    "Daemon response:\nDynamic Playlist updated\n{resp:?}"
                )?;
                Ok(())
            }
            Self::ShowBNF => {
                writeln!(stdout, "{BNF_GRAMMAR}")?;
                Ok(())
            }
            Self::Export { path } => {
                client.dynamic_playlist_export(ctx, path.clone()).await??;
                writeln!(
                    stdout,
                    "Daemon response:\nDynamic playlists exported to {}",
                    path.display()
                )?;
                Ok(())
            }
            Self::Import { path } => {
                let resp: Vec<DynamicPlaylist> =
                    client.dynamic_playlist_import(ctx, path.clone()).await??;
                writeln!(
                    stdout,
                    "Daemon response:\n{}",
                    printing::dynamic_playlist_list("Dynamic Playlists", &resp)?
                )?;
                Ok(())
            }
        }
    }
}

impl CommandHandler for super::CollectionCommand {
    type Output = anyhow::Result<()>;

    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send>(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
        stdout: &mut W1,
        _: &mut W2,
    ) -> Self::Output {
        match self {
            Self::List => {
                let resp: Box<[Collection]> = client.library_collections_full(ctx).await??;
                writeln!(
                    stdout,
                    "Daemon response:\n{}",
                    printing::collection_list("Collections", &resp)?
                )?;
                Ok(())
            }
            Self::Get { id } => {
                let resp: Option<Collection> = client
                    .collection_get(
                        ctx,
                        RecordId {
                            tb: collection::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                    )
                    .await?;
                writeln!(stdout, "Daemon response:\n{resp:?}")?;
                Ok(())
            }
            Self::Songs { id } => {
                match client
                    .collection_get_songs(
                        ctx,
                        RecordId {
                            tb: collection::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                    )
                    .await?
                {
                    Some(songs) => {
                        let songs = songs.into_iter().map(Into::into).collect::<Box<_>>();
                        writeln!(
                            stdout,
                            "Daemon response:\n{}",
                            printing::song_list("Songs", &songs, false)?
                        )?;
                    }
                    None => {
                        writeln!(stdout, "Daemon response:\ncollection not found")?;
                    }
                }
                Ok(())
            }
            Self::Recluster => {
                let resp: Result<&str, _> = client
                    .library_recluster(ctx)
                    .await?
                    .map(|()| "reclustering started");
                writeln!(stdout, "Daemon response:\n{resp:?}")?;
                Ok(())
            }
            Self::Freeze { id, name } => {
                let resp: RecordId = client
                    .collection_freeze(
                        ctx,
                        RecordId {
                            tb: collection::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                        name.to_owned(),
                    )
                    .await??;
                writeln!(stdout, "Daemon response:\n{resp}")?;
                Ok(())
            }
        }
    }
}

impl CommandHandler for super::RadioCommand {
    type Output = anyhow::Result<()>;

    async fn handle<W1: std::fmt::Write + Send, W2: std::fmt::Write + Send>(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
        stdout: &mut W1,
        stderr: &mut W2,
    ) -> Self::Output {
        match self {
            Self::Song { id, n } => {
                let resp: Box<[RecordId]> = client
                    .radio_get_similar_ids(
                        ctx,
                        vec![RecordId {
                            tb: song::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        }],
                        *n,
                    )
                    .await??;
                writeln!(stdout, "Daemon response:\n{}", printing::thing_list(&resp)?)?;
                Ok(())
            }
            Self::Artist { id, n } => {
                let resp: Box<[RecordId]> = client
                    .radio_get_similar_ids(
                        ctx,
                        vec![RecordId {
                            tb: artist::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        }],
                        *n,
                    )
                    .await??;
                writeln!(stdout, "Daemon response:\n{}", printing::thing_list(&resp)?)?;
                Ok(())
            }
            Self::Album { id, n } => {
                let resp: Box<[RecordId]> = client
                    .radio_get_similar_ids(
                        ctx,
                        vec![RecordId {
                            tb: album::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        }],
                        *n,
                    )
                    .await??;
                writeln!(stdout, "Daemon response:\n{}", printing::thing_list(&resp)?)?;
                Ok(())
            }
            Self::Playlist { id, n } => {
                let resp: Box<[RecordId]> = client
                    .radio_get_similar_ids(
                        ctx,
                        vec![RecordId {
                            tb: playlist::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        }],
                        *n,
                    )
                    .await??;
                writeln!(stdout, "Daemon response:\n{}", printing::thing_list(&resp)?)?;
                Ok(())
            }
            Self::Pipe { n } => {
                let stdin = std::io::stdin();
                if stdin.is_terminal() {
                    bail!("No input provided, this command is meant to be used with a pipe");
                }
                let list: Vec<RecordId> =
                    utils::parse_things_from_lines(stdin.lock().lines().filter_map(|l| match l {
                        Ok(line) => Some(line),
                        Err(e) => {
                            writeln!(stderr, "Error reading from stdin: {e}").ok();
                            None
                        }
                    }));

                let resp: Box<[RecordId]> = client.radio_get_similar_ids(ctx, list, *n).await??;
                writeln!(stdout, "Daemon response:\n{}", printing::thing_list(&resp)?)?;
                Ok(())
            }
        }
    }
}
