use crate::handlers::printing;

use super::{
    Command, CommandHandler, CurrentTarget, LibraryCommand, LibraryGetTarget, LibraryListTarget,
    PlaylistAddTarget, PlaylistGetMethod, QueueCommand, RandTarget, SearchTarget, SeekCommand,
    VolumeCommand,
};

use mecomp_core::state::{
    library::{LibraryBrief, LibraryFull, LibraryHealth},
    SeekType,
};
use mecomp_storage::db::schemas::{
    album::{self, Album, AlbumBrief},
    artist::{self, Artist, ArtistBrief},
    collection::{self, Collection, CollectionBrief},
    playlist::{self, Playlist, PlaylistBrief},
    song::{self, Song, SongBrief},
    Id, Thing,
};
use one_or_many::OneOrMany;

impl CommandHandler for Command {
    type Output = anyhow::Result<()>;

    async fn handle(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
    ) -> Self::Output {
        match self {
            Self::Ping => {
                let resp: String = client.ping(ctx).await?;
                println!("Daemon response:\n{resp}");
                Ok(())
            }
            Self::Stop => {
                client.daemon_shutdown(ctx).await?;
                println!("Daemon stopping, check the daemon logs for more information");
                Ok(())
            }
            Self::Library { command } => command.handle(ctx, client).await,
            Self::State => {
                if let Some(state) = client.state_audio(ctx).await? {
                    println!("{}", printing::audio_state(&state)?);
                } else {
                    println!("Daemon response:\nNo audio state available");
                }
                Ok(())
            }
            Self::Current { target } => {
                match target {
                    CurrentTarget::Artist => {
                        let resp: OneOrMany<Artist> = client.current_artist(ctx).await?;
                        println!("Daemon response:\n{resp:#?}");
                    }
                    CurrentTarget::Album => {
                        let resp: Option<Album> = client.current_album(ctx).await?;
                        println!("Daemon response:\n{resp:#?}");
                    }
                    CurrentTarget::Song => {
                        let resp: Option<Song> = client.current_song(ctx).await?;
                        println!("Daemon response:\n{resp:#?}");
                    }
                }
                Ok(())
            }
            Self::Rand { target } => {
                match target {
                    RandTarget::Artist => {
                        let resp: Option<Artist> = client.rand_artist(ctx).await?;
                        println!("Daemon response:\n{resp:#?}");
                    }
                    RandTarget::Album => {
                        let resp: Option<Album> = client.rand_album(ctx).await?;
                        println!("Daemon response:\n{resp:#?}");
                    }
                    RandTarget::Song => {
                        let resp: Option<Song> = client.rand_song(ctx).await?;
                        println!("Daemon response:\n{resp:#?}");
                    }
                }
                Ok(())
            }
            Self::Search { target, query } => {
                match target {
                    None => {
                        println!(
                            "Daemon response:\n{:?}",
                            client.search(ctx, query.clone()).await?
                        );
                    }
                    Some(SearchTarget::Artist) => {
                        let resp: Box<[Artist]> = client.search_artist(ctx, query.clone()).await?;
                        println!(
                            "Daemon response:\n{}",
                            printing::artist_list("Artists", &resp)?
                        );
                    }
                    Some(SearchTarget::Album) => {
                        let resp: Box<[Album]> = client.search_album(ctx, query.clone()).await?;
                        println!(
                            "Daemon response:\n{}",
                            printing::album_list("Albums", &resp)?
                        );
                    }
                    Some(SearchTarget::Song) => {
                        let resp: Box<[Song]> = client.search_song(ctx, query.clone()).await?;
                        println!(
                            "Daemon response:\n{}",
                            printing::song_list("Songs", &resp, false)?
                        );
                    }
                }
                Ok(())
            }
            Self::Playback { command } => command.handle(ctx, client).await,
            Self::Queue { command } => command.handle(ctx, client).await,
            Self::Playlist { command } => command.handle(ctx, client).await,
            Self::Collection { command } => command.handle(ctx, client).await,
            Self::Radio { command } => command.handle(ctx, client).await,
        }
    }
}

