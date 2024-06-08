use clap::Parser;

mod handlers;

use handlers::CommandHandler;

/// Options configurable via the CLI.
#[derive(Debug, Parser)]
#[command(name = "mecomp-cli", version = env!("CARGO_PKG_VERSION"), about)]
struct Flags {
    /// Sets the port number to listen on.
    #[clap(long, default_value = "6600")]
    port: u16,
    /// subcommand to run
    #[clap(subcommand)]
    subcommand: Option<handlers::Command>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let flags = Flags::parse();

    let client = mecomp_core::rpc::init_client(flags.port).await?;

    let ctx = tarpc::context::current();

    if let Some(command) = flags.subcommand {
        command.handle(ctx, client).await?;
    } else {
        eprintln!("No subcommand provided");
    }

    Ok(())
}
