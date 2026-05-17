use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::errors::{CommandError, FS_PATH_NOT_FOUND};

/// 工作区条目
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntry {
    /// 唯一标识
    pub id: String,
    /// 工作区名称
    pub name: String,
    /// 工作区根路径
    pub path: String,
    /// 覆盖默认作者名，为空时使用全局设置
    #[serde(default)]
    pub author_name_override: String,
    /// 创建时间（ISO 8601 格式）
    pub created_at: String,
}

/// 工作区配置，包含所有已注册的工作区
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacesConfig {
    #[serde(default)]
    pub workspaces: Vec<WorkspaceEntry>,
}

/// 获取工作区配置文件路径
fn config_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("config").join("workspaces.json")
}

/// 从磁盘加载工作区配置，文件不存在时返回默认值
pub fn load_workspaces(data_dir: &Path) -> Result<WorkspacesConfig, CommandError> {
    let path = config_path(data_dir);
    if !path.exists() {
        return Ok(WorkspacesConfig::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let config: WorkspacesConfig = serde_json::from_str(&content)?;
    Ok(config)
}

/// 将工作区配置保存到磁盘
pub fn save_workspaces(data_dir: &Path, config: &WorkspacesConfig) -> Result<(), CommandError> {
    let path = config_path(data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(config)?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// 添加工作区，返回新创建的工作区条目
pub fn add_workspace(
    config: &mut WorkspacesConfig,
    path: &str,
    name: &str,
) -> Result<WorkspaceEntry, CommandError> {
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();

    let entry = WorkspaceEntry {
        id: id.clone(),
        name: name.to_string(),
        path: path.to_string(),
        author_name_override: String::new(),
        created_at,
    };

    config.workspaces.push(entry.clone());
    Ok(entry)
}

/// 移除指定 ID 的工作区
pub fn remove_workspace(config: &mut WorkspacesConfig, id: &str) -> Result<(), CommandError> {
    let index = config
        .workspaces
        .iter()
        .position(|w| w.id == id)
        .ok_or_else(|| {
            CommandError::fs(
                FS_PATH_NOT_FOUND,
                format!("工作区 '{}' 不存在", id),
            )
        })?;

    config.workspaces.remove(index);
    Ok(())
}
