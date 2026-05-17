use async_trait::async_trait;
use serde_json::{json, Value};

use crate::models::skill::SkillResult;
use super::registry::Skill;

/// 注册所有内置技能
pub fn register_builtin_skills(registry: &mut super::registry::SkillRegistry) {
    registry.register(Box::new(GenerateDocumentSkill));
    registry.register(Box::new(ReadDocumentSkill));
    registry.register(Box::new(ModifyDocumentSkill));
    registry.register(Box::new(DeleteDocumentSkill));
    registry.register(Box::new(ConvertFormatSkill));
    registry.register(Box::new(SearchDocumentsSkill));
    registry.register(Box::new(AnalyzeDocumentSkill));
    registry.register(Box::new(ListWorkspaceSkill));
    registry.register(Box::new(BatchProcessSkill));
}

/// 生成文档技能
struct GenerateDocumentSkill;

#[async_trait]
impl Skill for GenerateDocumentSkill {
    fn skill_name(&self) -> &str { "generate_document" }
    fn description(&self) -> &str { "生成新的文档，支持 Word、Excel、PPT、PDF、Markdown 格式" }
    fn category(&self) -> &str { "document" }
    fn supported_types(&self) -> Vec<String> {
        vec!["docx".into(), "xlsx".into(), "pptx".into(), "pdf".into(), "md".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "enum": ["docx", "xlsx", "pptx", "pdf", "md"],
                    "description": "文档格式"
                },
                "path": {
                    "type": "string",
                    "description": "输出文件路径（相对于工作区）"
                },
                "title": {
                    "type": "string",
                    "description": "文档标题"
                },
                "content": {
                    "type": "string",
                    "description": "文档内容（纯文本或结构化 JSON）"
                },
                "template": {
                    "type": "string",
                    "description": "模板文件路径（可选）"
                }
            },
            "required": ["format", "path", "content"]
        })
    }
    async fn execute(&self, _params: Value) -> SkillResult {
        SkillResult {
            success: true,
            output: Some(json!({"placeholder": true, "message": "文档生成功能待接入 Sidecar"})),
            error: None,
            duration_ms: 0,
        }
    }
}

/// 读取文档技能
struct ReadDocumentSkill;

#[async_trait]
impl Skill for ReadDocumentSkill {
    fn skill_name(&self) -> &str { "read_document" }
    fn description(&self) -> &str { "读取文档内容，支持提取文本、表格、属性等信息" }
    fn category(&self) -> &str { "document" }
    fn supported_types(&self) -> Vec<String> {
        vec!["docx".into(), "xlsx".into(), "pptx".into(), "pdf".into(), "md".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "include_formatting": {
                    "type": "boolean",
                    "description": "是否包含格式信息",
                    "default": false
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, _params: Value) -> SkillResult {
        SkillResult {
            success: true,
            output: Some(json!({"placeholder": true, "message": "文档读取功能待接入 Sidecar"})),
            error: None,
            duration_ms: 0,
        }
    }
}

/// 修改文档技能
struct ModifyDocumentSkill;

#[async_trait]
impl Skill for ModifyDocumentSkill {
    fn skill_name(&self) -> &str { "modify_document" }
    fn description(&self) -> &str { "修改已有文档，支持文本替换、添加段落、添加表格等操作" }
    fn category(&self) -> &str { "document" }
    fn supported_types(&self) -> Vec<String> {
        vec!["docx".into(), "xlsx".into(), "pptx".into(), "md".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "operations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": {
                                "type": "string",
                                "enum": ["replace", "add_paragraph", "add_heading", "add_table", "set_cell", "append", "prepend"],
                                "description": "操作类型"
                            }
                        }
                    },
                    "description": "修改操作列表"
                }
            },
            "required": ["path", "operations"]
        })
    }
    async fn execute(&self, _params: Value) -> SkillResult {
        SkillResult {
            success: true,
            output: Some(json!({"placeholder": true, "message": "文档修改功能待接入 Sidecar"})),
            error: None,
            duration_ms: 0,
        }
    }
}

/// 删除文档技能
struct DeleteDocumentSkill;

