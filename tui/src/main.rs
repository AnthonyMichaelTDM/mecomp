use clap::{CommandFactory, Parser};
use mecomp_core::config::Settings;
#[cfg(feature = "autostart-daemon")]
use mecomp_core::is_server_running;
use mecomp_prost::init_client_with_retry;
use mecomp_tui::{
    Subscriber,
    state::Dispatcher,
    termination::{Interrupted, create_termination},
    ui::{UiManager, init_panic_hook},
};
use tokio::sync::mpsc;

/// Options configurable via the CLI.
#[derive(Debug, Parser)]
#[command(name = "mecomp-tui", version = env!("CARGO_PKG_VERSION"), about)]
struct Flags {
    /// Set the TCP port that the daemon is running on
    #[clap(
        long,
        value_hint = clap::ValueHint::Other
    )]
    port: Option<u16>,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> anyhow::Result<()> {
    clap_complete::CompleteEnv::with_factory(Flags::command).complete();
    init_panic_hook();

    let flags = Flags::parse();

    let config_file = Settings::get_config_path()?;
    let settings = Settings::init(config_file, flags.port, None)?;

    // initialize colors
    mecomp_tui::ui::colors::initialize_colors(settings.tui.colors.clone());

    // check if the server is running, and if it's not, try to start it
    #[cfg(feature = "autostart-daemon")]
    let server_process = MaybeDaemonHandler::start(settings.daemon.rpc_port).await?;

    // initialize the client
    let daemon = init_client_with_retry::<5, 1>(settings.daemon.rpc_port).await?;

    // initialize the signal handlers

    let (terminator, mut interrupt_rx) = create_termination();
    let (dispatcher, state_receivers) = Dispatcher::new();
    let (action_tx, action_rx) = mpsc::unbounded_channel();

    if let Err(e) = tokio::try_join!(
        dispatcher.main_loop(
            daemon.clone(),
            terminator,
            action_rx,
            interrupt_rx.resubscribe()
        ),
        UiManager::new(action_tx.clone()).main_loop(
            daemon.clone(),
            settings,
            state_receivers,
            interrupt_rx.resubscribe()
        ),
        Subscriber.main_loop(daemon, action_tx, interrupt_rx.resubscribe())
    ) {
        eprintln!("unexpected error: {e:?}");
    } else if let Ok(reason) = interrupt_rx.recv().await {
        match reason {
            Interrupted::UserInt => println!("exited per user request"),
            Interrupted::OsSigInt => println!("exited because of an os sig int"),
            Interrupted::OsSigTerm => println!("exited because of an os sig term"),
            Interrupted::OsSigQuit => println!("exited because of an os sig quit"),
        }
    } else {
        eprintln!("exited because of an unexpected error");
    }

    #[cfg(feature = "autostart-daemon")]
    drop(server_process);

    Ok(())
}

/// Handler for the Daemon process, which will be started if the Daemon is not already running on the given port.
///
/// Used so we can ensure that the Daemon is stopped when the TUI is stopped, by defining a Drop implementation.
#[cfg(feature = "autostart-daemon")]
struct MaybeDaemonHandler {
    process: Option<std::process::Child>,
}

#[cfg(feature = "autostart-daemon")]
impl MaybeDaemonHandler {
    /// Start the Daemon process if it is not already running on the given port.
    async fn start(port: u16) -> anyhow::Result<Self> {
        let process = if is_server_running(port) {
            None
        } else {
            // if mecomp-daemon is in the path, start it, otherwise look for it in the same directory as this binary
            eprintln!("starting mecomp-daemon");

            let mut child = std::process::Command::new("mecomp-daemon")
                .arg("--port")
                .arg(port.to_string())
                .stderr(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .spawn()
                .or_else(|_| {
                    let mut path = std::env::current_exe()?;
                    path.pop();
                    path.push("mecomp-daemon");

                    std::process::Command::new(path)
                        .arg("--port")
                        .arg(port.to_string())
                        .stderr(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .spawn()
                })
                .map_err(|e| anyhow::anyhow!("failed to start mecomp-daemon: {e}"))?;

            println!("waiting for the server to start");

            // give the server some time to start
            while !is_server_running(port) && child.try_wait()?.is_none() {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }

            Some(child)
        };
        Ok(Self { process })
    }
}

#[cfg(feature = "autostart-daemon")]
impl Drop for MaybeDaemonHandler {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            println!("killing the server process");
            if let Err(e) = process.kill() {
                eprintln!("couldn't kill the server process: {e:?}");
            }
        }
    }
}