impl CommandHandler for LibraryCommand {
    type Output = anyhow::Result<()>;

    #[allow(clippy::too_many_lines)]
    async fn handle(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
    ) -> Self::Output {
        match self {
            Self::Rescan => {
                let resp: Result<(), _> = client.library_rescan(ctx).await?;
                if let Err(e) = resp {
                    println!("Daemon response:\n{e}");
                } else {
                    println!("Daemon response:\nLibrary rescan started");
                }
                Ok(())
            }
            Self::Brief => {
                let resp: Result<LibraryBrief, _> = client.library_brief(ctx).await?;
                println!("Daemon response:\n{resp:#?}");
                Ok(())
            }
            Self::Full => {
                let resp: Result<LibraryFull, _> = client.library_full(ctx).await?;
                println!("Daemon response:\n{resp:?}");
                Ok(())
            }
            Self::Health => {
                let resp: Result<LibraryHealth, _> = client.library_health(ctx).await?;
                println!("Daemon response:\n{resp:#?}");
                Ok(())
            }
            Self::List { full, target } => {
                if *full {
                    match target {
                        LibraryListTarget::Artists => {
                            let resp: Box<[Artist]> = client.library_artists_full(ctx).await??;
                            println!(
                                "Daemon response:\n{}",
                                printing::artist_list("Artists", &resp)?
                            );
                        }
                        LibraryListTarget::Albums => {
                            let resp: Box<[Album]> = client.library_albums_full(ctx).await??;
                            println!(
                                "Daemon response:\n{}",
                                printing::album_list("Albums", &resp)?
                            );
                        }
                        LibraryListTarget::Songs => {
                            let resp: Box<[Song]> = client.library_songs_full(ctx).await??;
                            println!(
                                "Daemon response:\n{}",
                                printing::song_list("Songs", &resp, false)?
                            );
                        }
                    }
                } else {
                    match target {
                        LibraryListTarget::Artists => {
                            let resp: Box<[ArtistBrief]> =
                                client.library_artists_brief(ctx).await??;
                            println!(
                                "Daemon response:\n{}",
                                printing::artist_brief_list("Artists", &resp)?
                            );
                        }
                        LibraryListTarget::Albums => {
                            let resp: Box<[AlbumBrief]> =
                                client.library_albums_brief(ctx).await??;
                            println!(
                                "Daemon response:\n{}",
                                printing::album_brief_list("Albums", &resp)?
                            );
                        }
                        LibraryListTarget::Songs => {
                            let resp: Box<[SongBrief]> = client.library_songs_brief(ctx).await??;
                            println!(
                                "Daemon response:\n{}",
                                printing::song_brief_list("Songs", &resp)?
                            );
                        }
                    }
                }

                Ok(())
            }
            Self::Get { target, id } => {
                match target {
                    LibraryGetTarget::Artist => {
                        let resp: Option<Artist> = client
                            .library_artist_get(
                                ctx,
                                Thing {
                                    tb: artist::TABLE_NAME.to_owned(),
                                    id: Id::String(id.to_owned()),
                                },
                            )
                            .await?;
                        println!("Daemon response:\n{resp:#?}");
                    }
                    LibraryGetTarget::Album => {
                        let resp: Option<Album> = client
                            .library_album_get(
                                ctx,
                                Thing {
                                    tb: album::TABLE_NAME.to_owned(),
                                    id: Id::String(id.to_owned()),
                                },
                            )
                            .await?;
                        println!("Daemon response:\n{resp:#?}");
                    }
                    LibraryGetTarget::Song => {
                        let resp: Option<Song> = client
                            .library_song_get(
                                ctx,
                                Thing {
                                    tb: song::TABLE_NAME.to_owned(),
                                    id: Id::String(id.to_owned()),
                                },
                            )
                            .await?;
                        println!("Daemon response:\n{resp:#?}");
                    }
                    LibraryGetTarget::Playlist => {
                        let resp: Option<Playlist> = client
                            .library_playlist_get(
                                ctx,
                                Thing {
                                    tb: playlist::TABLE_NAME.to_owned(),
                                    id: Id::String(id.to_owned()),
                                },
                            )
                            .await?;
                        println!("Daemon response:\n{resp:#?}");
                    }
                }
                Ok(())
            }
        }
    }
}

