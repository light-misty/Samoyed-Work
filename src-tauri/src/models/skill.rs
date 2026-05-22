use serde::{Deserialize, Serialize};

/// Skill 信息
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    /// "document" | "data" | "format" | "custom"
    pub category: String,
    /// 是否为内置 Skill
    pub is_builtin: bool,
    /// 是否已启用
    pub enabled: bool,
    pub version: String,
    /// 参数 JSON Schema
    pub params_schema: Option<serde_json::Value>,
    /// 支持的文档类型
    pub supported_types: Vec<String>,
}

/// Skill 执行结果
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillResult {
    pub success: bool,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// Skill 展示信息
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    pub title: String,
    pub description: String,
    pub icon: Option<String>,
}

/// 自定义 Skill 配置
/// 存储为 JSON 文件，位于 {app_data_dir}/config/custom_skills/{id}.json
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CustomSkillConfig {
    /// 唯一标识，自动生成（UUID）
    pub id: String,
    /// Skill 名称（同时作为工具调用名，需符合 function naming 规范）
    pub name: String,
    /// Skill 描述（LLM 根据此描述决定是否调用）
    pub description: String,
    /// 分类: "document" | "data" | "format" | "custom"
    pub category: String,
    /// 提示词模板，支持 {{param_name}} 占位符
    /// LLM 调用此 Skill 时，参数会被替换到模板中，
    /// 渲染后的文本作为工具结果返回给 LLM，指导其后续行为
    pub prompt_template: String,
    /// 支持的文档类型
    pub supported_types: Vec<String>,
    /// 参数 JSON Schema，定义 Skill 接受的参数结构
    pub params_schema: Option<serde_json::Value>,
    /// 版本号
    pub version: String,
    /// 创建时间（ISO 8601）
    pub created_at: String,
    /// 更新时间（ISO 8601）
    pub updated_at: String,
}
