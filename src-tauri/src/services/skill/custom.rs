use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value};

use crate::errors::CommandError;
use crate::models::skill::{CustomSkillConfig, SkillResult};
use super::registry::{Skill, SkillRegistry};

/// 自定义 Skill 存储目录名
const CUSTOM_SKILLS_DIR: &str = "custom_skills";

/// 自定义 Skill 加载器
/// 负责从 JSON 文件加载、保存、删除自定义 Skill 配置
pub struct CustomSkillLoader {
    /// 自定义 Skill 存储目录
    skills_dir: PathBuf,
}

impl CustomSkillLoader {
    /// 创建加载器实例，使用应用数据目录下的 config/custom_skills/ 路径
    pub fn new(app_data_dir: &Path) -> Self {
        let skills_dir = app_data_dir.join("config").join(CUSTOM_SKILLS_DIR);
        // 确保目录存在
        if !skills_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&skills_dir) {
                log::error!("创建自定义 Skill 目录失败: {}, 路径: {:?}", e, skills_dir);
            }
        }
        Self { skills_dir }
    }

    /// 加载所有自定义 Skill 配置
    pub fn load_all(&self) -> Vec<CustomSkillConfig> {
        if !self.skills_dir.exists() {
            log::info!("自定义 Skill 目录不存在，跳过加载");
            return Vec::new();
        }

        let mut configs = Vec::new();
        let entries = match std::fs::read_dir(&self.skills_dir) {
            Ok(e) => e,
            Err(e) => {
                log::error!("读取自定义 Skill 目录失败: {}", e);
                return Vec::new();
            }
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    match serde_json::from_str::<CustomSkillConfig>(&content) {
                        Ok(config) => {
                            log::info!("加载自定义 Skill: {} ({})", config.name, config.id);
                            configs.push(config);
                        }
                        Err(e) => {
                            log::error!("解析自定义 Skill 配置失败: {:?}, 错误: {}", path, e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("读取自定义 Skill 文件失败: {:?}, 错误: {}", path, e);
                }
            }
        }

        log::info!("自定义 Skill 加载完成, 共 {} 个", configs.len());
        configs
    }

    /// 保存自定义 Skill 配置到 JSON 文件
    pub fn save(&self, config: &CustomSkillConfig) -> Result<(), CommandError> {
        // 确保目录存在
        if !self.skills_dir.exists() {
            std::fs::create_dir_all(&self.skills_dir).map_err(|e| {
                CommandError::config(5001, format!("创建自定义 Skill 目录失败: {}", e))
            })?;
        }

        let file_path = self.skills_dir.join(format!("{}.json", config.id));
        let content = serde_json::to_string_pretty(config).map_err(|e| {
            CommandError::config(5002, format!("序列化自定义 Skill 配置失败: {}", e))
        })?;

        std::fs::write(&file_path, content).map_err(|e| {
            CommandError::config(5003, format!("写入自定义 Skill 文件失败: {}", e))
        })?;

        log::info!("自定义 Skill 已保存: {} ({})", config.name, config.id);
        Ok(())
    }

    /// 删除自定义 Skill 配置文件
    pub fn delete(&self, skill_id: &str) -> Result<(), CommandError> {
        let file_path = self.skills_dir.join(format!("{}.json", skill_id));

        if !file_path.exists() {
            return Err(CommandError::config(5004, format!("自定义 Skill 不存在: {}", skill_id)));
        }

        std::fs::remove_file(&file_path).map_err(|e| {
            CommandError::config(5005, format!("删除自定义 Skill 文件失败: {}", e))
        })?;

        log::info!("自定义 Skill 已删除: {}", skill_id);
        Ok(())
    }

    /// 获取单个自定义 Skill 配置
    pub fn load_one(&self, skill_id: &str) -> Result<CustomSkillConfig, CommandError> {
        let file_path = self.skills_dir.join(format!("{}.json", skill_id));

        if !file_path.exists() {
            return Err(CommandError::config(5004, format!("自定义 Skill 不存在: {}", skill_id)));
        }

        let content = std::fs::read_to_string(&file_path).map_err(|e| {
            CommandError::config(5003, format!("读取自定义 Skill 文件失败: {}", e))
        })?;

        serde_json::from_str::<CustomSkillConfig>(&content).map_err(|e| {
            CommandError::config(5002, format!("解析自定义 Skill 配置失败: {}", e))
        })
    }

    /// 将所有已加载的自定义 Skill 注册到 SkillRegistry
    pub fn register_all(&self, registry: &mut SkillRegistry) {
        let configs = self.load_all();
        for config in configs {
            let skill = PromptBasedSkill::from_config(config);
            registry.register(Box::new(skill));
        }
    }
}

