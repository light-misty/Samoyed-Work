use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::models::skill::SkillResult;
use crate::services::document::DocumentService;
use super::registry::Skill;

/// 将相对路径解析为绝对路径
fn resolve_path(path: &str, workspace_root: &str) -> String {
    if path.is_empty() {
        return path.to_string();
    }
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        return path.to_string();
    }
    let root = std::path::Path::new(workspace_root);
    root.join(path).to_string_lossy().to_string()
}

/// 注册所有内置技能
pub fn register_builtin_skills(
    registry: &mut super::registry::SkillRegistry,
    doc_service: Arc<DocumentService>,
) {
    log::info!("开始注册内置技能");
    registry.register_builtin(Box::new(GenerateDocumentSkill::new(doc_service.clone())));
    registry.register_builtin(Box::new(ReadDocumentSkill::new(doc_service.clone())));
    registry.register_builtin(Box::new(ModifyDocumentSkill::new(doc_service.clone())));
    registry.register_builtin(Box::new(ConvertFormatSkill::new(doc_service.clone())));
    registry.register_builtin(Box::new(AnalyzeDocumentSkill::new(doc_service.clone())));
    registry.register_builtin(Box::new(BatchProcessSkill::new(doc_service)));
    log::info!("内置技能注册完成, 共注册 6 个技能");
}

/// 生成文档技能
struct GenerateDocumentSkill {
    doc_service: Arc<DocumentService>,
}

impl GenerateDocumentSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for GenerateDocumentSkill {
    fn skill_name(&self) -> &str { "generate_document" }
    fn description(&self) -> &str { "生成新的文档，支持 Word、Excel、PPT、PDF、Markdown 格式" }
    fn category(&self) -> &str { "document" }
    fn is_builtin(&self) -> bool { true }
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
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();
        let doc_type = params["format"].as_str().unwrap_or("docx");
        let output_path = params["path"].as_str().unwrap_or("");
        let title = params["title"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        let resolved_path = resolve_path(output_path, workspace_root);

        let content = match params["content"].as_str() {
            Some(s) => s.to_string(),
            None => {
                if !params["content"].is_null() {
                    serde_json::to_string(&params["content"]).unwrap_or_default()
                } else {
                    String::new()
                }
            }
        };

        let mut sidecar_params = json!({
            "path": resolved_path,
            "title": title,
            "content": content,
        });

        if let Some(template) = params["template"].as_str() {
            if !template.is_empty() {
                sidecar_params["template"] = json!(template);
            }
        }

        match self.doc_service.process("generate", doc_type, sidecar_params).await {
            Ok(data) => SkillResult {
                success: true,
                output: Some(data),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
            },
            Err(e) => SkillResult {
                success: false,
                output: None,
                error: Some(e.message),
                duration_ms: start.elapsed().as_millis() as u64,
            },
        }
    }
}

/// 读取文档技能
struct ReadDocumentSkill {
    doc_service: Arc<DocumentService>,
}

impl ReadDocumentSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for ReadDocumentSkill {
    fn skill_name(&self) -> &str { "read_document" }
    fn description(&self) -> &str { "读取结构化文档内容（Word/Excel/PPT/PDF），提取文本、表格、属性" }
    fn category(&self) -> &str { "document" }
    fn is_builtin(&self) -> bool { true }
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
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let resolved_path = resolve_path(file_path, workspace_root);
        let extension = std::path::Path::new(&resolved_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("docx");
        let doc_type = match extension {
            "docx" => "docx",
            "xlsx" => "xlsx",
            "pptx" => "pptx",
            "pdf" => "pdf",
            "md" | "markdown" => "md",
            _ => "docx",
        };

        let sidecar_params = json!({
            "path": resolved_path,
            "include_formatting": params["include_formatting"].as_bool().unwrap_or(false),
        });

        match self.doc_service.process("read", doc_type, sidecar_params).await {
            Ok(data) => SkillResult {
                success: true,
                output: Some(data),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
            },
            Err(e) => SkillResult {
                success: false,
                output: None,
                error: Some(e.message),
                duration_ms: start.elapsed().as_millis() as u64,
            },
        }
    }
}

