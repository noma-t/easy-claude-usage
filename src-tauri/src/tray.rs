use std::sync::Mutex;

use tauri::{
    image::Image,
    menu::{CheckMenuItemBuilder, MenuBuilder, MenuItem, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition,
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_updater::{Update, UpdaterExt};

const WINDOW_LABEL: &str = "main";
const REFRESH_EVENT: &str = "usage://refresh";

const CHECK_LABEL: &str = "アップデートを確認";
const APPLY_LABEL: &str = "アップデート";

// 通常アイコンと同じ図案(icons/32x32.png)にバッジの丸を焼き込んだものをトレイに出し分ける
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/32x32.png");
// CSSの --status-warning と統一した色
const BADGE_COLOR: [u8; 4] = [0xfa, 0xb2, 0x19, 0xff];

struct UpdateState {
    pending: Mutex<Option<Update>>,
    tray: TrayIcon,
    menu_item: MenuItem<tauri::Wry>,
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let refresh = MenuItemBuilder::with_id("refresh", "再取得").build(app)?;
    let update_item = MenuItemBuilder::with_id("update_action", CHECK_LABEL).build(app)?;
    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);
    let autostart = CheckMenuItemBuilder::with_id("autostart", "スタートアップ時に起動")
        .checked(autostart_enabled)
        .build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "終了").build(app)?;
    let menu = MenuBuilder::new(app)
        .item(&refresh)
        .item(&update_item)
        .item(&autostart)
        .separator()
        .item(&quit)
        .build()?;

    let tray = TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().cloned().expect("default window icon is set in tauri.conf.json"))
        .tooltip("Claude Usage")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "refresh" => show_and_notify(app, None),
            "update_action" => handle_update_action(app),
            "autostart" => toggle_autostart(app, &autostart),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            {
                toggle_window(tray.app_handle(), position);
            }
        })
        .build(app)?;

    app.manage(UpdateState {
        pending: Mutex::new(None),
        tray,
        menu_item: update_item,
    });

    let startup_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        check_for_update(&startup_handle).await;
    });

    Ok(())
}

fn toggle_autostart(app: &AppHandle, item: &tauri::menu::CheckMenuItem<tauri::Wry>) {
    let autostart = app.autolaunch();
    let enabled = autostart.is_enabled().unwrap_or(false);
    let result = if enabled { autostart.disable() } else { autostart.enable() };
    if result.is_ok() {
        let _ = item.set_checked(!enabled);
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
        if let Ok(size) = window.outer_size() {
            let x = position.x - (size.width as f64 / 2.0);
            let y = position.y - size.height as f64 - 8.0;
            let _ = window.set_position(PhysicalPosition::new(x.max(0.0), y.max(0.0)));
        }
    }

    let _ = window.show();
    let _ = window.set_focus();
    let _ = app.emit(REFRESH_EVENT, ());
}

// 未確認/更新なしの時は手動チェックを、更新が見つかっている時はダウンロード&適用を行う
fn handle_update_action(app: &AppHandle) {
    let has_pending = app.state::<UpdateState>().pending.lock().unwrap().is_some();
    let app = app.clone();
    if has_pending {
        tauri::async_runtime::spawn(async move { apply_update(&app).await });
    } else {
        tauri::async_runtime::spawn(async move { check_for_update(&app).await });
    }
}

async fn check_for_update(app: &AppHandle) {
    let result = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(err) => {
            eprintln!("updater init failed: {err}");
            return;
        }
    };

    let state = app.state::<UpdateState>();
    match result {
        Ok(Some(update)) => {
            *state.pending.lock().unwrap() = Some(update);
            let _ = state.menu_item.set_text(APPLY_LABEL);
            let _ = state.tray.set_icon(Some(badge_icon()));
        }
        Ok(None) => {
            *state.pending.lock().unwrap() = None;
            let _ = state.menu_item.set_text(CHECK_LABEL);
            let _ = state.tray.set_icon(app.default_window_icon().cloned());
        }
        Err(err) => eprintln!("update check failed: {err}"),
    }
}

async fn apply_update(app: &AppHandle) {
    let update = app.state::<UpdateState>().pending.lock().unwrap().take();
    let Some(update) = update else { return };

    if let Err(err) = update.download_and_install(|_, _| {}, || {}).await {
        eprintln!("update install failed: {err}");
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
