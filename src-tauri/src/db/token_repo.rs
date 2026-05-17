use rusqlite::Connection;
use chrono::Utc;
use crate::errors::CommandError;

/// 记录一次 Token 使用明细
pub fn record_usage(
    conn: &Connection,
    id: &str,
    session_id: &str,
    workspace_id: &str,
    provider: &str,
    model: &str,
    input_tokens: i64,
    output_tokens: i64,
) -> Result<(), CommandError> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO token_usage
            (id, session_id, workspace_id, llm_provider, llm_model, input_tokens, output_tokens, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![id, session_id, workspace_id, provider, model, input_tokens, output_tokens, now],
    )?;
    Ok(())
}

/// 获取指定会话的累计 Token 用量，返回 (input_tokens, output_tokens)
pub fn get_session_usage(conn: &Connection, session_id: &str) -> (i64, i64) {
    conn.query_row(
        "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0)
         FROM token_usage WHERE session_id = ?1",
        rusqlite::params![session_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .unwrap_or((0, 0))
}

/// 获取指定日期的 Token 用量，返回 (input_tokens, output_tokens)
/// date 参数格式为 "YYYY-MM-DD"
pub fn get_daily_usage(conn: &Connection, workspace_id: Option<&str>, date: &str) -> (i64, i64) {
    let start = format!("{}T00:00:00.000Z", date);
    let end = format!("{}T23:59:59.999Z", date);

    if let Some(wid) = workspace_id {
        conn.query_row(
            "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0)
             FROM token_usage
             WHERE workspace_id = ?1 AND created_at >= ?2 AND created_at <= ?3",
            rusqlite::params![wid, start, end],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or((0, 0))
    } else {
        conn.query_row(
            "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0)
             FROM token_usage
             WHERE created_at >= ?1 AND created_at <= ?2",
            rusqlite::params![start, end],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or((0, 0))
    }
}

/// 按 Provider + Model 分组统计 Token 用量
/// 返回 Vec<(provider, model, total_input, total_output)>
pub fn get_usage_by_provider(
    conn: &Connection,
    start: Option<&str>,
    end: Option<&str>,
) -> Vec<(String, String, i64, i64)> {
    let mut sql = String::from(
        "SELECT llm_provider, llm_model,
                COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0)
         FROM token_usage WHERE 1=1"
    );
    let mut param_idx = 1u32;
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(s) = start {
        sql.push_str(&format!(" AND created_at >= ?{}", param_idx));
        param_values.push(Box::new(format!("{}T00:00:00.000Z", s)));
        param_idx += 1;
    }

    if let Some(e) = end {
        sql.push_str(&format!(" AND created_at <= ?{}", param_idx));
        param_values.push(Box::new(format!("{}T23:59:59.999Z", e)));
    }

    sql.push_str(" GROUP BY llm_provider, llm_model ORDER BY llm_provider, llm_model");

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
        let provider: String = match row.get(0) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let model: String = match row.get(1) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let total_input: i64 = match row.get(2) {
            Ok(v) => v,
            Err(_) => 0,
        };
        let total_output: i64 = match row.get(3) {
            Ok(v) => v,
            Err(_) => 0,
        };
        result.push((provider, model, total_input, total_output));
    }
    result
}
