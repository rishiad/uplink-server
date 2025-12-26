use std::path::PathBuf;
use tracing::{error, info};
use tracing_appender::rolling;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() {
    let log_dir = PathBuf::from("/tmp");
    let file_appender = rolling::never(&log_dir, "uplink-fs.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug")))
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .with(fmt::layer().with_writer(std::io::stderr))
        .init();

    info!("uplink-fs starting");

    let socket_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/uplink-fs.sock"));

    if let Err(e) = uplink_fs::run(&socket_path).await {
        error!(error = %e, "Fatal error");
        std::process::exit(1);
    }
}


