//! uplink-fs: Filesystem service for VSCode remote
//!
//! Provides filesystem operations over a Unix socket using MessagePack protocol
//! Wire format: [1 byte tag][4 byte length BE][MessagePack payload]

mod ops;
mod protocol;
mod watcher;

use protocol::*;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use watcher::{create_watcher_manager, SharedWatcherManager, WatchEvent};

/// Start the filesystem server, listening on the given Unix socket path
pub async fn run(socket_path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _ = std::fs::remove_file(socket_path);
    let listener = UnixListener::bind(socket_path)?;

    println!("uplink-fs listening on {}", socket_path.display());
    info!(path = %socket_path.display(), "uplink-fs listening");

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

async fn handle_client(stream: UnixStream) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (sock_read, sock_write) = stream.into_split();
    let sock_write = Arc::new(Mutex::new(sock_write));

    let (watcher_manager, mut watch_rx) = create_watcher_manager();

    // Forward watch events to client
    let sock_write_clone = sock_write.clone();
    let watch_task = tokio::spawn(async move {
        while let Some(event) = watch_rx.recv().await {
            match event {
                WatchEvent::Change(e) => {
                    let _ = send_msg(&sock_write_clone, MSG_FILE_CHANGE, &e).await;
                }
                WatchEvent::Error(e) => {
                    let _ = send_msg(&sock_write_clone, MSG_WATCH_ERROR, &e).await;
                }
            }
        }
    });

    let request_task = handle_requests(sock_read, sock_write.clone(), watcher_manager);

    tokio::select! {
        _ = watch_task => {},
        r = request_task => { r?; },
    }

    Ok(())
}

