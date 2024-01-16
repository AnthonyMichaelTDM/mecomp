use std::net::SocketAddr;

use tarpc::context;

use crate::definition::Rpc;

#[derive(Clone)]
pub struct RpcServer(pub SocketAddr);

impl Rpc for RpcServer {
    async fn ping(self, _: context::Context) -> String {
        "pong".to_string()
    }
}
