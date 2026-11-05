//! 下载模块（Node sidecar）的进程管理：启动 + 握手、心跳、崩溃后退避重启、调用超时、日志收集。
//!
//! 模块只通过 JSON-RPC（stdio）交互，协议见 `docs/downloader-protocol.md`。

pub mod commands;
pub mod protocol;
#[cfg(test)]
mod tests;

use parking_lot::{Condvar, Mutex};
use protocol::{code, AdapterInfo, ConfigureResult, HelloResult, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SidecarError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    /// 模块返回的错误（保留错误码，`URL_EXPIRED` 等语义在 message 里也能看到）
    #[error("{message}")]
    Rpc { code: i64, message: String, data: Option<Value> },
    #[error("下载模块调用超时: {0}")]
    Timeout(String),
    #[error("下载模块不可用: {0}")]
    Unavailable(String),
    #[error("{0}")]
    Message(String),
}

impl SidecarError {
    #[allow(dead_code)]
    pub fn code(&self) -> Option<i64> {
        match self {
            SidecarError::Rpc { code, .. } => Some(*code),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, SidecarError>;

pub type SidecarEventHandler = Arc<dyn Fn(Value) + Send + Sync>;
pub type StatusHandler = Arc<dyn Fn(&DownloaderStatus) + Send + Sync>;

/// 管理策略（测试里可以调小）。
#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    pub heartbeat_interval: Duration,
    /// 连续几次心跳失败就判定卡死并强制重启
    pub max_missed_heartbeats: u32,
    pub backoff_base: Duration,
    pub backoff_max: Duration,
    /// 连续崩溃超过这个次数就停止自动重启，等用户手动重启
    pub max_consecutive_failures: u32,
    /// 进程稳定运行这么久之后，连续崩溃计数清零
    pub stable_after: Duration,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval: Duration::from_secs(5),
            max_missed_heartbeats: 3,
            backoff_base: Duration::from_secs(1),
            backoff_max: Duration::from_secs(30),
            max_consecutive_failures: 5,
            stable_after: Duration::from_secs(60),
        }
    }
}

impl SupervisorConfig {
    /// 第 n 次（从 1 开始）连续失败后的重启等待：base × 2^(n-1)，封顶 max。
    pub fn backoff(&self, failures: u32) -> Duration {
        let factor = 1u32.checked_shl(failures.saturating_sub(1).min(16)).unwrap_or(u32::MAX);
        self.backoff_base.saturating_mul(factor).min(self.backoff_max)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleState {
    /// 还没启动 / 已停止
    Stopped,
    /// 正在启动并握手
    Starting,
    Running,
    /// 崩溃或卡死后等待退避重启
    Restarting,
    /// 连续崩溃次数超过上限，停止自动重启
    Failed,
    /// 协议版本不兼容，不会自动重启
    Incompatible,
}

/// 设置页显示的模块状态（`get_downloader_status` / `downloader_status` 事件）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloaderStatus {
    pub state: ModuleState,
    pub pid: Option<u32>,
    pub module_name: Option<String>,
    pub module_version: Option<String>,
    pub runtime: Option<String>,
    /// 模块报告的协议版本
    pub protocol_version: Option<u32>,
    /// 主程序要求的协议版本
    pub host_protocol_version: u32,
    /// 本次运行以来自动重启的次数
    pub restart_count: u32,
    pub consecutive_failures: u32,
    pub started_at: Option<String>,
    pub last_heartbeat_at: Option<String>,
    pub last_heartbeat_ms: Option<u64>,
    pub next_restart_in_ms: Option<u64>,
    pub last_exit: Option<String>,
    pub last_error: Option<String>,
    pub adapters: Vec<AdapterInfo>,
    pub methods: Vec<String>,
    pub script_path: String,
    pub log_path: Option<String>,
}

struct Proc {
    child: Child,
    stdin: ChildStdin,
    generation: u64,
    started: Instant,
}

struct Pending {
    generation: u64,
    tx: mpsc::Sender<Result<Value>>,
}

struct Inner {
    state: ModuleState,
    pid: Option<u32>,
    hello: Option<HelloResult>,
    adapters: Vec<AdapterInfo>,
    restart_count: u32,
    consecutive_failures: u32,
    missed_heartbeats: u32,
    started_at: Option<chrono::DateTime<chrono::Local>>,
    last_heartbeat_at: Option<chrono::DateTime<chrono::Local>>,
    last_heartbeat_ms: Option<u64>,
    next_restart_at: Option<Instant>,
    last_exit: Option<String>,
    last_error: Option<String>,
}

const LOG_RING: usize = 500;
const LOG_ROTATE_BYTES: u64 = 5 * 1024 * 1024;

pub struct SidecarManager {
    script_path: PathBuf,
    config: SupervisorConfig,
    proc: Mutex<Option<Proc>>,
    start_lock: Mutex<()>,
    pending: Arc<Mutex<HashMap<u64, Pending>>>,
    next_id: AtomicU64,
    generation: AtomicU64,
    event_handler: Mutex<Option<SidecarEventHandler>>,
    status_handler: Mutex<Option<StatusHandler>>,
    inner: Mutex<Inner>,
    logs: Arc<LogStore>,
    log_path: Option<PathBuf>,
    /// 最近一次 adapter 开关，重启后重新下发
    adapter_switches: Mutex<Option<Value>>,
    wake: Wake,
    shutting_down: AtomicBool,
}

impl SidecarManager {
    pub fn new(script_path: PathBuf, log_dir: Option<PathBuf>) -> Self {
        Self::with_config(script_path, log_dir, SupervisorConfig::default())
    }

