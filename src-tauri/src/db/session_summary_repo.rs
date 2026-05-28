use rusqlite::Connection;
use chrono::Utc;
use crate::errors::CommandError;
use crate::models::context_memory::ContextSessionSummary;

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
            id, session_id, workspace_id, user_goal, result_summary,
            files_involved, tools_used, errors_resolved, now,
        ],
    )?;
    Ok(())
}

/// 查询指定工作区最近的会话摘要
/// 按创建时间降序排列，最多返回 limit 条
pub fn list_summaries_by_workspace(
    conn: &Connection,
    workspace_id: &str,
    limit: u32,
) -> Vec<ContextSessionSummary> {
    let mut stmt = match conn.prepare(
        "SELECT id, session_id, workspace_id, user_goal, result_summary,
                files_involved, tools_used, errors_resolved, created_at
         FROM session_summaries
         WHERE workspace_id = ?1
         ORDER BY created_at DESC
         LIMIT ?2",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut rows = match stmt.query(rusqlite::params![workspace_id, limit]) {
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
pub fn delete_summary_by_session(
    conn: &Connection,
    session_id: &str,
) -> Result<(), CommandError> {
    conn.execute(
        "DELETE FROM session_summaries WHERE session_id = ?1",
        rusqlite::params![session_id],
    )?;
    Ok(())
}
