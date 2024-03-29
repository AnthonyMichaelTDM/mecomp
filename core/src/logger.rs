//----------------------------------------------------------------------------------------- std lib
use std::io::Write;
use std::time::Instant;
//--------------------------------------------------------------------------------- other libraries
use log::info;
use once_cell::sync::Lazy;
#[cfg(feature = "otel_tracing")]
use opentelemetry::KeyValue;
#[cfg(feature = "otel_tracing")]
use opentelemetry_otlp::WithExportConfig as _;
#[cfg(feature = "otel_tracing")]
use opentelemetry_sdk::Resource;
use tracing_subscriber::layer::SubscriberExt as _;

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
                // Longest PATH in the repo: `daemon/src/services/library.rs`   - `30` characters
                // Longest file in the repo: `daemon/src/controller.rs`         - `3` digits
                //
                // Use `utils/longest.sh` to find this.
                //
                //      Longest PATH ---|        |--- Longest file
                //                      |        |
                //                      v        v
                "| {} | {: >9.3} | {: >30} @ {: <3} | {}",
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

pub fn init_tracing() -> impl tracing::Subscriber {
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
            opentelemetry_sdk::trace::config()
                .with_resource(Resource::new(vec![KeyValue::new(
                    opentelemetry_semantic_conventions::resource::SERVICE_NAME.as_ref(),
                    "mecomp-daemon",
                )]))
                .with_id_generator(opentelemetry_sdk::trace::RandomIdGenerator::default())
                .with_sampler(opentelemetry_sdk::trace::Sampler::AlwaysOn),
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)
        .expect("Failed to create tracing layer");

    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::EnvFilter::builder()
            .parse("off,mecomp=trace")
            .unwrap(),
    );

    #[cfg(feature = "otel_tracing")]
    let subscriber = subscriber.with(tracing_opentelemetry::layer().with_tracer(tracer));

    #[cfg(feature = "flame")]
    let (flame_layer, _guard) = tracing_flame::FlameLayer::with_file("tracing.folded").unwrap();
    #[cfg(feature = "flame")]
    let subscriber = subscriber.with(flame_layer);

    subscriber
}

pub fn shutdown_tracing() {
    opentelemetry::global::shutdown_tracer_provider();
}

// #[cfg(feature = "otel_tracing")]
// fn tracing_default(filter: tracing::Level) -> impl tracing::Subscriber {
//     // If `RUST_LOG` isn't set, override it and disables
//     // all library crate logs except for mecomp and its sub-crates.
//     let mut env = String::new();
//     #[allow(clippy::option_if_let_else)]
//     match std::env::var("RUST_LOG") {
//         Ok(e) => {
//             std::env::set_var("RUST_LOG", &e);
//             env = e;
//         }
//         // SOMEDAY:
//         // Support frontend names without *mecomp*.
//         _ => std::env::set_var("RUST_LOG", format!("off,mecomp={filter}")),
//     }

//     let subscriber = tracing_subscriber::fmt()
//         // configure formatting settings
//         .with_ansi(true)
//         .with_env_filter(EnvFilter::from_default_env())
//         .with_level(true)
//         .with_thread_ids(true)
//         .with_thread_names(true)
//         .with_file(true)
//         .with_line_number(true)
//         .with_timer(tracing_subscriber::fmt::time::uptime())
//         .with_span_events(FmtSpan::CLOSE)
//         .compact()
//         // build the subscriber
//         .finish();

//     if env.is_empty() {
//         info!("Log Level (Flag) ... {}", filter);
//     } else {
//         info!("Log Level (RUST_LOG) ... {}", env);
//     }

//     return subscriber;
// }
