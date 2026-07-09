use crate::errors::CommandError;
use crate::models::context_memory::ContextSessionSummary;
use chrono::Utc;
use rusqlite::Connection;

/// 创建会话摘要
#[allow(clippy::too_many_arguments)]
pub fn create_session_summary(
    conn: &Connection,
    id: &str,
    session_id: &str,
    workspace_id: &str,
    user_goal: &str,
    result_summary: &str,
    files_involved: &str,
    tools_used: &str,
    errors_resolved: &str,
) -> Result<(), CommandError> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO session_summaries
            (id, session_id, workspace_id, user_goal, result_summary,
             files_involved, tools_used, errors_resolved, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            id,
            session_id,
            workspace_id,
            user_goal,
            result_summary,
            files_involved,
            tools_used,
            errors_resolved,
            now,
        ],
    )?;
    Ok(())
}

/// 查询指定工作区最近的会话摘要
/// 按创建时间降序排列，最多返回 limit 条
/// exclude_session_id: 可选排除的会话ID，用于避免将当前会话的摘要重新注入自身上下文
pub fn list_summaries_by_workspace(
    conn: &Connection,
    workspace_id: &str,
    limit: u32,
    exclude_session_id: Option<&str>,
) -> Vec<ContextSessionSummary> {
    // 根据是否排除特定会话构建不同的SQL
    let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) =
        if let Some(exclude_id) = exclude_session_id {
            (
                "SELECT id, session_id, workspace_id, user_goal, result_summary,
                    files_involved, tools_used, errors_resolved, created_at
             FROM session_summaries
             WHERE workspace_id = ?1 AND session_id != ?3
             ORDER BY created_at DESC
             LIMIT ?2"
                    .to_string(),
                vec![
                    Box::new(workspace_id.to_string()),
                    Box::new(limit),
                    Box::new(exclude_id.to_string()),
                ],
            )
        } else {
            (
                "SELECT id, session_id, workspace_id, user_goal, result_summary,
                    files_involved, tools_used, errors_resolved, created_at
             FROM session_summaries
             WHERE workspace_id = ?1
             ORDER BY created_at DESC
             LIMIT ?2"
                    .to_string(),
                vec![Box::new(workspace_id.to_string()), Box::new(limit)],
            )
        };

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    // 将 Box<dyn ToSql> 转换为 rusqlite 可用的引用切片
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut rows = match stmt.query(param_refs.as_slice()) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut result = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        let summary = ContextSessionSummary {
            id: row.get(0).unwrap_or_default(),
            session_id: row.get(1).unwrap_or_default(),
            workspace_id: row.get(2).unwrap_or_default(),
            user_goal: row.get(3).unwrap_or_default(),
            result_summary: row.get(4).unwrap_or_default(),
            files_involved: row.get(5).unwrap_or_default(),
            tools_used: row.get(6).unwrap_or_default(),
            errors_resolved: row.get(7).unwrap_or_default(),
            created_at: row.get(8).unwrap_or_default(),
        };
        result.push(summary);
    }
    result
}

/// 删除指定会话的摘要
pub fn delete_summary_by_session(conn: &Connection, session_id: &str) -> Result<(), CommandError> {
    conn.execute(
        "DELETE FROM session_summaries WHERE session_id = ?1",
        rusqlite::params![session_id],
    )?;
    Ok(())
}
