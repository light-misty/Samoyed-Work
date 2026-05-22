use serde::{Deserialize, Serialize};

/// Prompt 模板
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    /// 模板内容，支持 {{变量名}} 占位符
    pub content: String,
    /// 分类: "document" | "analysis" | "conversion" | "custom"
    pub category: String,
    /// 是否为内置模板（内置模板不可删除）
    pub is_builtin: bool,
    /// 变量定义列表，JSON 序列化存储
    pub variables: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

/// 模板变量定义
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TemplateVariable {
    /// 变量名，对应 content 中的 {{name}}
    pub name: String,
    /// 变量类型: "string" | "number" | "boolean" | "select"
    #[serde(rename = "type")]
    pub var_type: String,
    /// 显示标签
    pub label: String,
    /// 默认值
    pub default_value: Option<serde_json::Value>,
    /// select 类型的可选项
    pub options: Option<Vec<String>>,
}

/// 创建模板参数
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateTemplateParams {
    pub name: String,
    pub description: String,
    pub content: String,
    pub category: String,
    pub variables: Option<serde_json::Value>,
}

/// 更新模板参数
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTemplateParams {
    pub name: Option<String>,
    pub description: Option<String>,
    pub content: Option<String>,
    pub category: Option<String>,
    pub variables: Option<serde_json::Value>,
}
