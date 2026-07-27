//! Skill 工具:Agent 通过此工具按需加载 Skill 的详细内容
//! 支持 action=load(加载指定 Skill)和 action=list(列出可用 Skill)

use crate::errors::{self, CommandError};
use crate::models::tool::ToolResult;
use crate::services::skill::registry::SkillRegistry;
use crate::services::tool::trait_def::Tool;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

/// Skill 工具:按需加载领域能力(Skill)的完整 markdown 内容
/// 系统提示词中仅注入 Skill 清单,Agent 通过此工具加载实际内容
pub struct SkillTool {
    /// Skill 注册表(共享只读访问)
    registry: Arc<SkillRegistry>,
}

impl SkillTool {
    /// 创建 SkillTool 实例
    pub fn new(registry: Arc<SkillRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for SkillTool {
    fn tool_name(&self) -> &str {
        "skill"
    }

    fn description(&self) -> &str {
        "Load the full content of a domain capability (Skill). A list of available Skills \
         is provided in the system prompt; use this tool to load a Skill's complete markdown content on demand."
    }

    fn category(&self) -> &str {
        "skill"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["load", "list"],
                    "description": "Action type: load=load full content of a specified Skill, list=list all available Skills"
                },
                "name": {
                    "type": "string",
                    "description": "Skill 名称(action=load 时必填)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult {
        let start = std::time::Instant::now();
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let workspace_path = params
            .get("_workspace_path")
            .and_then(|v| v.as_str())
            .filter(|p| !p.is_empty());

        let result = match action {
            // 列出所有可用 Skill（含工作区 Skill）
            "list" => {
                let mut all_skills = self.registry.list_all();
                if let Some(ws_path) = workspace_path {
                    let ws_skills =
                        crate::services::skill::loader::SkillLoader::load_workspace_skills(ws_path);
                    // 合并工作区 Skill，同名不覆盖（工作区优先级低于注册表 Skill）
                    let registry_names: std::collections::HashSet<String> = all_skills
                        .iter()
                        .map(|s| s.frontmatter.name.clone())
                        .collect();
                    for skill in ws_skills {
                        if !registry_names.contains(&skill.frontmatter.name) {
                            all_skills.push(skill);
                        }
                    }
                }
                let total = all_skills.len();
                let summary: Vec<Value> = all_skills
                    .iter()
                    .map(|s| {
                        json!({
                            "name": s.frontmatter.name,
                            "description": s.frontmatter.description,
                            "when": s.frontmatter.when,
                            "modes": s.frontmatter.modes,
                            "tags": s.frontmatter.tags,
                            "readOnly": s.frontmatter.read_only,
                            "source": format!("{:?}", s.source),
                        })
                    })
                    .collect();
                Ok(json!({
                    "skills": summary,
                    "total": total,
                }))
            }
            // 加载指定 Skill 的完整内容（先查注册表，再查工作区）
            "load" => match params.get("name").and_then(|v| v.as_str()) {
                Some(name) => {
                    // 先查注册表
                    if let Some(skill) = self.registry.get_by_name(name) {
                        Ok(json!({
                            "name": skill.frontmatter.name,
                            "description": skill.frontmatter.description,
                            "when": skill.frontmatter.when,
                            "modes": skill.frontmatter.modes,
                            "tags": skill.frontmatter.tags,
                            "readOnly": skill.frontmatter.read_only,
                            "source": format!("{:?}", skill.source),
                            "content": skill.content,
                            "filePath": skill.file_path.to_string_lossy(),
                        }))
                    } else if let Some(ws_path) = workspace_path {
                        // 再查工作区
                        let ws_skills =
                            crate::services::skill::loader::SkillLoader::load_workspace_skills(
                                ws_path,
                            );
                        match ws_skills.into_iter().find(|s| s.frontmatter.name == name) {
                            Some(skill) => Ok(json!({
                                "name": skill.frontmatter.name,
                                "description": skill.frontmatter.description,
                                "when": skill.frontmatter.when,
                                "modes": skill.frontmatter.modes,
                                "tags": skill.frontmatter.tags,
                                "readOnly": skill.frontmatter.read_only,
                                "source": format!("{:?}", skill.source),
                                "content": skill.content,
                                "filePath": skill.file_path.to_string_lossy(),
                            })),
                            None => Err(CommandError::tool(
                                errors::TOOL_NOT_FOUND,
                                format!("Skill 不存在: {}", name),
                            )),
                        }
                    } else {
                        Err(CommandError::tool(
                            errors::TOOL_NOT_FOUND,
                            format!("Skill 不存在: {}", name),
                        ))
                    }
                }
                None => Err(CommandError::tool(
                    errors::TOOL_INVALID_PARAMS,
                    "load 操作需要 name 参数".to_string(),
                )),
            },
            _ => Err(CommandError::tool(
                errors::TOOL_INVALID_PARAMS,
                format!("未知操作: {}", action),
            )),
        };

        let duration_ms = start.elapsed().as_millis() as u64;
        match result {
            Ok(output) => ToolResult {
                success: true,
                output: Some(output),
                error: None,
                duration_ms,
                error_code: None,
            },
            Err(e) => ToolResult {
                success: false,
                output: None,
                error: Some(e.to_string()),
                duration_ms,
                error_code: Some(e.code),
            },
        }
    }
}
