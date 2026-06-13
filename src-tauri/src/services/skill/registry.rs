use std::collections::HashMap;
use std::sync::Arc;
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
/// 使用 Arc<dyn Skill> 存储技能，允许在锁外执行技能，避免长时间持锁阻塞其他操作
/// 内置技能始终启用，不可禁用
pub struct SkillRegistry {
    skills: HashMap<String, Arc<dyn Skill>>,
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// 注册内置技能
    pub fn register(&mut self, skill: Box<dyn Skill>) {
        let name = skill.skill_name().to_string();
        log::info!("注册技能: {}", name);
        self.skills.insert(name.clone(), Arc::from(skill));
        log::debug!("技能注册完成: {}, 当前注册总数: {}", name, self.skills.len());
    }

    /// 获取技能的 Arc 引用（可在锁外使用）
    pub fn get_arc(&self, name: &str) -> Option<Arc<dyn Skill>> {
        self.skills.get(name).cloned()
    }

    /// 获取技能
    pub fn get(&self, name: &str) -> Option<&dyn Skill> {
        self.skills.get(name).map(|s| s.as_ref())
    }

    /// 执行技能
    pub async fn execute(&self, name: &str, params: Value) -> SkillResult {
        log::info!("执行技能: {}", name);
        match self.skills.get(name) {
            Some(skill) => {
                log::debug!("找到技能: {}, 开始执行", name);
                let result = skill.execute(params).await;
                if result.success {
                    log::info!("技能执行成功: {}, 耗时: {}ms", name, result.duration_ms);
                } else {
                    log::error!("技能执行失败: {}, 错误: {}", name, result.error.as_deref().unwrap_or("未知错误"));
                }
                result
            }
            None => {
                log::error!("技能不存在: {}", name);
                SkillResult {
                    success: false,
                    output: None,
                    error: Some(format!("技能不存在: {}", name)),
                    duration_ms: 0,
                }
            }
        }
    }

    /// 生成 OpenAI function calling 格式的工具定义（包含所有已注册技能）
    pub fn tool_definitions(&self) -> Vec<Value> {
        log::debug!("生成工具定义, 技能总数: {}", self.skills.len());
        let definitions: Vec<Value> = self.skills.values()
            .map(|skill| {
                json!({
                    "type": "function",
                    "function": {
                        "name": skill.skill_name(),
                        "description": skill.description(),
                        "parameters": skill.parameters(),
                    }
                })
            }).collect();
        log::debug!("工具定义生成完成, 数量: {}", definitions.len());
        definitions
    }

    /// 列出所有技能信息（内置技能始终启用）
    pub fn list_skills(&self) -> Vec<SkillInfo> {
        self.skills.values().map(|skill| {
            let skill_id = skill.skill_name();
            SkillInfo {
                id: skill_id.to_string(),
                name: skill_id.to_string(),
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

    /// 检查技能是否存在
    pub fn contains_skill(&self, skill_id: &str) -> bool {
        self.skills.contains_key(skill_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockSkill { name: String }

    impl MockSkill {
        fn new(name: &str) -> Self { Self { name: name.to_string() } }
    }

    #[async_trait]
    impl Skill for MockSkill {
        fn skill_name(&self) -> &str { &self.name }
        fn description(&self) -> &str { "mock skill" }
        fn parameters(&self) -> Value { json!({"type": "object"}) }
        fn is_builtin(&self) -> bool { false }
        async fn execute(&self, _params: Value) -> crate::models::skill::SkillResult {
            crate::models::skill::SkillResult {
                success: true, output: None, error: None, duration_ms: 0,
            }
        }
    }

    #[test]
    fn test_register_and_list() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(MockSkill::new("test_skill")));
        let skills = registry.list_skills();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, "test_skill");
        assert!(!skills[0].is_builtin);
        // 内置技能始终启用
        assert!(skills[0].enabled);
    }

    #[test]
    fn test_all_skills_in_tool_definitions() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(MockSkill::new("skill_a")));
        registry.register(Box::new(MockSkill::new("skill_b")));
        let defs = registry.tool_definitions();
        // 所有注册的技能都应出现在工具定义中
        assert_eq!(defs.len(), 2);
    }
}
