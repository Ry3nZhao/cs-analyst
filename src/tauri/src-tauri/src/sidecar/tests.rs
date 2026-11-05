//! 进程管理测试：用一个很小的假模块脚本（需要 PATH 里有 node，没有就跳过）。

use super::*;

const FAKE: &str = r#"
const readline = require('node:readline');
const proto = Number(process.env.FAKE_PROTO || '1');
const out = (o) => process.stdout.write(JSON.stringify(o) + '\n');
readline.createInterface({ input: process.stdin }).on('line', (line) => {
  const req = JSON.parse(line);
  switch (req.method) {
    case 'hello':
      if (require('fs').existsSync(__filename + '.crash')) process.exit(3);
      return out({ id: req.id, result: { protocolVersion: proto, module: { name: 'fake', version: '9.9.9' }, pid: process.pid,
        methods: ['hello', 'ping', 'configure', 'echo', 'slow', 'die'],
        adapters: [{ platform: 'valve', label: 'Valve', enabled: true, available: true, capabilities: ['discover'] }] } });
    case 'ping': return out({ id: req.id, result: { pong: true, uptimeMs: 1, inflight: 0 } });
    case 'configure':
      return out({ id: req.id, result: { adapters: [{ platform: 'valve', label: 'Valve', enabled: req.params.adapters.valve, available: true, capabilities: [] }] } });
    case 'echo': return out({ id: req.id, result: req.params });
    case 'fail': return out({ id: req.id, error: { code: -32004, message: 'URL_EXPIRED: gone', data: { platform: 'renown' } } });
    case 'slow': return; // 永不回复
    case 'die': process.exit(7);
  }
});
console.error('fake started');
"#;

fn node_available() -> bool {
    Command::new("node").arg("--version").output().is_ok()
}

fn fake_script(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cs-analyst-sidecar-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.cjs"));
    std::fs::write(&path, FAKE).unwrap();
    path
}

fn fast_config() -> SupervisorConfig {
    SupervisorConfig {
        heartbeat_interval: Duration::from_millis(100),
        max_missed_heartbeats: 2,
        backoff_base: Duration::from_millis(50),
        backoff_max: Duration::from_millis(200),
        max_consecutive_failures: 3,
        stable_after: Duration::from_secs(60),
    }
}

fn wait_until(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn backoff_doubles_and_caps() {
    let c = SupervisorConfig::default();
    assert_eq!(c.backoff(1), Duration::from_secs(1));
    assert_eq!(c.backoff(2), Duration::from_secs(2));
    assert_eq!(c.backoff(3), Duration::from_secs(4));
    assert_eq!(c.backoff(10), Duration::from_secs(30));
}

#[test]
fn handshake_call_and_errors() {
    if !node_available() {
        return;
    }
    let m = Arc::new(SidecarManager::with_config(fake_script("basic"), None, fast_config()));
    m.set_adapter_switches(&[("valve".into(), false)]).unwrap();
    m.start().unwrap();
    let s = m.status();
    assert_eq!(s.state, ModuleState::Running);
    assert_eq!(s.module_version.as_deref(), Some("9.9.9"));
    assert_eq!(s.protocol_version, Some(PROTOCOL_VERSION));
    // 启动时已下发开关
    assert!(!s.adapters[0].enabled);

    assert_eq!(m.call("echo", json!({ "a": 1 })).unwrap(), json!({ "a": 1 }));
    // 模块未声明的方法直接拒绝
    let err = m.call("fail", json!({})).unwrap_err();
    assert_eq!(err.code(), Some(code::METHOD_NOT_FOUND));
    // 超时
    let err = m.request("slow", json!({}), Duration::from_millis(200)).unwrap_err();
    assert!(matches!(err, SidecarError::Timeout(_)));
    let err = m.request("fail", json!({}), Duration::from_secs(5)).unwrap_err();
    assert_eq!(err.code(), Some(code::URL_EXPIRED));
    assert!(err.to_string().contains("URL_EXPIRED"));
    m.shutdown();
}

#[test]
fn crash_is_restarted_with_backoff() {
    if !node_available() {
        return;
    }
    let m = Arc::new(SidecarManager::with_config(fake_script("crash"), None, fast_config()));
    m.start().unwrap();
    m.spawn_supervisor();
    let pid1 = m.status().pid.unwrap();

    // 进程中途退出：在途请求立即失败，监护线程随后拉起新进程
    let err = m.call("die", json!({})).unwrap_err();
    assert!(matches!(err, SidecarError::Unavailable(_)), "{err:?}");
    assert!(wait_until(Duration::from_secs(10), || {
        let s = m.status();
        s.state == ModuleState::Running && s.pid.is_some_and(|p| p != pid1)
    }));
    let s = m.status();
    assert_eq!(s.restart_count, 1);
    assert!(s.last_exit.is_some());
    assert_eq!(m.call("echo", json!(1)).unwrap(), json!(1));
    assert!(m.logs(100).iter().any(|l| l.contains("fake started")), "stderr 被收集");
    m.shutdown();
}

#[test]
fn gives_up_after_repeated_failures_until_manual_restart() {
    if !node_available() {
        return;
    }
    let script = fake_script("giveup");
    let flag = PathBuf::from(format!("{}.crash", script.display()));
    std::fs::write(&flag, b"").unwrap();
    let m = Arc::new(SidecarManager::with_config(script, None, fast_config()));
    m.spawn_supervisor();
    let _ = m.start();
    assert!(wait_until(Duration::from_secs(15), || m.status().state == ModuleState::Failed));
    assert!(m.call("echo", json!(1)).is_err());
    std::fs::remove_file(&flag).unwrap();
    m.restart().unwrap();
    assert_eq!(m.status().state, ModuleState::Running);
    m.shutdown();
}

#[test]
fn incompatible_protocol_is_not_restarted() {
    if !node_available() {
        return;
    }
    let script = fake_script("incompat").with_file_name("incompat2.cjs");
    std::fs::write(&script, FAKE.replace("process.env.FAKE_PROTO || '1'", "'2'")).unwrap();
    let m = Arc::new(SidecarManager::with_config(script, None, fast_config()));
    let err = m.start().unwrap_err();
    assert!(err.to_string().contains("协议版本不兼容"), "{err}");
    assert_eq!(m.status().state, ModuleState::Incompatible);
    assert!(m.start().is_err());
}
