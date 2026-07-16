use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SidecarError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, SidecarError>;

pub type SidecarEventHandler = Arc<dyn Fn(Value) + Send + Sync>;

#[derive(Serialize)]
struct RpcRequest {
    id: u64,
    method: String,
    params: Value,
}

#[derive(Deserialize)]
struct RpcResponse {
    id: Option<u64>,
    result: Option<Value>,
    error: Option<RpcError>,
}

#[derive(Deserialize)]
struct RpcError {
    message: String,
}

pub struct SidecarManager {
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
    start_lock: Mutex<()>,
    pending: Arc<Mutex<HashMap<u64, mpsc::Sender<Result<Value>>>>>,
    next_id: AtomicU64,
    script_path: PathBuf,
    event_handler: Mutex<Option<SidecarEventHandler>>,
}

impl SidecarManager {
    pub fn new(script_path: PathBuf) -> Self {
        Self {
            child: Mutex::new(None),
            stdin: Mutex::new(None),
            start_lock: Mutex::new(()),
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(1),
            script_path,
            event_handler: Mutex::new(None),
        }
    }

    pub fn set_event_handler(&self, handler: SidecarEventHandler) {
        *self.event_handler.lock().unwrap() = Some(handler);
    }

    pub fn is_running(&self) -> bool {
        let mut child_guard = self.child.lock().unwrap();
        let Some(child) = child_guard.as_mut() else {
            return false;
        };
        match child.try_wait() {
            Ok(Some(_)) => {
                *child_guard = None;
                *self.stdin.lock().unwrap() = None;
                false
            }
            Ok(None) => true,
            Err(_) => {
                *child_guard = None;
                *self.stdin.lock().unwrap() = None;
                false
            }
        }
    }

    pub fn start(&self) -> Result<()> {
        let _start_guard = self.start_lock.lock().unwrap();
        if self.is_running() {
            return Ok(());
        }

        let node = which_node();
        let mut child = Command::new(&node)
            .arg(&self.script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdin = child.stdin.take().ok_or_else(|| SidecarError::Message("no stdin".into()))?;
        let stdout = child.stdout.take().ok_or_else(|| SidecarError::Message("no stdout".into()))?;
        let event_handler = self.event_handler.lock().unwrap().clone();
        spawn_stdout_reader(stdout, Arc::clone(&self.pending), event_handler);
        *self.stdin.lock().unwrap() = Some(stdin);
        *self.child.lock().unwrap() = Some(child);
        Ok(())
    }

    pub fn call(&self, method: &str, params: Value) -> Result<Value> {
        if !self.is_running() {
            self.start()?;
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = RpcRequest {
            id,
            method: method.to_string(),
            params,
        };
        let line = serde_json::to_string(&request)?;
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap().insert(id, tx);

        let write_result = {
            let mut stdin_guard = self.stdin.lock().unwrap();
            let stdin = stdin_guard
                .as_mut()
                .ok_or_else(|| SidecarError::Message("sidecar stdin closed".into()))?;
            writeln!(stdin, "{line}")?;
            stdin.flush()
        };
        if let Err(err) = write_result {
            self.pending.lock().unwrap().remove(&id);
            *self.child.lock().unwrap() = None;
            *self.stdin.lock().unwrap() = None;
            return Err(err.into());
        }

        let timeout_secs = if method == "resolve_valve_authcode_urls" {
            600
        } else if method == "download" {
            300
        } else {
            180
        };

        match rx.recv_timeout(Duration::from_secs(timeout_secs)) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.pending.lock().unwrap().remove(&id);
                Err(SidecarError::Message(format!("sidecar call timed out: {method}")))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(SidecarError::Message("sidecar response channel closed".into()))
            }
        }
    }
}

fn spawn_stdout_reader(
    stdout: ChildStdout,
    pending: Arc<Mutex<HashMap<u64, mpsc::Sender<Result<Value>>>>>,
    event_handler: Option<SidecarEventHandler>,
) {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    fail_pending(&pending, "sidecar stdout closed");
                    break;
                }
                Ok(_) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let parsed: Value = match serde_json::from_str(&line) {
                        Ok(value) => value,
                        Err(_) => continue,
                    };
                    if parsed.get("event").is_some() {
                        if let Some(ref handler) = event_handler {
                            handler(parsed);
                        }
                        continue;
                    }
                    let response: RpcResponse = match serde_json::from_value(parsed) {
                        Ok(response) => response,
                        Err(_) => continue,
                    };
                    let Some(id) = response.id else {
                        continue;
                    };
                    if let Some(tx) = pending.lock().unwrap().remove(&id) {
                        let result = if let Some(err) = response.error {
                            Err(SidecarError::Message(err.message))
                        } else {
                            Ok(response.result.unwrap_or(Value::Null))
                        };
                        let _ = tx.send(result);
                    }
                }
                Err(err) => {
                    fail_pending(&pending, format!("sidecar stdout error: {err}"));
                    break;
                }
            }
        }
    });
}

fn fail_pending(
    pending: &Arc<Mutex<HashMap<u64, mpsc::Sender<Result<Value>>>>>,
    message: impl Into<String>,
) {
    let message = message.into();
    let senders: Vec<_> = pending.lock().unwrap().drain().map(|(_, tx)| tx).collect();
    for tx in senders {
        let _ = tx.send(Err(SidecarError::Message(message.clone())));
    }
}

fn which_node() -> String {
    std::env::var("CS_ANALYST_NODE").unwrap_or_else(|_| "node".to_string())
}

pub fn resolve_sidecar_script() -> PathBuf {
    if let Ok(path) = std::env::var("CS_ANALYST_SIDECAR") {
        return PathBuf::from(path);
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_dist = manifest_dir.join("../../../sidecar/dist/index.js");
    if src_dist.exists() {
        return src_dist;
    }
    let dev_path = manifest_dir.join("../../../packages/sidecar/dist/index.js");
    if dev_path.exists() {
        return dev_path;
    }
    manifest_dir.join("resources/sidecar/index.js")
}
