use tauri::State;

use crate::errors::CommandError;
use crate::models::skill::{CustomSkillConfig, SkillInfo};
use crate::services::skill::custom::{create_custom_skill_config, update_custom_skill_config_timestamp};
use crate::AppState;

/// 列出所有 Skill（内置 + 自定义）
#[tauri::command]
pub async fn list_skills(state: State<'_, AppState>) -> Result<Vec<SkillInfo>, CommandError> {
    log::info!("list_skills: 查询所有 Skill");
    let skills = {
        let reg = state.skill_registry.lock().await;
        reg.list_skills()
    };
    log::info!("list_skills: 查询完成, 共 {} 个 Skill", skills.len());
    Ok(skills)
}

/// 列出所有自定义 Skill 配置
#[tauri::command]
pub async fn list_custom_skills(state: State<'_, AppState>) -> Result<Vec<CustomSkillConfig>, CommandError> {
    log::info!("list_custom_skills: 查询所有自定义 Skill");
    let configs = state.custom_skill_loader.load_all();
    log::info!("list_custom_skills: 查询完成, 共 {} 个", configs.len());
    Ok(configs)
}

/// 切换 Skill 启用/禁用状态，并持久化到配置
#[tauri::command]
pub async fn toggle_skill(
    skill_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("toggle_skill: skill_id={}, enabled={}", skill_id, enabled);

    // 更新注册表中的状态
    let disabled_list = {
        let mut registry = state.skill_registry.lock().await;
        registry.toggle_skill(&skill_id, enabled)
    };

    // 持久化到配置文件
    let cfg_manager = state.config.lock().await;
    let mut settings = cfg_manager.load_app_settings().map_err(|e| {
        log::error!("加载应用设置失败: {}", e);
        e
    })?;
    settings.disabled_skills = disabled_list;
    cfg_manager.save_app_settings(&settings).map_err(|e| {
        log::error!("保存应用设置失败: {}", e);
        e
    })?;

    log::info!("toggle_skill: 状态已持久化, skill_id={}, enabled={}", skill_id, enabled);
    Ok(())
}

/// 添加自定义 Skill
/// 创建配置文件并注册到 SkillRegistry
#[tauri::command]
pub async fn add_custom_skill(
    config: CustomSkillConfig,
    state: State<'_, AppState>,
) -> Result<CustomSkillConfig, CommandError> {
    log::info!("add_custom_skill: name={}", config.name);

    // 校验名称不为空
    if config.name.trim().is_empty() {
        return Err(CommandError::config(5006, "Skill 名称不能为空".to_string()));
    }

    // 校验提示词模板不为空
    if config.prompt_template.trim().is_empty() {
        return Err(CommandError::config(5007, "提示词模板不能为空".to_string()));
    }

    // 如果前端未提供 id，则自动生成
    let config = if config.id.is_empty() {
        create_custom_skill_config(
            config.name,
            config.description,
            config.category,
            config.prompt_template,
            config.supported_types,
            config.params_schema,
        )
    } else {
        config
    };

    // 保存配置文件
    state.custom_skill_loader.save(&config)?;

    // 注册到 SkillRegistry
    {
        let mut registry = state.skill_registry.lock().await;
        let skill = crate::services::skill::custom::PromptBasedSkill::from_config(config.clone());
        registry.register(Box::new(skill));
    }

    log::info!("add_custom_skill: 添加成功, id={}", config.id);
    Ok(config)
}

/// 更新自定义 Skill
/// 更新配置文件并重新注册到 SkillRegistry
#[tauri::command]
pub async fn update_custom_skill(
    config: CustomSkillConfig,
    state: State<'_, AppState>,
) -> Result<CustomSkillConfig, CommandError> {
    log::info!("update_custom_skill: id={}, name={}", config.id, config.name);

    // 校验 Skill 存在
    state.custom_skill_loader.load_one(&config.id)?;

    // 校验名称不为空
    if config.name.trim().is_empty() {
        return Err(CommandError::config(5006, "Skill 名称不能为空".to_string()));
    }

    // 校验提示词模板不为空
    if config.prompt_template.trim().is_empty() {
        return Err(CommandError::config(5007, "提示词模板不能为空".to_string()));
    }

    // 更新时间戳
    let config = update_custom_skill_config_timestamp(config);

    // 保存配置文件
    state.custom_skill_loader.save(&config)?;

    // 先从注册表中移除旧的 Skill，再注册新的
    {
        let mut registry = state.skill_registry.lock().await;
        // SkillRegistry 目前没有 unregister 方法，需要先移除再添加
        // 由于 register 会覆盖同名 Skill，直接注册即可
        let skill = crate::services::skill::custom::PromptBasedSkill::from_config(config.clone());
        registry.register(Box::new(skill));
    }

    log::info!("update_custom_skill: 更新成功, id={}", config.id);
    Ok(config)
}

/// 删除自定义 Skill
/// 删除配置文件并从 SkillRegistry 中移除
#[tauri::command]
pub async fn delete_custom_skill(
    skill_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("delete_custom_skill: skill_id={}", skill_id);

    // 删除配置文件
    state.custom_skill_loader.delete(&skill_id)?;

    // 从 SkillRegistry 中移除
    {
        let mut registry = state.skill_registry.lock().await;
        registry.unregister(&skill_id);
    }

    log::info!("delete_custom_skill: 删除成功, skill_id={}", skill_id);
    Ok(())
}
