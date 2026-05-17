use tauri::State;

use crate::errors::CommandError;
use crate::models::skill::{CustomSkillConfig, SkillInfo};
use crate::AppState;

/// 列出所有 Skill
#[tauri::command]
pub async fn list_skills(_state: State<'_, AppState>) -> Result<Vec<SkillInfo>, CommandError> {
    log::info!("list_skills: 查询所有 Skill");
    let skills = vec![
        SkillInfo {
            id: "doc-create".to_string(),
            name: "文档创建".to_string(),
            description: "根据自然语言描述创建 Word/Excel/PPT/PDF 文档".to_string(),
            category: "document".to_string(),
            is_builtin: true,
            enabled: true,
            version: "1.0.0".to_string(),
            params_schema: None,
            supported_types: vec![
                "docx".to_string(),
                "xlsx".to_string(),
                "pptx".to_string(),
                "pdf".to_string(),
            ],
        },
        SkillInfo {
            id: "doc-edit".to_string(),
            name: "文档编辑".to_string(),
            description: "修改已有文档的内容、格式和样式".to_string(),
            category: "document".to_string(),
            is_builtin: true,
            enabled: true,
            version: "1.0.0".to_string(),
            params_schema: None,
            supported_types: vec![
                "docx".to_string(),
                "xlsx".to_string(),
                "pptx".to_string(),
            ],
        },
        SkillInfo {
            id: "doc-convert".to_string(),
            name: "格式转换".to_string(),
            description: "在不同文档格式之间进行转换".to_string(),
            category: "format".to_string(),
            is_builtin: true,
            enabled: true,
            version: "1.0.0".to_string(),
            params_schema: None,
            supported_types: vec![
                "docx".to_string(),
                "pdf".to_string(),
                "md".to_string(),
                "html".to_string(),
            ],
        },
        SkillInfo {
            id: "data-analysis".to_string(),
            name: "数据分析".to_string(),
            description: "对 Excel/CSV 数据进行分析和可视化".to_string(),
            category: "data".to_string(),
            is_builtin: true,
            enabled: true,
            version: "1.0.0".to_string(),
            params_schema: None,
            supported_types: vec!["xlsx".to_string(), "csv".to_string()],
        },
    ];
    log::info!("list_skills: 查询完成, 共 {} 个 Skill", skills.len());
    Ok(skills)
}

/// 切换 Skill 启用/禁用状态
#[tauri::command]
pub async fn toggle_skill(
    skill_id: String,
    enabled: bool,
    _state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "Skill '{}' 已{}",
        skill_id,
        if enabled { "启用" } else { "禁用" }
    );
    Ok(())
}

/// 添加自定义 Skill
#[tauri::command]
pub async fn add_custom_skill(
    config: CustomSkillConfig,
    _state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("添加自定义 Skill: {}", config.name);
    Ok(())
}

/// 删除自定义 Skill
#[tauri::command]
pub async fn delete_custom_skill(
    skill_id: String,
    _state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("删除自定义 Skill: {}", skill_id);
    Ok(())
}
