use tauri::State;

use crate::config::app_settings::AppSettings;
use crate::errors::CommandError;
use crate::AppState;

/// 获取应用设置
#[tauri::command]
pub async fn get_settings(
    state: State<'_, AppState>,
) -> Result<AppSettings, CommandError> {
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
