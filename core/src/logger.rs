//----------------------------------------------------------------------------------------- std lib
use std::io::Write;
use std::time::Instant;
//--------------------------------------------------------------------------------- other libraries
use log::info;
use once_cell::sync::Lazy;
#[cfg(feature = "otel_tracing")]
use opentelemetry::trace::TracerProvider as _;
#[cfg(feature = "otel_tracing")]
use opentelemetry::KeyValue;
#[cfg(feature = "otel_tracing")]
use opentelemetry_otlp::WithExportConfig as _;
#[cfg(feature = "otel_tracing")]
use opentelemetry_sdk::Resource;
#[cfg(any(feature = "otel_tracing", feature = "flame"))]
use tracing_subscriber::layer::SubscriberExt as _;
#[cfg(feature = "otel_tracing")]
use tracing_subscriber::Layer as _;

// This will get initialized below.
/// Returns the init [`Instant`]
pub static INIT_INSTANT: Lazy<Instant> = Lazy::new(Instant::now);

/// Returns the seconds since [`INIT_INSTANT`].
#[cfg(not(tarpaulin_include))]
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
#[cfg(not(tarpaulin_include))]
pub fn init_logger(filter: log::LevelFilter) {
    // Initialize timer.

    use crate::format_duration;
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
                // Longest PATH in the repo: `storage/src/db/schemas/collection.rs` - `36` characters
                // Longest file in the repo: `daemon/src/controller.rs`             - `4` digits
                //
                // Use `scripts/longest.sh` to find this.
                //
                //      Longest PATH ---|        |--- Longest file
                //                      |        |
                //                      v        v
                "| {} | {} | {: >36} @ {: <4} | {}",
                style.set_bold(true).value(level),
                buf.style()
                    .set_dimmed(true)
                    .value(format_duration(&now.elapsed())),
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

/// Initializes the tracing layer.
///
/// # Panics
///
/// panics if the tracing layers cannot be initialized.
#[must_use]
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
    let filter = tracing_subscriber::EnvFilter::builder()
        .parse("trace,h2=off")
        .unwrap();

    #[cfg(feature = "otel_tracing")]
    std::env::set_var("OTEL_BSP_MAX_EXPORT_BATCH_SIZE", "12");
    #[cfg(feature = "otel_tracing")]
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://localhost:4317"),
        )
        .with_trace_config(
            opentelemetry_sdk::trace::Config::default()
                .with_resource(Resource::new(vec![KeyValue::new(
                    opentelemetry_semantic_conventions::resource::SERVICE_NAME,
                    "mecomp-daemon",
                )]))
                .with_id_generator(opentelemetry_sdk::trace::RandomIdGenerator::default())
                .with_sampler(opentelemetry_sdk::trace::Sampler::AlwaysOn),
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)
        .expect("Failed to create tracing layer")
        .tracer("mecomp-daemon");

    #[cfg(feature = "otel_tracing")]
    let subscriber = subscriber.with(
        tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_filter(filter),
    );

    subscriber
}

#[allow(clippy::missing_const_for_fn)]
pub fn shutdown_tracing() {
    #[cfg(feature = "otel_tracing")]
    opentelemetry::global::shutdown_tracer_provider();
}
