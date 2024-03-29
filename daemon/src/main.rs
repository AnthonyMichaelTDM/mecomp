//-------------------------------------------------------------------------------- MECOMP libraries
use mecomp_daemon::{config::SETTINGS, start_daemon};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    start_daemon(log::LevelFilter::Debug, SETTINGS.as_ref()).await
}
