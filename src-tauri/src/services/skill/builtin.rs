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

/// 执行 read 操作的通用逻辑
async fn execute_read(
    doc_service: &DocumentService,
    doc_type: &str,
    params: Value,
) -> SkillResult {
    let start = Instant::now();
    let file_path = params["path"].as_str().unwrap_or("");
    let workspace_root = params["workspace_root"].as_str().unwrap_or("");
    let resolved_path = resolve_path(file_path, workspace_root);

    let mut sidecar_params = json!({
        "path": resolved_path,
    });

    // read 操作的通用参数
    if !params["include_formatting"].is_null() {
        sidecar_params["include_formatting"] = json!(params["include_formatting"].as_bool().unwrap_or(false));
    }

    // Excel read 专用参数
    if let Some(sheet) = params["sheet"].as_str() {
        sidecar_params["sheet"] = json!(sheet);
    }
    if let Some(range) = params["range"].as_str() {
        sidecar_params["range"] = json!(range);
    }

    match doc_service.process("read", doc_type, sidecar_params).await {
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

/// 执行 convert 操作的通用逻辑
async fn execute_convert(
    doc_service: &DocumentService,
    doc_type: &str,
    params: Value,
) -> SkillResult {
    let start = Instant::now();
    let file_path = params["path"].as_str().unwrap_or("");
    let target_format = params["target_format"].as_str().unwrap_or("pdf");
    let output_path = params["output_path"].as_str().unwrap_or("");
    let workspace_root = params["workspace_root"].as_str().unwrap_or("");

    let resolved_source = resolve_path(file_path, workspace_root);

    let output_path = if output_path.is_empty() {
        let stem = std::path::Path::new(&resolved_source)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        format!("{}.{}", stem, target_format)
    } else {
        resolve_path(output_path, workspace_root)
    };

    let mut sidecar_params = json!({
        "path": resolved_source,
        "output_path": output_path,
        "format": target_format,
    });

    // Excel convert 专用参数
    if let Some(sheet) = params["sheet"].as_str() {
        sidecar_params["sheet"] = json!(sheet);
    }

    match doc_service.process("convert", doc_type, sidecar_params).await {
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

/// 执行 analyze 操作的通用逻辑
async fn execute_analyze(
    doc_service: &DocumentService,
    doc_type: &str,
    params: Value,
) -> SkillResult {
    let start = Instant::now();
    let file_path = params["path"].as_str().unwrap_or("");
    let workspace_root = params["workspace_root"].as_str().unwrap_or("");
    let resolved_path = resolve_path(file_path, workspace_root);

    let sidecar_params = json!({
        "path": resolved_path,
    });

    match doc_service.process("analyze", doc_type, sidecar_params).await {
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

/// 注册所有内置技能
pub fn register_builtin_skills(
    registry: &mut super::registry::SkillRegistry,
    doc_service: Arc<DocumentService>,
) {
    log::info!("开始注册内置技能");
    registry.register(Box::new(DocxSkill::new(doc_service.clone())));
    registry.register(Box::new(XlsxSkill::new(doc_service.clone())));
    registry.register(Box::new(PptxSkill::new(doc_service.clone())));
    registry.register(Box::new(PdfSkill::new(doc_service.clone())));
    registry.register(Box::new(CodeInterpreterSkill::new(doc_service)));
    log::info!("内置技能注册完成, 共注册 5 个技能");
}

// ============================================================================
// DocxSkill - Word 文档技能
// ============================================================================

/// Word 文档技能
/// 聚合 read/convert/analyze 三种操作
struct DocxSkill {
    doc_service: Arc<DocumentService>,
}

impl DocxSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for DocxSkill {
    fn skill_name(&self) -> &str { "docx_skill" }
    fn description(&self) -> &str {
        "Word文档(.docx)处理技能，支持读取、格式转换、分析三种操作。转换支持docx/pdf/md/txt/html等格式。"
    }
    fn category(&self) -> &str { "document" }
    fn is_builtin(&self) -> bool { true }
    fn supported_types(&self) -> Vec<String> {
        vec!["docx".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["read", "convert", "analyze"],
                    "description": "操作类型: read=读取文档, convert=格式转换, analyze=分析文档"
                },
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "include_formatting": {
                    "type": "boolean",
                    "description": "[read] 是否包含格式信息，默认 false",
                    "default": false
                },
                "target_format": {
                    "type": "string",
                    "enum": ["docx", "xlsx", "pptx", "pdf", "md", "txt", "csv", "html"],
                    "description": "[convert] 目标格式"
                },
                "output_path": {
                    "type": "string",
                    "description": "[convert] 输出文件路径（可选，默认自动生成）"
                }
            },
            "required": ["action", "path"]
        })
    }
    async fn execute(&self, params: Value) -> SkillResult {
        let action = params["action"].as_str().unwrap_or("");
        match action {
            "read" => execute_read(&self.doc_service, "docx", params).await,
            "convert" => execute_convert(&self.doc_service, "docx", params).await,
            "analyze" => execute_analyze(&self.doc_service, "docx", params).await,
            _ => SkillResult {
                success: false,
                output: None,
                error: Some(format!("DocxSkill 不支持的操作类型: {}", action)),
                duration_ms: 0,
            },
        }
    }
}

// ============================================================================
// XlsxSkill - Excel 文档技能
// ============================================================================

/// Excel 文档技能
/// 聚合 read/convert/analyze 三种操作
struct XlsxSkill {
    doc_service: Arc<DocumentService>,
}

impl XlsxSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for XlsxSkill {
    fn skill_name(&self) -> &str { "xlsx_skill" }
    fn description(&self) -> &str {
        "Excel文档(.xlsx)处理技能，支持读取、格式转换、分析三种操作。转换支持xlsx/pdf/csv/html等格式。"
    }
    fn category(&self) -> &str { "document" }
    fn is_builtin(&self) -> bool { true }
    fn supported_types(&self) -> Vec<String> {
        vec!["xlsx".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["read", "convert", "analyze"],
                    "description": "操作类型: read=读取文档, convert=格式转换, analyze=分析文档"
                },
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "sheet": {
                    "type": "string",
                    "description": "[read/convert] 工作表名称"
                },
                "range": {
                    "type": "string",
                    "description": "[read] 单元格范围，如 A1:D10"
                },
                "include_formatting": {
                    "type": "boolean",
                    "description": "[read] 是否包含格式信息，默认 false",
                    "default": false
                },
                "target_format": {
                    "type": "string",
                    "enum": ["docx", "xlsx", "pptx", "pdf", "md", "txt", "csv", "html"],
                    "description": "[convert] 目标格式"
                },
                "output_path": {
                    "type": "string",
                    "description": "[convert] 输出文件路径（可选，默认自动生成）"
                }
            },
            "required": ["action", "path"]
        })
    }
    async fn execute(&self, params: Value) -> SkillResult {
        let action = params["action"].as_str().unwrap_or("");
        match action {
            "read" => execute_read(&self.doc_service, "xlsx", params).await,
            "convert" => execute_convert(&self.doc_service, "xlsx", params).await,
            "analyze" => execute_analyze(&self.doc_service, "xlsx", params).await,
            _ => SkillResult {
                success: false,
                output: None,
                error: Some(format!("XlsxSkill 不支持的操作类型: {}", action)),
                duration_ms: 0,
            },
        }
    }
}

