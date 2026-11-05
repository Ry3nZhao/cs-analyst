//! 托盘：打开 / 立即发现 / 暂停·继续下载 / 退出；关闭窗口时最小化到托盘（可在设置里关掉）。

use super::settings;
use crate::AppState;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Window, WindowEvent, Wry};

const MAIN_WINDOW: &str = "main";

pub fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn pause_label(paused: bool) -> &'static str {
    if paused {
        "继续下载"
    } else {
        "暂停下载"
    }
}

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let paused = app.try_state::<AppState>().map(|s| s.orchestrator.is_paused()).unwrap_or(true);
    let open = MenuItem::with_id(app, "open", "打开 CS Analyst", true, None::<&str>)?;
    let discover = MenuItem::with_id(app, "discover", "立即发现", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "toggle_pause", pause_label(paused), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu: Menu<Wry> = Menu::with_items(app, &[&open, &discover, &pause, &sep, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("CS Analyst")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "discover" => {
                if let Some(state) = app.try_state::<AppState>() {
                    if !state.orchestrator.start_discover_background(app.clone(), "发现 Demo") {
                        super::notify::show(app, "CS Analyst", "已有后台任务正在运行，稍后再试", None);
                    }
                }
            }
            "toggle_pause" => {
                if let Some(state) = app.try_state::<AppState>() {
                    // 与命令 toggle_queue_pause 相同
                    let paused = state.orchestrator.toggle_pause();
                    if !paused {
                        let _ = state.orchestrator.start_queue_background(app.clone(), "恢复下载队列");
                    }
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;

    // 暂停状态也可能在页面里切换：轮询同步菜单文字（很便宜）
    let handle = app.clone();
    std::thread::spawn(move || {
        let mut last = paused;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(800));
            let Some(state) = handle.try_state::<AppState>() else { continue };
            let now = state.orchestrator.is_paused();
            if now != last {
                let _ = pause.set_text(pause_label(now));
                last = now;
            }
        }
    });
    Ok(())
}

/// 在 `Builder::on_window_event` 里调用。
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if window.label() != MAIN_WINDOW {
        return;
    }
    if let WindowEvent::CloseRequested { api, .. } = event {
        let app = window.app_handle();
        let to_tray = app
            .try_state::<AppState>()
            .and_then(|s| settings::get(&s.db.lock().conn).ok())
            .map(|s| s.close_to_tray)
            .unwrap_or(false);
        // 托盘没建起来时不能藏窗口，否则用户再也找不回来
        if to_tray && app.tray_by_id("main").is_some() {
            api.prevent_close();
            let _ = window.hide();
        }
    }
}
