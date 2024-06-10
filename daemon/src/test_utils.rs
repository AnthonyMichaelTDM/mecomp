//! utilitites used for testing
//!
//! NOTE: most of the stuff related to setting up database state is in the `mecomp_storage` crate
//! behind the `test_utils` feature flag.

use std::sync::OnceLock;

use mecomp_core::logger::{init_logger, init_tracing};

static INIT: OnceLock<()> = OnceLock::new();

pub fn init() {
    INIT.get_or_init(|| {
        init_logger(log::LevelFilter::Debug);
        if let Err(e) = tracing::subscriber::set_global_default(init_tracing()) {
            panic!("Error setting global default tracing subscriber: {e:?}")
        }
    });
}
