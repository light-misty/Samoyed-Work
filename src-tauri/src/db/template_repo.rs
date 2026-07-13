use crate::errors::CommandError;
use crate::models::PromptTemplate;
use chrono::Utc;
use rusqlite::Connection;

/// 查询所有模板，按更新时间降序排列
pub fn list_templates(conn: &Connection) -> Result<Vec<PromptTemplate>, CommandError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, content, category, is_builtin, variables, created_at, updated_at
         FROM prompt_templates ORDER BY updated_at DESC"
    )?;

    let templates = stmt.query_map([], |row| {
        let is_builtin: i64 = row.get(5)?;
        let variables_str: Option<String> = row.get(6)?;
        let variables = variables_str.and_then(|s| serde_json::from_str(&s).ok());

        Ok(PromptTemplate {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            content: row.get(3)?,
            category: row.get(4)?,
            is_builtin: is_builtin != 0,
            variables,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        })
    })?;

    templates.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// 根据 ID 获取单个模板
pub fn get_template(conn: &Connection, id: &str) -> Result<PromptTemplate, CommandError> {
    conn.query_row(
        "SELECT id, name, description, content, category, is_builtin, variables, created_at, updated_at
         FROM prompt_templates WHERE id = ?1",
        rusqlite::params![id],
        |row| {
            let is_builtin: i64 = row.get(5)?;
            let variables_str: Option<String> = row.get(6)?;
            let variables = variables_str
                .and_then(|s| serde_json::from_str(&s).ok());

            Ok(PromptTemplate {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                content: row.get(3)?,
                category: row.get(4)?,
                is_builtin: is_builtin != 0,
                variables,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        },
    )
    .map_err(Into::into)
}

/// 创建新模板
pub fn create_template(
    conn: &Connection,
    id: &str,
    name: &str,
    description: &str,
    content: &str,
    category: &str,
    variables: Option<&serde_json::Value>,
) -> Result<(), CommandError> {
    let now = Utc::now().to_rfc3339();
    let variables_str = variables.map(|v| v.to_string());

    conn.execute(
        "INSERT INTO prompt_templates (id, name, description, content, category, is_builtin, variables, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8)",
        rusqlite::params![id, name, description, content, category, variables_str, now, now],
    )?;
    Ok(())
}

/// 更新模板（仅允许更新非内置模板）
pub fn update_template(
    conn: &Connection,
    id: &str,
    name: Option<&str>,
    description: Option<&str>,
    content: Option<&str>,
    category: Option<&str>,
    variables: Option<&serde_json::Value>,
) -> Result<(), CommandError> {
    // 检查是否为内置模板
    let is_builtin: i64 = conn.query_row(
        "SELECT is_builtin FROM prompt_templates WHERE id = ?1",
        rusqlite::params![id],
        |row| row.get(0),
    )?;

    if is_builtin != 0 {
        return Err(CommandError::db(
            crate::errors::DB_CONSTRAINT_VIOLATION,
            "内置模板不可修改",
        ));
    }

    let now = Utc::now().to_rfc3339();

    // 动态构建 UPDATE 语句
    let mut set_clauses: Vec<String> = vec!["updated_at = ?1".to_string()];
    let mut param_idx = 2u32;
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now)];

    if let Some(n) = name {
        set_clauses.push(format!("name = ?{}", param_idx));
        param_values.push(Box::new(n.to_string()));
        param_idx += 1;
    }
    if let Some(d) = description {
        set_clauses.push(format!("description = ?{}", param_idx));
        param_values.push(Box::new(d.to_string()));
        param_idx += 1;
    }
    if let Some(c) = content {
        set_clauses.push(format!("content = ?{}", param_idx));
        param_values.push(Box::new(c.to_string()));
        param_idx += 1;
    }
    if let Some(cat) = category {
        set_clauses.push(format!("category = ?{}", param_idx));
        param_values.push(Box::new(cat.to_string()));
        param_idx += 1;
    }
    if let Some(v) = variables {
        set_clauses.push(format!("variables = ?{}", param_idx));
        param_values.push(Box::new(v.to_string()));
        param_idx += 1;
    }

    let sql = format!(
        "UPDATE prompt_templates SET {} WHERE id = ?{}",
        set_clauses.join(", "),
        param_idx
    );
    param_values.push(Box::new(id.to_string()));

    // 构建 params 切片引用
    let params: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, params.as_slice())?;

    Ok(())
}

/// 删除模板（仅允许删除非内置模板）
pub fn delete_template(conn: &Connection, id: &str) -> Result<(), CommandError> {
    // 检查是否为内置模板
    let is_builtin: i64 = conn.query_row(
        "SELECT is_builtin FROM prompt_templates WHERE id = ?1",
        rusqlite::params![id],
        |row| row.get(0),
    )?;

    if is_builtin != 0 {
        return Err(CommandError::db(
            crate::errors::DB_CONSTRAINT_VIOLATION,
            "内置模板不可删除",
        ));
    }

    conn.execute(
        "DELETE FROM prompt_templates WHERE id = ?1",
        rusqlite::params![id],
    )?;
    Ok(())
}

/// 按分类查询模板
pub fn list_templates_by_category(
    conn: &Connection,
    category: &str,
) -> Result<Vec<PromptTemplate>, CommandError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, content, category, is_builtin, variables, created_at, updated_at
         FROM prompt_templates WHERE category = ?1 ORDER BY updated_at DESC"
    )?;

    let templates = stmt.query_map(rusqlite::params![category], |row| {
        let is_builtin: i64 = row.get(5)?;
        let variables_str: Option<String> = row.get(6)?;
        let variables = variables_str.and_then(|s| serde_json::from_str(&s).ok());

        Ok(PromptTemplate {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            content: row.get(3)?,
            category: row.get(4)?,
            is_builtin: is_builtin != 0,
            variables,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        })
    })?;

    templates.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}
