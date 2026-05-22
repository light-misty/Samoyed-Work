use tauri::State;
use uuid::Uuid;

use crate::errors::CommandError;
use crate::models::{PromptTemplate, CreateTemplateParams, UpdateTemplateParams};
use crate::AppState;

/// 列出所有 Prompt 模板
#[tauri::command]
pub async fn list_templates(state: State<'_, AppState>) -> Result<Vec<PromptTemplate>, CommandError> {
    log::info!("list_templates: 查询所有模板");
    let conn = state.db.conn()?;
    let templates = crate::db::template_repo::list_templates(&conn)?;
    log::info!("list_templates: 查询完成, 共 {} 个模板", templates.len());
    Ok(templates)
}

/// 获取单个 Prompt 模板
#[tauri::command]
pub async fn get_template(
    template_id: String,
    state: State<'_, AppState>,
) -> Result<PromptTemplate, CommandError> {
    log::info!("get_template: template_id={}", template_id);
    let conn = state.db.conn()?;
    crate::db::template_repo::get_template(&conn, &template_id)
}

/// 创建 Prompt 模板
#[tauri::command]
pub async fn create_template(
    params: CreateTemplateParams,
    state: State<'_, AppState>,
) -> Result<PromptTemplate, CommandError> {
    log::info!("create_template: name={}", params.name);
    let id = format!("tpl-{}", &Uuid::new_v4().to_string().replace("-", "")[..12]);
    let conn = state.db.conn()?;
    crate::db::template_repo::create_template(
        &conn,
        &id,
        &params.name,
        &params.description,
        &params.content,
        &params.category,
        params.variables.as_ref(),
    )?;
    crate::db::template_repo::get_template(&conn, &id)
}

/// 更新 Prompt 模板
#[tauri::command]
pub async fn update_template(
    template_id: String,
    params: UpdateTemplateParams,
    state: State<'_, AppState>,
) -> Result<PromptTemplate, CommandError> {
    log::info!("update_template: template_id={}", template_id);
    let conn = state.db.conn()?;
    crate::db::template_repo::update_template(
        &conn,
        &template_id,
        params.name.as_deref(),
        params.description.as_deref(),
        params.content.as_deref(),
        params.category.as_deref(),
        params.variables.as_ref(),
    )?;
    crate::db::template_repo::get_template(&conn, &template_id)
}

/// 删除 Prompt 模板
#[tauri::command]
pub async fn delete_template(
    template_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("delete_template: template_id={}", template_id);
    let conn = state.db.conn()?;
    crate::db::template_repo::delete_template(&conn, &template_id)
}
