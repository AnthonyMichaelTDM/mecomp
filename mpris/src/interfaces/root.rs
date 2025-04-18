//! Implements the root interface of the MPRIS specification.
//!
//! [org.mpris.MediaPlayer2](https://specifications.freedesktop.org/mpris-spec/latest/Media_Player.html)

use mpris_server::{
    RootInterface,
    zbus::{Error as ZbusError, fdo},
};
use tarpc::context::Context;

use crate::Mpris;

pub const SUPPORTED_MIME_TYPES: [&str; 4] = ["audio/mp3", "audio/wav", "audio/ogg", "audio/flac"];

impl RootInterface for Mpris {
    async fn raise(&self) -> fdo::Result<()> {
        Ok(())
    }

    async fn quit(&self) -> fdo::Result<()> {
        let ctx = Context::current();
        let daemon_read_lock = self.daemon().await;
        if let Some(daemon) = daemon_read_lock.as_ref() {
            daemon
                .daemon_shutdown(ctx)
                .await
                .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        }
        drop(daemon_read_lock);

        Ok(())
    }

    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _: bool) -> Result<(), ZbusError> {
        Err(ZbusError::Unsupported)
    }

    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_raise(&self) -> fdo::Result<bool> {
        // TODO: Maybe in the future we can implement raising the player by starting the GUI.
        Ok(false)
    }

    async fn has_track_list(&self) -> fdo::Result<bool> {
        // TODO: when we implement the track list interface, we should return true here.
        Ok(false)
    }

    async fn identity(&self) -> fdo::Result<String> {
        Ok("MECOMP Music Player".to_string())
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        // TODO: bundle a desktop entry with the application so we can support this
        Err(fdo::Error::Failed("Desktop entry not found".to_string()))
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["file".to_string()])
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(SUPPORTED_MIME_TYPES
            .iter()
            .map(ToString::to_string)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    #[tokio::test]
    /// """
    /// The media player may be unable to control how its user interface is displayed, or it may not have a graphical user interface at all.
    /// In this case, the [CanRaise] property is false and this method does nothing.
    /// """
    ///
    /// Mecomp does not have a graphical user interface, so it does not support raising.
    async fn raise() {
        let mpris = Mpris::new(0);
        let result = mpris.can_raise().await;
        assert_eq!(result, Ok(false));
        let result = mpris.raise().await;
        assert_eq!(result, Ok(()));
    }

    #[tokio::test]
    /// """
    /// The media player may refuse to allow clients to shut it down. In this case, the [CanQuit] property is false and this method does nothing.
    /// Note: Media players which can be D-Bus activated, or for which there is no sensibly easy way to terminate a running instance
    /// (via the main interface or a notification area icon for example) should allow clients to use this method. Otherwise, it should not be needed.
    /// """
    ///
    /// Mecomp allows clients to shut it down.
    async fn quit() {
        let mpris = Mpris::new(0);
        let result = mpris.can_quit().await;
        assert_eq!(result, Ok(true));
        // this is safe to do since there is no daemon running.
        let result = mpris.quit().await;
        assert_eq!(result, Ok(()));
    }

    #[tokio::test]
    /// """
    /// If false, attempting to set [Fullscreen] will have no effect, and may raise an error.
    /// If true, attempting to set [Fullscreen] will not raise an error, and (if it is different from the current value)
    /// will cause the media player to attempt to enter or exit fullscreen mode.
    ///
    /// Note that the media player may be unable to fulfil the request. In this case, the value will not change.
    /// If the media player knows in advance that it will not be able to fulfil the request, however, this property should be false.
    /// """
    /// Mecomp does not support fullscreen mode.
    async fn fullscreen() {
        let mpris = Mpris::new(0);
        let result = mpris.fullscreen().await;
        assert_eq!(result, Ok(false));
        let result = mpris.can_set_fullscreen().await;
        assert_eq!(result, Ok(false));
        let result = mpris.set_fullscreen(true).await;
        assert_eq!(result, Err(ZbusError::Unsupported));
    }

    #[tokio::test]
    /// """
    /// Indicates whether the /org/mpris/MediaPlayer2 object implements the [TrackList interface].
    /// """
    ///
    /// Mecomp currently does not implement the TrackList interface.
    async fn has_track_list() {
        let mpris = Mpris::new(0);
        let result = mpris.has_track_list().await;
        assert_eq!(result, Ok(false));
    }

    #[tokio::test]
    /// """
    /// A friendly name to identify the media player to users (eg: "VLC media player").
    /// """
    ///
    /// Mecomp identifies itself as "MECOMP Music Player".
    async fn identity() {
        let mpris = Mpris::new(0);
        let result = mpris.identity().await;
        assert_eq!(result, Ok("MECOMP Music Player".to_string()));
    }

    #[tokio::test]
    /// """
    /// The desktop entry file as described in the [Desktop Entry Specification](https://specifications.freedesktop.org/desktop-entry-spec/latest/).
    /// """
    ///
    /// Mecomp currently doesn't have a desktop app, so it does not make sense to return a desktop entry.
    /// TODO: Once I've implemented the GUI, it should ship with a desktop entry that can be returned here.
    async fn desktop_entry() {
        let mpris = Mpris::new(0);
        let result = mpris.desktop_entry().await;
        assert_eq!(
            result,
            Err(fdo::Error::Failed("Desktop entry not found".to_string()))
        );
    }

    #[tokio::test]
    /// """
    /// The URI schemes supported by the media player.
    /// """
    ///
    /// Mecomp can only play files from the local filesystem, so it supports the "file" URI scheme.
    async fn supported_uri_schemes() {
        let mpris = Mpris::new(0);
        let result = mpris.supported_uri_schemes().await;
        assert_eq!(result, Ok(vec!["file".to_string()]));
    }

    #[tokio::test]
    /// """
    /// The mime-types supported by the media player.
    /// """
    ///
    /// Mecomp can play anything that it can decode, so mime-types supported by both the [lofty-rs](https://crates.io/crates/lofty) and [rodio](https://0crates.io/crates/rodio) crates are supported.
    /// So, mp3, wav, ogg (vorbis), and flac
    async fn supported_mime_types() {
        let mpris = Mpris::new(0);
        let result = mpris.supported_mime_types().await;
        assert_eq!(
            result,
            Ok(SUPPORTED_MIME_TYPES
                .iter()
                .map(ToString::to_string)
                .collect())
        );
    }
}
