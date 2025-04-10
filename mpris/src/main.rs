use clap::Parser;
use log::LevelFilter;

use mecomp_core::logger::init_logger;
use mecomp_mpris::{Mpris, Subscriber};

/// Options configurable via the CLI.
#[derive(Debug, Parser)]
#[command(name = "mecomp-mpris", version = env!("CARGO_PKG_VERSION"), about)]
struct Flags {
    /// Sets the port number to listen on.
    #[clap(long, default_value = "6600")]
    port: u16,
    /// Set the log level.
    #[clap(long, default_value = "info")]
    log_level: LevelFilter,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    // parse the CLI flags
    let flags = Flags::parse();

    // initialize the logger
    init_logger(flags.log_level, None);

    // create a new Mpris instance
    let mpris = Mpris::new(flags.port);

    // connect to the daemon
    if let Err(e) = mpris.connect_with_retry().await {
        log::error!("Failed to connect to daemon: {e}");
        return;
    }

    let bus_name_suffix = format!("mecomp.mpris.port{}.pid{}", mpris.port, std::process::id());

    // start the Mpris server
    let server = match mpris.start_server(&bus_name_suffix).await {
        Ok(server) => server,
        Err(e) => {
            log::error!("Failed to start Mpris server: {e}");
            return;
        }
    };

    // start the event loop (listens for UDP events from the daemon forever)
    if let Err(e) = Subscriber.main_loop(&server).await {
        log::error!("Failed to start subscriber: {e}");
        return;
    }
    // std::future::pending::<()>().await;
}
