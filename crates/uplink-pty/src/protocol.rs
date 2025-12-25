//! Protocol message types for uplink-pty
//!
//! Wire format: [1 byte tag][4 byte length BE][MessagePack payload]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Message type tags - requests (client to server)
pub const MSG_CREATE: u8 = 1;
pub const MSG_INPUT: u8 = 2;
pub const MSG_RESIZE: u8 = 3;
pub const MSG_KILL: u8 = 4;

// Message type tags - responses (server to client)
pub const MSG_CREATED: u8 = 10;
pub const MSG_OK: u8 = 11;
pub const MSG_ERROR: u8 = 12;

// Message type tags - events (server to client)
pub const MSG_DATA: u8 = 20;
pub const MSG_EXIT: u8 = 21;

/// Request to create a new terminal
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRequest {
    pub id: u32,
    pub shell: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
    pub cols: u16,
    pub rows: u16,
}

/// Request to send input to a terminal
#[derive(Debug, Serialize, Deserialize)]
pub struct InputRequest {
    pub id: u32,
    pub terminal_id: u32,
    pub data: Vec<u8>,
}

/// Request to resize a terminal
#[derive(Debug, Serialize, Deserialize)]
pub struct ResizeRequest {
    pub id: u32,
    pub terminal_id: u32,
    pub cols: u16,
    pub rows: u16,
}

/// Request to kill a terminal
#[derive(Debug, Serialize, Deserialize)]
pub struct KillRequest {
    pub id: u32,
    pub terminal_id: u32,
}

/// Response: terminal created successfully
#[derive(Debug, Serialize, Deserialize)]
pub struct CreatedResponse {
    pub id: u32,
    pub terminal_id: u32,
    pub pid: u32,
}

/// Response: request completed successfully
#[derive(Debug, Serialize, Deserialize)]
pub struct OkResponse {
    pub id: u32,
}

/// Response: request failed
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub id: u32,
    pub message: String,
}

/// Event: terminal output data
#[derive(Debug, Serialize, Deserialize)]
pub struct DataEvent {
    pub terminal_id: u32,
    pub data: Vec<u8>,
}

/// Event: terminal process exited
#[derive(Debug, Serialize, Deserialize)]
pub struct ExitEvent {
    pub terminal_id: u32,
    pub code: Option<i32>,
}
