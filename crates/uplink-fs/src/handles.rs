//! File handle management for streaming read/write operations

use crate::ops::FsError;
use std::collections::HashMap;
use std::io::{ErrorKind, SeekFrom};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

pub struct HandleManager {
    next_fd: u32,
    handles: HashMap<u32, File>,
}

impl HandleManager {
    pub fn new() -> Self {
        Self {
            next_fd: 1,
            handles: HashMap::new(),
        }
    }

    pub async fn open(&mut self, path: &str, create: bool, truncate: bool) -> Result<u32, FsError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(create)
            .truncate(truncate)
            .open(path)
            .await
            .map_err(|e| FsError {
                message: e.to_string(),
                code: match e.kind() {
                    ErrorKind::NotFound => "FileNotFound",
                    ErrorKind::PermissionDenied => "NoPermissions",
                    _ => "Unknown",
                },
            })?;

        let fd = self.next_fd;
        self.next_fd += 1;
        self.handles.insert(fd, file);
        Ok(fd)
    }

    pub async fn close(&mut self, fd: u32) -> Result<(), FsError> {
        match self.handles.remove(&fd) {
            Some(file) => {
                drop(file);
                Ok(())
            }
            None => Err(FsError {
                message: "Invalid file descriptor".into(),
                code: "Unknown",
            }),
        }
    }

    pub async fn read(&mut self, fd: u32, pos: u64, len: u32) -> Result<(Vec<u8>, u32), FsError> {
        let file = self.handles.get_mut(&fd).ok_or_else(|| FsError {
            message: "Invalid file descriptor".into(),
            code: "Unknown",
        })?;

        file.seek(SeekFrom::Start(pos)).await.map_err(|e| FsError {
            message: e.to_string(),
            code: "Unknown",
        })?;

        let mut buf = vec![0u8; len as usize];
        let bytes_read = file.read(&mut buf).await.map_err(|e| FsError {
            message: e.to_string(),
            code: "Unknown",
        })?;

        buf.truncate(bytes_read);
        Ok((buf, bytes_read as u32))
    }

    pub async fn write(&mut self, fd: u32, pos: u64, data: &[u8]) -> Result<u32, FsError> {
        let file = self.handles.get_mut(&fd).ok_or_else(|| FsError {
            message: "Invalid file descriptor".into(),
            code: "Unknown",
        })?;

        file.seek(SeekFrom::Start(pos)).await.map_err(|e| FsError {
            message: e.to_string(),
            code: "Unknown",
        })?;

        let bytes_written = file.write(data).await.map_err(|e| FsError {
            message: e.to_string(),
            code: "Unknown",
        })?;

        file.flush().await.map_err(|e| FsError {
            message: e.to_string(),
            code: "Unknown",
        })?;

        Ok(bytes_written as u32)
    }
}
