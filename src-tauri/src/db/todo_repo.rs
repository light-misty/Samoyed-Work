//! Todo 仓库:持久化 Todo 列表,支持跨迭代保持任务状态

use crate::errors::CommandError;
use crate::models::todo::{TodoItem, TodoList};
use rusqlite::Connection;

/// 获取指定会话的 Todo 列表
/// 无记录时返回空的 TodoList
pub fn get_todo_list(conn: &Connection, session_id: &str) -> Result<TodoList, CommandError> {
    // 查询 todo_lists 表,获取 items_json 和 updated_at
    let result: Result<(String, String), rusqlite::Error> = conn.query_row(
        "SELECT items_json, updated_at FROM todo_lists WHERE session_id = ?1",
        rusqlite::params![session_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );

    match result {
        Ok((items_json, updated_at_str)) => {
            // 反序列化 items_json 为 Vec<TodoItem>
            let items: Vec<TodoItem> = serde_json::from_str(&items_json)?;
            // 将 ISO 8601 时间字符串转换为毫秒时间戳
            let updated_at = parse_iso_to_millis(&updated_at_str);
            Ok(TodoList {
                session_id: session_id.to_string(),
                items,
                updated_at,
            })
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            // 无记录时返回空的 TodoList
            Ok(TodoList::new(session_id.to_string()))
        }
        Err(e) => Err(e.into()),
    }
}

/// 保存 Todo 列表(upsert)
/// 使用 ON CONFLICT(session_id) DO UPDATE 实现 upsert,updated_at 由数据库自动更新
pub fn save_todo_list(conn: &Connection, todo_list: &TodoList) -> Result<(), CommandError> {
    // 将 items 序列化为 JSON
    let items_json = serde_json::to_string(&todo_list.items)?;

    conn.execute(
        "INSERT INTO todo_lists (session_id, items_json, updated_at)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(session_id) DO UPDATE SET
            items_json = excluded.items_json,
            updated_at = excluded.updated_at",
        rusqlite::params![todo_list.session_id, items_json],
    )?;
    Ok(())
}

/// 删除指定会话的 Todo 列表
pub fn delete_todo_list(conn: &Connection, session_id: &str) -> Result<(), CommandError> {
    conn.execute(
        "DELETE FROM todo_lists WHERE session_id = ?1",
        rusqlite::params![session_id],
    )?;
    Ok(())
}

/// 将 ISO 8601 时间字符串解析为毫秒时间戳
/// 解析失败时返回 0
fn parse_iso_to_millis(s: &str) -> u64 {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.timestamp_millis() as u64)
        .unwrap_or(0)
}
