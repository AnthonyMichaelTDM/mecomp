use clap::Parser;

mod handlers;

use handlers::{utils::WriteAdapter, CommandHandler};

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

#[tokio::main(flavor = "current_thread")]
#[cfg(not(tarpaulin_include))]
async fn main() -> anyhow::Result<()> {
    let flags = Flags::parse();

    let client = mecomp_core::rpc::init_client(flags.port).await?;

    let ctx = tarpc::context::current();

    let mut stdout_adapter = WriteAdapter(std::io::stdout());
    let mut stderr_adapter = WriteAdapter(std::io::stderr());

    if let Some(command) = flags.subcommand {
        command
            .handle(ctx, client, &mut stdout_adapter, &mut stderr_adapter)
            .await?;
    } else {
        eprintln!("No subcommand provided");
    }

    Ok(())
}
