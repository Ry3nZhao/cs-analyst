use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
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
    next_id: AtomicU64,
    script_path: PathBuf,
}

impl SidecarManager {
    pub fn new(script_path: PathBuf) -> Self {
        Self {
            child: Mutex::new(None),
            stdin: Mutex::new(None),
            next_id: AtomicU64::new(1),
            script_path,
        }
    }

    pub fn is_running(&self) -> bool {
        self.child.lock().unwrap().is_some()
    }

    pub fn start(&self) -> Result<()> {
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

        {
            let mut stdin_guard = self.stdin.lock().unwrap();
            let stdin = stdin_guard
                .as_mut()
                .ok_or_else(|| SidecarError::Message("sidecar stdin closed".into()))?;
            writeln!(stdin, "{line}")?;
            stdin.flush()?;
        }

        let mut child_guard = self.child.lock().unwrap();
        let child = child_guard
            .as_mut()
            .ok_or_else(|| SidecarError::Message("sidecar not running".into()))?;
        let stdout = child
            .stdout
            .as_mut()
            .ok_or_else(|| SidecarError::Message("no stdout".into()))?;
        let mut reader = BufReader::new(stdout);

        loop {
            let mut response_line = String::new();
            reader.read_line(&mut response_line)?;
            if response_line.trim().is_empty() {
                continue;
            }
            let parsed: Value = serde_json::from_str(&response_line)?;
            if parsed.get("event").is_some() {
                continue;
            }
            let response: RpcResponse = serde_json::from_str(&response_line)?;
            if response.id != Some(id) {
                continue;
            }
            if let Some(err) = response.error {
                return Err(SidecarError::Message(err.message));
            }
            return Ok(response.result.unwrap_or(Value::Null));
        }
    }
}

fn which_node() -> String {
    std::env::var("CS_DEMO_ANALYST_NODE").unwrap_or_else(|_| "node".to_string())
}

pub fn resolve_sidecar_script() -> PathBuf {
    if let Ok(path) = std::env::var("CS_DEMO_ANALYST_SIDECAR") {
        return PathBuf::from(path);
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dev_path = manifest_dir
        .join("../../../packages/sidecar/dist/index.js");
    if dev_path.exists() {
        return dev_path;
    }
    manifest_dir.join("resources/sidecar/index.js")
}
