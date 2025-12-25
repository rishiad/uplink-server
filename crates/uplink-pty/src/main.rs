use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let socket_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/uplink-pty.sock"));

    if let Err(e) = uplink_pty::run(&socket_path).await {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
