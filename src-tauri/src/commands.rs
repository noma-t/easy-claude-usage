use tauri::{Manager, PhysicalPosition, PhysicalSize};

const MIN_CONTENT_HEIGHT: f64 = 80.0;
const MAX_CONTENT_HEIGHT: f64 = 560.0;

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

// フロントエンドが実コンテンツの高さを計測して呼び出す。ポップアップはトレイアイコン
// 付近の下端を基準に表示されるため、高さが変わっても下端の位置を保ったまま
// 上方向に伸縮させる。
//
// set_size()は(このウィンドウのようにshadow付きのundecoratedウィンドウでは)
// 影の分のオフセットを内部で加算した上でOSに outer size を指示する、つまり
// 引数はinner size(コンテンツ領域)として扱われる。outer_size()の値をそのまま
// 渡すと影のオフセット分が呼び出すたびに重複加算されてしまうため、
// サイズ指定にはinner_size、位置計算にはouter_size/outer_positionを使う。
#[tauri::command]
pub fn resize_window(app: tauri::AppHandle, height: f64) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let (Ok(inner_size), Ok(outer_size), Ok(outer_position), Ok(scale_factor)) = (
        window.inner_size(),
        window.outer_size(),
        window.outer_position(),
        window.scale_factor(),
    ) else {
        return;
    };

    let clamped = height.clamp(MIN_CONTENT_HEIGHT, MAX_CONTENT_HEIGHT);
    let new_inner_height = (clamped * scale_factor).round() as u32;
    if new_inner_height == inner_size.height {
        return;
    }

    let bottom = outer_position.y + outer_size.height as i32;
    let _ = window.set_size(PhysicalSize::new(inner_size.width, new_inner_height));

    if let Ok(new_outer_size) = window.outer_size() {
        let new_y = bottom - new_outer_size.height as i32;
        let _ = window.set_position(PhysicalPosition::new(outer_position.x, new_y.max(0)));
    }
}
