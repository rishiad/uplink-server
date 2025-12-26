//! Test client for uplink-fs

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

// Message tags
const MSG_STAT: u8 = 1;
const MSG_READ_DIR: u8 = 7;
const MSG_STAT_RESULT: u8 = 22;
const MSG_DIR_ENTRIES: u8 = 24;
const MSG_ERROR: u8 = 21;

fn send_msg(stream: &mut UnixStream, tag: u8, payload: &[u8]) {
    stream.write_all(&[tag]).unwrap();
    stream.write_all(&(payload.len() as u32).to_be_bytes()).unwrap();
    stream.write_all(payload).unwrap();
}

fn recv_msg(stream: &mut UnixStream) -> (u8, Vec<u8>) {
    let mut tag = [0u8; 1];
    stream.read_exact(&mut tag).unwrap();
    let mut len = [0u8; 4];
    stream.read_exact(&mut len).unwrap();
    let len = u32::from_be_bytes(len) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).unwrap();
    (tag[0], buf)
}

fn main() {
    let mut stream = UnixStream::connect("/tmp/uplink-fs.sock").expect("Failed to connect");
    println!("Connected to uplink-fs");

    // Test 1: stat /tmp
    println!("\n=== Test: stat /tmp ===");
    let req = rmp_serde::to_vec_named(&serde_json::json!({"id": 1, "path": "/tmp"})).unwrap();
    send_msg(&mut stream, MSG_STAT, &req);
    let (tag, data) = recv_msg(&mut stream);
    match tag {
        MSG_STAT_RESULT => {
            let result: serde_json::Value = rmp_serde::from_slice(&data).unwrap();
            println!("stat result: {:?}", result);
        }
        MSG_ERROR => {
            let err: serde_json::Value = rmp_serde::from_slice(&data).unwrap();
            println!("error: {:?}", err);
        }
        _ => println!("unexpected tag: {}", tag),
    }

    // Test 2: readdir /tmp
    println!("\n=== Test: readdir /tmp ===");
    let req = rmp_serde::to_vec_named(&serde_json::json!({"id": 2, "path": "/tmp"})).unwrap();
    send_msg(&mut stream, MSG_READ_DIR, &req);
    let (tag, data) = recv_msg(&mut stream);
    match tag {
        MSG_DIR_ENTRIES => {
            let result: serde_json::Value = rmp_serde::from_slice(&data).unwrap();
            let entries = result.get("entries").and_then(|e| e.as_array());
            println!("readdir: {} entries", entries.map(|e| e.len()).unwrap_or(0));
            if let Some(entries) = entries {
                for e in entries.iter().take(5) {
                    println!("  {:?}", e);
                }
                if entries.len() > 5 {
                    println!("  ... and {} more", entries.len() - 5);
                }
            }
        }
        MSG_ERROR => {
            let err: serde_json::Value = rmp_serde::from_slice(&data).unwrap();
            println!("error: {:?}", err);
        }
        _ => println!("unexpected tag: {}", tag),
    }

    // Test 3: stat non-existent file
    println!("\n=== Test: stat /nonexistent ===");
    let req = rmp_serde::to_vec_named(&serde_json::json!({"id": 3, "path": "/nonexistent"})).unwrap();
    send_msg(&mut stream, MSG_STAT, &req);
    let (tag, data) = recv_msg(&mut stream);
    match tag {
        MSG_ERROR => {
            let err: serde_json::Value = rmp_serde::from_slice(&data).unwrap();
            println!("expected error: {:?}", err);
        }
        _ => println!("unexpected tag: {}", tag),
    }

    println!("\n=== All tests passed ===");
}
