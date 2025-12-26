//! File watcher using notify crate

use crate::protocol::*;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::debug;

pub type WatchKey = (String, u32); // (session_id, req_id)

pub struct WatcherManager {
    watchers: HashMap<WatchKey, RecommendedWatcher>,
    event_tx: mpsc::Sender<WatchEvent>,
}

pub enum WatchEvent {
    Change(FileChangeEvent),
    Error(WatchErrorEvent),
}

impl WatcherManager {
    pub fn new(event_tx: mpsc::Sender<WatchEvent>) -> Self {
        Self {
            watchers: HashMap::new(),
            event_tx,
        }
    }

    pub fn watch(
        &mut self,
        session_id: String,
        req_id: u32,
        path: &str,
        recursive: bool,
    ) -> Result<(), String> {
        let key = (session_id.clone(), req_id);
        
        if self.watchers.contains_key(&key) {
            return Err("Watch already exists".into());
        }

        let tx = self.event_tx.clone();
        let session = session_id.clone();
        
        let watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                let tx = tx.clone();
                let session = session.clone();
                
                match res {
                    Ok(event) => {
                        let changes: Vec<FileChange> = event.paths.iter().map(|p| {
                            let change_type = match event.kind {
                                notify::EventKind::Create(_) => 1, // Added
                                notify::EventKind::Remove(_) => 2, // Deleted
                                _ => 0, // Updated
                            };
                            FileChange {
                                change_type,
                                path: p.to_string_lossy().into_owned(),
                            }
                        }).collect();
                        
                        if !changes.is_empty() {
                            let _ = tx.blocking_send(WatchEvent::Change(FileChangeEvent {
                                session_id: session,
                                changes,
                            }));
                        }
                    }
                    Err(e) => {
                        let _ = tx.blocking_send(WatchEvent::Error(WatchErrorEvent {
                            session_id: session,
                            message: e.to_string(),
                        }));
                    }
                }
            },
            Config::default(),
        ).map_err(|e| e.to_string())?;

        let mut watcher = watcher;
        let mode = if recursive { RecursiveMode::Recursive } else { RecursiveMode::NonRecursive };
        watcher.watch(Path::new(path), mode).map_err(|e| e.to_string())?;
        
        debug!(session_id, req_id, path, recursive, "Watch started");
        self.watchers.insert(key, watcher);
        Ok(())
    }

    pub fn unwatch(&mut self, session_id: &str, req_id: u32) {
        let key = (session_id.to_string(), req_id);
        if self.watchers.remove(&key).is_some() {
            debug!(session_id, req_id, "Watch stopped");
        }
    }
}

pub type SharedWatcherManager = Arc<Mutex<WatcherManager>>;

pub fn create_watcher_manager() -> (SharedWatcherManager, mpsc::Receiver<WatchEvent>) {
    let (tx, rx) = mpsc::channel(256);
    let manager = Arc::new(Mutex::new(WatcherManager::new(tx)));
    (manager, rx)
}
