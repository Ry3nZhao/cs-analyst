//! 下载模块 JSON-RPC 协议的 Rust 侧定义。TS 侧见 `src/sidecar/protocol`，文档见 `docs/downloader-protocol.md`。

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// 主程序实现的协议主版本；模块 `hello` 返回的版本必须相同。
pub const PROTOCOL_VERSION: u32 = 1;

/// 错误码（与 `RpcErrorCode` 一致）。
#[allow(dead_code)]
pub mod code {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL: i64 = -32000;
    pub const ADAPTER_DISABLED: i64 = -32001;
    pub const ADAPTER_UNAVAILABLE: i64 = -32002;
    pub const ADAPTER_FAILED: i64 = -32003;
    pub const URL_EXPIRED: i64 = -32004;
    pub const UNKNOWN_PLATFORM: i64 = -32005;
}

/// 每个方法的调用超时。
pub fn timeout_for(method: &str) -> Duration {
    Duration::from_secs(match method {
        "hello" => 20,
        "ping" => 5,
        "configure" | "shutdown" => 10,
        "resolve_5eplay_player_id" => 30,
        "discover" => 180,
        // 登录 Steam + 逐个向 GC 请求，量大时较慢
        "resolve_valve_authcode_urls" => 600,
        // 几百 MB 的 demo 在慢速网络下要很久；卡住的连接由模块内的 HTTP 超时处理
        "download" => 1800,
        _ => 120,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AdapterInfo {
    pub platform: String,
    pub label: String,
    pub enabled: bool,
    pub available: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModuleIdent {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloResult {
    pub protocol_version: u32,
    pub module: ModuleIdent,
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub adapters: Vec<AdapterInfo>,
    #[serde(default)]
    pub runtime: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigureResult {
    #[serde(default)]
    pub adapters: Vec<AdapterInfo>,
}
