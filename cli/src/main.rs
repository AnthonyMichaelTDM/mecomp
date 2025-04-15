use clap::{CommandFactory, Parser};

mod handlers;

use handlers::{utils::WriteAdapter, CommandHandler};

/// Options configurable via the CLI.
#[derive(Debug, Parser)]
#[command(name = "mecomp-cli", version = env!("CARGO_PKG_VERSION"), about)]
struct Flags {
    /// Set the TCP port that the daemon is running on
    #[clap(long, default_value = "6600", value_hint = clap::ValueHint::Other)]
    port: u16,
    /// subcommand to run
    #[clap(subcommand)]
    subcommand: Option<handlers::Command>,
}

#[test]
fn verify_cli() {
    Flags::command().debug_assert();
}

#[tokio::main(flavor = "current_thread")]
#[cfg(not(tarpaulin_include))]
async fn main() -> anyhow::Result<()> {
    clap_complete::CompleteEnv::with_factory(Flags::command).complete();

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
