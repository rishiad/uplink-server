//! Filesystem operations using tokio::fs

use crate::protocol::*;
use std::path::Path;
use tokio::fs;

pub async fn stat(path: &str) -> Result<(u8, u64, u64, u64), String> {
    let meta = fs::metadata(path).await.map_err(|e| e.to_string())?;
    
    let file_type = if meta.is_symlink() {
        FILE_TYPE_SYMLINK
    } else if meta.is_dir() {
        FILE_TYPE_DIRECTORY
    } else if meta.is_file() {
        FILE_TYPE_FILE
    } else {
        FILE_TYPE_UNKNOWN
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

pub async fn read_file(path: &str) -> Result<Vec<u8>, String> {
    fs::read(path).await.map_err(|e| e.to_string())
}

pub async fn write_file(path: &str, data: &[u8], create: bool, overwrite: bool) -> Result<(), String> {
    let exists = Path::new(path).exists();
    
    if exists && !overwrite {
        return Err("File exists and overwrite is false".into());
    }
    if !exists && !create {
        return Err("File does not exist and create is false".into());
    }

    // Ensure parent directory exists
    if let Some(parent) = Path::new(path).parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
        }
    }

    fs::write(path, data).await.map_err(|e| e.to_string())
}

pub async fn delete(path: &str, recursive: bool) -> Result<(), String> {
    let meta = fs::metadata(path).await.map_err(|e| e.to_string())?;
    
    if meta.is_dir() {
        if recursive {
            fs::remove_dir_all(path).await.map_err(|e| e.to_string())
        } else {
            fs::remove_dir(path).await.map_err(|e| e.to_string())
        }
    } else {
        fs::remove_file(path).await.map_err(|e| e.to_string())
    }
}

pub async fn rename(old_path: &str, new_path: &str, overwrite: bool) -> Result<(), String> {
    if !overwrite && Path::new(new_path).exists() {
        return Err("Target exists and overwrite is false".into());
    }
    fs::rename(old_path, new_path).await.map_err(|e| e.to_string())
}

pub async fn copy(src: &str, dest: &str, overwrite: bool) -> Result<(), String> {
    if !overwrite && Path::new(dest).exists() {
        return Err("Target exists and overwrite is false".into());
    }
    fs::copy(src, dest).await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn read_dir(path: &str) -> Result<Vec<DirEntry>, String> {
    let mut entries = Vec::new();
    let mut dir = fs::read_dir(path).await.map_err(|e| e.to_string())?;
    
    while let Some(entry) = dir.next_entry().await.map_err(|e| e.to_string())? {
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

pub async fn mkdir(path: &str) -> Result<(), String> {
    fs::create_dir_all(path).await.map_err(|e| e.to_string())
}

pub async fn realpath(path: &str) -> Result<String, String> {
    fs::canonicalize(path)
        .await
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}
