use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};
use std::os::unix::net::UnixStream;

const MSG_CREATE: u8 = 1;
const MSG_INPUT: u8 = 2;
const MSG_CREATED: u8 = 10;
const MSG_OK: u8 = 11;
const MSG_ERROR: u8 = 12;
const MSG_DATA: u8 = 20;
const MSG_EXIT: u8 = 21;

#[derive(Debug, Serialize)]
struct CreateRequest {
    id: u32,
    shell: String,
    args: Vec<String>,
    cwd: String,
    env: HashMap<String, String>,
    cols: u16,
    rows: u16,
}

#[derive(Debug, Serialize)]
struct InputRequest {
    id: u32,
    terminal_id: u32,
    data: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct CreatedResponse {
    id: u32,
    terminal_id: u32,
    pid: u32,
}

#[derive(Debug, Deserialize)]
struct OkResponse {
    id: u32,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    id: u32,
    message: String,
}

#[derive(Debug, Deserialize)]
struct DataEvent {
    terminal_id: u32,
    data: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct ExitEvent {
    terminal_id: u32,
    code: Option<i32>,
}

fn main() -> io::Result<()> {
    let mut stream = UnixStream::connect("/tmp/uplink-pty.sock")?;

    let req = CreateRequest {
        id: 1,
        shell: "/bin/bash".into(),
        args: vec!["-l".into()],
        cwd: std::env::var("HOME").unwrap_or("/".into()),
        env: HashMap::new(),
        cols: 80,
        rows: 24,
    };

    send_msg(&mut stream, MSG_CREATE, &req)?;
    println!("Sent create request");

    let (tag, data) = read_msg(&mut stream)?;
    let terminal_id = match tag {
        MSG_CREATED => {
            let resp: CreatedResponse = rmp_serde::from_slice(&data).unwrap();
            println!("Created terminal {} with pid {}", resp.terminal_id, resp.pid);
            resp.terminal_id
        }
        MSG_ERROR => {
            let resp: ErrorResponse = rmp_serde::from_slice(&data).unwrap();
            eprintln!("Error: {}", resp.message);
            return Ok(());
        }
        _ => {
            eprintln!("Unexpected response tag: {}", tag);
            return Ok(());
        }
    };

    stream.set_nonblocking(true)?;

    // Read initial shell output (prompt)
    std::thread::sleep(std::time::Duration::from_millis(200));
    loop {
        match read_msg(&mut stream) {
            Ok((MSG_DATA, data)) => {
                let event: DataEvent = rmp_serde::from_slice(&data).unwrap();
                print!("{}", String::from_utf8_lossy(&event.data));
                io::stdout().flush()?;
            }
            _ => break,
        }
    }

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;

        let req = InputRequest {
            id: 2,
            terminal_id,
            data: format!("{}\n", line).into_bytes(),
        };
        stream.set_nonblocking(false)?;
        send_msg(&mut stream, MSG_INPUT, &req)?;
        let _ = read_msg(&mut stream)?;

        stream.set_nonblocking(true)?;
        std::thread::sleep(std::time::Duration::from_millis(100));

        loop {
            match read_msg(&mut stream) {
                Ok((MSG_DATA, data)) => {
                    let event: DataEvent = rmp_serde::from_slice(&data).unwrap();
                    print!("{}", String::from_utf8_lossy(&event.data));
                    io::stdout().flush()?;
                }
                Ok((MSG_EXIT, data)) => {
                    let event: ExitEvent = rmp_serde::from_slice(&data).unwrap();
                    println!("\nTerminal exited with code {:?}", event.code);
                    return Ok(());
                }
                _ => break,
            }
        }
    }

    Ok(())
}

fn send_msg<T: Serialize>(stream: &mut UnixStream, tag: u8, msg: &T) -> io::Result<()> {
    let data = rmp_serde::to_vec(msg).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    stream.write_all(&[tag])?;
    stream.write_all(&(data.len() as u32).to_be_bytes())?;
    stream.write_all(&data)?;
    Ok(())
}

fn read_msg(stream: &mut UnixStream) -> io::Result<(u8, Vec<u8>)> {
    let mut tag = [0u8; 1];
    stream.read_exact(&mut tag)?;

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;

    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf)?;

    Ok((tag[0], buf))
}
