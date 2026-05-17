use rusqlite::Connection;
use crate::errors::CommandError;

/// 执行数据库初始化：建表、创建索引、插入版本记录
pub fn initialize_database(conn: &Connection) -> Result<(), CommandError> {
    create_tables(conn)?;
    create_indexes(conn)?;
    insert_initial_version(conn)?;
    Ok(())
}

/// 创建所有数据表
fn create_tables(conn: &Connection) -> Result<(), CommandError> {
    // schema_version 元数据表
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version     INTEGER NOT NULL PRIMARY KEY,
            description TEXT    NOT NULL,
            applied_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );"
    )?;

    // sessions 会话表
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sessions (
            id                  TEXT        NOT NULL PRIMARY KEY,
            workspace_id        TEXT        NOT NULL,
            title               TEXT        NOT NULL DEFAULT '新会话',
            created_at          TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at          TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            total_input_tokens  INTEGER     NOT NULL DEFAULT 0,
            total_output_tokens INTEGER     NOT NULL DEFAULT 0,
            llm_provider        TEXT        NOT NULL DEFAULT '',
            llm_model           TEXT        NOT NULL DEFAULT ''
        );"
    )?;

    // session_messages 消息表
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS session_messages (
            id                TEXT        NOT NULL PRIMARY KEY,
            session_id        TEXT        NOT NULL,
            role              TEXT        NOT NULL CHECK (role IN ('user', 'assistant', 'tool')),
            content           TEXT        NOT NULL DEFAULT '',
            tool_name         TEXT        DEFAULT NULL,
            tool_args         TEXT        DEFAULT NULL,
            tool_result       TEXT        DEFAULT NULL,
            thinking_content  TEXT        DEFAULT NULL,
            input_tokens      INTEGER     NOT NULL DEFAULT 0,
            output_tokens     INTEGER     NOT NULL DEFAULT 0,
            created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );"
    )?;

    // version_snapshots 版本快照表
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS version_snapshots (
            id                TEXT        NOT NULL PRIMARY KEY,
            workspace_id      TEXT        NOT NULL,
            session_id        TEXT        NOT NULL,
            file_path         TEXT        NOT NULL,
            snapshot_path     TEXT        NOT NULL,
            operation         TEXT        NOT NULL DEFAULT '',
            created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );"
    )?;

    // token_usage Token统计表
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS token_usage (
            id                TEXT        NOT NULL PRIMARY KEY,
            session_id        TEXT        NOT NULL,
            workspace_id      TEXT        NOT NULL,
            llm_provider      TEXT        NOT NULL DEFAULT '',
            llm_model         TEXT        NOT NULL DEFAULT '',
            input_tokens      INTEGER     NOT NULL DEFAULT 0,
            output_tokens     INTEGER     NOT NULL DEFAULT 0,
            created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );"
    )?;

    Ok(())
}

/// 创建所有索引
fn create_indexes(conn: &Connection) -> Result<(), CommandError> {
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_sessions_workspace_id
            ON sessions (workspace_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_updated_at
            ON sessions (updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_sessions_created_at
            ON sessions (created_at DESC);

        CREATE INDEX IF NOT EXISTS idx_session_messages_session_id
            ON session_messages (session_id);
        CREATE INDEX IF NOT EXISTS idx_session_messages_session_id_created_at
            ON session_messages (session_id, created_at ASC);
        CREATE INDEX IF NOT EXISTS idx_session_messages_role
            ON session_messages (role);

        CREATE INDEX IF NOT EXISTS idx_version_snapshots_workspace_id
            ON version_snapshots (workspace_id);
        CREATE INDEX IF NOT EXISTS idx_version_snapshots_session_id
            ON version_snapshots (session_id);
        CREATE INDEX IF NOT EXISTS idx_version_snapshots_file_path
            ON version_snapshots (file_path);
        CREATE INDEX IF NOT EXISTS idx_version_snapshots_created_at
            ON version_snapshots (created_at DESC);

        CREATE INDEX IF NOT EXISTS idx_token_usage_session_id
            ON token_usage (session_id);
        CREATE INDEX IF NOT EXISTS idx_token_usage_workspace_id
            ON token_usage (workspace_id);
        CREATE INDEX IF NOT EXISTS idx_token_usage_created_at
            ON token_usage (created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_token_usage_workspace_created
            ON token_usage (workspace_id, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_token_usage_provider_model
            ON token_usage (llm_provider, llm_model);"
    )?;
    Ok(())
}

/// 插入初始版本记录（仅在表为空时插入）
fn insert_initial_version(conn: &Connection) -> Result<(), CommandError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_version",
        [],
        |row| row.get(0),
    )?;

    if count == 0 {
        conn.execute(
            "INSERT INTO schema_version (version, description) VALUES (?1, ?2)",
            rusqlite::params![1, "初始建表：sessions, session_messages, version_snapshots, token_usage"],
        )?;
    }

    Ok(())
}