impl CommandHandler for super::PlaybackCommand {
    type Output = anyhow::Result<()>;

    async fn handle(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
    ) -> Self::Output {
        match self {
            Self::Toggle => {
                client.playback_toggle(ctx).await?;
                println!("Daemon response:\nplayback toggled");
                Ok(())
            }
            Self::Play => {
                client.playback_play(ctx).await?;
                println!("Daemon response:\nplayback started");
                Ok(())
            }
            Self::Pause => {
                client.playback_pause(ctx).await?;
                println!("Daemon response:\nplayback paused");
                Ok(())
            }
            Self::Stop => {
                client.playback_clear_player(ctx).await?;
                println!("Daemon response:\nplayback stopped");
                Ok(())
            }
            Self::Restart => {
                client.playback_restart(ctx).await?;
                println!("Daemon response:\nplayback restarted");
                Ok(())
            }
            Self::Next => {
                client.playback_next(ctx).await?;
                println!("Daemon response:\nnext track started");
                Ok(())
            }
            Self::Previous => {
                client.playback_skip_backward(ctx, 1).await?;
                println!("Daemon response:\nprevious track started");
                Ok(())
            }
            Self::Seek { command } => command.handle(ctx, client).await,
            Self::Volume { command } => command.handle(ctx, client).await,
            Self::Repeat { mode } => {
                let mode: mecomp_core::state::RepeatMode = (*mode).into();
                client.playback_repeat(ctx, mode).await?;
                println!("Daemon response:\nrepeat mode set to {mode}");
                Ok(())
            }
            Self::Shuffle => {
                client.playback_shuffle(ctx).await?;
                println!("Daemon response:\nqueue shuffled");
                Ok(())
            }
        }
    }
}

impl CommandHandler for SeekCommand {
    type Output = anyhow::Result<()>;

    async fn handle(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
    ) -> Self::Output {
        match self {
            Self::Forward { amount } => {
                client
                    .playback_seek(ctx, SeekType::RelativeForwards, *amount)
                    .await?;
                println!("Daemon response:\nseeked forward by {amount}");
            }
            Self::Backward { amount } => {
                client
                    .playback_seek(ctx, SeekType::RelativeBackwards, *amount)
                    .await?;
                println!("Daemon response:\nseeked backward by {amount}");
            }
            Self::To { position } => {
                client
                    .playback_seek(ctx, SeekType::Absolute, *position)
                    .await?;
                println!("Daemon response:\nseeked to position {position}");
            }
        }
        Ok(())
    }
}

impl CommandHandler for VolumeCommand {
    type Output = anyhow::Result<()>;

    async fn handle(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
    ) -> Self::Output {
        match self {
            Self::Set { volume } => {
                client.playback_volume(ctx, *volume / 100.0).await?;
                println!("Daemon response:\nvolume set to {volume}");
                Ok(())
            }
            Self::Increase { amount } => {
                client.playback_volume_up(ctx, *amount / 100.0).await?;
                println!("Daemon response:\nvolume increased by {amount}");
                Ok(())
            }
            Self::Decrease { amount } => {
                client.playback_volume_down(ctx, *amount / 100.0).await?;
                println!("Daemon response:\nvolume decreased by {amount}");
                Ok(())
            }
            Self::Mute => {
                client.playback_mute(ctx).await?;
                println!("Daemon response:\nvolume muted");
                Ok(())
            }
            Self::Unmute => {
                client.playback_unmute(ctx).await?;
                println!("Daemon response:\nvolume unmuted");
                Ok(())
            }
        }
    }
}