/// 修改文档技能
struct ModifyDocumentSkill {
    doc_service: Arc<DocumentService>,
}

impl ModifyDocumentSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for ModifyDocumentSkill {
    fn skill_name(&self) -> &str { "modify_document" }
    fn description(&self) -> &str { "修改已有文档，支持文本替换、添加段落、添加表格等操作" }
    fn category(&self) -> &str { "document" }
    fn is_builtin(&self) -> bool { true }
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
                            },
                            "index": {
                                "type": "integer",
                                "description": "段落索引（从0开始），用于 replace 操作按索引替换整段内容"
                            },
                            "text": {
                                "type": "string",
                                "description": "新文本内容，用于 replace（按索引替换）或 add_paragraph/add_heading 操作"
                            },
                            "old": {
                                "type": "string",
                                "description": "要替换的旧文本，用于 replace 操作的全文搜索替换模式"
                            },
                            "new": {
                                "type": "string",
                                "description": "替换后的新文本，用于 replace 操作的全文搜索替换模式"
                            },
                            "level": {
                                "type": "integer",
                                "description": "标题级别（1-6），用于 add_heading 操作"
                            }
                        },
                        "required": ["type"]
                    },
                    "description": "修改操作列表。replace 操作支持两种模式：1) 按索引替换整段：提供 index 和 text；2) 全文搜索替换：提供 old 和 new"
                }
            },
            "required": ["path", "operations"]
        })
    }
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let resolved_path = resolve_path(file_path, workspace_root);
        let extension = std::path::Path::new(&resolved_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("docx");
        let doc_type = match extension {
            "docx" => "docx",
            "xlsx" => "xlsx",
            "pptx" => "pptx",
            "md" | "markdown" => "md",
            _ => "docx",
        };

        let sidecar_params = json!({
            "path": resolved_path,
            "operations": params["operations"],
        });

        match self.doc_service.process("modify", doc_type, sidecar_params).await {
            Ok(data) => SkillResult {
                success: true,
                output: Some(data),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
            },
            Err(e) => SkillResult {
                success: false,
                output: None,
                error: Some(e.message),
                duration_ms: start.elapsed().as_millis() as u64,
            },
        }
    }
}

/// 格式转换技能
struct ConvertFormatSkill {
    doc_service: Arc<DocumentService>,
}

impl ConvertFormatSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for ConvertFormatSkill {
    fn skill_name(&self) -> &str { "convert_format" }
    fn description(&self) -> &str { "文档格式转换，如 Word 转 PDF、Markdown 转 Word 等" }
    fn category(&self) -> &str { "document" }
    fn is_builtin(&self) -> bool { true }
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
                    "enum": ["docx", "xlsx", "pptx", "pdf", "md", "txt", "csv", "html"],
                    "description": "目标格式（docx/xlsx/pptx/pdf/md/txt/csv/html）"
                },
                "output_path": {
                    "type": "string",
                    "description": "输出文件路径（可选，默认自动生成）"
                }
            },
            "required": ["source_path", "target_format"]
        })
    }
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();
        let source_path = params["source_path"].as_str().unwrap_or("");
        let target_format = params["target_format"].as_str().unwrap_or("pdf");
        let output_path = params["output_path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        let resolved_source = resolve_path(source_path, workspace_root);

        let output_path = if output_path.is_empty() {
            let stem = std::path::Path::new(&resolved_source)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            format!("{}.{}", stem, target_format)
        } else {
            resolve_path(output_path, workspace_root)
        };

        let source_extension = std::path::Path::new(&resolved_source)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("docx");
        let source_doc_type = match source_extension {
            "docx" => "docx",
            "xlsx" => "xlsx",
            "pptx" => "pptx",
            "pdf" => "pdf",
            "md" | "markdown" => "md",
            _ => "docx",
        };

        let sidecar_params = json!({
            "path": resolved_source,
            "output_path": output_path,
            "format": target_format,
        });

        match self.doc_service.process("convert", source_doc_type, sidecar_params).await {
            Ok(data) => SkillResult {
                success: true,
                output: Some(data),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
            },
            Err(e) => SkillResult {
                success: false,
                output: None,
                error: Some(e.message),
                duration_ms: start.elapsed().as_millis() as u64,
            },
        }
    }
}