/// 提示词模板型自定义 Skill
/// LLM 调用此 Skill 时，参数被替换到提示词模板中，
/// 渲染后的文本作为工具结果返回给 LLM，指导其后续行为
pub struct PromptBasedSkill {
    config: CustomSkillConfig,
}

impl PromptBasedSkill {
    /// 从配置创建实例
    pub fn from_config(config: CustomSkillConfig) -> Self {
        Self { config }
    }

    /// 渲染提示词模板
    /// 将 {{param_name}} 格式的占位符替换为实际参数值
    fn render_template(&self, params: &Value) -> String {
        let mut result = self.config.prompt_template.clone();

        // 从参数中提取所有键值对
        if let Some(obj) = params.as_object() {
            for (key, value) in obj {
                let placeholder = format!("{{{{{}}}}}", key);
                // 将 JSON 值转换为字符串
                let str_value = match value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Null => String::new(),
                    // 数组和对象序列化为 JSON 字符串
                    other => serde_json::to_string(other).unwrap_or_default(),
                };
                result = result.replace(&placeholder, &str_value);
            }
        }

        // 处理 workspace_root 特殊参数（由 executor 注入，不在模板中暴露）
        // 不需要替换，因为自定义 Skill 不应该直接操作文件系统

        result
    }
}

#[async_trait]
impl Skill for PromptBasedSkill {
    fn skill_name(&self) -> &str {
        // 使用 "custom_" 前缀避免与内置 Skill 名称冲突
        // 但如果 config.id 本身已经有前缀则直接使用
        &self.config.id
    }

    fn description(&self) -> &str {
        &self.config.description
    }

    fn category(&self) -> &str {
        &self.config.category
    }

    fn is_builtin(&self) -> bool {
        false
    }

    fn supported_types(&self) -> Vec<String> {
        self.config.supported_types.clone()
    }

    fn parameters(&self) -> Value {
        // 如果配置中提供了参数 Schema，直接使用
        if let Some(ref schema) = self.config.params_schema {
            return schema.clone();
        }

        // 否则从模板中自动推断参数
        // 扫描 {{param_name}} 占位符生成基础 Schema
        let mut properties = HashMap::new();
        let mut required = Vec::new();

        // 使用正则或简单扫描提取 {{xxx}} 占位符
        let template = &self.config.prompt_template;
        let mut start = 0;
        while let Some(pos) = template[start..].find("{{") {
            let abs_pos = start + pos;
            if let Some(end_pos) = template[abs_pos..].find("}}") {
                let param_name = template[abs_pos + 2..abs_pos + end_pos].trim();
                if !param_name.is_empty() && !param_name.contains(' ') {
                    properties.insert(
                        param_name.to_string(),
                        json!({
                            "type": "string",
                            "description": format!("{}参数", param_name)
                        }),
                    );
                    required.push(param_name.to_string());
                }
                start = abs_pos + end_pos + 2;
            } else {
                break;
            }
        }

        json!({
            "type": "object",
            "properties": properties,
            "required": required,
        })
    }

    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();

        // 渲染提示词模板
        let rendered = self.render_template(&params);

        // 返回渲染后的提示词作为工具结果
        // LLM 会将此结果作为上下文，指导其后续行为
        SkillResult {
            success: true,
            output: Some(json!({
                "prompt": rendered,
                "skill_name": self.config.name,
                "skill_id": self.config.id,
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }
}

/// 创建新的自定义 Skill 配置
/// 自动生成 id 和时间戳
pub fn create_custom_skill_config(
    name: String,
    description: String,
    category: String,
    prompt_template: String,
    supported_types: Vec<String>,
    params_schema: Option<Value>,
) -> CustomSkillConfig {
    let now = Utc::now().to_rfc3339();
    // 生成符合 function naming 规范的 ID：custom_ 前缀 + 名称的 snake_case 形式
    let id = format!("custom_{}", name.to_lowercase().replace([' ', '-'], "_"));

    CustomSkillConfig {
        id,
        name,
        description,
        category,
        prompt_template,
        supported_types,
        params_schema,
        version: "1.0.0".to_string(),
        created_at: now.clone(),
        updated_at: now,
    }
}

/// 更新自定义 Skill 配置的时间戳
pub fn update_custom_skill_config_timestamp(mut config: CustomSkillConfig) -> CustomSkillConfig {
    config.updated_at = Utc::now().to_rfc3339();
    config
}
