//! uplink-pty: PTY service for VSCode remote terminals
//!
//! Provides multi-terminal support over a Unix socket using MessagePack protocol
//! Wire format: [1 byte tag][4 byte length][MessagePack payload]

mod protocol;
mod terminal;

use protocol::*;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};

/// Start the PTY server, listening on the given Unix socket path
pub async fn run(socket_path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _ = std::fs::remove_file(socket_path);
    let listener = UnixListener::bind(socket_path)?;

    // Print to stdout for Node.js startup detection, then log via tracing
    println!("uplink-pty listening on {}", socket_path.display());
    info!(path = %socket_path.display(), "uplink-pty listening");

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                info!("Client connected");
                if let Err(e) = handle_client(stream).await {
                    error!(error = %e, "Client error");
                }
                info!("Client disconnected");
            }
            Err(e) => {
                error!(error = %e, "Accept error");
            }
        }
    }
}

/// Handle a single client connection
/// Spawns tasks for: PTY output forwarding, exit event forwarding, and request handling
async fn handle_client(stream: UnixStream) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    debug!("Setting up client handler");
    let (sock_read, sock_write) = stream.into_split();
    let sock_write = Arc::new(Mutex::new(sock_write));

    let registry = Arc::new(Mutex::new(terminal::TerminalRegistry::new()));

    // Channels for PTY events (output data and process exit)
    let (output_tx, mut output_rx) = mpsc::channel::<(u32, Vec<u8>)>(64);
    let (exit_tx, mut exit_rx) = mpsc::channel::<(u32, Option<i32>)>(16);

    // Forward PTY output to client as DataEvent messages
    let sock_write_clone = sock_write.clone();
    let output_task = tokio::spawn(async move {
        debug!("Output task started");
        while let Some((terminal_id, data)) = output_rx.recv().await {
            debug!(terminal_id, bytes = data.len(), "Sending PTY output");
            let event = DataEvent { terminal_id, data };
            if send_msg(&sock_write_clone, MSG_DATA, &event).await.is_err() {
                warn!("Output send failed, stopping output task");
                break;
            }
        }
        debug!("Output task ended");
    });

    // Forward PTY exit events to client as ExitEvent messages
    let sock_write_clone = sock_write.clone();
    let exit_task = tokio::spawn(async move {
        debug!("Exit task started");
        while let Some((terminal_id, code)) = exit_rx.recv().await {
            info!(terminal_id, code = ?code, "Terminal exited");
            let event = ExitEvent { terminal_id, code };
            let _ = send_msg(&sock_write_clone, MSG_EXIT, &event).await;
        }
        debug!("Exit task ended");
    });

    // Handle incoming requests from client
    let request_task = handle_requests(sock_read, sock_write.clone(), registry, output_tx, exit_tx);

    // Run all tasks concurrently, exit when any completes
    debug!("Starting select on tasks");
    tokio::select! {
        _ = output_task => { debug!("Output task completed"); },
        _ = exit_task => { debug!("Exit task completed"); },
        r = request_task => {
            debug!(result = ?r.is_ok(), "Request task completed");
            r?;
        },
    }

    Ok(())
}