    pub fn with_config(script_path: PathBuf, log_dir: Option<PathBuf>, config: SupervisorConfig) -> Self {
        let log_path = log_dir.map(|d| d.join("downloader.log"));
        let log_file = log_path.as_ref().and_then(|p| open_log_file(p).ok());
        Self {
            script_path,
            config,
            proc: Mutex::new(None),
            start_lock: Mutex::new(()),
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(1),
            generation: AtomicU64::new(0),
            event_handler: Mutex::new(None),
            status_handler: Mutex::new(None),
            inner: Mutex::new(Inner {
                state: ModuleState::Stopped,
                pid: None,
                hello: None,
                adapters: Vec::new(),
                restart_count: 0,
                consecutive_failures: 0,
                missed_heartbeats: 0,
                started_at: None,
                last_heartbeat_at: None,
                last_heartbeat_ms: None,
                next_restart_at: None,
                last_exit: None,
                last_error: None,
            }),
            logs: Arc::new(LogStore { ring: Mutex::new(VecDeque::with_capacity(LOG_RING)), file: Mutex::new(log_file) }),
            log_path,
            adapter_switches: Mutex::new(None),
            wake: Arc::new((Mutex::new(false), Condvar::new())),
            shutting_down: AtomicBool::new(false),
        }
    }

    pub fn set_event_handler(&self, handler: SidecarEventHandler) {
        *self.event_handler.lock() = Some(handler);
    }

    pub fn set_status_handler(&self, handler: StatusHandler) {
        *self.status_handler.lock() = Some(handler);
    }

    pub fn is_running(&self) -> bool {
        self.inner.lock().state == ModuleState::Running && self.process_alive()
    }

