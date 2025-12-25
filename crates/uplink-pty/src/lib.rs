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

/// Start the PTY server, listening on the given Unix socket path
pub async fn run(socket_path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _ = std::fs::remove_file(socket_path);
    let listener = UnixListener::bind(socket_path)?;

    println!("uplink-pty listening on {}", socket_path.display());

    loop {
        let (stream, _) = listener.accept().await?;
        println!("Client connected");
        if let Err(e) = handle_client(stream).await {
            eprintln!("Client error: {e}");
        }
        println!("Client disconnected");
    }
}

/// Handle a single client connection
/// Spawns tasks for: PTY output forwarding, exit event forwarding, and request handling
async fn handle_client(stream: UnixStream) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (sock_read, sock_write) = stream.into_split();
    let sock_write = Arc::new(Mutex::new(sock_write));

    let registry = Arc::new(Mutex::new(terminal::TerminalRegistry::new()));

    // Channels for PTY events (output data and process exit)
    let (output_tx, mut output_rx) = mpsc::channel::<(u32, Vec<u8>)>(64);
    let (exit_tx, mut exit_rx) = mpsc::channel::<(u32, Option<i32>)>(16);

    // Forward PTY output to client as DataEvent messages
    let sock_write_clone = sock_write.clone();
    let output_task = tokio::spawn(async move {
        while let Some((terminal_id, data)) = output_rx.recv().await {
            let event = DataEvent { terminal_id, data };
            if send_msg(&sock_write_clone, MSG_DATA, &event).await.is_err() {
                break;
            }
        }
    });

    // Forward PTY exit events to client as ExitEvent messages
    let sock_write_clone = sock_write.clone();
    let exit_task = tokio::spawn(async move {
        while let Some((terminal_id, code)) = exit_rx.recv().await {
            let event = ExitEvent { terminal_id, code };
            let _ = send_msg(&sock_write_clone, MSG_EXIT, &event).await;
        }
    });

    // Handle incoming requests from client
    let request_task = handle_requests(sock_read, sock_write.clone(), registry, output_tx, exit_tx);

    // Run all tasks concurrently, exit when any completes
    tokio::select! {
        _ = output_task => {},
        _ = exit_task => {},
        r = request_task => { r?; },
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
            break; // Client disconnected
        }

        let mut len_buf = [0u8; 4];
        sock_read.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;

        let mut msg_buf = vec![0u8; len];
        sock_read.read_exact(&mut msg_buf).await?;

        match tag[0] {
            MSG_CREATE => {
                let req: CreateRequest = rmp_serde::from_slice(&msg_buf)?;
                let mut reg = registry.lock().await;
                match reg.create(&req.shell, &req.args, &req.cwd, &req.env, req.cols, req.rows, output_tx.clone(), exit_tx.clone()) {
                    Ok((terminal_id, pid)) => {
                        let resp = CreatedResponse { id: req.id, terminal_id, pid };
                        send_msg(&sock_write, MSG_CREATED, &resp).await?;
                    }
                    Err(e) => {
                        let resp = ErrorResponse { id: req.id, message: e.to_string() };
                        send_msg(&sock_write, MSG_ERROR, &resp).await?;
                    }
                }
            }
            MSG_INPUT => {
                let req: InputRequest = rmp_serde::from_slice(&msg_buf)?;
                let mut reg = registry.lock().await;
                if let Some(term) = reg.get_mut(req.terminal_id) {
                    let _ = term.write(&req.data);
                }
                let resp = OkResponse { id: req.id };
                send_msg(&sock_write, MSG_OK, &resp).await?;
            }
            MSG_RESIZE => {
                let req: ResizeRequest = rmp_serde::from_slice(&msg_buf)?;
                let reg = registry.lock().await;
                if let Some(term) = reg.terminals.get(&req.terminal_id) {
                    let _ = term.resize(req.cols, req.rows);
                }
                let resp = OkResponse { id: req.id };
                send_msg(&sock_write, MSG_OK, &resp).await?;
            }
            MSG_KILL => {
                let req: KillRequest = rmp_serde::from_slice(&msg_buf)?;
                let mut reg = registry.lock().await;
                reg.remove(req.terminal_id);
                let resp = OkResponse { id: req.id };
                send_msg(&sock_write, MSG_OK, &resp).await?;
            }
            _ => {
                let resp = ErrorResponse { id: 0, message: "unknown message type".into() };
                send_msg(&sock_write, MSG_ERROR, &resp).await?;
            }
        }
    }
    Ok(())
}

/// Send a tagged MessagePack message to the client
async fn send_msg<T: serde::Serialize>(
    sock: &Arc<Mutex<tokio::net::unix::OwnedWriteHalf>>,
    tag: u8,
    msg: &T,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let data = rmp_serde::to_vec_named(msg)?;  // Use named fields for JS compatibility
    let mut sock = sock.lock().await;
    sock.write_all(&[tag]).await?;
    sock.write_all(&(data.len() as u32).to_be_bytes()).await?;
    sock.write_all(&data).await?;
    Ok(())
}
