//! Skill 注册表:管理已加载的 Skill，提供按模式过滤和查询接口

use crate::errors::CommandError;
use crate::models::skill::Skill;
use crate::services::skill::loader::SkillLoader;
use std::collections::HashMap;
use std::sync::RwLock;

/// Skill 注册表
/// 持有 SkillLoader 和已加载的 Skill 列表(按名称索引)
pub struct SkillRegistry {
    /// Skill 加载器
    loader: SkillLoader,
    /// 已加载的 Skill(按 name 索引)
    skills: RwLock<HashMap<String, Skill>>,
}

impl SkillRegistry {
    /// 创建 Skill 注册表
    pub fn new(loader: SkillLoader) -> Self {
        Self {
            loader,
            skills: RwLock::new(HashMap::new()),
        }
    }

    /// 重新加载所有 Skill(从文件系统重新扫描)
    /// 返回加载的 Skill 数量
    pub fn reload(&self) -> Result<usize, CommandError> {
        let loaded = self.loader.load_all()?;
        let count = loaded.len();
        let mut skills = self
            .skills
            .write()
            .map_err(|e| CommandError::runtime(7001, format!("Skill 注册表锁中毒: {}", e)))?;
        skills.clear();
        for skill in loaded {
            skills.insert(skill.frontmatter.name.clone(), skill);
        }
        log::info!("Skill 注册表已重新加载，共 {} 个 Skill", count);
        Ok(count)
    }

    /// 检查是否有变更并重载(用于热重载场景)
    /// 简化实现:直接调用 reload(文件监听触发时总是重载)
    pub fn reload_if_changed(&self) -> Result<usize, CommandError> {
        self.reload()
    }

    /// 获取所有 Skill(按名称排序)
    pub fn list_all(&self) -> Vec<Skill> {
        let skills = self.skills.read().unwrap_or_else(|e| {
            log::error!("Skill 注册表读锁失败: {}", e);
            panic!("Skill 注册表锁中毒")
        });
        let mut result: Vec<Skill> = skills.values().cloned().collect();
        result.sort_by(|a, b| a.frontmatter.name.cmp(&b.frontmatter.name));
        result
    }

    /// 按模式过滤 Skill
    /// 返回适用于指定模式的 Skill 列表(按名称排序)
    pub fn list_by_mode(&self, mode: &str) -> Vec<Skill> {
        self.list_all()
            .into_iter()
            .filter(|s| s.is_applicable_to_mode(mode))
            .collect()
    }

    /// 按名称获取 Skill
    pub fn get_by_name(&self, name: &str) -> Option<Skill> {
        let skills = self.skills.read().unwrap_or_else(|e| {
            log::error!("Skill 注册表读锁失败: {}", e);
            panic!("Skill 注册表锁中毒")
        });
        skills.get(name).cloned()
    }

    /// 生成系统提示词中的 Skill 清单段
    /// 格式:
    /// ```text
    /// ## Available Skills
    ///
    /// - skill-name: description (when: trigger)
    /// - another-skill: description
    ///
    /// Use the `skill` tool with action=load to load a skill's full content.
    /// ```
    /// 若无 Skill，返回空字符串
    pub fn build_summary_for_prompt(&self, mode: &str) -> String {
        let skills = self.list_by_mode(mode);
        if skills.is_empty() {
            return String::new();
        }
        let mut summary = String::from("\n\n## Available Skills\n\n");
        for skill in &skills {
            summary.push_str(&skill.to_summary_line());
            summary.push('\n');
        }
        summary.push_str("\nUse the `skill` tool with action=load to load a skill's full content.");
        summary
    }
}