    pub fn status(&self) -> DownloaderStatus {
        let inner = self.inner.lock();
        let fmt = |t: &chrono::DateTime<chrono::Local>| t.to_rfc3339();
        DownloaderStatus {
            state: inner.state,
            pid: inner.pid,
            module_name: inner.hello.as_ref().map(|h| h.module.name.clone()),
            module_version: inner.hello.as_ref().map(|h| h.module.version.clone()),
            runtime: inner.hello.as_ref().and_then(|h| h.runtime.clone()),
            protocol_version: inner.hello.as_ref().map(|h| h.protocol_version),
            host_protocol_version: PROTOCOL_VERSION,
            restart_count: inner.restart_count,
            consecutive_failures: inner.consecutive_failures,
            started_at: inner.started_at.as_ref().map(fmt),
            last_heartbeat_at: inner.last_heartbeat_at.as_ref().map(fmt),
            last_heartbeat_ms: inner.last_heartbeat_ms,
            next_restart_in_ms: inner
                .next_restart_at
                .map(|t| t.saturating_duration_since(Instant::now()).as_millis() as u64),
            last_exit: inner.last_exit.clone(),
            last_error: inner.last_error.clone(),
            adapters: inner.adapters.clone(),
            methods: inner.hello.as_ref().map(|h| h.methods.clone()).unwrap_or_default(),
            script_path: self.script_path.to_string_lossy().to_string(),
            log_path: self.log_path.as_ref().map(|p| p.to_string_lossy().to_string()),
        }
    }

    pub fn logs(&self, limit: usize) -> Vec<String> {
        let logs = self.logs.ring.lock();
        let skip = logs.len().saturating_sub(limit);
        logs.iter().skip(skip).cloned().collect()
    }

    // -----------------------------------------------------------------------------------------
    // 启动 / 停止

    /// 启动进程并握手。已在运行则直接返回。`Failed` / `Incompatible` 状态下需要 [`Self::restart`]。
    pub fn start(&self) -> Result<()> {
        let _guard = self.start_lock.lock();
        if self.is_running() {
            return Ok(());
        }
        {
            let inner = self.inner.lock();
            match inner.state {
                ModuleState::Failed | ModuleState::Incompatible => {
                    return Err(SidecarError::Unavailable(
                        inner.last_error.clone().unwrap_or_else(|| "已停止".into()),
                    ))
                }
                _ => {}
            }
        }
        self.spawn_and_handshake()
    }

    /// 手动重启：清除失败计数，强制拉起（包括 `Failed` / `Incompatible` 状态）。
    pub fn restart(&self) -> Result<()> {
        let _guard = self.start_lock.lock();
        self.log("手动重启下载模块");
        self.kill_process("手动重启");
        {
            let mut inner = self.inner.lock();
            inner.consecutive_failures = 0;
            inner.next_restart_at = None;
            inner.last_error = None;
            inner.state = ModuleState::Stopped;
        }
        self.spawn_and_handshake()
    }

    /// 应用退出时调用：请模块自行退出，超时则强杀。
    pub fn shutdown(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
        if self.process_alive() {
            let _ = self.request("shutdown", json!({}), Duration::from_secs(2));
        }
        self.kill_process("应用退出");
        self.set_state(ModuleState::Stopped);
        self.notify_supervisor();
    }

    fn spawn_and_handshake(&self) -> Result<()> {
        self.set_state(ModuleState::Starting);
        let spawned = self.spawn_process();
        if let Err(err) = spawned {
            let msg = format!("启动失败: {err}");
            self.log(&msg);
            self.inner.lock().last_error = Some(msg.clone());
            self.on_start_failure();
            return Err(SidecarError::Unavailable(msg));
        }

        let host = json!({ "name": "cs-analyst", "version": env!("CARGO_PKG_VERSION") });
        let hello = self.request(
            "hello",
            json!({ "protocolVersion": PROTOCOL_VERSION, "host": host }),
            protocol::timeout_for("hello"),
        );
        let hello: HelloResult = match hello.and_then(|v| serde_json::from_value(v).map_err(Into::into)) {
            Ok(h) => h,
            Err(SidecarError::Rpc { code: code::METHOD_NOT_FOUND, .. }) => {
                return Err(self.incompatible("模块不支持 hello 握手（版本过旧）".into()));
            }
            Err(err) => {
                let msg = format!("握手失败: {err}");
                self.log(&msg);
                self.kill_process("握手失败");
                self.inner.lock().last_error = Some(msg.clone());
                self.on_start_failure();
                return Err(SidecarError::Unavailable(msg));
            }
        };
        if hello.protocol_version != PROTOCOL_VERSION {
            return Err(self.incompatible(format!(
                "协议版本不兼容：模块 v{}，主程序需要 v{PROTOCOL_VERSION}",
                hello.protocol_version
            )));
        }

        self.log(&format!(
            "握手成功: {} {}（协议 v{}，{} 个 adapter）",
            hello.module.name,
            hello.module.version,
            hello.protocol_version,
            hello.adapters.len()
        ));
        {
            let mut inner = self.inner.lock();
            inner.adapters = hello.adapters.clone();
            inner.hello = Some(hello);
            inner.state = ModuleState::Running;
            inner.missed_heartbeats = 0;
            inner.next_restart_at = None;
            inner.last_error = None;
            inner.last_heartbeat_at = Some(chrono::Local::now());
        }
        // 重启后重新下发 adapter 开关
        let switches = self.adapter_switches.lock().clone();
        if let Some(switches) = switches {
            let _ = self.send_configure(switches);
        }
        self.emit_status();
        Ok(())
    }

