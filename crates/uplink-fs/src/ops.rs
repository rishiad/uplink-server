//! Filesystem operations using tokio::fs

use crate::protocol::*;
use std::io::ErrorKind;
use std::path::Path;
use tokio::fs;

/// Error with code for client-side mapping
pub struct FsError {
    pub message: String,
    pub code: &'static str,
}

impl FsError {
    fn from_io(e: std::io::Error) -> Self {
        let code = match e.kind() {
            ErrorKind::NotFound => "FileNotFound",
            ErrorKind::AlreadyExists => "FileExists",
            ErrorKind::PermissionDenied => "NoPermissions",
            ErrorKind::IsADirectory => "FileIsADirectory",
            _ => "Unknown",
        };
        Self { message: e.to_string(), code }
    }

    fn new(message: impl Into<String>, code: &'static str) -> Self {
        Self { message: message.into(), code }
    }
}

pub async fn stat(path: &str) -> Result<(u8, u64, u64, u64), FsError> {
    // Use symlink_metadata to detect symlinks (doesn't follow them)
    let symlink_meta = fs::symlink_metadata(path).await.map_err(FsError::from_io)?;
    let is_symlink = symlink_meta.is_symlink();
    
    // Get actual metadata (follows symlinks) for size/times
    let meta = if is_symlink {
        fs::metadata(path).await.unwrap_or(symlink_meta.clone())
    } else {
        symlink_meta.clone()
    };
    
    // Combine symlink flag with actual type
    let base_type = if meta.is_dir() {
        FILE_TYPE_DIRECTORY
    } else if meta.is_file() {
        FILE_TYPE_FILE
    } else {
        FILE_TYPE_UNKNOWN
    };
    
    let file_type = if is_symlink {
        base_type | FILE_TYPE_SYMLINK
    } else {
        base_type
    };

    let ctime = meta.created()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let mtime = meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let size = meta.len();

    Ok((file_type, ctime, mtime, size))
}

pub async fn read_file(path: &str) -> Result<Vec<u8>, FsError> {
    fs::read(path).await.map_err(FsError::from_io)
}

pub async fn write_file(path: &str, data: &[u8], create: bool, overwrite: bool) -> Result<(), FsError> {
    let exists = Path::new(path).exists();
    
    if exists && !overwrite {
        return Err(FsError::new("File exists and overwrite is false", "FileExists"));
    }
    if !exists && !create {
        return Err(FsError::new("File does not exist and create is false", "FileNotFound"));
    }

    // Ensure parent directory exists
    if let Some(parent) = Path::new(path).parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).await.map_err(FsError::from_io)?;
        }
    }

    fs::write(path, data).await.map_err(FsError::from_io)
}

pub async fn delete(path: &str, recursive: bool) -> Result<(), FsError> {
    let meta = fs::metadata(path).await.map_err(FsError::from_io)?;
    
    if meta.is_dir() {
        if recursive {
            fs::remove_dir_all(path).await.map_err(FsError::from_io)
        } else {
            fs::remove_dir(path).await.map_err(FsError::from_io)
        }
    } else {
        fs::remove_file(path).await.map_err(FsError::from_io)
    }
}

pub async fn rename(old_path: &str, new_path: &str, overwrite: bool) -> Result<(), FsError> {
    if !overwrite && Path::new(new_path).exists() {
        return Err(FsError::new("Target exists and overwrite is false", "FileExists"));
    }
    fs::rename(old_path, new_path).await.map_err(FsError::from_io)
}

pub async fn copy(src: &str, dest: &str, overwrite: bool) -> Result<(), FsError> {
    if !overwrite && Path::new(dest).exists() {
        return Err(FsError::new("Target exists and overwrite is false", "FileExists"));
    }
    fs::copy(src, dest).await.map_err(FsError::from_io)?;
    Ok(())
}

pub async fn read_dir(path: &str) -> Result<Vec<DirEntry>, FsError> {
    let mut entries = Vec::new();
    let mut dir = fs::read_dir(path).await.map_err(FsError::from_io)?;
    
    while let Some(entry) = dir.next_entry().await.map_err(FsError::from_io)? {
        let name = entry.file_name().to_string_lossy().into_owned();
        let file_type = match entry.file_type().await {
            Ok(ft) => {
                if ft.is_symlink() {
                    FILE_TYPE_SYMLINK
                } else if ft.is_dir() {
                    FILE_TYPE_DIRECTORY
                } else if ft.is_file() {
                    FILE_TYPE_FILE
                } else {
                    FILE_TYPE_UNKNOWN
                }
            }
            Err(_) => FILE_TYPE_UNKNOWN,
        };
        entries.push(DirEntry { name, file_type });
    }
    
    Ok(entries)
}

pub async fn mkdir(path: &str) -> Result<(), FsError> {
    fs::create_dir_all(path).await.map_err(FsError::from_io)
}

pub async fn realpath(path: &str) -> Result<String, FsError> {
    fs::canonicalize(path)
        .await
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(FsError::from_io)
}

pub async fn clone_file(src: &str, dest: &str) -> Result<(), FsError> {
    // Ensure parent directory exists
    if let Some(parent) = Path::new(dest).parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).await.map_err(FsError::from_io)?;
        }
    }
    fs::copy(src, dest).await.map_err(FsError::from_io)?;
    Ok(())
}
