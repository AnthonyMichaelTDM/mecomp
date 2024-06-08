use super::{
    Command, CommandHandler, CurrentTarget, LibraryCommand, LibraryGetTarget, LibraryListTarget,
    PlaylistAddTarget, PlaylistGetMethod, QueueCommand, RandTarget, SearchTarget, SeekCommand,
    VolumeCommand,
};

use mecomp_core::state::SeekType;
use mecomp_storage::db::schemas::{album, artist, collection, playlist, song, Id, Thing};

impl CommandHandler for Command {
    type Output = anyhow::Result<()>;

    async fn handle(
        &self,
        ctx: tarpc::context::Context,
        client: mecomp_core::rpc::MusicPlayerClient,
    ) -> Self::Output {
        match self {
            Self::Ping => {
                let resp = client.ping(ctx).await?;
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
                let resp = client.state_audio(ctx).await?;
                println!("Daemon response:\n{resp:?}");
                Ok(())
            }
            Self::Current { target } => {
                match target {
                    CurrentTarget::Artist => {
                        println!("Daemon response:\n{:?}", client.current_artist(ctx).await?);
                    }
                    CurrentTarget::Album => {
                        println!("Daemon response:\n{:?}", client.current_album(ctx).await?);
                    }
                    CurrentTarget::Song => {
                        println!("Daemon response:\n{:?}", client.current_song(ctx).await?);
                    }
                }
                Ok(())
            }
            Self::Rand { target } => {
                match target {
                    RandTarget::Artist => {
                        println!("Daemon response:\n{:?}", client.rand_artist(ctx).await?);
                    }
                    RandTarget::Album => {
                        println!("Daemon response:\n{:?}", client.rand_album(ctx).await?);
                    }
                    RandTarget::Song => {
                        println!("Daemon response:\n{:?}", client.rand_song(ctx).await?);
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
                        println!(
                            "Daemon response:\n{:?}",
                            client.search_artist(ctx, query.clone()).await?
                        );
                    }
                    Some(SearchTarget::Album) => {
                        println!(
                            "Daemon response:\n{:?}",
                            client.search_album(ctx, query.clone()).await?
                        );
                    }
                    Some(SearchTarget::Song) => {
                        println!(
                            "Daemon response:\n{:?}",
                            client.search_song(ctx, query.clone()).await?
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
                let resp = client.library_rescan(ctx).await?;
                if let Err(e) = resp {
                    println!("Daemon response:\n{e}");
                } else {
                    println!("Daemon response:\nLibrary rescan started");
                }
                Ok(())
            }
            Self::Brief => {
                let resp = client.library_brief(ctx).await?;
                println!("Daemon response:\n{resp:?}");
                Ok(())
            }
            Self::Full => {
                let resp = client.library_full(ctx).await?;
                println!("Daemon response:\n{resp:?}");
                Ok(())
            }
            Self::Health => {
                let resp = client.library_health(ctx).await?;
                println!("Daemon response:\n{resp:?}");
                Ok(())
            }
            Self::List { full, target } => {
                if *full {
                    match target {
                        LibraryListTarget::Artists => {
                            println!(
                                "Daemon response:\n{:?}",
                                client.library_artists_full(ctx).await?
                            );
                        }
                        LibraryListTarget::Albums => {
                            println!(
                                "Daemon response:\n{:?}",
                                client.library_albums_full(ctx).await?
                            );
                        }
                        LibraryListTarget::Songs => {
                            println!(
                                "Daemon response:\n{:?}",
                                client.library_songs_full(ctx).await?
                            );
                        }
                    }
                } else {
                    match target {
                        LibraryListTarget::Artists => {
                            println!(
                                "Daemon response:\n{:?}",
                                client.library_artists_brief(ctx).await?
                            );
                        }
                        LibraryListTarget::Albums => {
                            println!(
                                "Daemon response:\n{:?}",
                                client.library_albums_brief(ctx).await?
                            );
                        }
                        LibraryListTarget::Songs => {
                            println!(
                                "Daemon response:\n{:?}",
                                client.library_songs_brief(ctx).await?
                            );
                        }
                    }
                }

                Ok(())
            }
            Self::Get { target, id } => {
                match target {
                    LibraryGetTarget::Artist => {
                        println!(
                            "Daemon response:\n{:?}",
                            client
                                .library_artist_get(
                                    ctx,
                                    Thing {
                                        tb: artist::TABLE_NAME.to_owned(),
                                        id: Id::String(id.to_owned())
                                    }
                                )
                                .await?
                        );
                    }
                    LibraryGetTarget::Album => {
                        println!(
                            "Daemon response:\n{:?}",
                            client
                                .library_album_get(
                                    ctx,
                                    Thing {
                                        tb: album::TABLE_NAME.to_owned(),
                                        id: Id::String(id.to_owned())
                                    }
                                )
                                .await?
                        );
                    }
                    LibraryGetTarget::Song => {
                        println!(
                            "Daemon response:\n{:?}",
                            client
                                .library_song_get(
                                    ctx,
                                    Thing {
                                        tb: song::TABLE_NAME.to_owned(),
                                        id: Id::String(id.to_owned())
                                    }
                                )
                                .await?
                        );
                    }
                    LibraryGetTarget::Playlist => {
                        println!(
                            "Daemon response:\n{:?}",
                            client
                                .library_playlist_get(
                                    ctx,
                                    Thing {
                                        tb: playlist::TABLE_NAME.to_owned(),
                                        id: Id::String(id.to_owned())
                                    }
                                )
                                .await?
                        );
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
                let resp = client.state_queue(ctx).await?;
                println!("Daemon response:\n{resp:?}");
                Ok(())
            }
            Self::Add { target, id } => {
                let resp = match target {
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
                };

                println!("Daemon response:\n{resp:?}");

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
                let resp = client.playlist_list(ctx).await?;
                println!("Daemon response:\n{resp:?}");
                Ok(())
            }
            Self::Get { method, target } => {
                let resp = match method {
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

                println!("Daemon response:\n{resp:?}");
                Ok(())
            }
            Self::Create { name } => {
                let resp = client.playlist_new(ctx, name.clone()).await?;
                println!("Daemon response:\n{resp:?}");
                Ok(())
            }
            Self::Delete { id } => {
                let resp = client
                    .playlist_remove(
                        ctx,
                        Thing {
                            tb: playlist::TABLE_NAME.to_owned(),
                            id: Id::String(id.clone()),
                        },
                    )
                    .await?;
                println!("Daemon response:\n{resp:?}");
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
                let resp = client
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
                    .await?;
                println!("Daemon response:\n{resp:?}");

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
                let resp = client.collection_list(ctx).await?;
                println!("Daemon response:\n{resp:?}");
                Ok(())
            }
            Self::Get { id } => {
                let resp = client
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
                let resp = client
                    .collection_recluster(ctx)
                    .await?
                    .map(|()| "reclustering started");
                println!("Daemon response:\n{resp:?}");
                Ok(())
            }
            Self::Freeze { id, name } => {
                let resp = client
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
                let resp = client
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
                let resp = client
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
                let resp = client
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