#[async_trait]
impl Skill for DeleteDocumentSkill {
    fn skill_name(&self) -> &str { "delete_document" }
    fn description(&self) -> &str { "删除指定文档文件" }
    fn category(&self) -> &str { "document" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "create_snapshot": {
                    "type": "boolean",
                    "description": "删除前是否创建快照",
                    "default": true
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, _params: Value) -> SkillResult {
        SkillResult {
            success: true,
            output: Some(json!({"placeholder": true, "message": "文档删除功能待接入 Sidecar"})),
            error: None,
            duration_ms: 0,
        }
    }
}

/// 格式转换技能
struct ConvertFormatSkill;

#[async_trait]
impl Skill for ConvertFormatSkill {
    fn skill_name(&self) -> &str { "convert_format" }
    fn description(&self) -> &str { "文档格式转换，如 Word 转 PDF、Markdown 转 Word 等" }
    fn category(&self) -> &str { "document" }
    fn supported_types(&self) -> Vec<String> {
        vec!["docx".into(), "xlsx".into(), "pptx".into(), "pdf".into(), "md".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source_path": {
                    "type": "string",
                    "description": "源文件路径"
                },
                "target_format": {
                    "type": "string",
                    "enum": ["docx", "xlsx", "pptx", "pdf", "md", "txt"],
                    "description": "目标格式"
                },
                "output_path": {
                    "type": "string",
                    "description": "输出文件路径（可选，默认自动生成）"
                }
            },
            "required": ["source_path", "target_format"]
        })
    }
    async fn execute(&self, _params: Value) -> SkillResult {
        SkillResult {
            success: true,
            output: Some(json!({"placeholder": true, "message": "格式转换功能待接入 Sidecar"})),
            error: None,
            duration_ms: 0,
        }
    }
}

/// 搜索文档技能
struct SearchDocumentsSkill;

#[async_trait]
impl Skill for SearchDocumentsSkill {
    fn skill_name(&self) -> &str { "search_documents" }
    fn description(&self) -> &str { "在工作区中搜索文档，支持按文件名或内容搜索" }
    fn category(&self) -> &str { "workspace" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "搜索关键词"
                },
                "extensions": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "限定文件扩展名"
                },
                "include_content": {
                    "type": "boolean",
                    "description": "是否搜索文件内容",
                    "default": false
                },
                "max_results": {
                    "type": "integer",
                    "description": "最大结果数",
                    "default": 50
                }
            },
            "required": ["query"]
        })
    }
    async fn execute(&self, _params: Value) -> SkillResult {
        SkillResult {
            success: true,
            output: Some(json!({"placeholder": true, "message": "文档搜索功能待接入 Sidecar"})),
            error: None,
            duration_ms: 0,
        }
    }
}

/// 分析文档技能
struct AnalyzeDocumentSkill;

#[async_trait]
impl Skill for AnalyzeDocumentSkill {
    fn skill_name(&self) -> &str { "analyze_document" }
    fn description(&self) -> &str { "分析文档结构和统计信息，如字数、段落数、标题层级等" }
    fn category(&self) -> &str { "document" }
    fn supported_types(&self) -> Vec<String> {
        vec!["docx".into(), "xlsx".into(), "pptx".into(), "pdf".into(), "md".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, _params: Value) -> SkillResult {
        SkillResult {
            success: true,
            output: Some(json!({"placeholder": true, "message": "文档分析功能待接入 Sidecar"})),
            error: None,
            duration_ms: 0,
        }
    }
}

/// 列出工作区文件技能
struct ListWorkspaceSkill;

#[async_trait]
impl Skill for ListWorkspaceSkill {
    fn skill_name(&self) -> &str { "list_workspace" }
    fn description(&self) -> &str { "列出工作区中的文件和目录结构" }
    fn category(&self) -> &str { "workspace" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "目录路径（相对于工作区根目录，默认为根目录）"
                },
                "depth": {
                    "type": "integer",
                    "description": "遍历深度，默认1",
                    "default": 1
                },
                "extensions": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "筛选文件扩展名"
                }
            }
        })
    }
    async fn execute(&self, _params: Value) -> SkillResult {
        SkillResult {
            success: true,
            output: Some(json!({"placeholder": true, "message": "工作区列表功能待接入 Sidecar"})),
            error: None,
            duration_ms: 0,
        }
    }
}

/// 批量处理技能
struct BatchProcessSkill;

#[async_trait]
impl Skill for BatchProcessSkill {
    fn skill_name(&self) -> &str { "batch_process" }
    fn description(&self) -> &str { "批量处理多个文档，支持批量转换、修改、分析等操作" }
    fn category(&self) -> &str { "document" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["convert", "modify", "analyze"],
                    "description": "批量操作类型"
                },
                "paths": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "文件路径列表"
                },
                "params": {
                    "type": "object",
                    "description": "操作参数"
                }
            },
            "required": ["operation", "paths"]
        })
    }
    async fn execute(&self, _params: Value) -> SkillResult {
        SkillResult {
            success: true,
            output: Some(json!({"placeholder": true, "message": "批量处理功能待接入 Sidecar"})),
            error: None,
            duration_ms: 0,
        }
    }
}