impl CommandHandler for QueueCommand {
    type Output = anyhow::Result<()>;

    async fn handle(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
    ) -> Self::Output {
        match self {
            Self::Clear => {
                client.playback_clear(ctx).await?;
                println!("Daemon response:\nqueue cleared");
                Ok(())
            }
            Self::List => {
                let resp: Option<Box<[Song]>> = client.state_queue(ctx).await?;
                if let Some(songs) = resp {
                    println!(
                        "Daemon response:\n{}",
                        printing::song_list("Queue", &songs, true)?
                    );
                } else {
                    println!("Daemon response:\nNo queue available");
                }

                Ok(())
            }
            Self::Add { target, id } => {
                let message: &str = match target {
                    super::QueueAddTarget::Artist => client
                        .queue_add_artist(
                            ctx,
                            Thing {
                                tb: artist::TABLE_NAME.to_owned(),
                                id: Id::String(id.clone()),
                            },
                        )
                        .await?
                        .map(|()| "artist added to queue"),
                    super::QueueAddTarget::Album => client
                        .queue_add_album(
                            ctx,
                            Thing {
                                tb: album::TABLE_NAME.to_owned(),
                                id: Id::String(id.clone()),
                            },
                        )
                        .await?
                        .map(|()| "album added to queue"),
                    super::QueueAddTarget::Song => client
                        .queue_add_song(
                            ctx,
                            Thing {
                                tb: song::TABLE_NAME.to_owned(),
                                id: Id::String(id.clone()),
                            },
                        )
                        .await?
                        .map(|()| "song added to queue"),
                    super::QueueAddTarget::Playlist => client
                        .queue_add_playlist(
                            ctx,
                            Thing {
                                tb: playlist::TABLE_NAME.to_owned(),
                                id: Id::String(id.clone()),
                            },
                        )
                        .await?
                        .map(|()| "playlist added to queue"),
                    super::QueueAddTarget::Collection => client
                        .queue_add_collection(
                            ctx,
                            Thing {
                                tb: collection::TABLE_NAME.to_owned(),
                                id: Id::String(id.clone()),
                            },
                        )
                        .await?
                        .map(|()| "collection added to queue"),
                }?;

                println!("Daemon response:\n{message}");

                Ok(())
            }
            Self::Remove { start, end } => {
                client.queue_remove_range(ctx, *start..*end).await?;
                println!("Daemon response:\nitems removed from queue");
                Ok(())
            }
            Self::Set { index } => {
                client.queue_set_index(ctx, *index).await?;
                println!("Daemon response:\ncurrent song set to index {index}");
                Ok(())
            }
        }
    }
}

impl CommandHandler for super::PlaylistCommand {
    type Output = anyhow::Result<()>;