// ============================================================================
// PptxSkill - PPT 文档技能
// ============================================================================

/// PPT 文档技能
/// 聚合 read/convert/analyze 三种操作
struct PptxSkill {
    doc_service: Arc<DocumentService>,
}

impl PptxSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for PptxSkill {
    fn skill_name(&self) -> &str { "pptx_skill" }
    fn description(&self) -> &str {
        "PPT演示文稿(.pptx)处理技能，支持读取、格式转换、分析三种操作。转换支持pptx/pdf等格式。"
    }
    fn category(&self) -> &str { "document" }
    fn is_builtin(&self) -> bool { true }
    fn supported_types(&self) -> Vec<String> {
        vec!["pptx".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["read", "convert", "analyze"],
                    "description": "操作类型: read=读取文档, convert=格式转换, analyze=分析文档"
                },
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "target_format": {
                    "type": "string",
                    "enum": ["docx", "xlsx", "pptx", "pdf", "md", "txt", "csv", "html"],
                    "description": "[convert] 目标格式"
                },
                "output_path": {
                    "type": "string",
                    "description": "[convert] 输出文件路径（可选，默认自动生成）"
                }
            },
            "required": ["action", "path"]
        })
    }
    async fn execute(&self, params: Value) -> SkillResult {
        let action = params["action"].as_str().unwrap_or("");
        match action {
            "read" => execute_read(&self.doc_service, "pptx", params).await,
            "convert" => execute_convert(&self.doc_service, "pptx", params).await,
            "analyze" => execute_analyze(&self.doc_service, "pptx", params).await,
            _ => SkillResult {
                success: false,
                output: None,
                error: Some(format!("PptxSkill 不支持的操作类型: {}", action)),
                duration_ms: 0,
            },
        }
    }
}

