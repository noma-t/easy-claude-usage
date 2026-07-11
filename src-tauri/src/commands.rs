#[tauri::command]
pub async fn get_usage() -> Result<String, String> {
    crate::process::fetch_usage().await
}

#[tauri::command]
pub fn hide_window(window: tauri::Window) {
    let _ = window.hide();
}
