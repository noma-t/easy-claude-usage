use tauri::Manager;

#[tauri::command]
pub async fn get_usage() -> Result<String, String> {
    crate::process::fetch_usage().await
}

#[tauri::command]
pub fn hide_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}
