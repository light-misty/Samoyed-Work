use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::models::skill::{SkillInfo, SkillResult};

/// Skill trait，所有技能必须实现此接口
#[async_trait]
pub trait Skill: Send + Sync {
    /// 技能名称（唯一标识）
    fn skill_name(&self) -> &str;

    /// 技能描述
    fn description(&self) -> &str;

    /// 参数 JSON Schema
    fn parameters(&self) -> Value;

    /// 技能分类
    fn category(&self) -> &str {
        "document"
    }

    /// 是否为内置技能
    fn is_builtin(&self) -> bool {
        true
    }

    /// 支持的文档类型
    fn supported_types(&self) -> Vec<String> {
        vec![]
    }

    /// 执行技能
    async fn execute(&self, params: Value) -> SkillResult;
}

/// Skill 注册表
pub struct SkillRegistry {
    skills: HashMap<String, Box<dyn Skill>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// 注册技能
    pub fn register(&mut self, skill: Box<dyn Skill>) {
        self.skills.insert(skill.skill_name().to_string(), skill);
    }

    /// 获取技能
    pub fn get(&self, name: &str) -> Option<&dyn Skill> {
        self.skills.get(name).map(|s| s.as_ref())
    }

    /// 执行技能
    pub async fn execute(&self, name: &str, params: Value) -> SkillResult {
        match self.skills.get(name) {
            Some(skill) => skill.execute(params).await,
            None => SkillResult {
                success: false,
                output: None,
                error: Some(format!("技能不存在: {}", name)),
                duration_ms: 0,
            },
        }
    }

    /// 生成 OpenAI function calling 格式的工具定义
    pub fn tool_definitions(&self) -> Vec<Value> {
        self.skills.values().map(|skill| {
            json!({
                "type": "function",
                "function": {
                    "name": skill.skill_name(),
                    "description": skill.description(),
                    "parameters": skill.parameters(),
                }
            })
        }).collect()
    }

    /// 列出所有技能信息
    pub fn list_skills(&self) -> Vec<SkillInfo> {
        self.skills.values().map(|skill| {
            SkillInfo {
                id: skill.skill_name().to_string(),
                name: skill.skill_name().to_string(),
                description: skill.description().to_string(),
                category: skill.category().to_string(),
                is_builtin: skill.is_builtin(),
                enabled: true,
                version: "1.0.0".to_string(),
                params_schema: Some(skill.parameters()),
                supported_types: skill.supported_types(),
            }
        }).collect()
    }
}
