// #![deny(clippy::missing_inline_in_public_items)]
#![allow(clippy::missing_errors_doc)]

pub mod db;
pub mod errors;
pub mod util;

#[cfg(any(feature = "test_utils", test))]
pub mod test_utils;

/// Type alias for a `SurrealDB` Database Connection, by default using the local `Db` engine
pub type Surreal<C = surrealdb::engine::local::Db> = surrealdb::Surreal<C>;

// Re-exports
pub use surrealdb::Connection;
