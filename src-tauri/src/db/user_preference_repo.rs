use rusqlite::Connection;
use chrono::Utc;
use crate::errors::CommandError;
use crate::models::context_memory::UserPreference;

/// 创建或更新用户偏好
/// 如果 (category, key) 已存在，则增加观察次数并更新置信度
pub fn upsert_preference(
    conn: &Connection,
    id: &str,
    category: &str,
    key: &str,
    value: &str,
) -> Result<(), CommandError> {
    let now = Utc::now().to_rfc3339();

    // 先尝试更新已有记录
    let affected = conn.execute(
        "UPDATE user_preferences
         SET value = ?1, observation_count = observation_count + 1,
             confidence = MIN(1.0, confidence + 0.1), last_observed_at = ?2
         WHERE category = ?3 AND key = ?4",
        rusqlite::params![value, now, category, key],
    )?;

    if affected == 0 {
        // 不存在则插入新记录
        conn.execute(
            "INSERT INTO user_preferences
                (id, category, key, value, confidence, observation_count, last_observed_at)
             VALUES (?1, ?2, ?3, ?4, 0.5, 1, ?5)",
            rusqlite::params![id, category, key, value, now],
        )?;
    }

    Ok(())
}

/// 查询所有高置信度偏好（置信度 > 阈值）
pub fn list_high_confidence_preferences(
    conn: &Connection,
    min_confidence: f64,
) -> Vec<UserPreference> {
    let mut stmt = match conn.prepare(
        "SELECT id, category, key, value, confidence, observation_count, last_observed_at
         FROM user_preferences
         WHERE confidence >= ?1
         ORDER BY confidence DESC, observation_count DESC",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut rows = match stmt.query(rusqlite::params![min_confidence]) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut result = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        let pref = UserPreference {
            id: row.get(0).unwrap_or_default(),
            category: row.get(1).unwrap_or_default(),
            key: row.get(2).unwrap_or_default(),
            value: row.get(3).unwrap_or_default(),
            confidence: row.get(4).unwrap_or(0.5),
            observation_count: row.get(5).unwrap_or(1),
            last_observed_at: row.get(6).unwrap_or_default(),
        };
        result.push(pref);
    }
    result
}

/// 查询所有偏好
pub fn list_all_preferences(conn: &Connection) -> Vec<UserPreference> {
    let mut stmt = match conn.prepare(
        "SELECT id, category, key, value, confidence, observation_count, last_observed_at
         FROM user_preferences
         ORDER BY category, key",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut rows = match stmt.query([]) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut result = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        let pref = UserPreference {
            id: row.get(0).unwrap_or_default(),
            category: row.get(1).unwrap_or_default(),
            key: row.get(2).unwrap_or_default(),
            value: row.get(3).unwrap_or_default(),
            confidence: row.get(4).unwrap_or(0.5),
            observation_count: row.get(5).unwrap_or(1),
            last_observed_at: row.get(6).unwrap_or_default(),
        };
        result.push(pref);
    }
    result
}

/// 删除指定偏好
pub fn delete_preference(
    conn: &Connection,
    category: &str,
    key: &str,
) -> Result<(), CommandError> {
    conn.execute(
        "DELETE FROM user_preferences WHERE category = ?1 AND key = ?2",
        rusqlite::params![category, key],
    )?;
    Ok(())
}

/// 清除所有偏好
pub fn clear_all_preferences(conn: &Connection) -> Result<(), CommandError> {
    conn.execute("DELETE FROM user_preferences", [])?;
    Ok(())
}
