//! This module contains the service definitions.

#[tarpc::service]
pub trait Rpc {
    // misc
    async fn ping() -> String;

    // player
    // ...

    // library
    // ...
}