// ============================================================================
// PdfSkill - PDF 文档技能
// ============================================================================

/// PDF 文档技能
/// 聚合 read/convert/analyze 三种操作
struct PdfSkill {
    doc_service: Arc<DocumentService>,
}

impl PdfSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for PdfSkill {
    fn skill_name(&self) -> &str { "pdf_skill" }
    fn description(&self) -> &str {
        "PDF文档(.pdf)处理技能，支持读取、格式转换、分析三种操作。转换支持pdf/txt/md/html等格式。"
    }
    fn category(&self) -> &str { "document" }
    fn is_builtin(&self) -> bool { true }
    fn supported_types(&self) -> Vec<String> {
        vec!["pdf".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["read", "convert", "analyze"],
                    "description": "操作类型: read=读取文档, convert=格式转换, analyze=分析文档"
                },
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "target_format": {
                    "type": "string",
                    "enum": ["txt", "md", "html"],
                    "description": "[convert] 目标格式"
                },
                "output_path": {
                    "type": "string",
                    "description": "[convert] 输出文件路径（可选，默认自动生成）"
                }
            },
            "required": ["action", "path"]
        })
    }
    async fn execute(&self, params: Value) -> SkillResult {
        let action = params["action"].as_str().unwrap_or("");
        match action {
            "read" => execute_read(&self.doc_service, "pdf", params).await,
            "convert" => execute_convert(&self.doc_service, "pdf", params).await,
            "analyze" => execute_analyze(&self.doc_service, "pdf", params).await,
            _ => SkillResult {
                success: false,
                output: None,
                error: Some(format!("PdfSkill 不支持的操作类型: {}", action)),
                duration_ms: 0,
            },
        }
    }
}

// ============================================================================
// CodeInterpreterSkill - 代码解释器技能
// ============================================================================

/// 代码解释器技能
/// 让 Agent 自由编写 Python 代码生成/修改文档
/// 承担原有 generate 和 modify 操作的全部职责
struct CodeInterpreterSkill {
    doc_service: Arc<DocumentService>,
}

impl CodeInterpreterSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for CodeInterpreterSkill {
    fn skill_name(&self) -> &str { "code_interpreter_skill" }
    fn description(&self) -> &str {
        "代码解释器，通过编写和执行 Python 代码生成和修改文档。所有文档生成和修改操作都通过此技能完成。可用库: python-docx, openpyxl, python-pptx, reportlab, matplotlib, pandas, numpy, Pillow。可用 helper: create_word_doc(), save_word_doc() 等。"
    }
    fn category(&self) -> &str { "document" }
    fn is_builtin(&self) -> bool { true }
    fn supported_types(&self) -> Vec<String> {
        vec!["docx".into(), "xlsx".into(), "pptx".into(), "pdf".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "description": "要执行的 Python 代码。可用库: python-docx, openpyxl, python-pptx, reportlab, matplotlib, pandas, numpy, Pillow。可用 helper: create_word_doc(), save_word_doc(), create_excel_doc(), save_excel_doc(), create_ppt_doc(), save_ppt_doc(), create_pdf_doc(), save_pdf_doc(), create_chart(), save_chart()。工作目录变量: working_dir"
                },
                "description": {
                    "type": "string",
                    "description": "代码功能的简要描述，用于用户确认时展示"
                },
                "timeout": {
                    "type": "integer",
                    "description": "执行超时时间（秒），默认 60，最大 120",
                    "default": 60
                },
                "expected_files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "预期生成的文件名列表（如 [\"报告.docx\", \"chart.png\"]）"
                }
            },
            "required": ["code", "description"]
        })
    }
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();
        let code = params["code"].as_str().unwrap_or("");
        let description = params["description"].as_str().unwrap_or("");
        let timeout = params["timeout"].as_u64().unwrap_or(60).min(120);
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        if code.is_empty() {
            return SkillResult {
                success: false,
                output: None,
                error: Some("缺少代码内容".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }

        // 调用 Sidecar：action="execute", type="code"
        // Sidecar handle_request() 通过 getattr(handler, action) 路由
        // CodeHandler 实现了 execute() 方法
        let sidecar_params = json!({
            "code": code,
            "working_dir": workspace_root,
            "timeout": timeout,
        });

        match self.doc_service.process("execute", "code", sidecar_params).await {
            Ok(data) => {
                let mut output = data;
                output["description"] = json!(description);
                SkillResult {
                    success: true,
                    output: Some(output),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                }
            }
            Err(e) => SkillResult {
                success: false,
                output: None,
                error: Some(e.message),
                duration_ms: start.elapsed().as_millis() as u64,
            },
        }
    }
}
