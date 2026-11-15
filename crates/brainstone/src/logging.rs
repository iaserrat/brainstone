use tracing_subscriber::EnvFilter;

/// Installs JSON logging on stderr, filtered by `RUST_LOG` (default `info`).
pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .json()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .init();
}
