use tauri::{
    menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition,
};
use tauri_plugin_autostart::ManagerExt;

const WINDOW_LABEL: &str = "main";
const REFRESH_EVENT: &str = "usage://refresh";
const UPDATE_CHECK_EVENT: &str = "update://check";

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let refresh = MenuItemBuilder::with_id("refresh", "再取得").build(app)?;
    let check_update = MenuItemBuilder::with_id("check_update", "アップデートを確認").build(app)?;
    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);
    let autostart = CheckMenuItemBuilder::with_id("autostart", "スタートアップ時に起動")
        .checked(autostart_enabled)
        .build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "終了").build(app)?;
    let menu = MenuBuilder::new(app)
        .item(&refresh)
        .item(&check_update)
        .item(&autostart)
        .separator()
        .item(&quit)
        .build()?;

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().cloned().expect("default window icon is set in tauri.conf.json"))
        .tooltip("Claude Usage")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "refresh" => show_and_notify(app, None),
            "check_update" => show_and_notify_update(app),
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

fn show_and_notify_update(app: &AppHandle) {
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        return;
    };

    let _ = window.show();
    let _ = window.set_focus();
    let _ = app.emit(UPDATE_CHECK_EVENT, ());
}
