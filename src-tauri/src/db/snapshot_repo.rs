use rusqlite::Connection;
use chrono::Utc;
use crate::errors::CommandError;
use crate::models::VersionInfo;

/// 创建版本快照记录
pub fn create_snapshot(
    conn: &Connection,
    id: &str,
    workspace_id: &str,
    session_id: &str,
    file_path: &str,
    snapshot_path: &str,
    operation: &str,
) -> Result<(), CommandError> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO version_snapshots
            (id, workspace_id, session_id, file_path, snapshot_path, operation, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, workspace_id, session_id, file_path, snapshot_path, operation, now],
    )?;
    Ok(())
}

/// 查询版本快照列表，支持按工作区和文件路径筛选
pub fn list_snapshots(
    conn: &Connection,
    workspace_id: Option<&str>,
    file_path: Option<&str>,
) -> Vec<VersionInfo> {
    let mut sql = String::from(
        "SELECT id, workspace_id, session_id, file_path, snapshot_path, operation, created_at
         FROM version_snapshots WHERE 1=1"
    );
    let mut param_idx = 1u32;
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(wid) = workspace_id {
        sql.push_str(&format!(" AND workspace_id = ?{}", param_idx));
        param_values.push(Box::new(wid.to_string()));
        param_idx += 1;
    }

    if let Some(fp) = file_path {
        sql.push_str(&format!(" AND file_path = ?{}", param_idx));
        param_values.push(Box::new(fp.to_string()));
    }

    sql.push_str(" ORDER BY created_at DESC");

    let params: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut rows = match stmt.query(params.as_slice()) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut result = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        let session_id: String = match row.get(2) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let operation: String = match row.get(5) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let info = VersionInfo {
            version_id: match row.get(0) {
                Ok(v) => v,
                Err(_) => continue,
            },
            path: match row.get(4) {
                Ok(v) => v,
                Err(_) => continue,
            },
            timestamp: match row.get(6) {
                Ok(v) => v,
                Err(_) => continue,
            },
            operation: operation.clone(),
            description: operation,
            size: 0,
            session_id: if session_id.is_empty() {
                None
            } else {
                Some(session_id)
            },
        };
        result.push(info);
    }
    result
}

/// 删除指定快照记录
pub fn delete_snapshot(conn: &Connection, id: &str) -> Result<(), CommandError> {
    let affected = conn.execute(
        "DELETE FROM version_snapshots WHERE id = ?1",
        rusqlite::params![id],
    )?;
    if affected == 0 {
        return Err(CommandError::db(
            crate::errors::DB_RECORD_NOT_FOUND,
            format!("快照不存在: {}", id),
        ));
    }
    Ok(())
}

/// 根据保留策略清理过期快照
/// 返回被清理的快照 ID 列表（用于删除对应的快照文件）
/// policy: "byCount" | "byDays" | "both"
/// max_count: 按数量保留时，每个文件最多保留的快照数
/// max_days: 按天数保留时，保留最近多少天的快照
pub fn cleanup_snapshots(
    conn: &Connection,
    workspace_id: &str,
    file_path: &str,
    policy: &str,
    max_count: u32,
    max_days: u32,
) -> Vec<String> {
    let mut deleted_ids = Vec::new();

    match policy {
        "byCount" => {
            // 按数量保留：每个文件只保留最近的 max_count 个快照
            deleted_ids = cleanup_by_count(conn, workspace_id, file_path, max_count);
        }
        "byDays" => {
            // 按天数保留：删除超过 max_days 天的快照
            deleted_ids = cleanup_by_days(conn, workspace_id, file_path, max_days);
        }
        "both" => {
            // 两者都满足：先按数量清理，再按天数清理
            let by_count = cleanup_by_count(conn, workspace_id, file_path, max_count);
            let by_days = cleanup_by_days(conn, workspace_id, file_path, max_days);
            // 合并去重
            let mut seen = std::collections::HashSet::new();
            for id in by_count.into_iter().chain(by_days) {
                if seen.insert(id.clone()) {
                    deleted_ids.push(id);
                }
            }
        }
        _ => {}
    }

    if !deleted_ids.is_empty() {
        log::info!(
            "快照清理完成: workspace_id={}, file_path={}, policy={}, 清理数量={}",
            workspace_id, file_path, policy, deleted_ids.len()
        );
    }

    deleted_ids
}

/// 按数量保留：每个文件只保留最近的 max_count 个快照，删除更早的
fn cleanup_by_count(
    conn: &Connection,
    workspace_id: &str,
    file_path: &str,
    max_count: u32,
) -> Vec<String> {
    if max_count == 0 {
        return Vec::new();
    }

    // 查询该文件的所有快照，按创建时间降序排列
    let sql = "SELECT id FROM version_snapshots
               WHERE workspace_id = ?1 AND file_path = ?2
               ORDER BY created_at DESC";

    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut rows = match stmt.query(rusqlite::params![workspace_id, file_path]) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut all_ids = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        if let Ok(id) = row.get::<_, String>(0) {
            all_ids.push(id);
        }
    }

    // 保留前 max_count 个（最新的），删除其余的
    let to_delete: Vec<&str> = all_ids.iter()
        .skip(max_count as usize)
        .map(|s| s.as_str())
        .collect();

    let mut deleted = Vec::new();
    for id in to_delete {
        if conn.execute("DELETE FROM version_snapshots WHERE id = ?1", rusqlite::params![id]).is_ok() {
            deleted.push(id.to_string());
        }
    }

    deleted
}

/// 按天数保留：删除超过 max_days 天的快照
fn cleanup_by_days(
    conn: &Connection,
    workspace_id: &str,
    file_path: &str,
    max_days: u32,
) -> Vec<String> {
    if max_days == 0 {
        return Vec::new();
    }

    // 计算截止时间：只保留最近 max_days 天的快照
    let cutoff = chrono::Utc::now() - chrono::Duration::days(max_days as i64);
    let cutoff_str = cutoff.to_rfc3339();

    let sql = "SELECT id FROM version_snapshots
               WHERE workspace_id = ?1 AND file_path = ?2 AND created_at < ?3";

    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut rows = match stmt.query(rusqlite::params![workspace_id, file_path, cutoff_str]) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut to_delete = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        if let Ok(id) = row.get::<_, String>(0) {
            to_delete.push(id);
        }
    }

    let mut deleted = Vec::new();
    for id in &to_delete {
        if conn.execute("DELETE FROM version_snapshots WHERE id = ?1", rusqlite::params![id]).is_ok() {
            deleted.push(id.clone());
        }
    }

    deleted
}
