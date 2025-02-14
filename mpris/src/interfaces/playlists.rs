//! Implements the playlists interface of the MPRIS specification.
//!
//! [org.mpris.MediaPlayer2.Playlists](https://specifications.freedesktop.org/mpris-spec/latest/Playlists_Interface.html)

use mpris_server::{
    zbus::{fdo, Error as ZbusError},
    Playlist, PlaylistId, PlaylistOrdering, PlaylistsInterface,
};
use tarpc::context::Context;

use crate::Mpris;

impl PlaylistsInterface for Mpris {
    async fn activate_playlist(&self, playlist_id: PlaylistId) -> fdo::Result<()> {
        todo!()
    }

    async fn get_playlists(
        &self,
        index: u32,
        max_count: u32,
        order: PlaylistOrdering,
        reverse_order: bool,
    ) -> fdo::Result<Vec<Playlist>> {
        todo!()
    }

    async fn playlist_count(&self) -> fdo::Result<u32> {
        todo!()
    }

    async fn orderings(&self) -> fdo::Result<Vec<PlaylistOrdering>> {
        Ok(vec![PlaylistOrdering::CreationDate])
    }

    async fn active_playlist(&self) -> fdo::Result<Option<Playlist>> {
        // NOTE: MECOMP doesn't have a concept of an active playlist, playlists are just collections of songs and
        // they are added to the queue just like any other collection (e.g. album, artist, genre, etc.)
        Ok(None)
    }
}
