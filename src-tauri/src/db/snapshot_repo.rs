use rusqlite::{params, Connection};

use crate::models::snapshot::SnapshotRecord;

/// 创建快照记录
pub fn create_snapshot(conn: &Connection, record: &SnapshotRecord) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO session_snapshots (id, session_id, message_id, kind, snapshot_ref, workspace_path, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            record.id,
            record.session_id,
            record.message_id,
            record.kind,
            record.snapshot_ref,
            record.workspace_path,
            record.created_at,
        ],
    )?;
    Ok(())
}

/// 根据 ID 获取快照记录
pub fn get_snapshot_by_id(
    conn: &Connection,
    id: &str,
) -> Result<Option<SnapshotRecord>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, message_id, kind, snapshot_ref, workspace_path, created_at
         FROM session_snapshots WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(row_to_snapshot(row)?));
    }
    Ok(None)
}

/// 根据关联的 user 消息 ID 获取快照记录
pub fn get_snapshot_by_message_id(
    conn: &Connection,
    message_id: &str,
) -> Result<Option<SnapshotRecord>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, message_id, kind, snapshot_ref, workspace_path, created_at
         FROM session_snapshots WHERE message_id = ?1 ORDER BY created_at DESC LIMIT 1",
    )?;
    let mut rows = stmt.query(params![message_id])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(row_to_snapshot(row)?));
    }
    Ok(None)
}

/// 列出会话的所有快照记录（含未关联消息的 redo 基线快照）
pub fn list_snapshots_by_session(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<SnapshotRecord>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, message_id, kind, snapshot_ref, workspace_path, created_at
         FROM session_snapshots WHERE session_id = ?1 ORDER BY created_at ASC",
    )?;
    let records = stmt
        .query_map(params![session_id], row_to_snapshot)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(records)
}

/// 更新快照关联的 user 消息 ID（快照先创建，user 消息持久化后回填）
pub fn update_snapshot_message_id(
    conn: &Connection,
    snapshot_id: &str,
    message_id: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE session_snapshots SET message_id = ?1 WHERE id = ?2",
        params![message_id, snapshot_id],
    )?;
    Ok(())
}

/// 删除指定 ID 列表的快照记录
pub fn delete_snapshots_by_ids(
    conn: &Connection,
    ids: &[String],
) -> Result<usize, rusqlite::Error> {
    if ids.is_empty() {
        return Ok(0);
    }
    let placeholders: Vec<String> = (0..ids.len()).map(|i| format!("?{}", i + 1)).collect();
    let sql = format!(
        "DELETE FROM session_snapshots WHERE id IN ({})",
        placeholders.join(", ")
    );
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    for id in ids {
        params_vec.push(Box::new(id.clone()));
    }
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|p| p.as_ref()).collect();
    let affected = conn.execute(&sql, rusqlite::params_from_iter(param_refs))?;
    Ok(affected)
}

/// 删除会话的全部快照记录（删除会话时级联清理）
pub fn delete_snapshots_by_session(
    conn: &Connection,
    session_id: &str,
) -> Result<usize, rusqlite::Error> {
    let affected = conn.execute(
        "DELETE FROM session_snapshots WHERE session_id = ?1",
        params![session_id],
    )?;
    Ok(affected)
}

fn row_to_snapshot(row: &rusqlite::Row<'_>) -> rusqlite::Result<SnapshotRecord> {
    Ok(SnapshotRecord {
        id: row.get(0)?,
        session_id: row.get(1)?,
        message_id: row.get(2)?,
        kind: row.get(3)?,
        snapshot_ref: row.get(4)?,
        workspace_path: row.get(5)?,
        created_at: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init::initialize_database;
    use rusqlite::Connection;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize_database(&conn).unwrap();
        conn
    }

    fn sample_record(id: &str, message_id: Option<&str>) -> SnapshotRecord {
        SnapshotRecord {
            id: id.to_string(),
            session_id: "sess_1".to_string(),
            message_id: message_id.map(|s| s.to_string()),
            kind: "git".to_string(),
            snapshot_ref: "abc123".to_string(),
            workspace_path: "D:/ws".to_string(),
            created_at: "2026-08-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_create_and_get_by_id() {
        let conn = test_conn();
        let record = sample_record("snap_1", Some("msg_1"));
        create_snapshot(&conn, &record).unwrap();

        let got = get_snapshot_by_id(&conn, "snap_1").unwrap().unwrap();
        assert_eq!(got.id, "snap_1");
        assert_eq!(got.session_id, "sess_1");
        assert_eq!(got.message_id.as_deref(), Some("msg_1"));
        assert_eq!(got.kind, "git");
        assert_eq!(got.snapshot_ref, "abc123");
        assert_eq!(got.workspace_path, "D:/ws");
    }

    #[test]
    fn test_get_by_message_id() {
        let conn = test_conn();
        create_snapshot(&conn, &sample_record("snap_1", Some("msg_1"))).unwrap();
        create_snapshot(&conn, &sample_record("snap_2", Some("msg_2"))).unwrap();

        let got = get_snapshot_by_message_id(&conn, "msg_2").unwrap().unwrap();
        assert_eq!(got.id, "snap_2");
        assert!(get_snapshot_by_message_id(&conn, "msg_999")
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_update_message_id() {
        let conn = test_conn();
        // redo 基线快照先创建（无 message_id），后续回填
        create_snapshot(&conn, &sample_record("snap_1", None)).unwrap();
        update_snapshot_message_id(&conn, "snap_1", "msg_9").unwrap();
        let got = get_snapshot_by_id(&conn, "snap_1").unwrap().unwrap();
        assert_eq!(got.message_id.as_deref(), Some("msg_9"));
    }

    #[test]
    fn test_list_and_delete_by_session() {
        let conn = test_conn();
        create_snapshot(&conn, &sample_record("snap_1", Some("msg_1"))).unwrap();
        create_snapshot(&conn, &sample_record("snap_2", None)).unwrap();

        let list = list_snapshots_by_session(&conn, "sess_1").unwrap();
        assert_eq!(list.len(), 2);

        // 删除 redo 基线快照（message_id 为 NULL 的）
        let redo_ids: Vec<String> = list
            .iter()
            .filter(|r| r.message_id.is_none())
            .map(|r| r.id.clone())
            .collect();
        let affected = delete_snapshots_by_ids(&conn, &redo_ids).unwrap();
        assert_eq!(affected, 1);
        assert!(get_snapshot_by_id(&conn, "snap_2").unwrap().is_none());

        // 按会话删除
        let affected = delete_snapshots_by_session(&conn, "sess_1").unwrap();
        assert_eq!(affected, 1);
        assert!(get_snapshot_by_id(&conn, "snap_1").unwrap().is_none());
    }
}