/// Process incoming requests from the client
/// Dispatches to appropriate handler based on message tag
async fn handle_requests(
    mut sock_read: tokio::net::unix::OwnedReadHalf,
    sock_write: Arc<Mutex<tokio::net::unix::OwnedWriteHalf>>,
    registry: Arc<Mutex<terminal::TerminalRegistry>>,
    output_tx: mpsc::Sender<(u32, Vec<u8>)>,
    exit_tx: mpsc::Sender<(u32, Option<i32>)>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        // Wire format: [1 byte tag][4 byte length BE][payload]
        let mut tag = [0u8; 1];
        if sock_read.read_exact(&mut tag).await.is_err() {
            debug!("Client disconnected (read tag failed)");
            break; // Client disconnected
        }

        let mut len_buf = [0u8; 4];
        if let Err(e) = sock_read.read_exact(&mut len_buf).await {
            error!(error = %e, "Failed to read message length");
            break;
        }
        let len = u32::from_be_bytes(len_buf) as usize;

        let mut msg_buf = vec![0u8; len];
        if let Err(e) = sock_read.read_exact(&mut msg_buf).await {
            error!(error = %e, len, "Failed to read message body");
            break;
        }

        debug!(tag = tag[0], len, "Received message");

        match tag[0] {
            MSG_CREATE => {
                let req: CreateRequest = match rmp_serde::from_slice(&msg_buf) {
                    Ok(r) => r,
                    Err(e) => {
                        error!(error = %e, "Failed to decode CreateRequest");
                        continue;
                    }
                };
                info!(id = req.id, shell = %req.shell, cwd = %req.cwd, "Creating terminal");
                let mut reg = registry.lock().await;
                match reg.create(&req.shell, &req.args, &req.cwd, &req.env, req.cols, req.rows, output_tx.clone(), exit_tx.clone()) {
                    Ok((terminal_id, pid)) => {
                        info!(terminal_id, pid, "Terminal created");
                        let resp = CreatedResponse { id: req.id, terminal_id, pid };
                        send_msg(&sock_write, MSG_CREATED, &resp).await?;
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to create terminal");
                        let resp = ErrorResponse { id: req.id, message: e.to_string() };
                        send_msg(&sock_write, MSG_ERROR, &resp).await?;
                    }
                }
            }
            MSG_INPUT => {
                let req: InputRequest = match rmp_serde::from_slice(&msg_buf) {
                    Ok(r) => r,
                    Err(e) => {
                        error!(error = %e, "Failed to decode InputRequest");
                        continue;
                    }
                };
                debug!(terminal_id = req.terminal_id, bytes = req.data.len(), "Input");
                let mut reg = registry.lock().await;
                if let Some(term) = reg.get_mut(req.terminal_id) {
                    if let Err(e) = term.write(&req.data) {
                        warn!(error = %e, "Write to PTY failed");
                    }
                } else {
                    warn!(terminal_id = req.terminal_id, "Terminal not found for input");
                }
                let resp = OkResponse { id: req.id };
                send_msg(&sock_write, MSG_OK, &resp).await?;
            }
            MSG_RESIZE => {
                let req: ResizeRequest = match rmp_serde::from_slice(&msg_buf) {
                    Ok(r) => r,
                    Err(e) => {
                        error!(error = %e, "Failed to decode ResizeRequest");
                        continue;
                    }
                };
                debug!(terminal_id = req.terminal_id, cols = req.cols, rows = req.rows, "Resize");
                let reg = registry.lock().await;
                if let Some(term) = reg.terminals.get(&req.terminal_id) {
                    if let Err(e) = term.resize(req.cols, req.rows) {
                        warn!(error = %e, "Resize failed");
                    }
                }
                let resp = OkResponse { id: req.id };
                send_msg(&sock_write, MSG_OK, &resp).await?;
            }
            MSG_KILL => {
                let req: KillRequest = match rmp_serde::from_slice(&msg_buf) {
                    Ok(r) => r,
                    Err(e) => {
                        error!(error = %e, "Failed to decode KillRequest");
                        continue;
                    }
                };
                info!(terminal_id = req.terminal_id, "Killing terminal");
                let mut reg = registry.lock().await;
                reg.remove(req.terminal_id);
                let resp = OkResponse { id: req.id };
                send_msg(&sock_write, MSG_OK, &resp).await?;
            }
            _ => {
                warn!(tag = tag[0], "Unknown message type");
                let resp = ErrorResponse { id: 0, message: "unknown message type".into() };
                send_msg(&sock_write, MSG_ERROR, &resp).await?;
            }
        }
    }
    Ok(())
}

/// Send a tagged MessagePack message to the client
/// Returns a specific error type to allow callers to handle write failures appropriately
async fn send_msg<T: serde::Serialize>(
    sock: &Arc<Mutex<tokio::net::unix::OwnedWriteHalf>>,
    tag: u8,
    msg: &T,
) -> Result<(), SendError> {
    let data = rmp_serde::to_vec_named(msg).map_err(|e| SendError::Serialize(e.to_string()))?;
    debug!(tag, len = data.len(), "Sending message");
    let mut sock = sock.lock().await;
    sock.write_all(&[tag]).await.map_err(|e| SendError::Write(e.to_string()))?;
    sock.write_all(&(data.len() as u32).to_be_bytes()).await.map_err(|e| SendError::Write(e.to_string()))?;
    sock.write_all(&data).await.map_err(|e| SendError::Write(e.to_string()))?;
    Ok(())
}

#[derive(Debug)]
enum SendError {
    Serialize(String),
    Write(String),
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendError::Serialize(e) => write!(f, "serialization failed: {}", e),
            SendError::Write(e) => write!(f, "socket write failed: {}", e),
        }
    }
}

impl std::error::Error for SendError {}

// SendError is Send + Sync because it only contains String which is Send + Sync
unsafe impl Send for SendError {}
unsafe impl Sync for SendError {}
