//! Skill 模型定义
//! Skill 是可注入的领域能力包,通过 SKILL.md 文件定义

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Skill 来源类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillSource {
    /// 全局目录(~/.agent/skills/)
    Global,
    /// 项目目录(.agent/skills/)
    Project,
    /// 配置路径
    Configured,
}

/// Skill frontmatter 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillFrontmatter {
    /// Skill 名称(唯一标识)
    pub name: String,
    /// 简短描述(用于系统提示词清单)
    pub description: String,
    /// 触发条件(可选,Agent 据此判断是否加载)
    #[serde(default)]
    pub when: Option<String>,
    /// 适用 Agent 模式(可选,默认适用 plan/build/document 所有模式)
    /// 文档相关 Skill 可设置为 ["document"] 仅在 Document 模式下可见
    #[serde(default)]
    pub modes: Vec<String>,
    /// 标签(可选,用于分类)
    #[serde(default)]
    pub tags: Vec<String>,
    /// 是否为只读 Skill(不修改文件)
    #[serde(default)]
    pub read_only: bool,
}

/// Skill 完整定义
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    /// Skill 元数据
    pub frontmatter: SkillFrontmatter,
    /// markdown 正文(Skill 详细说明)
    pub content: String,
    /// Skill 来源
    pub source: SkillSource,
    /// SKILL.md 文件路径
    pub file_path: PathBuf,
    /// Skill 目录路径(SKILL.md 所在目录)
    pub dir_path: PathBuf,
    /// 最后修改时间(UNIX 时间戳,秒)
    pub modified_at: u64,
}

impl Skill {
    /// 检查 Skill 是否适用于指定 Agent 模式
    /// 支持 "plan" / "build" / "document" 三种模式字符串
    /// 若 modes 为空,默认适用于所有模式(含 document)
    pub fn is_applicable_to_mode(&self, mode: &str) -> bool {
        if self.frontmatter.modes.is_empty() {
            // 默认适用于所有模式(plan/build/document)
            return true;
        }
        self.frontmatter.modes.iter().any(|m| m == mode)
    }

    /// 生成系统提示词中的 Skill 清单条目
    pub fn to_summary_line(&self) -> String {
        let when_hint = self
            .frontmatter
            .when
            .as_ref()
            .map(|w| format!(" (when: {})", w))
            .unwrap_or_default();
        format!(
            "- {}: {}{}",
            self.frontmatter.name, self.frontmatter.description, when_hint
        )
    }
}
