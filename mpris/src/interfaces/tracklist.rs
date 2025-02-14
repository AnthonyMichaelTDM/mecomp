//! Implements the tracklist interface of the MPRIS specification.
//!
//! [org.mpris.MediaPlayer2.TrackList](https://specifications.freedesktop.org/mpris-spec/latest/Track_List_Interface.html)

use mpris_server::{
    zbus::{fdo, Error as ZbusError},
    Metadata, TrackId, TrackListInterface, Uri,
};
use tarpc::context::Context;

use crate::Mpris;

impl TrackListInterface for Mpris {
    async fn get_tracks_metadata(&self, track_ids: Vec<TrackId>) -> fdo::Result<Vec<Metadata>> {
        todo!()
    }

    async fn add_track(
        &self,
        uri: Uri,
        after_track: TrackId,
        set_as_current: bool,
    ) -> fdo::Result<()> {
        todo!()
    }

    async fn remove_track(&self, track_id: TrackId) -> fdo::Result<()> {
        todo!()
    }

    async fn go_to(&self, track_id: TrackId) -> fdo::Result<()> {
        todo!()
    }

    async fn tracks(&self) -> fdo::Result<Vec<TrackId>> {
        todo!()
    }

    async fn can_edit_tracks(&self) -> fdo::Result<bool> {
        todo!()
    }
}
