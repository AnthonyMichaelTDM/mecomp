//! This module contains the service definitions.

#![allow(clippy::future_not_send)]

#[tarpc::service]
pub trait Rpc {
    // misc
    async fn ping() -> String;

    // player
    // ...

    // library
    // ...
}
