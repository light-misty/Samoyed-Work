//! Skill 仓库:管理 Skill 的启用/禁用配置
//! Skill 内容本身从文件系统加载,数据库仅存储用户覆盖

use crate::errors::CommandError;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// Skill 覆盖配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillOverride {
    pub id: String,
    pub skill_name: String,
    pub workspace_id: String,
    pub enabled: bool,
    pub custom_config: Option<String>,
}

/// 插入或更新 Skill 覆盖配置
/// 使用 ON CONFLICT(skill_name, workspace_id) DO UPDATE 实现 upsert
/// enabled 在数据库中存为 INTEGER(0/1),此处从 bool 转换
pub fn upsert_override(
    conn: &Connection,
    override_config: &SkillOverride,
) -> Result<(), CommandError> {
    // bool 转 i32(0/1)
    let enabled_i32: i32 = if override_config.enabled { 1 } else { 0 };

    conn.execute(
        "INSERT INTO skill_overrides (id, skill_name, workspace_id, enabled, custom_config, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(skill_name, workspace_id) DO UPDATE SET
            enabled = excluded.enabled,
            custom_config = excluded.custom_config,
            updated_at = excluded.updated_at",
        rusqlite::params![
            override_config.id,
            override_config.skill_name,
            override_config.workspace_id,
            enabled_i32,
            override_config.custom_config,
        ],
    )?;
    Ok(())
}

/// 查询指定工作区的 Skill 覆盖配置
/// 无记录时返回空 Vec
pub fn list_overrides_by_workspace(
    conn: &Connection,
    workspace_id: &str,
) -> Result<Vec<SkillOverride>, CommandError> {
    let mut stmt = conn.prepare(
        "SELECT id, skill_name, workspace_id, enabled, custom_config
         FROM skill_overrides
         WHERE workspace_id = ?1",
    )?;

    // 逐行映射,enabled 从 i32 转为 bool(非 0 即真)
    let overrides = stmt.query_map(rusqlite::params![workspace_id], |row| {
        let enabled_i32: i32 = row.get(3)?;
        Ok(SkillOverride {
            id: row.get(0)?,
            skill_name: row.get(1)?,
            workspace_id: row.get(2)?,
            enabled: enabled_i32 != 0,
            custom_config: row.get(4)?,
        })
    })?;

    let mut result = Vec::new();
    for override_config in overrides {
        result.push(override_config?);
    }
    Ok(result)
}

/// 删除 Skill 覆盖配置
pub fn delete_override(conn: &Connection, id: &str) -> Result<(), CommandError> {
    conn.execute(
        "DELETE FROM skill_overrides WHERE id = ?1",
        rusqlite::params![id],
    )?;
    Ok(())
}
