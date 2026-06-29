use crate::errors::{CommandError, FS_IO_ERROR, FS_PATH_NOT_FOUND};
use serde::Serialize;
use tauri::Manager;

/// 日志路径信息
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogPathInfo {
    /// 日志目录路径（供前端展示）
    log_source: String,
}

/// 解析当前活跃的日志目录路径
fn resolve_log_dir_path(app_handle: &tauri::AppHandle) -> std::path::PathBuf {
    match crate::utils::logger::current_log_dir() {
        Some(dir) => dir.to_path_buf(),
        None => crate::utils::logger::resolve_log_dir(
            app_handle.path().app_log_dir().ok(),
            app_handle.path().app_data_dir().ok(),
        ),
    }
}

/// 获取日志目录路径
#[tauri::command]
pub async fn get_log_path(app_handle: tauri::AppHandle) -> Result<LogPathInfo, CommandError> {
    let log_dir = resolve_log_dir_path(&app_handle);
    Ok(LogPathInfo {
        log_source: log_dir.to_string_lossy().to_string(),
    })
}

/// 在系统文件管理器中打开指定目录
#[tauri::command]
pub async fn open_directory(path: String) -> Result<(), CommandError> {
    let dir = std::path::Path::new(&path);
    if !dir.exists() {
        return Err(CommandError::fs(
            FS_PATH_NOT_FOUND,
            format!("目录不存在: {}", path),
        ));
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| CommandError::fs(FS_IO_ERROR, format!("打开目录失败: {}", e)))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| CommandError::fs(FS_IO_ERROR, format!("打开目录失败: {}", e)))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| CommandError::fs(FS_IO_ERROR, format!("打开目录失败: {}", e)))?;
    }

    Ok(())
}