    fn spawn_process(&self) -> Result<()> {
        let node = std::env::var("CS_ANALYST_NODE").unwrap_or_else(|_| "node".to_string());
        let mut cmd = Command::new(&node);
        cmd.arg(&self.script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take().ok_or_else(|| SidecarError::Message("no stdin".into()))?;
        let stdout = child.stdout.take().ok_or_else(|| SidecarError::Message("no stdout".into()))?;
        let stderr = child.stderr.take().ok_or_else(|| SidecarError::Message("no stderr".into()))?;
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let pid = child.id();

        self.spawn_stdout_reader(stdout, generation);
        self.spawn_stderr_reader(stderr);
        *self.proc.lock() = Some(Proc { child, stdin, generation, started: Instant::now() });
        {
            let mut inner = self.inner.lock();
            inner.pid = Some(pid);
            inner.started_at = Some(chrono::Local::now());
            inner.hello = None;
        }
        self.log(&format!("进程已启动 pid={pid} script={}", self.script_path.display()));
        Ok(())
    }

    fn incompatible(&self, msg: String) -> SidecarError {
        self.log(&msg);
        self.kill_process("协议不兼容");
        {
            let mut inner = self.inner.lock();
            inner.state = ModuleState::Incompatible;
            inner.last_error = Some(msg.clone());
            inner.next_restart_at = None;
        }
        self.emit_status();
        SidecarError::Unavailable(msg)
    }

    /// 启动或握手失败：计入连续失败，安排下一次重启或进入 Failed。
    fn on_start_failure(&self) {
        let mut inner = self.inner.lock();
        inner.consecutive_failures += 1;
        self.schedule_restart_locked(&mut inner);
        drop(inner);
        self.emit_status();
    }

    fn schedule_restart_locked(&self, inner: &mut Inner) {
        if self.shutting_down.load(Ordering::SeqCst) {
            inner.state = ModuleState::Stopped;
            return;
        }
        if inner.consecutive_failures > self.config.max_consecutive_failures {
            inner.state = ModuleState::Failed;
            inner.next_restart_at = None;
            let msg = format!(
                "连续失败 {} 次，已停止自动重启（可在设置页手动重启）",
                inner.consecutive_failures
            );
            self.log(&msg);
            inner.last_error = Some(msg);
        } else {
            let delay = self.config.backoff(inner.consecutive_failures);
            inner.state = ModuleState::Restarting;
            inner.next_restart_at = Some(Instant::now() + delay);
            self.log(&format!(
                "{} ms 后自动重启（连续失败 {} 次）",
                delay.as_millis(),
                inner.consecutive_failures
            ));
        }
    }

    fn process_alive(&self) -> bool {
        let mut guard = self.proc.lock();
        match guard.as_mut() {
            Some(p) => matches!(p.child.try_wait(), Ok(None)),
            None => false,
        }
    }

    fn kill_process(&self, reason: &str) {
        let proc = self.proc.lock().take();
        if let Some(mut p) = proc {
            if matches!(p.child.try_wait(), Ok(None)) {
                self.log(&format!("结束进程 pid={}（{reason}）", p.child.id()));
                let _ = p.child.kill();
            }
            let _ = p.child.wait();
            fail_pending(&self.pending, p.generation, "下载模块进程已停止");
        }
        self.inner.lock().pid = None;
    }

    // -----------------------------------------------------------------------------------------
    // 调用

    /// 调用模块方法：没在运行就先启动（崩溃后等待退避时也会立即尝试拉起），每个方法有各自的超时。
    pub fn call(&self, method: &str, params: Value) -> Result<Value> {
        if !self.is_running() {
            self.start()?;
        }
        let known = {
            let inner = self.inner.lock();
            inner.hello.as_ref().map(|h| h.methods.iter().any(|m| m == method))
        };
        if known == Some(false) {
            return Err(SidecarError::Rpc {
                code: code::METHOD_NOT_FOUND,
                message: format!("下载模块不支持方法 {method}"),
                data: None,
            });
        }
        self.request(method, params, protocol::timeout_for(method))
    }

    /// 发一个请求并等待回复（不负责启动）。
    fn request(&self, method: &str, params: Value, timeout: Duration) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let line = serde_json::to_string(&json!({ "id": id, "method": method, "params": params }))?;
        let (tx, rx) = mpsc::channel();
        {
            let mut guard = self.proc.lock();
            let proc = guard.as_mut().ok_or_else(|| SidecarError::Unavailable("进程未运行".into()))?;
            self.pending.lock().insert(id, Pending { generation: proc.generation, tx });
            let written = writeln!(proc.stdin, "{line}").and_then(|_| proc.stdin.flush());
            if let Err(err) = written {
                self.pending.lock().remove(&id);
                drop(guard);
                self.notify_supervisor();
                return Err(err.into());
            }
        }
        match rx.recv_timeout(timeout) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.pending.lock().remove(&id);
                if method != "ping" {
                    self.log(&format!("调用超时: {method}（{} s）", timeout.as_secs()));
                }
                Err(SidecarError::Timeout(method.to_string()))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(SidecarError::Unavailable("回复通道已关闭".into()))
            }
        }
    }

    /// 设置 adapter 开关（平台 → 是否启用）。会记住并在每次重启后重新下发。
    pub fn set_adapter_switches(&self, switches: &[(String, bool)]) -> Result<()> {
        let map: serde_json::Map<String, Value> =
            switches.iter().map(|(p, on)| (p.clone(), Value::Bool(*on))).collect();
        let value = Value::Object(map);
        *self.adapter_switches.lock() = Some(value.clone());
        if self.is_running() {
            self.send_configure(value)?;
        }
        Ok(())
    }

    fn send_configure(&self, adapters: Value) -> Result<()> {
        let result = self.request("configure", json!({ "adapters": adapters }), protocol::timeout_for("configure"))?;
        let parsed: ConfigureResult = serde_json::from_value(result)?;
        self.inner.lock().adapters = parsed.adapters;
        self.emit_status();
        Ok(())
    }

    // -----------------------------------------------------------------------------------------
    // 监护线程：心跳、崩溃检测、退避重启

    pub fn spawn_supervisor(self: &Arc<Self>) {
        let this = Arc::clone(self);
        std::thread::Builder::new()
            .name("downloader-supervisor".into())
            .spawn(move || this.supervise())
            .expect("spawn supervisor");
    }

    fn notify_supervisor(&self) {
        wake(&self.wake);
    }

    fn supervise(&self) {
        loop {
            {
                let (lock, cvar) = &*self.wake;
                let mut woken = lock.lock();
                if !*woken {
                    let wait = self.next_wait();
                    cvar.wait_for(&mut woken, wait);
                }
                *woken = false;
            }
            if self.shutting_down.load(Ordering::SeqCst) {
                return;
            }
            self.tick();
        }
    }

    fn next_wait(&self) -> Duration {
        let inner = self.inner.lock();
        match (inner.state, inner.next_restart_at) {
            (ModuleState::Restarting, Some(at)) => {
                at.saturating_duration_since(Instant::now()).min(self.config.heartbeat_interval)
            }
            _ => self.config.heartbeat_interval,
        }
    }

    fn tick(&self) {
        // 1. 进程是否已退出（崩溃 / 被结束）
        let exited = {
            let mut guard = self.proc.lock();
            match guard.as_mut() {
                Some(p) => match p.child.try_wait() {
                    Ok(Some(status)) => Some((status.code(), p.started.elapsed(), p.generation)),
                    Ok(None) => None,
                    Err(_) => Some((None, p.started.elapsed(), p.generation)),
                },
                None => None,
            }
        };
        if let Some((exit_code, ran_for, generation)) = exited {
            self.proc.lock().take();
            fail_pending(&self.pending, generation, "下载模块进程已退出");
            let desc = match exit_code {
                Some(c) => format!("退出码 {c}"),
                None => "被终止".to_string(),
            };
            self.log(&format!("进程意外退出（{desc}，运行了 {} s）", ran_for.as_secs()));
            let mut inner = self.inner.lock();
            if matches!(inner.state, ModuleState::Running | ModuleState::Starting) {
                if ran_for >= self.config.stable_after {
                    inner.consecutive_failures = 0;
                }
                inner.consecutive_failures += 1;
                inner.pid = None;
                inner.last_exit = Some(format!("{} {desc}", chrono::Local::now().format("%H:%M:%S")));
                self.schedule_restart_locked(&mut inner);
            }
            drop(inner);
            self.emit_status();
            return;
        }

        let state = self.inner.lock().state;
        match state {
            ModuleState::Running => self.heartbeat(),
            ModuleState::Restarting => {
                let due = self.inner.lock().next_restart_at.is_none_or(|t| Instant::now() >= t);
                if due {
                    let _guard = self.start_lock.lock();
                    // 期间可能已被 call() 拉起
                    if self.is_running() {
                        return;
                    }
                    self.inner.lock().restart_count += 1;
                    self.log("自动重启下载模块");
                    let _ = self.spawn_and_handshake();
                }
            }
            _ => {}
        }
    }

    fn heartbeat(&self) {
        let t = Instant::now();
        match self.request("ping", json!({}), protocol::timeout_for("ping")) {
            Ok(_) => {
                let mut inner = self.inner.lock();
                inner.missed_heartbeats = 0;
                inner.last_heartbeat_at = Some(chrono::Local::now());
                inner.last_heartbeat_ms = Some(t.elapsed().as_millis() as u64);
                if inner.consecutive_failures > 0
                    && inner.started_at.is_some_and(|s| {
                        (chrono::Local::now() - s).to_std().unwrap_or_default() >= self.config.stable_after
                    })
                {
                    inner.consecutive_failures = 0;
                }
            }
            Err(err) => {
                let missed = {
                    let mut inner = self.inner.lock();
                    inner.missed_heartbeats += 1;
                    inner.missed_heartbeats
                };
                self.log(&format!("心跳失败 {missed}/{}: {err}", self.config.max_missed_heartbeats));
                if missed >= self.config.max_missed_heartbeats {
                    self.log("心跳连续失败，判定卡死，强制结束进程");
                    if let Some(p) = self.proc.lock().as_mut() {
                        let _ = p.child.kill();
                    }
                    // 下一轮 tick 按崩溃处理
                    self.notify_supervisor();
                }
            }
        }
        self.emit_status();
    }

    // -----------------------------------------------------------------------------------------
    // 输出读取与日志

    fn spawn_stdout_reader(&self, stdout: std::process::ChildStdout, generation: u64) {
        let pending = Arc::clone(&self.pending);
        let handler = self.event_handler.lock().clone();
        let waker = Arc::clone(&self.wake);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                let Ok(parsed) = serde_json::from_str::<Value>(line.trim()) else { continue };
                if parsed.get("event").is_some() {
                    if let Some(ref h) = handler {
                        h(parsed);
                    }
                    continue;
                }
                let Some(id) = parsed.get("id").and_then(Value::as_u64) else { continue };
                let Some(p) = pending.lock().remove(&id) else { continue };
                let result = match parsed.get("error").filter(|e| !e.is_null()) {
                    Some(err) => Err(SidecarError::Rpc {
                        code: err.get("code").and_then(Value::as_i64).unwrap_or(code::INTERNAL),
                        message: err.get("message").and_then(Value::as_str).unwrap_or("unknown error").to_string(),
                        data: err.get("data").cloned(),
                    }),
                    None => Ok(parsed.get("result").cloned().unwrap_or(Value::Null)),
                };
                let _ = p.tx.send(result);
            }
            fail_pending(&pending, generation, "下载模块输出已关闭");
            wake(&waker);
        });
    }

    fn spawn_stderr_reader(&self, stderr: std::process::ChildStderr) {
        let logs = Arc::clone(&self.logs);
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(|l| l.ok()) {
                logs.push(&line);
            }
        });
    }

    fn log(&self, message: &str) {
        self.logs.push(&format!("[host] {message}"));
    }

    fn set_state(&self, state: ModuleState) {
        self.inner.lock().state = state;
        self.emit_status();
    }

    fn emit_status(&self) {
        let handler = self.status_handler.lock().clone();
        if let Some(h) = handler {
            h(&self.status());
        }
    }
}

