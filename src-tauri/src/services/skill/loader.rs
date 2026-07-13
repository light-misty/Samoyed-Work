//! Skill 加载器:从文件系统扫描并解析 SKILL.md 文件
//! 支持三个加载目录:全局(~/.agent/skills/)、项目(.agent/skills/)、配置路径
//! 加载顺序:全局 -> 配置 -> 项目(后加载的覆盖先加载的同名 Skill)

use crate::errors::{CommandError, CONFIG_INVALID_FORMAT};
use crate::models::skill::{Skill, SkillFrontmatter, SkillSource};
use std::path::{Path, PathBuf};
use yaml_front_matter::{Document, YamlFrontMatter};

/// Skill 加载器
/// 负责从多个目录扫描 SKILL.md 文件并解析为 Skill 实例
pub struct SkillLoader {
    /// 全局 Skill 目录(~/.agent/skills/)
    global_dir: PathBuf,
    /// 项目 Skill 目录(.agent/skills/),可空
    project_dir: Option<PathBuf>,
    /// 配置的额外 Skill 目录
    extra_dirs: Vec<PathBuf>,
}

impl SkillLoader {
    /// 创建 Skill 加载器
    ///
    /// # 参数
    /// - `global_dir`: 全局 Skill 目录路径
    /// - `project_dir`: 项目 Skill 目录路径(可选)
    /// - `extra_dirs`: 通过配置注入的额外 Skill 目录列表
    pub fn new(
        global_dir: PathBuf,
        project_dir: Option<PathBuf>,
        extra_dirs: Vec<PathBuf>,
    ) -> Self {
        Self {
            global_dir,
            project_dir,
            extra_dirs,
        }
    }

    /// 扫描所有目录并加载 Skill
    ///
    /// 加载顺序:全局 -> 配置 -> 项目
    /// 后加载的同名 Skill 会覆盖先加载的(项目目录优先级最高)
    ///
    /// # 返回
    /// 按 Skill 名称去重后的列表
    pub fn load_all(&self) -> Result<Vec<Skill>, CommandError> {
        let mut skills: Vec<Skill> = Vec::new();
        // 记录已加载 Skill 名称及其在 skills 中的索引,用于覆盖逻辑
        let mut seen_names: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        // 1. 加载全局目录(优先级最低)
        self.load_from_dir(
            &self.global_dir,
            SkillSource::Global,
            &mut skills,
            &mut seen_names,
        )?;

        // 2. 加载配置目录(中等优先级)
        for dir in &self.extra_dirs {
            self.load_from_dir(dir, SkillSource::Configured, &mut skills, &mut seen_names)?;
        }

        // 3. 加载项目目录(优先级最高,可覆盖同名 Skill)
        if let Some(project_dir) = &self.project_dir {
            self.load_from_dir(
                project_dir,
                SkillSource::Project,
                &mut skills,
                &mut seen_names,
            )?;
        }

        log::info!("已加载 {} 个 Skill", skills.len());
        Ok(skills)
    }

    /// 从单个目录加载 Skill
    ///
    /// 遍历目录下的子目录,每个子目录应包含 SKILL.md 文件
    /// 同名 Skill 按加载顺序覆盖(后加载的覆盖先加载的)
    ///
    /// # 参数
    /// - `dir`: 待扫描的 Skill 目录
    /// - `source`: Skill 来源类型
    /// - `skills`: 已加载的 Skill 列表(可变借用,用于追加或覆盖)
    /// - `seen_names`: 已加载 Skill 名称到索引的映射(用于覆盖逻辑)
    fn load_from_dir(
        &self,
        dir: &Path,
        source: SkillSource,
        skills: &mut Vec<Skill>,
        seen_names: &mut std::collections::HashMap<String, usize>,
    ) -> Result<(), CommandError> {
        // 目录不存在或非目录时跳过(非错误)
        if !dir.exists() || !dir.is_dir() {
            return Ok(());
        }

        log::debug!("扫描 Skill 目录: {} (来源: {:?})", dir.display(), source);

        // 遍历目录下的子目录,每个子目录应包含 SKILL.md
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            // 仅处理子目录(每个 Skill 独立一个子目录)
            if !path.is_dir() {
                continue;
            }

            // 子目录下的 SKILL.md 文件
            let skill_md_path = path.join("SKILL.md");
            if !skill_md_path.exists() {
                continue;
            }

            // 解析 SKILL.md 文件,失败时记录警告并跳过(不影响其他 Skill 加载)
            match self.parse_skill_file(&skill_md_path, &path, source.clone()) {
                Ok(skill) => {
                    let name = skill.frontmatter.name.clone();
                    if let Some(idx) = seen_names.get(&name) {
                        // 同名 Skill 覆盖(后加载的优先级更高)
                        log::info!("Skill '{}' 被 {:?} 来源覆盖", name, source);
                        skills[*idx] = skill;
                    } else {
                        // 新 Skill,追加到列表
                        seen_names.insert(name, skills.len());
                        skills.push(skill);
                    }
                }
                Err(e) => {
                    // 解析失败不影响其他 Skill,仅记录警告
                    log::warn!("解析 Skill 文件失败: {} - {}", skill_md_path.display(), e);
                }
            }
        }

        Ok(())
    }

    /// 解析单个 SKILL.md 文件
    ///
    /// 读取文件内容,使用 yaml-front-matter crate 解析 YAML frontmatter 和 markdown 正文,
    /// 并获取文件最后修改时间。
    ///
    /// # 参数
    /// - `file_path`: SKILL.md 文件路径
    /// - `dir_path`: Skill 所在目录路径(SKILL.md 的父目录)
    /// - `source`: Skill 来源类型
    ///
    /// # 错误
    /// - 文件读取失败:返回文件系统错误(通过 From<io::Error> 自动转换)
    /// - frontmatter 解析失败:返回 CONFIG_INVALID_FORMAT 错误
    fn parse_skill_file(
        &self,
        file_path: &Path,
        dir_path: &Path,
        source: SkillSource,
    ) -> Result<Skill, CommandError> {
        // 读取 SKILL.md 文件全部内容
        let content = std::fs::read_to_string(file_path)?;

        // 解析 YAML frontmatter(使用 yaml_front_matter crate)
        // 返回 Document<SkillFrontmatter>,包含 metadata(frontmatter) 和 content(markdown 正文)
        let document: Document<SkillFrontmatter> =
            YamlFrontMatter::parse(&content).map_err(|e| {
                CommandError::config(
                    CONFIG_INVALID_FORMAT,
                    format!("Skill frontmatter 解析失败: {}", e),
                )
            })?;

        let frontmatter = document.metadata;
        let markdown_content = document.content;

        // 获取文件修改时间(UNIX 时间戳,秒)
        let metadata = std::fs::metadata(file_path)?;
        let modified_at = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(Skill {
            frontmatter,
            content: markdown_content,
            source,
            file_path: file_path.to_path_buf(),
            dir_path: dir_path.to_path_buf(),
            modified_at,
        })
    }
}
