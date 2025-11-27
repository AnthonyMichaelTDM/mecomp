use clap::{
    CommandFactory, Parser, ValueHint,
    builder::{PossibleValuesParser, TypedValueParser},
};
use log::LevelFilter;

use mecomp_core::logger::init_logger;
use mecomp_mpris::{Mpris, Subscriber};

/// Options configurable via the CLI.
#[derive(Debug, Parser)]
#[command(name = "mecomp-mpris", version = env!("CARGO_PKG_VERSION"), about)]
struct Flags {
    /// Set the TCP port that the daemon is running on
    #[clap(
        long,
        default_value = "6600",
        value_hint = ValueHint::Other,
    )]
    port: u16,
    /// Set the log level
    #[clap(
        long,
        default_value = "info",
        value_parser = PossibleValuesParser::new([ "off", "trace", "debug", "info", "warn", "error"])
        .map(|s| s.parse::<LevelFilter>().unwrap())
    )]
    log_level: LevelFilter,
}

#[tokio::main()]
async fn main() {
    clap_complete::CompleteEnv::with_factory(Flags::command).complete();

    // parse the CLI flags
    let flags = Flags::parse();

    // initialize the logger
    init_logger(flags.log_level, None);

    // connect to the daemon
    let Ok(daemon) = mecomp_prost::init_client_with_retry::<5, 1>(flags.port)
        .await
        .inspect_err(|e| log::error!("Failed to connect to daemon: {e}"))
    else {
        return;
    };

    // create the Mpris instance
    let mpris = Mpris::new_with_daemon(daemon);

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
