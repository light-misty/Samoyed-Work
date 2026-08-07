use crate::errors::CommandError;
use crate::models::{AttachmentMeta, Message, MessageRole, ToolCall};
use chrono::Utc;
use rusqlite::Connection;

#[allow(clippy::too_many_arguments)]
pub fn create_message(
    conn: &Connection,
    id: &str,
    session_id: &str,
    role: &str,
    content: &str,
    tool_name: Option<&str>,
    tool_args: Option<&str>,
    tool_result: Option<&str>,
    tool_call_id: Option<&str>,
    thinking_content: Option<&str>,
    reasoning_content: Option<&str>,
    attachments: Option<&[AttachmentMeta]>,
    metadata: Option<&str>,
    branch_id: &str,
    branch_group_id: Option<&str>,
) -> Result<(), CommandError> {
    let now = Utc::now().to_rfc3339();
    // 附件序列化为 JSON 字符串存储
    let attachments_json =
        attachments.map(|atts| serde_json::to_string(atts).unwrap_or_else(|_| "[]".to_string()));
    conn.execute(
        "INSERT INTO session_messages
            (id, session_id, role, content, tool_name, tool_args, tool_result,
             tool_call_id, thinking_content, reasoning_content, attachments, metadata,
             branch_id, branch_group_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        rusqlite::params![
            id,
            session_id,
            role,
            content,
            tool_name,
            tool_args,
            tool_result,
            tool_call_id,
            thinking_content,
            reasoning_content,
            attachments_json,
            metadata,
            branch_id,
            branch_group_id,
            now,
        ],
    )?;
    Ok(())
}

pub fn list_messages(conn: &Connection, session_id: &str, branch_id: &str) -> Vec<Message> {
    let mut stmt = match conn.prepare(
        "SELECT id, session_id, role, content, tool_name, tool_args, tool_result,
                tool_call_id, thinking_content, reasoning_content, attachments, metadata,
                branch_id, branch_group_id, created_at
         FROM session_messages
         WHERE session_id = ?1 AND branch_id = ?2
         ORDER BY created_at ASC",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut rows = match stmt.query(rusqlite::params![session_id, branch_id]) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut result = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        let role_str: String = match row.get(2) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let content: String = match row.get(3) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let tool_name: Option<String> = row.get(4).ok().flatten();
        let tool_args: Option<String> = row.get(5).ok().flatten();
        let tool_result: Option<String> = row.get(6).ok().flatten();
        let tool_call_id: Option<String> = row.get(7).ok().flatten();
        let reasoning_content: Option<String> = row.get(9).ok().flatten();
        let attachments_json: Option<String> = row.get(10).ok().flatten();
        let metadata_json: Option<String> = row.get(11).ok().flatten();
        // 分支相关字段（阶段 6 新增）
        let branch_id_val: Option<String> = row.get(12).ok().flatten();
        let branch_group_id_val: Option<String> = row.get(13).ok().flatten();
        let msg_id: String = match row.get(0) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let created_at: String = match row.get(14) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // 反序列化附件
        let attachments: Option<Vec<AttachmentMeta>> = attachments_json
            .and_then(|json| serde_json::from_str(&json).ok())
            .filter(|atts: &Vec<AttachmentMeta>| !atts.is_empty());

        let message_role = match role_str.as_str() {
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            "tool" => MessageRole::Tool,
            _ => MessageRole::User,
        };

        let tool_calls = if role_str == "tool" {
            // tool 消息：优先使用数据库中存储的 tool_call_id
            // 如果 tool_call_id 不存在（旧数据），回退到使用 msg_id
            let call_id = tool_call_id.unwrap_or_else(|| msg_id.clone());
            let name = tool_name.unwrap_or_default();
            let arguments = tool_args
                .and_then(|args| serde_json::from_str(&args).ok())
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            let result_val = tool_result.and_then(|res| serde_json::from_str(&res).ok());
            Some(vec![ToolCall {
                id: call_id,
                name,
                arguments,
                result: result_val,
            }])
        } else if role_str == "assistant" {
            match (tool_name, tool_args) {
                (Some(ref name_str), Some(ref args_str)) => {
                    // 尝试解析为多个 tool_calls（JSON 数组格式）
                    if let Ok(names) = serde_json::from_str::<Vec<String>>(name_str) {
                        if let Ok(args_list) = serde_json::from_str::<Vec<String>>(args_str) {
                            // 尝试从 tool_call_id 字段恢复原始 id 列表
                            let ids: Vec<String> = tool_call_id
                                .as_ref()
                                .and_then(|id_str| serde_json::from_str::<Vec<String>>(id_str).ok())
                                .unwrap_or_else(|| {
                                    // 旧数据回退：使用 msg_id_index 格式
                                    names
                                        .iter()
                                        .enumerate()
                                        .map(|(i, _)| format!("{}_{}", msg_id, i))
                                        .collect()
                                });

                            let calls: Vec<ToolCall> = names
                                .iter()
                                .zip(args_list.iter())
                                .zip(ids.iter())
                                .map(|((name, args), id)| {
                                    let arguments = serde_json::from_str(args).unwrap_or(
                                        serde_json::Value::Object(serde_json::Map::new()),
                                    );
                                    ToolCall {
                                        id: id.clone(),
                                        name: name.clone(),
                                        arguments,
                                        result: None,
                                    }
                                })
                                .collect();
                            if !calls.is_empty() {
                                Some(calls)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        // 单个 tool_call
                        let arguments = serde_json::from_str(args_str)
                            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                        // 优先使用 tool_call_id 字段中存储的原始 id
                        let call_id = tool_call_id.unwrap_or_else(|| msg_id.clone());
                        Some(vec![ToolCall {
                            id: call_id,
                            name: name_str.clone(),
                            arguments,
                            result: None,
                        }])
                    }
                }
                _ => None,
            }
        } else {
            None
        };

        result.push(Message {
            id: msg_id,
            role: message_role,
            content,
            tool_calls,
            reasoning_content,
            attachments,
            metadata: metadata_json.and_then(|json| serde_json::from_str(&json).ok()),
            branch_id: branch_id_val,
            branch_group_id: branch_group_id_val,
            created_at,
        });
    }
    result
}

pub fn delete_messages_by_session(conn: &Connection, session_id: &str) -> Result<(), CommandError> {
    conn.execute(
        "DELETE FROM session_messages WHERE session_id = ?1",
        rusqlite::params![session_id],
    )?;
    Ok(())
}

/// 按 ID 列表批量删除会话中的指定消息
/// 使用参数化 IN 查询防止 SQL 注入
/// branch_id 作为防御性过滤，确保不会跨分支删除消息
pub fn delete_messages_by_ids(
    conn: &Connection,
    session_id: &str,
    message_ids: &[String],
    branch_id: &str,
) -> Result<(), CommandError> {
    if message_ids.is_empty() {
        return Ok(());
    }
    // 构建参数化占位符 (?1, ?2, ...)
    let placeholders: Vec<String> = (0..message_ids.len())
        .map(|i| format!("?{}", i + 1))
        .collect();
    // session_id 占 ?{N+1}，branch_id 占 ?{N+2}
    let sql = format!(
        "DELETE FROM session_messages WHERE session_id = ?{} AND branch_id = ?{} AND id IN ({})",
        message_ids.len() + 1,
        message_ids.len() + 2,
        placeholders.join(", ")
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    for id in message_ids {
        params.push(Box::new(id.clone()));
    }
    params.push(Box::new(session_id.to_string()));
    params.push(Box::new(branch_id.to_string()));
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, rusqlite::params_from_iter(param_refs))?;
    Ok(())
}

/// 获取指定消息之前的所有消息（用于分支创建时复制前缀）
/// 返回 created_at ASC 排序的消息列表，不含 before_message_id 本身
pub fn list_messages_before(
    conn: &Connection,
    session_id: &str,
    branch_id: &str,
    before_message_id: &str,
) -> Vec<Message> {
    // 先找到 before_message_id 的 created_at
    let target_created_at: Option<String> = conn
        .query_row(
            "SELECT created_at FROM session_messages WHERE id = ?1 AND session_id = ?2 AND branch_id = ?3",
            rusqlite::params![before_message_id, session_id, branch_id],
            |row| row.get(0),
        )
        .ok();

    let target_created_at = match target_created_at {
        Some(t) => t,
        None => return Vec::new(),
    };

    // 查询该时间之前的所有消息（row 映射与 list_messages 完全一致）
    let mut stmt = match conn.prepare(
        "SELECT id, session_id, role, content, tool_name, tool_args, tool_result,
                tool_call_id, thinking_content, reasoning_content, attachments, metadata,
                branch_id, branch_group_id, created_at
         FROM session_messages
         WHERE session_id = ?1 AND branch_id = ?2 AND created_at < ?3
         ORDER BY created_at ASC",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut rows = match stmt.query(rusqlite::params![session_id, branch_id, target_created_at]) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut result = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        let role_str: String = match row.get(2) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let content: String = match row.get(3) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let tool_name: Option<String> = row.get(4).ok().flatten();
        let tool_args: Option<String> = row.get(5).ok().flatten();
        let tool_result: Option<String> = row.get(6).ok().flatten();
        let tool_call_id: Option<String> = row.get(7).ok().flatten();
        let reasoning_content: Option<String> = row.get(9).ok().flatten();
        let attachments_json: Option<String> = row.get(10).ok().flatten();
        let metadata_json: Option<String> = row.get(11).ok().flatten();
        // 分支相关字段（阶段 6 新增）
        let branch_id_val: Option<String> = row.get(12).ok().flatten();
        let branch_group_id_val: Option<String> = row.get(13).ok().flatten();
        let msg_id: String = match row.get(0) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let created_at: String = match row.get(14) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // 反序列化附件
        let attachments: Option<Vec<AttachmentMeta>> = attachments_json
            .and_then(|json| serde_json::from_str(&json).ok())
            .filter(|atts: &Vec<AttachmentMeta>| !atts.is_empty());

        let message_role = match role_str.as_str() {
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            "tool" => MessageRole::Tool,
            _ => MessageRole::User,
        };

        let tool_calls = if role_str == "tool" {
            // tool 消息：优先使用数据库中存储的 tool_call_id
            // 如果 tool_call_id 不存在（旧数据），回退到使用 msg_id
            let call_id = tool_call_id.unwrap_or_else(|| msg_id.clone());
            let name = tool_name.unwrap_or_default();
            let arguments = tool_args
                .and_then(|args| serde_json::from_str(&args).ok())
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            let result_val = tool_result.and_then(|res| serde_json::from_str(&res).ok());
            Some(vec![ToolCall {
                id: call_id,
                name,
                arguments,
                result: result_val,
            }])
        } else if role_str == "assistant" {
            match (tool_name, tool_args) {
                (Some(ref name_str), Some(ref args_str)) => {
                    // 尝试解析为多个 tool_calls（JSON 数组格式）
                    if let Ok(names) = serde_json::from_str::<Vec<String>>(name_str) {
                        if let Ok(args_list) = serde_json::from_str::<Vec<String>>(args_str) {
                            // 尝试从 tool_call_id 字段恢复原始 id 列表
                            let ids: Vec<String> = tool_call_id
                                .as_ref()
                                .and_then(|id_str| serde_json::from_str::<Vec<String>>(id_str).ok())
                                .unwrap_or_else(|| {
                                    // 旧数据回退：使用 msg_id_index 格式
                                    names
                                        .iter()
                                        .enumerate()
                                        .map(|(i, _)| format!("{}_{}", msg_id, i))
                                        .collect()
                                });

                            let calls: Vec<ToolCall> = names
                                .iter()
                                .zip(args_list.iter())
                                .zip(ids.iter())
                                .map(|((name, args), id)| {
                                    let arguments = serde_json::from_str(args).unwrap_or(
                                        serde_json::Value::Object(serde_json::Map::new()),
                                    );
                                    ToolCall {
                                        id: id.clone(),
                                        name: name.clone(),
                                        arguments,
                                        result: None,
                                    }
                                })
                                .collect();
                            if !calls.is_empty() {
                                Some(calls)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        // 单个 tool_call
                        let arguments = serde_json::from_str(args_str)
                            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                        // 优先使用 tool_call_id 字段中存储的原始 id
                        let call_id = tool_call_id.unwrap_or_else(|| msg_id.clone());
                        Some(vec![ToolCall {
                            id: call_id,
                            name: name_str.clone(),
                            arguments,
                            result: None,
                        }])
                    }
                }
                _ => None,
            }
        } else {
            None
        };

        result.push(Message {
            id: msg_id,
            role: message_role,
            content,
            tool_calls,
            reasoning_content,
            attachments,
            metadata: metadata_json.and_then(|json| serde_json::from_str(&json).ok()),
            branch_id: branch_id_val,
            branch_group_id: branch_group_id_val,
            created_at,
        });
    }
    result
}

/// 获取指定消息（用于回退边界校验：确认角色、分支归属）
pub fn get_message(
    conn: &Connection,
    session_id: &str,
    message_id: &str,
) -> Result<Option<(String, String, String)>, rusqlite::Error> {
    // 返回 (role, branch_id, created_at)
    conn.query_row(
        "SELECT role, branch_id, created_at FROM session_messages
         WHERE id = ?1 AND session_id = ?2",
        rusqlite::params![message_id, session_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .map(Some)
    .or_else(|e| {
        if e == rusqlite::Error::QueryReturnedNoRows {
            Ok(None)
        } else {
            Err(e)
        }
    })
}

/// 更新指定消息的 metadata（用于回填快照信息：snapshot: {kind, createdAt}）
pub fn update_message_metadata(
    conn: &Connection,
    message_id: &str,
    session_id: &str,
    metadata: &serde_json::Value,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE session_messages SET metadata = ?1 WHERE id = ?2 AND session_id = ?3",
        rusqlite::params![metadata.to_string(), message_id, session_id],
    )?;
    Ok(())
}

/// 查询指定消息（含）及其后的所有消息（回退范围查询）
/// 返回 created_at ASC 排序的消息列表，行映射与 list_messages 完全一致
pub fn list_messages_from(
    conn: &Connection,
    session_id: &str,
    branch_id: &str,
    from_message_id: &str,
) -> Vec<Message> {
    // 先找到 from_message_id 的 created_at
    let target_created_at: Option<String> = conn
        .query_row(
            "SELECT created_at FROM session_messages WHERE id = ?1 AND session_id = ?2 AND branch_id = ?3",
            rusqlite::params![from_message_id, session_id, branch_id],
            |row| row.get(0),
        )
        .ok();

    let target_created_at = match target_created_at {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut stmt = match conn.prepare(
        "SELECT id, session_id, role, content, tool_name, tool_args, tool_result,
                tool_call_id, thinking_content, reasoning_content, attachments, metadata,
                branch_id, branch_group_id, created_at
         FROM session_messages
         WHERE session_id = ?1 AND branch_id = ?2 AND created_at >= ?3
         ORDER BY created_at ASC",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut rows = match stmt.query(rusqlite::params![session_id, branch_id, target_created_at]) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut result = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        let role_str: String = match row.get(2) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let content: String = match row.get(3) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let tool_name: Option<String> = row.get(4).ok().flatten();
        let tool_args: Option<String> = row.get(5).ok().flatten();
        let tool_result: Option<String> = row.get(6).ok().flatten();
        let tool_call_id: Option<String> = row.get(7).ok().flatten();
        let reasoning_content: Option<String> = row.get(9).ok().flatten();
        let attachments_json: Option<String> = row.get(10).ok().flatten();
        let metadata_json: Option<String> = row.get(11).ok().flatten();
        // 分支相关字段（阶段 6 新增）
        let branch_id_val: Option<String> = row.get(12).ok().flatten();
        let branch_group_id_val: Option<String> = row.get(13).ok().flatten();
        let msg_id: String = match row.get(0) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let created_at: String = match row.get(14) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // 反序列化附件
        let attachments: Option<Vec<AttachmentMeta>> = attachments_json
            .and_then(|json| serde_json::from_str(&json).ok())
            .filter(|atts: &Vec<AttachmentMeta>| !atts.is_empty());

        let message_role = match role_str.as_str() {
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            "tool" => MessageRole::Tool,
            _ => MessageRole::User,
        };

        let tool_calls = if role_str == "tool" {
            // tool 消息：优先使用数据库中存储的 tool_call_id
            // 如果 tool_call_id 不存在（旧数据），回退到使用 msg_id
            let call_id = tool_call_id.unwrap_or_else(|| msg_id.clone());
            let name = tool_name.unwrap_or_default();
            let arguments = tool_args
                .and_then(|args| serde_json::from_str(&args).ok())
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            let result_val = tool_result.and_then(|res| serde_json::from_str(&res).ok());
            Some(vec![ToolCall {
                id: call_id,
                name,
                arguments,
                result: result_val,
            }])
        } else if role_str == "assistant" {
            match (tool_name, tool_args) {
                (Some(ref name_str), Some(ref args_str)) => {
                    // 尝试解析为多个 tool_calls（JSON 数组格式）
                    if let Ok(names) = serde_json::from_str::<Vec<String>>(name_str) {
                        if let Ok(args_list) = serde_json::from_str::<Vec<String>>(args_str) {
                            // 尝试从 tool_call_id 字段恢复原始 id 列表
                            let ids: Vec<String> = tool_call_id
                                .as_ref()
                                .and_then(|id_str| serde_json::from_str::<Vec<String>>(id_str).ok())
                                .unwrap_or_else(|| {
                                    // 旧数据回退：使用 msg_id_index 格式
                                    names
                                        .iter()
                                        .enumerate()
                                        .map(|(i, _)| format!("{}_{}", msg_id, i))
                                        .collect()
                                });

                            let calls: Vec<ToolCall> = names
                                .iter()
                                .zip(args_list.iter())
                                .zip(ids.iter())
                                .map(|((name, args), id)| {
                                    let arguments = serde_json::from_str(args).unwrap_or(
                                        serde_json::Value::Object(serde_json::Map::new()),
                                    );
                                    ToolCall {
                                        id: id.clone(),
                                        name: name.clone(),
                                        arguments,
                                        result: None,
                                    }
                                })
                                .collect();
                            if !calls.is_empty() {
                                Some(calls)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        // 单个 tool_call
                        let arguments = serde_json::from_str(args_str)
                            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                        // 优先使用 tool_call_id 字段中存储的原始 id
                        let call_id = tool_call_id.unwrap_or_else(|| msg_id.clone());
                        Some(vec![ToolCall {
                            id: call_id,
                            name: name_str.clone(),
                            arguments,
                            result: None,
                        }])
                    }
                }
                _ => None,
            }
        } else {
            None
        };

        result.push(Message {
            id: msg_id,
            role: message_role,
            content,
            tool_calls,
            reasoning_content,
            attachments,
            metadata: metadata_json.and_then(|json| serde_json::from_str(&json).ok()),
            branch_id: branch_id_val,
            branch_group_id: branch_group_id_val,
            created_at,
        });
    }
    result
}

/// 统计会话全部分支的消息总数（回退时判断是否需要删除整个会话）
pub fn count_session_messages(conn: &Connection, session_id: &str) -> usize {
    conn.query_row(
        "SELECT COUNT(*) FROM session_messages WHERE session_id = ?1",
        rusqlite::params![session_id],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

/// 统计指定消息（含）及其后的消息数量（回退时计算隐藏消息数）
pub fn count_messages_from(
    conn: &Connection,
    session_id: &str,
    branch_id: &str,
    from_message_id: &str,
) -> usize {
    let target_created_at: Option<String> = conn
        .query_row(
            "SELECT created_at FROM session_messages WHERE id = ?1 AND session_id = ?2 AND branch_id = ?3",
            rusqlite::params![from_message_id, session_id, branch_id],
            |row| row.get(0),
        )
        .ok();

    let target_created_at = match target_created_at {
        Some(t) => t,
        None => return 0,
    };

    conn.query_row(
        "SELECT COUNT(*) FROM session_messages
         WHERE session_id = ?1 AND branch_id = ?2 AND created_at >= ?3",
        rusqlite::params![session_id, branch_id, target_created_at],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

/// 删除指定消息（含）及其后的所有消息（回退清理时使用）
/// branch_id 作为防御性过滤，确保不会跨分支删除消息
pub fn delete_messages_from(
    conn: &Connection,
    session_id: &str,
    branch_id: &str,
    from_message_id: &str,
) -> Result<usize, CommandError> {
    let target_created_at: Option<String> = conn
        .query_row(
            "SELECT created_at FROM session_messages WHERE id = ?1 AND session_id = ?2 AND branch_id = ?3",
            rusqlite::params![from_message_id, session_id, branch_id],
            |row| row.get(0),
        )
        .ok();

    let target_created_at = match target_created_at {
        Some(t) => t,
        None => return Ok(0),
    };

    let affected = conn.execute(
        "DELETE FROM session_messages
         WHERE session_id = ?1 AND branch_id = ?2 AND created_at >= ?3",
        rusqlite::params![session_id, branch_id, target_created_at],
    )?;
    Ok(affected)
}

/// 批量复制一个分支的消息到另一个分支（重新生成 msg_id）
/// 用于创建分支时复制分叉点之前的前缀消息
pub fn copy_messages_to_branch(
    conn: &Connection,
    source_session_id: &str,
    source_branch_id: &str,
    before_message_id: &str,
    target_branch_id: &str,
) -> Result<usize, CommandError> {
    let messages =
        list_messages_before(conn, source_session_id, source_branch_id, before_message_id);
    let count = messages.len();

    for msg in messages {
        let new_msg_id = format!("msg_{}", uuid::Uuid::new_v4());
        // MessageRole 枚举转换为字符串（参考 message.rs 中的 serde rename）
        let role_str = match msg.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
            MessageRole::Tool => "tool",
        };

        // Message 结构体只保留聚合后的 tool_calls，需从中重建数据库存储字段
        // - tool 角色：单条 tool_call，tool_result 有值
        // - assistant 角色 + 1 条 tool_call：单字符串格式，tool_result 为 None
        // - assistant 角色 + 多条 tool_call：JSON 数组格式，tool_result 为 None
        let (tool_name, tool_args, tool_result, tool_call_id) = match msg.tool_calls.as_ref() {
            Some(calls) if !calls.is_empty() => {
                if role_str == "tool" {
                    // tool 消息：单条 tool_call，保留 result
                    let tc = &calls[0];
                    (
                        Some(tc.name.clone()),
                        Some(serde_json::to_string(&tc.arguments).unwrap_or_default()),
                        tc.result
                            .as_ref()
                            .map(|r| serde_json::to_string(r).unwrap_or_default()),
                        Some(tc.id.clone()),
                    )
                } else if calls.len() == 1 {
                    // assistant + 单 tool_call：单字符串格式
                    let tc = &calls[0];
                    (
                        Some(tc.name.clone()),
                        Some(serde_json::to_string(&tc.arguments).unwrap_or_default()),
                        None,
                        Some(tc.id.clone()),
                    )
                } else {
                    // assistant + 多 tool_call：JSON 数组格式（参考 list_messages 解析逻辑）
                    let names: Vec<String> = calls.iter().map(|c| c.name.clone()).collect();
                    let args_list: Vec<String> = calls
                        .iter()
                        .map(|c| serde_json::to_string(&c.arguments).unwrap_or_default())
                        .collect();
                    let ids: Vec<String> = calls.iter().map(|c| c.id.clone()).collect();
                    (
                        Some(serde_json::to_string(&names).unwrap_or_default()),
                        Some(serde_json::to_string(&args_list).unwrap_or_default()),
                        None,
                        Some(serde_json::to_string(&ids).unwrap_or_default()),
                    )
                }
            }
            _ => (None, None, None, None),
        };

        // metadata 序列化为 JSON 字符串
        let metadata_str = msg
            .metadata
            .as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_default());
        create_message(
            conn,
            &new_msg_id,
            source_session_id, // 消息仍在同一 session
            role_str,
            &msg.content,
            tool_name.as_deref(),
            tool_args.as_deref(),
            tool_result.as_deref(),
            tool_call_id.as_deref(),
            None, // thinking_content 已弃用，复制时不保留
            msg.reasoning_content.as_deref(),
            msg.attachments.as_deref(),
            metadata_str.as_deref(),
            target_branch_id, // 新分支 ID
            None,             // branch_group_id 复制时为 None（只有分叉点 user 消息才有）
        )?;
    }

    Ok(count)
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

    /// 插入一条测试消息，created_at 由调用方指定以保证顺序可控
    fn insert_msg(conn: &Connection, id: &str, role: &str, content: &str, created_at: &str) {
        create_message(
            conn,
            id,
            "sess_1",
            role,
            content,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            "branch_main",
            None,
        )
        .unwrap();
        // 覆盖 created_at 为指定值（create_message 内部使用当前时间）
        conn.execute(
            "UPDATE session_messages SET created_at = ?1 WHERE id = ?2",
            rusqlite::params![created_at, id],
        )
        .unwrap();
    }

    #[test]
    fn test_get_message() {
        let conn = test_conn();
        insert_msg(&conn, "msg_1", "user", "第一条", "2026-08-07T00:00:01Z");

        let (role, branch_id, _created_at) =
            get_message(&conn, "sess_1", "msg_1").unwrap().unwrap();
        assert_eq!(role, "user");
        assert_eq!(branch_id, "branch_main");

        assert!(get_message(&conn, "sess_1", "msg_999").unwrap().is_none());
    }

    #[test]
    fn test_count_and_list_messages_from() {
        let conn = test_conn();
        insert_msg(&conn, "msg_1", "user", "第一条", "2026-08-07T00:00:01Z");
        insert_msg(&conn, "msg_2", "assistant", "回复", "2026-08-07T00:00:02Z");
        insert_msg(&conn, "msg_3", "user", "第二条", "2026-08-07T00:00:03Z");
        insert_msg(&conn, "msg_4", "assistant", "回复2", "2026-08-07T00:00:04Z");

        // 从 msg_2 开始：含 msg_2/3/4
        assert_eq!(
            count_messages_from(&conn, "sess_1", "branch_main", "msg_2"),
            3
        );
        // 从 msg_1 开始：全部 4 条
        assert_eq!(
            count_messages_from(&conn, "sess_1", "branch_main", "msg_1"),
            4
        );
        // 不存在的消息：0
        assert_eq!(
            count_messages_from(&conn, "sess_1", "branch_main", "msg_999"),
            0
        );

        let from_msgs = list_messages_from(&conn, "sess_1", "branch_main", "msg_3");
        assert_eq!(from_msgs.len(), 2);
        assert_eq!(from_msgs[0].id, "msg_3");
        assert_eq!(from_msgs[1].id, "msg_4");
    }

    #[test]
    fn test_delete_messages_from() {
        let conn = test_conn();
        insert_msg(&conn, "msg_1", "user", "第一条", "2026-08-07T00:00:01Z");
        insert_msg(&conn, "msg_2", "assistant", "回复", "2026-08-07T00:00:02Z");
        insert_msg(&conn, "msg_3", "user", "第二条", "2026-08-07T00:00:03Z");
        insert_msg(&conn, "msg_4", "assistant", "回复2", "2026-08-07T00:00:04Z");

        // 删除 msg_2 及之后（3 条），保留 msg_1
        let affected = delete_messages_from(&conn, "sess_1", "branch_main", "msg_2").unwrap();
        assert_eq!(affected, 3);
        let remain = list_messages(&conn, "sess_1", "branch_main");
        assert_eq!(remain.len(), 1);
        assert_eq!(remain[0].id, "msg_1");

        // 再次删除已不存在的消息：0
        let affected = delete_messages_from(&conn, "sess_1", "branch_main", "msg_2").unwrap();
        assert_eq!(affected, 0);
    }

    #[test]
    fn test_delete_messages_from_branch_isolation() {
        let conn = test_conn();
        insert_msg(&conn, "msg_1", "user", "分支A", "2026-08-07T00:00:01Z");
        // 另一分支的消息（branch_b）
        create_message(
            &conn, "msg_b1", "sess_1", "user", "分支B", None, None, None, None, None, None, None,
            None, "branch_b", None,
        )
        .unwrap();

        // 从 branch_b 的角度删除 msg_1 所在时间点：msg_1 不属于 branch_b，不受影响
        let affected = delete_messages_from(&conn, "sess_1", "branch_b", "msg_1").unwrap();
        assert_eq!(affected, 0);
        assert_eq!(list_messages(&conn, "sess_1", "branch_main").len(), 1);
    }

    #[test]
    fn test_update_message_metadata() {
        let conn = test_conn();
        insert_msg(&conn, "msg_1", "user", "第一条", "2026-08-07T00:00:01Z");

        // 更新 metadata（快照信息回填）
        let metadata = serde_json::json!({
            "snapshot": { "kind": "git", "createdAt": "2026-08-07T00:00:05Z" }
        });
        update_message_metadata(&conn, "msg_1", "sess_1", &metadata).unwrap();

        let msgs = list_messages(&conn, "sess_1", "branch_main");
        assert_eq!(msgs.len(), 1);
        assert_eq!(
            msgs[0].metadata.as_ref().unwrap()["snapshot"]["kind"],
            "git"
        );

        // 覆盖更新（原 metadata 被替换）
        let metadata2 = serde_json::json!({ "snapshot": { "kind": "files" } });
        update_message_metadata(&conn, "msg_1", "sess_1", &metadata2).unwrap();
        let msgs = list_messages(&conn, "sess_1", "branch_main");
        assert_eq!(
            msgs[0].metadata.as_ref().unwrap()["snapshot"]["kind"],
            "files"
        );
    }
}
