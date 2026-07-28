use tauri::State;

use crate::config::app_settings::AppSettings;
use crate::errors::CommandError;
use crate::AppState;

/// 获取应用设置
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, CommandError> {
    log::info!("获取应用设置");
    let config = state.config.lock().await;
    let settings = config.load_app_settings().map_err(|e| {
        log::error!("加载应用设置失败: {}", e);
        e
    })?;
    log::info!("获取应用设置成功");
    Ok(settings)
}

/// 更新应用设置，接收部分 JSON 合并到现有设置
#[tauri::command]
pub async fn update_settings(
    settings: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("更新应用设置");
    let config = state.config.lock().await;
    let current = config.load_app_settings().map_err(|e| {
        log::error!("加载应用设置失败: {}", e);
        e
    })?;

    // 将现有设置序列化为 JSON，与传入的 JSON 合并，再反序列化回来
    let mut current_json = serde_json::to_value(&current).map_err(|e| {
        log::error!("序列化应用设置失败: {}", e);
        e
    })?;
    json_merge(&mut current_json, &settings);
    let merged: AppSettings = serde_json::from_value(current_json).map_err(|e| {
        log::error!("反序列化合并后的设置失败: {}", e);
        e
    })?;

    config.save_app_settings(&merged).map_err(|e| {
        log::error!("保存应用设置失败: {}", e);
        e
    })?;
    log::info!("更新应用设置成功");
    Ok(())
}

/// 递归合并 JSON 对象，source 中的字段覆盖 target 中的同名字段
fn json_merge(target: &mut serde_json::Value, source: &serde_json::Value) {
    match (target, source) {
        (serde_json::Value::Object(t), serde_json::Value::Object(s)) => {
            for (key, value) in s {
                let entry = t.entry(key.clone()).or_insert(serde_json::Value::Null);
                json_merge(entry, value);
            }
        }
        (t, s) => {
            *t = s.clone();
        }
    }
}

/// 检查 Git Bash 是否可用
/// 优先检查用户配置的路径，若已配置则检测文件是否存在；
/// 若未配置或文件不存在，则从 PATH 环境变量自动检测
#[tauri::command]
pub async fn check_git_bash_path(state: State<'_, AppState>) -> Result<bool, CommandError> {
    let config = state.config.lock().await;
    let settings = config.load_app_settings().map_err(|e| {
        log::error!("加载应用设置失败: {}", e);
        e
    })?;

    // 1. 用户已配置路径且文件存在
    if !settings.git_bash_path.is_empty() {
        let path = std::path::Path::new(&settings.git_bash_path);
        if path.exists() {
            return Ok(true);
        }
    }

    // 2. 从 PATH 环境变量自动检测
    let found = find_git_bash_from_path_inner();
    Ok(found)
}

/// 从 PATH 环境变量查找 Git Bash（与 builtin.rs 中逻辑一致）
fn find_git_bash_from_path_inner() -> bool {
    let path_env = match std::env::var_os("PATH") {
        Some(v) => v,
        None => return false,
    };

    #[cfg(target_os = "windows")]
    {
        use std::path::PathBuf;
        let paths: Vec<PathBuf> = std::env::split_paths(&path_env).collect();

        // 策略 a: 直接查找 bash.exe
        for dir in &paths {
            let bash_candidate = dir.join("bash.exe");
            if bash_candidate.exists() {
                return true;
            }
        }

        // 策略 b: 从 git.exe 推断 bash.exe 位置
        for dir in &paths {
            let git_candidate = dir.join("git.exe");
            if git_candidate.exists() {
                if let Some(parent) = dir.parent() {
                    if parent.join("bin").join("bash.exe").exists() {
                        return true;
                    }
                    if parent.join("usr").join("bin").join("bash.exe").exists() {
                        return true;
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let paths: Vec<&std::path::Path> = std::env::split_paths(&path_env)
            .map(|p| p.as_path())
            .collect();
        for dir in paths {
            let bash_candidate = dir.join("bash");
            if bash_candidate.exists() {
                return true;
            }
        }
    }

    false
}