/// 分析文档技能
struct AnalyzeDocumentSkill {
    doc_service: Arc<DocumentService>,
}

impl AnalyzeDocumentSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for AnalyzeDocumentSkill {
    fn skill_name(&self) -> &str { "analyze_document" }
    fn description(&self) -> &str { "分析文档结构和统计信息，如字数、段落数、标题层级等" }
    fn category(&self) -> &str { "document" }
    fn is_builtin(&self) -> bool { true }
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
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let resolved_path = resolve_path(file_path, workspace_root);
        let extension = std::path::Path::new(&resolved_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("docx");
        let doc_type = match extension {
            "docx" => "docx",
            "xlsx" => "xlsx",
            "pptx" => "pptx",
            "pdf" => "pdf",
            "md" | "markdown" => "md",
            _ => "docx",
        };

        let sidecar_params = json!({
            "path": resolved_path,
        });

        match self.doc_service.process("analyze", doc_type, sidecar_params).await {
            Ok(data) => SkillResult {
                success: true,
                output: Some(data),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
            },
            Err(e) => SkillResult {
                success: false,
                output: None,
                error: Some(e.message),
                duration_ms: start.elapsed().as_millis() as u64,
            },
        }
    }
}

/// 批量处理技能
struct BatchProcessSkill {
    doc_service: Arc<DocumentService>,
}

impl BatchProcessSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for BatchProcessSkill {
    fn skill_name(&self) -> &str { "batch_process" }
    fn description(&self) -> &str { "批量处理多个文档，支持批量转换、修改、分析等操作" }
    fn category(&self) -> &str { "document" }
    fn is_builtin(&self) -> bool { true }
    fn supported_types(&self) -> Vec<String> {
        vec!["docx".into(), "xlsx".into(), "pptx".into(), "pdf".into(), "md".into()]
    }
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
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();
        let operation = params["operation"].as_str().unwrap_or("analyze");
        let paths = params["paths"].as_array().cloned().unwrap_or_default();
        let op_params = params["params"].clone();
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        let mut results = Vec::new();
        let mut all_success = true;

        for path_val in paths {
            let path_str = path_val.as_str().unwrap_or("");
            if path_str.is_empty() {
                continue;
            }

            let resolved_path = resolve_path(path_str, workspace_root);

            let extension = std::path::Path::new(&resolved_path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("docx");
            let doc_type = match extension {
                "docx" => "docx",
                "xlsx" => "xlsx",
                "pptx" => "pptx",
                "pdf" => "pdf",
                "md" | "markdown" => "md",
                _ => "docx",
            };

            let sidecar_params = match operation {
                "convert" => json!({
                    "path": resolved_path,
                    "output_path": op_params["output_path"],
                    "format": op_params.get("target_format").and_then(|v| v.as_str()).unwrap_or(extension),
                }),
                "modify" => json!({
                    "path": resolved_path,
                    "operations": op_params["operations"],
                }),
                _ => json!({
                    "path": resolved_path,
                }),
            };

            let action = match operation {
                "convert" => "convert",
                "modify" => "modify",
                _ => "analyze",
            };

            match self.doc_service.process(action, doc_type, sidecar_params).await {
                Ok(data) => results.push(json!({
                    "path": path_str,
                    "success": true,
                    "data": data,
                })),
                Err(e) => {
                    all_success = false;
                    results.push(json!({
                        "path": path_str,
                        "success": false,
                        "error": e.message,
                    }));
                }
            }
        }

        SkillResult {
            success: all_success,
            output: Some(json!({
                "operation": operation,
                "total": results.len(),
                "results": results,
            })),
            error: if all_success { None } else { Some("部分文件处理失败".to_string()) },
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }
}
