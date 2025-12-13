use clap::{CommandFactory, Parser};

mod handlers;

use handlers::{CommandHandler, utils::WriteAdapter};

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

#[cfg(not(tarpaulin_include))]
fn main() -> anyhow::Result<()> {
    clap_complete::CompleteEnv::with_factory(Flags::command).complete();

    let flags = Flags::parse();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let client = mecomp_prost::init_client(flags.port).await?;

        let mut stdout_adapter = WriteAdapter(std::io::stdout());
        let mut stderr_adapter = WriteAdapter(std::io::stderr());

        if let Some(command) = flags.subcommand {
            command
                .handle(
                    client,
                    &mut stdout_adapter,
                    &mut stderr_adapter,
                    &std::io::stdin(),
                )
                .await?;
        } else {
            eprintln!("No subcommand provided");
        }

        Ok(())
    })
}
