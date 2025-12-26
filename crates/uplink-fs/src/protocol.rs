//! Protocol message types for uplink-fs
//!
//! Wire format: [1 byte tag][4 byte length BE][MessagePack payload]

use serde::{Deserialize, Serialize};

// Request tags (client → server)
pub const MSG_STAT: u8 = 1;
pub const MSG_READ_FILE: u8 = 2;
pub const MSG_WRITE_FILE: u8 = 3;
pub const MSG_DELETE: u8 = 4;
pub const MSG_RENAME: u8 = 5;
pub const MSG_COPY: u8 = 6;
pub const MSG_READ_DIR: u8 = 7;
pub const MSG_MKDIR: u8 = 8;
pub const MSG_WATCH: u8 = 9;
pub const MSG_UNWATCH: u8 = 10;
pub const MSG_REALPATH: u8 = 11;

// Response tags (server → client)
pub const MSG_OK: u8 = 20;
pub const MSG_ERROR: u8 = 21;
pub const MSG_STAT_RESULT: u8 = 22;
pub const MSG_DATA: u8 = 23;
pub const MSG_DIR_ENTRIES: u8 = 24;
pub const MSG_REALPATH_RESULT: u8 = 25;

// Event tags (server → client, async)
pub const MSG_FILE_CHANGE: u8 = 30;
pub const MSG_WATCH_ERROR: u8 = 31;

// FileType constants (matches VSCode FileType enum)
pub const FILE_TYPE_UNKNOWN: u8 = 0;
pub const FILE_TYPE_FILE: u8 = 1;
pub const FILE_TYPE_DIRECTORY: u8 = 2;
pub const FILE_TYPE_SYMLINK: u8 = 64;

#[derive(Debug, Serialize, Deserialize)]
pub struct StatRequest {
    pub id: u32,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatResult {
    pub id: u32,
    pub file_type: u8,
    pub ctime: u64,
    pub mtime: u64,
    pub size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadFileRequest {
    pub id: u32,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataResponse {
    pub id: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WriteFileRequest {
    pub id: u32,
    pub path: String,
    pub data: Vec<u8>,
    pub create: bool,
    pub overwrite: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteRequest {
    pub id: u32,
    pub path: String,
    pub recursive: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RenameRequest {
    pub id: u32,
    pub old_path: String,
    pub new_path: String,
    pub overwrite: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CopyRequest {
    pub id: u32,
    pub src_path: String,
    pub dest_path: String,
    pub overwrite: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadDirRequest {
    pub id: u32,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub file_type: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirEntriesResponse {
    pub id: u32,
    pub entries: Vec<DirEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MkdirRequest {
    pub id: u32,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WatchRequest {
    pub id: u32,
    pub session_id: String,
    pub req_id: u32,
    pub path: String,
    pub recursive: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnwatchRequest {
    pub id: u32,
    pub session_id: String,
    pub req_id: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileChangeEvent {
    pub session_id: String,
    pub changes: Vec<FileChange>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileChange {
    pub change_type: u8, // 0=Updated, 1=Added, 2=Deleted
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WatchErrorEvent {
    pub session_id: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RealpathRequest {
    pub id: u32,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RealpathResult {
    pub id: u32,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OkResponse {
    pub id: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub id: u32,
    pub message: String,
}
