mod commands;
mod process;
mod tray;

use tauri::WindowEvent;
use tauri_plugin_autostart::ManagerExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::get_usage,
            commands::hide_window,
            tray::menu_refresh,
            tray::menu_toggle_autostart,
            tray::menu_quit,
            tray::menu_update_action,
        ])
        .setup(|app| {
            // 初回起動時はスタートアップ登録をデフォルトで有効化する
            let autostart = app.autolaunch();
            if !autostart.is_enabled().unwrap_or(false) {
                let _ = autostart.enable();
            }

            tray::setup(app.handle())?;
            Ok(())
        })
        // フライアウトUX: どのウィンドウでもフォーカスを失ったら自動的に隠す
        .on_window_event(|window, event| {
            if let WindowEvent::Focused(false) = event {
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
