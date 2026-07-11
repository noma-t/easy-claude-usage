use std::sync::Mutex;

use serde::Serialize;
use tauri::{
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow,
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_updater::{Update, UpdaterExt};

const WINDOW_LABEL: &str = "main";
const MENU_WINDOW_LABEL: &str = "tray-context-menu";
const REFRESH_EVENT: &str = "usage://refresh";
const MENU_STATE_EVENT: &str = "tray-context-menu://state";

const CHECK_LABEL: &str = "アップデートを確認";
const CHECKING_LABEL: &str = "確認中...";
const APPLY_LABEL: &str = "アップデート";
const APPLYING_LABEL: &str = "適用中...";

// 通常アイコンと同じ図案(icons/32x32.png)にバッジの丸を焼き込んだものをトレイに出し分ける
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/32x32.png");
// CSSの --status-warning と統一した色
const BADGE_COLOR: [u8; 4] = [0xfa, 0xb2, 0x19, 0xff];

struct UpdateState {
    pending: Mutex<Option<Update>>,
    tray: TrayIcon,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MenuState {
    update_label: String,
    update_busy: bool,
    autostart_enabled: bool,
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let tray = TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().cloned().expect("default window icon is set in tauri.conf.json"))
        .tooltip("Claude Usage")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            {
                match button {
                    MouseButton::Left => toggle_window(tray.app_handle(), position),
                    MouseButton::Right => show_context_menu(tray.app_handle(), position),
                    _ => {}
                }
            }
        })
        .build(app)?;

    app.manage(UpdateState {
        pending: Mutex::new(None),
        tray,
    });

    let startup_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        check_for_update(&startup_handle).await;
    });

    Ok(())
}

#[tauri::command]
pub fn menu_refresh(app: AppHandle) {
    if let Some(window) = app.get_webview_window(MENU_WINDOW_LABEL) {
        let _ = window.hide();
    }
    show_and_notify(&app, None);
}

#[tauri::command]
pub fn menu_toggle_autostart(app: AppHandle) -> bool {
    let autostart = app.autolaunch();
    let enabled = autostart.is_enabled().unwrap_or(false);
    let result = if enabled { autostart.disable() } else { autostart.enable() };
    if result.is_ok() {
        !enabled
    } else {
        enabled
    }
}

#[tauri::command]
pub fn menu_quit(app: AppHandle) {
    app.exit(0);
}

// 未確認/更新なしの時は手動チェックを、更新が見つかっている時はダウンロード&適用を行う
#[tauri::command]
pub fn menu_update_action(app: AppHandle) {
    let has_pending = app.state::<UpdateState>().pending.lock().unwrap().is_some();
    if has_pending {
        emit_menu_state(&app, APPLYING_LABEL, true);
        tauri::async_runtime::spawn(async move { apply_update(&app).await });
    } else {
        emit_menu_state(&app, CHECKING_LABEL, true);
        tauri::async_runtime::spawn(async move { check_for_update(&app).await });
    }
}

fn toggle_window(app: &AppHandle, click_position: PhysicalPosition<f64>) {
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        show_and_notify(app, Some(click_position));
    }
}

fn show_and_notify(app: &AppHandle, click_position: Option<PhysicalPosition<f64>>) {
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        return;
    };

    if let Some(position) = click_position {
        position_window_above(&window, position);
    }

    let _ = window.show();
    let _ = window.set_focus();
    let _ = app.emit(REFRESH_EVENT, ());
}

// トレイの右クリックメニューはネイティブメニューだと項目クリックで即座に閉じてしまい、
// "確認中..." のような進行状況を見せられないため、専用のポップアップウィンドウで代替する
fn show_context_menu(app: &AppHandle, click_position: PhysicalPosition<f64>) {
    let Some(window) = app.get_webview_window(MENU_WINDOW_LABEL) else {
        return;
    };

    if let Some(main_window) = app.get_webview_window(WINDOW_LABEL) {
        let _ = main_window.hide();
    }

    let _ = app.emit_to(MENU_WINDOW_LABEL, MENU_STATE_EVENT, current_menu_state(app));
    position_window_above(&window, click_position);
    let _ = window.show();
    let _ = window.set_focus();
}

fn position_window_above(window: &WebviewWindow, position: PhysicalPosition<f64>) {
    if let Ok(size) = window.outer_size() {
        let x = position.x - (size.width as f64 / 2.0);
        let y = position.y - size.height as f64 - 8.0;
        let _ = window.set_position(PhysicalPosition::new(x.max(0.0), y.max(0.0)));
    }
}

fn current_menu_state(app: &AppHandle) -> MenuState {
    let has_pending = app.state::<UpdateState>().pending.lock().unwrap().is_some();
    MenuState {
        update_label: if has_pending { APPLY_LABEL } else { CHECK_LABEL }.to_string(),
        update_busy: false,
        autostart_enabled: app.autolaunch().is_enabled().unwrap_or(false),
    }
}

fn emit_menu_state(app: &AppHandle, update_label: &str, update_busy: bool) {
    let state = MenuState {
        update_label: update_label.to_string(),
        update_busy,
        autostart_enabled: app.autolaunch().is_enabled().unwrap_or(false),
    };
    let _ = app.emit_to(MENU_WINDOW_LABEL, MENU_STATE_EVENT, state);
}

async fn check_for_update(app: &AppHandle) {
    let result = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(err) => {
            eprintln!("updater init failed: {err}");
            emit_menu_state(app, CHECK_LABEL, false);
            return;
        }
    };

    let state = app.state::<UpdateState>();
    match result {
        Ok(Some(update)) => {
            *state.pending.lock().unwrap() = Some(update);
            let _ = state.tray.set_icon(Some(badge_icon()));
            emit_menu_state(app, APPLY_LABEL, false);
        }
        Ok(None) => {
            *state.pending.lock().unwrap() = None;
            let _ = state.tray.set_icon(app.default_window_icon().cloned());
            emit_menu_state(app, CHECK_LABEL, false);
        }
        Err(err) => {
            eprintln!("update check failed: {err}");
            emit_menu_state(app, CHECK_LABEL, false);
        }
    }
}

async fn apply_update(app: &AppHandle) {
    let update = app.state::<UpdateState>().pending.lock().unwrap().take();
    let Some(update) = update else {
        emit_menu_state(app, CHECK_LABEL, false);
        return;
    };

    if let Err(err) = update.download_and_install(|_, _| {}, || {}).await {
        eprintln!("update install failed: {err}");
        emit_menu_state(app, CHECK_LABEL, false);
        return;
    }

    app.restart();
}

// icons/32x32.png の右上に丸を焼き込んだアイコンを都度生成する
fn badge_icon() -> Image<'static> {
    let base = Image::from_bytes(TRAY_ICON_BYTES).expect("bundled tray icon must be a valid PNG");
    let width = base.width();
    let height = base.height();
    let mut rgba = base.rgba().to_vec();

    let radius = (width.min(height) as f32 * 0.32).round() as i32;
    let cx = width as i32 - radius - 1;
    let cy = radius + 1;

    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= radius * radius {
                let idx = ((y as u32 * width + x as u32) * 4) as usize;
                rgba[idx..idx + 4].copy_from_slice(&BADGE_COLOR);
            }
        }
    }

    Image::new_owned(rgba, width, height)
}
