use rusqlite::Connection;
use crate::errors::CommandError;

/// 执行数据库初始化：建表、创建索引、插入版本记录
pub fn initialize_database(conn: &Connection) -> Result<(), CommandError> {
    log::info!("开始初始化数据库结构");

    create_tables(conn)?;
    create_indexes(conn)?;
    insert_initial_version(conn)?;

    log::info!("数据库结构初始化完成");
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

    // prompt_templates Prompt模板表
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS prompt_templates (
            id                TEXT        NOT NULL PRIMARY KEY,
            name              TEXT        NOT NULL,
            description       TEXT        NOT NULL DEFAULT '',
            content           TEXT        NOT NULL DEFAULT '',
            category          TEXT        NOT NULL DEFAULT 'custom',
            is_builtin        INTEGER     NOT NULL DEFAULT 0,
            variables         TEXT        DEFAULT NULL,
            created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );"
    )?;

    log::info!("数据表创建完成");
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
            ON token_usage (llm_provider, llm_model);

        CREATE INDEX IF NOT EXISTS idx_prompt_templates_category
            ON prompt_templates (category);
        CREATE INDEX IF NOT EXISTS idx_prompt_templates_is_builtin
            ON prompt_templates (is_builtin);
        CREATE INDEX IF NOT EXISTS idx_prompt_templates_updated_at
            ON prompt_templates (updated_at DESC);"
    )?;

    log::info!("索引创建完成");
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
            rusqlite::params![1, "初始建表：sessions, session_messages, version_snapshots, token_usage, prompt_templates"],
        )?;
        log::info!("已插入初始版本记录 (version=1)");
    } else {
        log::debug!("版本记录已存在 (count={})，跳过插入", count);
    }

    // 插入内置 Prompt 模板（仅在 prompt_templates 表为空时插入）
    seed_builtin_templates(conn)?;

    Ok(())
}

/// 插入内置模板种子数据
fn seed_builtin_templates(conn: &Connection) -> Result<(), CommandError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM prompt_templates WHERE is_builtin = 1",
        [],
        |row| row.get(0),
    )?;

    if count > 0 {
        log::debug!("内置模板已存在 (count={})，跳过种子数据", count);
        return Ok(());
    }

    let now = chrono::Utc::now().to_rfc3339();

    // 内置模板列表
    let builtin_templates: Vec<(&str, &str, &str, &str, &str)> = vec![
        (
            "builtin-weekly-report",
            "周报生成",
            "根据本周工作内容自动生成结构化周报文档",
            "请根据以下工作内容，帮我生成一份结构化的周报文档，保存为Word格式。要求包含：本周工作总结、关键进展、遇到的问题、下周计划。工作内容如下：{{content}}",
            "document",
        ),
        (
            "builtin-meeting-minutes",
            "会议纪要",
            "根据会议信息生成规范的会议纪要文档",
            "请根据以下会议信息，帮我生成一份规范的会议纪要文档，保存为Word格式。要求包含：会议主题、参会人员、会议时间、讨论内容、决议事项、后续行动项。会议信息如下：{{content}}",
            "document",
        ),
        (
            "builtin-data-analysis",
            "数据分析报告",
            "对Excel数据进行统计分析并生成分析报告",
            "请读取以下Excel文件的数据，进行统计分析，并生成一份数据分析报告。要求包含：数据概览、关键指标、趋势分析、异常发现、建议。文件路径：{{filePath}}，分析重点：{{focus}}",
            "analysis",
        ),
        (
            "builtin-format-convert",
            "格式转换",
            "将文档从一种格式转换为另一种格式",
            "请将文件 {{inputPath}} 从 {{sourceFormat}} 格式转换为 {{targetFormat}} 格式，保存到 {{outputPath}}",
            "conversion",
        ),
        (
            "builtin-doc-review",
            "文档审阅",
            "审阅文档内容，提出修改建议",
            "请审阅以下文档，检查内容的准确性、逻辑性和完整性，并提出具体的修改建议。文件路径：{{filePath}}，审阅重点：{{focus}}",
            "analysis",
        ),
        (
            "builtin-ppt-outline",
            "PPT大纲生成",
            "根据主题生成PPT大纲和内容",
            "请根据以下主题，帮我生成一份PPT演示文稿，保存为PPTX格式。要求包含：封面、目录、内容页（每页有标题和要点）、总结页。主题：{{topic}}，页数要求：{{pageCount}}页左右",
            "document",
        ),
    ];

    for (id, name, description, content, category) in &builtin_templates {
        conn.execute(
            "INSERT INTO prompt_templates (id, name, description, content, category, is_builtin, variables, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, NULL, ?6, ?7)",
            rusqlite::params![id, name, description, content, category, now, now],
        )?;
    }

    log::info!("已插入 {} 个内置模板", builtin_templates.len());
    Ok(())
}