/// 日志：内存环形缓冲 + 文件。reader 线程共享同一个 Arc。
struct LogStore {
    ring: Mutex<VecDeque<String>>,
    file: Mutex<Option<File>>,
}

impl LogStore {
    fn push(&self, message: &str) {
        push_log(&self.ring, &self.file, message);
    }
}

type Wake = Arc<(Mutex<bool>, Condvar)>;

fn wake(w: &Wake) {
    let (lock, cvar) = &**w;
    *lock.lock() = true;
    cvar.notify_all();
}

fn push_log(logs: &Mutex<VecDeque<String>>, file: &Mutex<Option<File>>, message: &str) {
    let line = format!("{} {message}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    eprintln!("[downloader] {message}");
    {
        let mut logs = logs.lock();
        if logs.len() >= LOG_RING {
            logs.pop_front();
        }
        logs.push_back(line.clone());
    }
    if let Some(f) = file.lock().as_mut() {
        let _ = writeln!(f, "{line}");
    }
}

fn open_log_file(path: &PathBuf) -> std::io::Result<File> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if std::fs::metadata(path).map(|m| m.len() > LOG_ROTATE_BYTES).unwrap_or(false) {
        let _ = std::fs::rename(path, path.with_extension("log.1"));
    }
    OpenOptions::new().create(true).append(true).open(path)
}

fn fail_pending(pending: &Arc<Mutex<HashMap<u64, Pending>>>, generation: u64, message: &str) {
    let failed: Vec<Pending> = {
        let mut map = pending.lock();
        let ids: Vec<u64> = map.iter().filter(|(_, p)| p.generation == generation).map(|(id, _)| *id).collect();
        ids.into_iter().filter_map(|id| map.remove(&id)).collect()
    };
    for p in failed {
        let _ = p.tx.send(Err(SidecarError::Unavailable(message.to_string())));
    }
}

pub fn resolve_sidecar_script() -> PathBuf {
    if let Ok(path) = std::env::var("CS_ANALYST_SIDECAR") {
        return PathBuf::from(path);
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // src/tauri/src-tauri → src/sidecar
    let src_dist = manifest_dir.join("../../sidecar/dist/index.js");
    if src_dist.exists() {
        return src_dist;
    }
    manifest_dir.join("resources/sidecar/index.js")
}
