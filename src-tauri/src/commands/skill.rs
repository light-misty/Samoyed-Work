use std::collections::HashSet;

use tauri::State;

use crate::errors::CommandError;
use crate::models::skill::SkillInfo;
use crate::services::skill::loader::SkillLoader;
use crate::AppState;

/// 列出所有可用 Skill
///
/// 返回全局、项目及工作区（若提供 workspace_path）的 Skill 摘要列表。
/// 同名 Skill 去重，优先级：注册表 > 工作区。
#[tauri::command]
pub async fn list_skills(
    state: State<'_, AppState>,
    workspace_path: Option<String>,
) -> Result<Vec<SkillInfo>, CommandError> {
    let registry = &state.skill_registry;
    let mut skills: Vec<SkillInfo> = registry.list_all().into_iter().map(Into::into).collect();

    if let Some(ws_path) = workspace_path {
        if !ws_path.is_empty() {
            let registry_names: HashSet<String> = skills.iter().map(|s| s.name.clone()).collect();
            let ws_skills = SkillLoader::load_workspace_skills(&ws_path);
            for skill in ws_skills {
                if !registry_names.contains(&skill.frontmatter.name) {
                    skills.push(SkillInfo::from(skill));
                }
            }
        }
    }

    skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(skills)
}

/// 获取指定 Skill 的完整 markdown 内容
///
/// 按名称从注册表或工作区查找 Skill，返回完整定义。
/// 若 name 为空或未找到，返回错误。
#[tauri::command]
pub async fn get_skill_content(
    state: State<'_, AppState>,
    name: String,
    workspace_path: Option<String>,
) -> Result<crate::models::skill::Skill, CommandError> {
    if name.is_empty() {
        return Err(CommandError::tool(
            crate::errors::TOOL_INVALID_PARAMS,
            "name 参数不能为空".to_string(),
        ));
    }

    // 先查注册表
    if let Some(skill) = state.skill_registry.get_by_name(&name) {
        return Ok(skill);
    }

    // 再查工作区
    if let Some(ws_path) = workspace_path {
        if !ws_path.is_empty() {
            let ws_skills = SkillLoader::load_workspace_skills(&ws_path);
            if let Some(skill) = ws_skills.into_iter().find(|s| s.frontmatter.name == name) {
                return Ok(skill);
            }
        }
    }

    Err(CommandError::tool(
        crate::errors::TOOL_NOT_FOUND,
        format!("Skill 不存在: {}", name),
    ))
}
