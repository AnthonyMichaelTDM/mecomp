mod state;
mod termination;
mod ui;

use std::sync::Arc;

use clap::Parser;
use mecomp_core::rpc::init_client;
use termination::{create_termination, Interrupted};
use ui::init_panic_hook;

/// Options configurable via the CLI.
#[derive(Debug, Parser)]
#[command(name = "mecomp-tui", version = env!("CARGO_PKG_VERSION"), about)]
struct Flags {
    /// Sets the port number to listen on.
    #[clap(long, default_value = "6600")]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_panic_hook();

    let flags = Flags::parse();

    let daemon = Arc::new(init_client(flags.port).await?);

    let (terminator, mut interrupt_rx) = create_termination();
    let (dispatcher, state_receivers) = state::Dispatcher::new();
    let (ui_manager, action_rx) = ui::UiManager::new();

    if let Err(e) = tokio::try_join!(
        dispatcher.main_loop(
            daemon.clone(),
            terminator,
            action_rx,
            interrupt_rx.resubscribe()
        ),
        ui_manager.main_loop(daemon, state_receivers, interrupt_rx.resubscribe())
    ) {
        panic!("unexpected error: {e:?}")
    }

    if let Ok(reason) = interrupt_rx.recv().await {
        match reason {
            Interrupted::UserInt => println!("exited per user request"),
            Interrupted::OsSigInt => println!("exited because of an os sig int"),
        }
    } else {
        println!("exited because of an unexpected error");
    }

    Ok(())
}
