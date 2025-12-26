//! Terminal management using portable-pty

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use tokio::sync::mpsc;

/// A running terminal instance
pub struct Terminal {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    _child: Box<dyn Child + Send + Sync>,
}

impl Terminal {
    /// Write data to the terminal's stdin
    pub fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(data)
    }

    /// Resize the terminal
    pub fn resize(&self, cols: u16, rows: u16) -> std::io::Result<()> {
        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        }).map_err(|e| std::io::Error::other(e.to_string()))
    }
}

/// Registry of active terminals.
pub struct TerminalRegistry {
    // id : terminal
    pub terminals: HashMap<u32, Terminal>,
    next_id: u32,
}

impl TerminalRegistry {
    pub fn new() -> Self {
        Self {
            terminals: HashMap::new(),
            next_id: 1,
        }
    }

    /// Create a new terminal with the given shell and dimensions
    /// Returns (terminal_id, pid) on success
    pub fn create(
        &mut self,
        shell: &str,
        args: &[String],
        cwd: &str,
        env: &HashMap<String, String>,
        cols: u16,
        rows: u16,
        output_tx: mpsc::Sender<(u32, Vec<u8>)>,
        exit_tx: mpsc::Sender<(u32, Option<i32>)>,
    ) -> Result<(u32, u32), Box<dyn std::error::Error + Send + Sync>> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(shell);
        for arg in args {
            cmd.arg(arg);
        }
        cmd.cwd(cwd);
        for (k, v) in env {
            cmd.env(k, v);
        }

        let child = pair.slave.spawn_command(cmd)?;
        let pid = child.process_id().unwrap_or(0);
        drop(pair.slave); // Close slave in parent process

        let id = self.next_id;
        self.next_id += 1;

        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        // Spawn blocking thread to read PTY output and forward to channel
        let terminal_id = id;
        tokio::task::spawn_blocking(move || {
            let mut reader = reader;
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if output_tx.blocking_send((terminal_id, buf[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = exit_tx.blocking_send((terminal_id, None));
        });

        self.terminals.insert(
            id,
            Terminal {
                writer,
                master: pair.master,
                _child: child,
            },
        );

        Ok((id, pid))
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut Terminal> {
        self.terminals.get_mut(&id)
    }

    pub fn remove(&mut self, id: u32) -> Option<Terminal> {
        self.terminals.remove(&id)
    }
}
