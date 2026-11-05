//! utilitites used for testing
//!
//! NOTE: most of the stuff related to setting up database state is in the [`mecomp_storage`] crate
//! behind the `test_utils` feature flag.
#![allow(clippy::missing_inline_in_public_items)]

use std::sync::OnceLock;

use crate::logger::{init_logger, init_tracing};

static INIT: OnceLock<()> = OnceLock::new();

/// Initialize the logger and tracing subscriber.
///
/// # Panics
///
/// Panics if the logger or tracing subscriber cannot be initialized.
pub fn init() {
    INIT.get_or_init(|| {
        init_logger(log::LevelFilter::Warn, None);
        if let Err(e) = tracing::subscriber::set_global_default(init_tracing()) {
            panic!("Error setting global default tracing subscriber: {e:?}")
        }
    });
}

/// Generate a new song ID using the [`mecomp_storage::db::schemas::song::Song::generate_id`] method and convert it to a [`mecomp_prost::RecordId`].
#[must_use]
pub fn generate_song_id() -> mecomp_prost::RecordId {
    let id = mecomp_storage::db::schemas::song::Song::generate_id();
    mecomp_prost::RecordId::from(id)
}