    #[allow(clippy::too_many_lines)]
    async fn handle(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
    ) -> Self::Output {
        match self {
            Self::List => {
                let resp: Box<[PlaylistBrief]> = client.playlist_list(ctx).await?;
                println!(
                    "Daemon response:\n{}",
                    printing::playlist_brief_list("Playlists", &resp)?
                );
                Ok(())
            }
            Self::Get { method, target } => {
                let resp: Option<Playlist> = match method {
                    PlaylistGetMethod::Id => {
                        client
                            .playlist_get(
                                ctx,
                                Thing {
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

                println!("Daemon response:\n{resp:#?}");
                Ok(())
            }
            Self::Create { name } => {
                let resp: Result<Thing, _> = client.playlist_new(ctx, name.clone()).await?;
                println!("Daemon response:\n{resp:#?}");
                Ok(())
            }
            Self::Delete { id } => {
                client
                    .playlist_remove(
                        ctx,
                        Thing {
                            tb: playlist::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                    )
                    .await??;
                println!("Daemon response:\nplaylist deleted");
                Ok(())
            }
            Self::Add {
                id,
                target,
                item_id,
            } => {
                let resp = match target {
                    PlaylistAddTarget::Artist => client
                        .playlist_add_artist(
                            ctx,
                            Thing {
                                tb: artist::TABLE_NAME.to_owned(),
                                id: Id::String(item_id.clone()),
                            },
                            Thing {
                                tb: playlist::TABLE_NAME.to_owned(),
                                id: Id::String(id.clone()),
                            },
                        )
                        .await?
                        .map(|()| "artist added to playlist"),
                    PlaylistAddTarget::Album => client
                        .playlist_add_album(
                            ctx,
                            Thing {
                                tb: album::TABLE_NAME.to_owned(),
                                id: Id::String(item_id.clone()),
                            },
                            Thing {
                                tb: playlist::TABLE_NAME.to_owned(),
                                id: Id::String(id.clone()),
                            },
                        )
                        .await?
                        .map(|()| "album added to playlist"),
                    PlaylistAddTarget::Song => client
                        .playlist_add_song(
                            ctx,
                            Thing {
                                tb: song::TABLE_NAME.to_owned(),
                                id: Id::String(item_id.clone()),
                            },
                            Thing {
                                tb: playlist::TABLE_NAME.to_owned(),
                                id: Id::String(id.clone()),
                            },
                        )
                        .await?
                        .map(|()| "song added to playlist"),
                };

                println!("Daemon response:\n{resp:?}");

                Ok(())
            }
            Self::Remove { id, item_ids } => {
                client
                    .playlist_remove_songs(
                        ctx,
                        Thing {
                            tb: playlist::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                        item_ids
                            .iter()
                            .map(|id| Thing {
                                tb: song::TABLE_NAME.to_owned(),
                                id: Id::String(id.clone()),
                            })
                            .collect(),
                    )
                    .await??;
                println!("Daemon response:\nsongs removed from playlist");

                Ok(())
            }
        }
    }
}

impl CommandHandler for super::CollectionCommand {
    type Output = anyhow::Result<()>;

    async fn handle(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
    ) -> Self::Output {
        match self {
            Self::List => {
                let resp: Box<[CollectionBrief]> = client.collection_list(ctx).await?;
                println!(
                    "Daemon response:\n{}",
                    printing::playlist_collection_list("Collections", &resp)?
                );
                Ok(())
            }
            Self::Get { id } => {
                let resp: Option<Collection> = client
                    .collection_get(
                        ctx,
                        Thing {
                            tb: collection::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                    )
                    .await?;
                println!("Daemon response:\n{resp:?}");
                Ok(())
            }
            Self::Recluster => {
                let resp: Result<&str, _> = client
                    .collection_recluster(ctx)
                    .await?
                    .map(|()| "reclustering started");
                println!("Daemon response:\n{resp:?}");
                Ok(())
            }
            Self::Freeze { id, name } => {
                let resp: Thing = client
                    .collection_freeze(
                        ctx,
                        Thing {
                            tb: collection::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                        name.to_owned(),
                    )
                    .await?;
                println!("Daemon response:\n{resp:?}");
                Ok(())
            }
        }
    }
}

impl CommandHandler for super::RadioCommand {
    type Output = anyhow::Result<()>;

    async fn handle(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
    ) -> Self::Output {
        match self {
            Self::Songs { id, n } => {
                let resp: Box<[Thing]> = client
                    .radio_get_similar_songs(
                        ctx,
                        Thing {
                            tb: artist::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                        *n,
                    )
                    .await?;
                println!("Daemon response:\n{resp:?}");
                Ok(())
            }
            Self::Artists { id, n } => {
                let resp: Box<[Thing]> = client
                    .radio_get_similar_artists(
                        ctx,
                        Thing {
                            tb: artist::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                        *n,
                    )
                    .await?;
                println!("Daemon response:\n{resp:?}");
                Ok(())
            }
            Self::Albums { id, n } => {
                let resp: Box<[Thing]> = client
                    .radio_get_similar_albums(
                        ctx,
                        Thing {
                            tb: artist::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                        *n,
                    )
                    .await?;
                println!("Daemon response:\n{resp:?}");
                Ok(())
            }
        }
    }
}