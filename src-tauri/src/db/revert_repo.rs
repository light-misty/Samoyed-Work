use rusqlite::{params, Connection};

/// 回退状态记录（staged 回退，支持撤销回退/redo）
pub struct RevertRecord {
    pub session_id: String,
    /// 回退边界：该消息及之后的消息被隐藏
    pub revert_message_id: String,
    /// redo 基线快照 ID（回退前的文件状态）
    pub redo_snapshot_id: String,
    pub created_at: String,
}

/// 保存/覆盖会话的回退状态
pub fn set_revert(conn: &Connection, record: &RevertRecord) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO session_reverts (session_id, revert_message_id, redo_snapshot_id, created_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(session_id) DO UPDATE SET
             revert_message_id = excluded.revert_message_id,
             redo_snapshot_id = excluded.redo_snapshot_id,
             created_at = excluded.created_at",
        params![
            record.session_id,
            record.revert_message_id,
            record.redo_snapshot_id,
            record.created_at,
        ],
    )?;
    Ok(())
}

/// 获取会话的回退状态
pub fn get_revert(
    conn: &Connection,
    session_id: &str,
) -> Result<Option<RevertRecord>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT session_id, revert_message_id, redo_snapshot_id, created_at
         FROM session_reverts WHERE session_id = ?1",
    )?;
    let mut rows = stmt.query(params![session_id])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(RevertRecord {
            session_id: row.get(0)?,
            revert_message_id: row.get(1)?,
            redo_snapshot_id: row.get(2)?,
            created_at: row.get(3)?,
        }));
    }
    Ok(None)
}

/// 清除会话的回退状态
pub fn clear_revert(conn: &Connection, session_id: &str) -> Result<(), rusqlite::Error> {
    conn.execute(
        "DELETE FROM session_reverts WHERE session_id = ?1",
        params![session_id],
    )?;
    Ok(())
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

    fn sample_record(revert_message_id: &str) -> RevertRecord {
        RevertRecord {
            session_id: "sess_1".to_string(),
            revert_message_id: revert_message_id.to_string(),
            redo_snapshot_id: "snap_redo".to_string(),
            created_at: "2026-08-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_set_and_get_revert() {
        let conn = test_conn();
        set_revert(&conn, &sample_record("msg_3")).unwrap();

        let got = get_revert(&conn, "sess_1").unwrap().unwrap();
        assert_eq!(got.revert_message_id, "msg_3");
        assert_eq!(got.redo_snapshot_id, "snap_redo");

        // 覆盖更新（连续回退时边界前移）
        set_revert(&conn, &sample_record("msg_2")).unwrap();
        let got = get_revert(&conn, "sess_1").unwrap().unwrap();
        assert_eq!(got.revert_message_id, "msg_2");

        // 无记录返回 None
        assert!(get_revert(&conn, "sess_999").unwrap().is_none());
    }

    #[test]
    fn test_clear_revert() {
        let conn = test_conn();
        set_revert(&conn, &sample_record("msg_3")).unwrap();
        clear_revert(&conn, "sess_1").unwrap();
        assert!(get_revert(&conn, "sess_1").unwrap().is_none());
    }
}
