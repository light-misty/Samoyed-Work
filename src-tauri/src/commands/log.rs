use crate::errors::{CommandError, FS_IO_ERROR, FS_PATH_NOT_FOUND};
use tauri::Manager;

/// 获取错误日志文件内容
/// 读取 Tauri 推荐日志目录下的 docagent.log 文件并返回其内容
#[tauri::command]
pub async fn get_error_log(app_handle: tauri::AppHandle) -> Result<String, CommandError> {
    log::info!("获取错误日志");

    // 日志文件路径：使用 Tauri 推荐的日志目录
    // 与 lib.rs 中日志初始化使用相同的目录
    let log_dir = app_handle
        .path()
        .app_log_dir()
        .unwrap_or_else(|_| {
            app_handle.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("log")).join("log")
        });
    let log_path = log_dir.join("docagent.log");

    if !log_path.exists() {
        log::warn!("日志文件不存在: {:?}", log_path);
        return Err(CommandError::fs(
            FS_PATH_NOT_FOUND,
            format!("日志文件不存在: {}", log_path.display()),
        ));
    }

    let content = std::fs::read_to_string(&log_path).map_err(|e| {
        log::error!("读取日志文件失败: {}", e);
        CommandError::fs(FS_IO_ERROR, format!("读取日志文件失败: {}", e))
    })?;

    log::info!("获取错误日志成功，长度: {} 字节", content.len());
    Ok(content)
}
