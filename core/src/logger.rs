//----------------------------------------------------------------------------------------- std lib
use std::io::Write;
use std::time::Instant;
//--------------------------------------------------------------------------------- other libraries
use log::info;
use once_cell::sync::Lazy;

// This will get initialized below.
/// Returns the init [`Instant`]
pub static INIT_INSTANT: Lazy<Instant> = Lazy::new(Instant::now);

/// Returns the seconds since [`INIT_INSTANT`].
pub fn uptime() -> u64 {
    INIT_INSTANT.elapsed().as_secs()
}

//---------------------------------------------------------------------------------------------------- Logger init function
#[allow(clippy::module_name_repetitions)]
/// Initializes the logger.
///
/// This enables console logging on all the internals of `Mecomp`.
///
/// Functionality is provided by [`log`].
///
/// The levels are:
/// - ERROR
/// - WARN
/// - INFO
/// - DEBUG
/// - TRACE
///
/// # Panics
/// This must only be called _once_.
pub fn init_logger(filter: log::LevelFilter) {
    // Initialize timer.
    let now = Lazy::force(&INIT_INSTANT);

    // If `RUST_LOG` isn't set, override it and disables
    // all library crate logs except for mecomp and its sub-crates.
    let mut env = String::new();
    #[allow(clippy::option_if_let_else)]
    match std::env::var("RUST_LOG") {
        Ok(e) => {
            std::env::set_var("RUST_LOG", &e);
            env = e;
        }
        // SOMEDAY:
        // Support frontend names without *mecomp*.
        _ => std::env::set_var("RUST_LOG", format!("off,mecomp={filter}")),
    }

    env_logger::Builder::new()
        .format(move |buf, record| {
            let mut style = buf.style();
            let level = match record.level() {
                log::Level::Debug => {
                    style.set_color(env_logger::fmt::Color::Blue);
                    "D"
                }
                log::Level::Trace => {
                    style.set_color(env_logger::fmt::Color::Magenta);
                    "T"
                }
                log::Level::Info => {
                    style.set_color(env_logger::fmt::Color::White);
                    "I"
                }
                log::Level::Warn => {
                    style.set_color(env_logger::fmt::Color::Yellow);
                    "W"
                }
                log::Level::Error => {
                    style.set_color(env_logger::fmt::Color::Red);
                    "E"
                }
            };
            writeln!(
                buf,
                // Longest PATH in the repo: `daemon/src/rpc_server/controller.rs`  - `35` characters
                // Longest file in the repo: `daemon/src/logger.rs`                 - `3` digits
                //
                // Use `utils/longest.sh` to find this.
                //
                //      Longest PATH ---|        |--- Longest file
                //                      |        |
                //                      v        v
                "| {} | {: >9.3} | {: >35} @ {: <3} | {}",
                style.set_bold(true).value(level),
                buf.style()
                    .set_dimmed(true)
                    .value(now.elapsed().as_secs_f32()),
                buf.style()
                    .set_dimmed(true)
                    .value(record.file_static().unwrap_or("???")),
                buf.style()
                    .set_dimmed(true)
                    .value(record.line().unwrap_or(0)),
                record.args(),
            )
        })
        .write_style(env_logger::WriteStyle::Always)
        .parse_default_env()
        .init();

    if env.is_empty() {
        info!("Log Level (Flag) ... {}", filter);
    } else {
        info!("Log Level (RUST_LOG) ... {}", env);
    }
}
