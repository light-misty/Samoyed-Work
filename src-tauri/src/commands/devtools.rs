use tauri::Manager;

/// 切换开发人员工具（DevTools）的显示状态
/// 如果 DevTools 已打开则关闭，否则打开
#[tauri::command]
pub async fn toggle_devtools(app: tauri::AppHandle) -> Result<bool, String> {
    let webview_window = app.get_webview_window("main").ok_or("未找到主窗口")?;

    #[cfg(debug_assertions)]
    {
        let is_open = webview_window.is_devtools_open();
        if is_open {
            webview_window.close_devtools();
            log::info!("开发人员工具已关闭");
        } else {
            webview_window.open_devtools();
            log::info!("开发人员工具已打开");
        }
        Ok(!is_open)
    }

    #[cfg(not(debug_assertions))]
    {
        // 生产构建中不启用 DevTools
        log::warn!("生产构建中不支持开发人员工具");
        Err("生产构建中不支持开发人员工具".to_string())
    }
}
