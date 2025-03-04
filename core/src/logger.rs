use std::io::Write;
use std::time::Instant;

use env_logger::fmt::style::{RgbColor, Style};
use log::info;
use once_cell::sync::Lazy;
#[cfg(feature = "otel_tracing")]
use opentelemetry::trace::TracerProvider as _;
#[cfg(feature = "otel_tracing")]
use opentelemetry_otlp::WithExportConfig as _;
#[cfg(feature = "otel_tracing")]
use opentelemetry_sdk::Resource;
#[cfg(any(feature = "otel_tracing", feature = "flame"))]
use tracing_subscriber::layer::SubscriberExt as _;
#[cfg(feature = "otel_tracing")]
use tracing_subscriber::Layer as _;

use crate::format_duration;

// This will get initialized below.
/// Returns the init [`Instant`]
pub static INIT_INSTANT: Lazy<Instant> = Lazy::new(Instant::now);

/// Returns the seconds since [`INIT_INSTANT`].
#[cfg(not(tarpaulin_include))]
#[inline]
pub fn uptime() -> u64 {
    INIT_INSTANT.elapsed().as_secs()
}

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
#[cfg(not(tarpaulin_include))]
#[allow(clippy::missing_inline_in_public_items)]
pub fn init_logger(filter: log::LevelFilter, log_file_path: Option<std::path::PathBuf>) {
    // Initialize timer.
    let now = Lazy::force(&INIT_INSTANT);

    // create a new log file (if enabled).
    let log_file = log_file_path.map(|path| {
        let path = path
            .is_dir()
            .then(|| path.join("mecomp.log"))
            .unwrap_or(path);

        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("Failed to create log file");

        log_file
    });

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
            let style = buf.default_level_style(record.level());
            let (level_style, level) = match record.level() {
                log::Level::Debug => (
                    style
                        .fg_color(Some(RgbColor::from((0, 0x80, 0x80)).into()))
                        .bold(),
                    "D",
                ),
                log::Level::Trace => (
                    style
                        .fg_color(Some(RgbColor::from((255, 0, 255)).into()))
                        .bold(),
                    "T",
                ),
                log::Level::Info => (
                    style
                        .fg_color(Some(RgbColor::from((255, 255, 255)).into()))
                        .bold(),
                    "I",
                ),
                log::Level::Warn => (
                    style
                        .fg_color(Some(RgbColor::from((255, 255, 0)).into()))
                        .bold(),
                    "W",
                ),
                log::Level::Error => (
                    style
                        .fg_color(Some(RgbColor::from((255, 0, 0)).into()))
                        .bold(),
                    "E",
                ),
            };

            let dimmed_style = Style::default().dimmed();

            let log_line = format!(
                // Longest PATH in the repo: `storage/src/db/schemas/dynamic/query.rs` - `39` characters
                // Longest file in the repo: `core/src/audio/mod.rs`                   - `4` digits
                //
                // Use `scripts/longest.sh` to find this.
                //
                //                                                                             Longest PATH ---|        |--- Longest file
                //                                                                                             |        |
                //                                                                                             v        v
                "| {level_style}{level}{level_style:#} | {dimmed_style}{}{dimmed_style:#} | {dimmed_style}{: >39} @ {: <4}{dimmed_style:#} | {}",
                format_duration(&now.elapsed()),
                process_file(record.file().unwrap_or("???")),
                record.line().unwrap_or(0),
                record.args(),
            );
            writeln!(buf, "{log_line}")?;

            // Write to log file (if enabled).
            if let Some(log_file) = &log_file {
                let mut log_file = log_file.try_clone().expect("Failed to clone log file");

                // Remove ANSI formatting from log line before writing to file.
                let unformatted_log_line: String = log_line
                    .replace(&level_style.render().to_string(), "")
                    .replace(&dimmed_style.render().to_string(), "")
                    .replace("\x1B[0m", "");

                writeln!(log_file, "{unformatted_log_line}")?;
                log_file.sync_all().expect("Failed to sync log file");
            }

            Ok(())
        })
        .write_style(env_logger::WriteStyle::Always)
        .parse_default_env()
        .init();

    if env.is_empty() {
        info!("Log Level (Flag) ... {filter}");
    } else {
        info!("Log Level (RUST_LOG) ... {env}");
    }
}

/// Sometimes the file paths we get are full file paths, in this case we don't care about anything before (and including) the `/mecomp/` part.
fn process_file(file: &str) -> &str {
    if file.contains("mecomp/") {
        file.split("mecomp/").last().unwrap_or(file)
    } else {
        file
    }
}

/// Initializes the tracing layer.
///
/// # Panics
///
/// panics if the tracing layers cannot be initialized.
#[must_use]
#[allow(clippy::missing_inline_in_public_items)]
pub fn init_tracing() -> impl tracing::Subscriber {
    let subscriber = tracing_subscriber::registry();

    #[cfg(feature = "flame")]
    let (flame_layer, _guard) = tracing_flame::FlameLayer::with_file("tracing.folded").unwrap();
    #[cfg(feature = "flame")]
    let subscriber = subscriber.with(flame_layer);

    #[cfg(not(feature = "verbose_tracing"))]
    #[allow(unused_variables)]
    let filter = tracing_subscriber::EnvFilter::builder()
        .parse("off,mecomp=trace")
        .unwrap();
    #[cfg(feature = "verbose_tracing")]
    #[allow(unused_variables)]
    let filter = tracing_subscriber::EnvFilter::new("trace")
        .add_directive("hyper=off".parse().unwrap())
        .add_directive("opentelemetry=off".parse().unwrap())
        .add_directive("tonic=off".parse().unwrap())
        .add_directive("h2=off".parse().unwrap())
        .add_directive("reqwest=off".parse().unwrap());

    #[cfg(feature = "otel_tracing")]
    std::env::set_var("OTEL_BSP_MAX_EXPORT_BATCH_SIZE", "12");
    #[cfg(feature = "otel_tracing")]
    let tracer = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_batch_exporter(
            opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                .with_endpoint("http://localhost:4317")
                .build()
                .expect("Failed to build OTLP exporter"),
        )
        .with_id_generator(opentelemetry_sdk::trace::RandomIdGenerator::default())
        .with_resource(Resource::builder().with_service_name("mecomp").build())
        .build()
        .tracer("mecomp");

    #[cfg(feature = "otel_tracing")]
    let subscriber = subscriber.with(
        tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_filter(filter),
    );

    subscriber
}
