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