async fn handle_requests(
    mut sock_read: tokio::net::unix::OwnedReadHalf,
    sock_write: Arc<Mutex<tokio::net::unix::OwnedWriteHalf>>,
    watcher_manager: SharedWatcherManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        let mut tag = [0u8; 1];
        if sock_read.read_exact(&mut tag).await.is_err() {
            break;
        }

        let mut len_buf = [0u8; 4];
        if sock_read.read_exact(&mut len_buf).await.is_err() {
            break;
        }
        let len = u32::from_be_bytes(len_buf) as usize;

        let mut msg_buf = vec![0u8; len];
        if sock_read.read_exact(&mut msg_buf).await.is_err() {
            break;
        }

        debug!(tag = tag[0], len, "Received message");

        match tag[0] {
            MSG_STAT => {
                let req: StatRequest = rmp_serde::from_slice(&msg_buf)?;
                match ops::stat(&req.path).await {
                    Ok((file_type, ctime, mtime, size)) => {
                        send_msg(&sock_write, MSG_STAT_RESULT, &StatResult {
                            id: req.id, file_type, ctime, mtime, size,
                        }).await?;
                    }
                    Err(e) => {
                        send_msg(&sock_write, MSG_ERROR, &ErrorResponse { id: req.id, message: e }).await?;
                    }
                }
            }
            MSG_READ_FILE => {
                let req: ReadFileRequest = rmp_serde::from_slice(&msg_buf)?;
                match ops::read_file(&req.path).await {
                    Ok(data) => {
                        send_msg(&sock_write, MSG_DATA, &DataResponse { id: req.id, data }).await?;
                    }
                    Err(e) => {
                        send_msg(&sock_write, MSG_ERROR, &ErrorResponse { id: req.id, message: e }).await?;
                    }
                }
            }
            MSG_WRITE_FILE => {
                let req: WriteFileRequest = rmp_serde::from_slice(&msg_buf)?;
                match ops::write_file(&req.path, &req.data, req.create, req.overwrite).await {
                    Ok(()) => {
                        send_msg(&sock_write, MSG_OK, &OkResponse { id: req.id }).await?;
                    }
                    Err(e) => {
                        send_msg(&sock_write, MSG_ERROR, &ErrorResponse { id: req.id, message: e }).await?;
                    }
                }
            }
            MSG_DELETE => {
                let req: DeleteRequest = rmp_serde::from_slice(&msg_buf)?;
                match ops::delete(&req.path, req.recursive).await {
                    Ok(()) => {
                        send_msg(&sock_write, MSG_OK, &OkResponse { id: req.id }).await?;
                    }
                    Err(e) => {
                        send_msg(&sock_write, MSG_ERROR, &ErrorResponse { id: req.id, message: e }).await?;
                    }
                }
            }
            MSG_RENAME => {
                let req: RenameRequest = rmp_serde::from_slice(&msg_buf)?;
                match ops::rename(&req.old_path, &req.new_path, req.overwrite).await {
                    Ok(()) => {
                        send_msg(&sock_write, MSG_OK, &OkResponse { id: req.id }).await?;
                    }
                    Err(e) => {
                        send_msg(&sock_write, MSG_ERROR, &ErrorResponse { id: req.id, message: e }).await?;
                    }
                }
            }
            MSG_COPY => {
                let req: CopyRequest = rmp_serde::from_slice(&msg_buf)?;
                match ops::copy(&req.src_path, &req.dest_path, req.overwrite).await {
                    Ok(()) => {
                        send_msg(&sock_write, MSG_OK, &OkResponse { id: req.id }).await?;
                    }
                    Err(e) => {
                        send_msg(&sock_write, MSG_ERROR, &ErrorResponse { id: req.id, message: e }).await?;
                    }
                }
            }
            MSG_READ_DIR => {
                let req: ReadDirRequest = rmp_serde::from_slice(&msg_buf)?;
                match ops::read_dir(&req.path).await {
                    Ok(entries) => {
                        send_msg(&sock_write, MSG_DIR_ENTRIES, &DirEntriesResponse { id: req.id, entries }).await?;
                    }
                    Err(e) => {
                        send_msg(&sock_write, MSG_ERROR, &ErrorResponse { id: req.id, message: e }).await?;
                    }
                }
            }
            MSG_MKDIR => {
                let req: MkdirRequest = rmp_serde::from_slice(&msg_buf)?;
                match ops::mkdir(&req.path).await {
                    Ok(()) => {
                        send_msg(&sock_write, MSG_OK, &OkResponse { id: req.id }).await?;
                    }
                    Err(e) => {
                        send_msg(&sock_write, MSG_ERROR, &ErrorResponse { id: req.id, message: e }).await?;
                    }
                }
            }
            MSG_WATCH => {
                let req: WatchRequest = rmp_serde::from_slice(&msg_buf)?;
                let mut mgr = watcher_manager.lock().await;
                match mgr.watch(req.session_id, req.req_id, &req.path, req.recursive) {
                    Ok(()) => {
                        send_msg(&sock_write, MSG_OK, &OkResponse { id: req.id }).await?;
                    }
                    Err(e) => {
                        send_msg(&sock_write, MSG_ERROR, &ErrorResponse { id: req.id, message: e }).await?;
                    }
                }
            }
            MSG_UNWATCH => {
                let req: UnwatchRequest = rmp_serde::from_slice(&msg_buf)?;
                let mut mgr = watcher_manager.lock().await;
                mgr.unwatch(&req.session_id, req.req_id);
                send_msg(&sock_write, MSG_OK, &OkResponse { id: req.id }).await?;
            }
            MSG_REALPATH => {
                let req: RealpathRequest = rmp_serde::from_slice(&msg_buf)?;
                match ops::realpath(&req.path).await {
                    Ok(path) => {
                        send_msg(&sock_write, MSG_REALPATH_RESULT, &RealpathResult { id: req.id, path }).await?;
                    }
                    Err(e) => {
                        send_msg(&sock_write, MSG_ERROR, &ErrorResponse { id: req.id, message: e }).await?;
                    }
                }
            }
            _ => {
                warn!(tag = tag[0], "Unknown message type");
                send_msg(&sock_write, MSG_ERROR, &ErrorResponse { id: 0, message: "unknown message type".into() }).await?;
            }
        }
    }
    Ok(())
}

async fn send_msg<T: serde::Serialize>(
    sock: &Arc<Mutex<tokio::net::unix::OwnedWriteHalf>>,
    tag: u8,
    msg: &T,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let data = rmp_serde::to_vec_named(msg)?;
    let mut sock = sock.lock().await;
    sock.write_all(&[tag]).await?;
    sock.write_all(&(data.len() as u32).to_be_bytes()).await?;
    sock.write_all(&data).await?;
    Ok(())
}
