//----------------------------------------------------------------------------------------- std lib
use std::net::SocketAddr;
//--------------------------------------------------------------------------------- other libraries
use tarpc::context;
//-------------------------------------------------------------------------------- MECOMP libraries
use mecomp_core::rpc::MusicPlayer;

#[derive(Clone)]
pub struct MusicPlayerServer(pub SocketAddr);

impl MusicPlayer for MusicPlayerServer {
    async fn ping(self, _: context::Context) -> String {
        "pong".to_string()
    }
}
