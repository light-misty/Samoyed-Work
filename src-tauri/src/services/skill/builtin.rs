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

/// 解析 operations 数组中的路径字段
/// 遍历每个操作，对其中的路径相关字段（files/image/outputPath 等）进行相对路径到绝对路径的转换
fn resolve_operation_paths(operations: &Value, workspace_root: &str) -> Value {
    let ops = match operations.as_array() {
        Some(arr) => arr,
        None => return operations.clone(),
    };

    let resolved: Vec<Value> = ops.iter().map(|op| {
        let mut resolved_op = op.clone();
        let op_type = op["type"].as_str().unwrap_or("");

        match op_type {
            "merge" => {
                // 合并操作: 解析 files 数组中的路径和 outputPath
                if let Some(files) = op["files"].as_array() {
                    let resolved_files: Vec<Value> = files.iter().map(|f| {
                        let f_str = f.as_str().unwrap_or("");
                        json!(resolve_path(f_str, workspace_root))
                    }).collect();
                    resolved_op["files"] = json!(resolved_files);
                }
                if let Some(output) = op["outputPath"].as_str() {
                    resolved_op["outputPath"] = json!(resolve_path(output, workspace_root));
                }
            }
            "split" => {
                // 拆分操作: 解析 outputDir
                if let Some(dir) = op["outputDir"].as_str() {
                    resolved_op["outputDir"] = json!(resolve_path(dir, workspace_root));
                }
            }
            "rotate" => {
                // 旋转操作: 解析 outputPath
                if let Some(output) = op["outputPath"].as_str() {
                    resolved_op["outputPath"] = json!(resolve_path(output, workspace_root));
                }
            }
            "addWatermark" => {
                // 水印操作: 解析 image 和 outputPath
                if let Some(image) = op["image"].as_str() {
                    resolved_op["image"] = json!(resolve_path(image, workspace_root));
                }
                if let Some(output) = op["outputPath"].as_str() {
                    resolved_op["outputPath"] = json!(resolve_path(output, workspace_root));
                }
            }
            "encrypt" => {
                // 加密操作: 解析 outputPath
                if let Some(output) = op["outputPath"].as_str() {
                    resolved_op["outputPath"] = json!(resolve_path(output, workspace_root));
                }
            }
            _ => {}
        }

        resolved_op
    }).collect();

    json!(resolved)
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
                "sheets": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "description": "工作表名称" },
                            "data": { "type": "array", "description": "行数据（二维数组）" },
                            "headers": { "type": "array", "description": "表头行" },
                            "cells": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "row": { "type": "integer", "description": "行号" },
                                        "col": { "type": "integer", "description": "列号" },
                                        "value": { "description": "单元格值" },
                                        "formula": { "type": "string", "description": "Excel 公式" }
                                    }
                                },
                                "description": "单元格列表（支持公式）"
                            },
                            "formulas": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "row": { "type": "integer", "description": "行号" },
                                        "col": { "type": "integer", "description": "列号" },
                                        "formula": { "type": "string", "description": "Excel 公式" }
                                    }
                                },
                                "description": "公式列表"
                            }
                        }
                    },
                    "description": "Excel 工作表列表（结构化数据，优先于 content）"
                },
                "slides": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string", "description": "幻灯片标题" },
                            "content": { "type": "string", "description": "幻灯片内容" },
                            "layout": { "type": "string", "description": "布局名称" }
                        }
                    },
                    "description": "PPT 幻灯片列表（结构化数据，优先于 content）"
                },
                "bookmarks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string", "description": "书签 ID" },
                            "text": { "type": "string", "description": "书签文本" }
                        }
                    },
                    "description": "Word 书签列表"
                },
                "hyperlinks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "text": { "type": "string", "description": "链接显示文本" },
                            "url": { "type": "string", "description": "外部链接 URL" },
                            "anchor": { "type": "string", "description": "内部书签锚点" }
                        }
                    },
                    "description": "Word 超链接列表"
                },
                "template": {
                    "type": "string",
                    "description": "模板文件路径（可选）"
                },
                "pageSize": {
                    "type": "string",
                    "enum": ["letter", "a4"],
                    "description": "Word/PDF 页面尺寸（letter=US Letter, a4=A4，默认 a4）"
                },
                "colorCoding": {
                    "type": "boolean",
                    "description": "是否启用颜色编码（Word/Excel 中蓝色=输入值、黑色=公式、绿色=跨表引用、红色=外部链接），默认 true",
                    "default": true
                },
                "includeToc": {
                    "type": "boolean",
                    "description": "Word 文档是否包含目录，默认 false",
                    "default": false
                },
                "header": {
                    "type": "string",
                    "description": "Word 文档页眉文本"
                },
                "footer": {
                    "type": "string",
                    "description": "Word 文档页脚文本"
                },
                "pageNumber": {
                    "type": "boolean",
                    "description": "Word 文档是否显示页码，默认 true",
                    "default": true
                },
                "useFormulas": {
                    "type": "boolean",
                    "description": "Excel 是否使用公式而非硬编码值，默认 true",
                    "default": true
                },
                "numberFormats": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "range": { "type": "string", "description": "单元格范围，如 B2:B10" },
                            "format": { "type": "string", "description": "格式类型: currency/percent/text/number/zero_dash/custom" }
                        }
                    },
                    "description": "Excel 数字格式列表"
                },
                "conditionalFormats": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "range": { "type": "string", "description": "单元格范围" },
                            "rule": { "type": "string", "description": "规则: greaterThan/lessThan/equal/between 等" },
                            "value": { "type": "string", "description": "规则值" },
                            "color": { "type": "string", "description": "高亮颜色（十六进制）" }
                        }
                    },
                    "description": "Excel 条件格式列表"
                },
                "colorScheme": {
                    "type": "string",
                    "enum": ["midnight", "forest", "coral", "ocean", "charcoal"],
                    "description": "PPT 颜色方案: midnight(深蓝)/forest(森林)/coral(珊瑚)/ocean(海洋)/charcoal(炭灰)"
                },
                "fonts": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "description": "标题字体" },
                        "body": { "type": "string", "description": "正文字体" }
                    },
                    "description": "PPT 字体配置"
                },
                "margins": {
                    "type": "object",
                    "properties": {
                        "top": { "type": "number", "description": "上边距（inch）" },
                        "right": { "type": "number", "description": "右边距（inch）" },
                        "bottom": { "type": "number", "description": "下边距（inch）" },
                        "left": { "type": "number", "description": "左边距（inch）" }
                    },
                    "description": "PPT 边距配置（单位: inch，默认 0.5）"
                },
                "subscripts": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "text": { "type": "string", "description": "下标文本" },
                            "position": { "type": "integer", "description": "插入位置" }
                        }
                    },
                    "description": "PDF 下标列表，使用 <sub> XML 标签"
                },
                "superscripts": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "text": { "type": "string", "description": "上标文本" },
                            "position": { "type": "integer", "description": "插入位置" }
                        }
                    },
                    "description": "PDF 上标列表，使用 <super> XML 标签"
                },
                "validate": {
                    "type": "boolean",
                    "description": "生成后是否执行文档质量验证，默认 false",
                    "default": false
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

        // 传递模板参数
        if let Some(template) = params["template"].as_str() {
            if !template.is_empty() {
                sidecar_params["template"] = json!(template);
            }
        }

        // Word 专用参数
        if let Some(page_size) = params["pageSize"].as_str() {
            sidecar_params["pageSize"] = json!(page_size);
        }
        if let Some(header) = params["header"].as_str() {
            sidecar_params["header"] = json!(header);
        }
        if let Some(footer) = params["footer"].as_str() {
            sidecar_params["footer"] = json!(footer);
        }
        if !params["pageNumber"].is_null() {
            sidecar_params["pageNumber"] = json!(params["pageNumber"].as_bool().unwrap_or(true));
        }
        if !params["includeToc"].is_null() {
            sidecar_params["includeToc"] = json!(params["includeToc"].as_bool().unwrap_or(false));
        }
        if !params["colorCoding"].is_null() {
            sidecar_params["colorCoding"] = json!(params["colorCoding"].as_bool().unwrap_or(true));
        }
        // Word 书签和超链接
        if !params["bookmarks"].is_null() {
            sidecar_params["bookmarks"] = params["bookmarks"].clone();
        }
        if !params["hyperlinks"].is_null() {
            sidecar_params["hyperlinks"] = params["hyperlinks"].clone();
        }

        // Excel 专用参数
        if !params["sheets"].is_null() {
            sidecar_params["sheets"] = params["sheets"].clone();
        }
        if !params["useFormulas"].is_null() {
            sidecar_params["useFormulas"] = json!(params["useFormulas"].as_bool().unwrap_or(true));
        }
        if !params["numberFormats"].is_null() {
            sidecar_params["numberFormats"] = params["numberFormats"].clone();
        }
        if !params["conditionalFormats"].is_null() {
            sidecar_params["conditionalFormats"] = params["conditionalFormats"].clone();
        }

        // PPT 专用参数
        if !params["slides"].is_null() {
            sidecar_params["slides"] = params["slides"].clone();
        }
        if let Some(color_scheme) = params["colorScheme"].as_str() {
            sidecar_params["colorScheme"] = json!(color_scheme);
        }
        if !params["fonts"].is_null() {
            sidecar_params["fonts"] = params["fonts"].clone();
        }
        if !params["margins"].is_null() {
            sidecar_params["margins"] = params["margins"].clone();
        }

        // PDF 专用参数
        if !params["subscripts"].is_null() {
            sidecar_params["subscripts"] = params["subscripts"].clone();
        }
        if !params["superscripts"].is_null() {
            sidecar_params["superscripts"] = params["superscripts"].clone();
        }

        match self.doc_service.process("generate", doc_type, sidecar_params).await {
            Ok(data) => {
                // 生成成功后执行文档验证（可选，默认关闭）
                let enable_validation = params["validate"].as_bool().unwrap_or(false);
                let mut output = data;
                if enable_validation {
                    // 获取生成的文件路径
                    if let Some(path) = output.get("path").and_then(|p| p.as_str()) {
                        let validate_params = json!({
                            "path": path,
                        });
                        match self.doc_service.process("validate", doc_type, validate_params).await {
                            Ok(validation_data) => {
                                output["validation"] = validation_data;
                            }
                            Err(_) => {
                                // 验证失败不影响主流程
                            }
                        }
                    }
                }
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
                                "enum": [
                                    "replace", "add_paragraph", "add_heading", "add_table",
                                    "set_cell", "append", "prepend",
                                    "addHeader", "addFooter", "addBookmark", "addHyperlink",
                                    "setPageSize", "addToc",
                                    "setFormula", "setFormat", "setColorCoding", "addConditionalFormat",
                                    "applyColorScheme", "setFont", "setMargins", "setSlideBackground",
                                    "merge", "split", "rotate", "addWatermark", "encrypt"
                                ],
                                "description": "操作类型。基础操作: replace/add_paragraph/add_heading/add_table/set_cell/append/prepend。Word 专用: addHeader/addFooter/addBookmark/addHyperlink/setPageSize/addToc。Excel 专用: setFormula/setFormat/setColorCoding/addConditionalFormat。PPT 专用: applyColorScheme/setFont/setMargins/setSlideBackground。PDF 专用: merge/split/rotate/addWatermark/encrypt"
                            },
                            "index": {
                                "type": "integer",
                                "description": "段落索引（从0开始），用于 replace 操作按索引替换整段内容"
                            },
                            "text": {
                                "type": "string",
                                "description": "文本内容，用于多种操作"
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
                            },
                            "pageNumber": {
                                "type": "boolean",
                                "description": "是否显示页码，用于 addFooter 操作"
                            },
                            "id": {
                                "type": "string",
                                "description": "书签 ID，用于 addBookmark 操作"
                            },
                            "url": {
                                "type": "string",
                                "description": "外部链接 URL，用于 addHyperlink 操作"
                            },
                            "anchor": {
                                "type": "string",
                                "description": "内部书签锚点，用于 addHyperlink 操作"
                            },
                            "size": {
                                "type": "string",
                                "description": "页面尺寸 (letter/a4)，用于 setPageSize 操作"
                            },
                            "sheet": {
                                "type": "string",
                                "description": "工作表名称，用于 Excel 操作"
                            },
                            "row": {
                                "type": "integer",
                                "description": "行号（从1开始），用于 Excel 操作"
                            },
                            "col": {
                                "type": "integer",
                                "description": "列号（从1开始），用于 Excel 操作"
                            },
                            "formula": {
                                "type": "string",
                                "description": "Excel 公式，用于 setFormula 操作"
                            },
                            "range": {
                                "type": "string",
                                "description": "单元格范围，用于 setFormat/setColorCoding/addConditionalFormat 操作"
                            },
                            "format": {
                                "type": "string",
                                "description": "数字格式类型，用于 setFormat 操作"
                            },
                            "colorType": {
                                "type": "string",
                                "description": "颜色编码类型 (input/formula/cross_ref/external/assumption)，用于 setColorCoding 操作"
                            },
                            "rule": {
                                "type": "string",
                                "description": "条件格式规则，用于 addConditionalFormat 操作"
                            },
                            "value": {
                                "description": "规则值或单元格值"
                            },
                            "color": {
                                "type": "string",
                                "description": "颜色值（十六进制），用于多种操作"
                            },
                            "scheme": {
                                "type": "string",
                                "description": "颜色方案名称 (midnight/forest/coral/ocean/charcoal)，用于 applyColorScheme 操作"
                            },
                            "element": {
                                "type": "string",
                                "description": "字体元素类型 (title/body/all)，用于 setFont 操作"
                            },
                            "font": {
                                "type": "string",
                                "description": "字体名称，用于 setFont 操作"
                            },
                            "fontSize": {
                                "type": "integer",
                                "description": "字体大小（pt），用于 setFont 操作"
                            },
                            "slideIndex": {
                                "type": "integer",
                                "description": "幻灯片索引（从0开始），用于 setSlideBackground 操作"
                            },
                            "files": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "要合并的 PDF 文件路径列表，用于 merge 操作"
                            },
                            "ranges": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "页码范围列表（如 ['1-5', '6-10']），用于 split 操作"
                            },
                            "pages": {
                                "type": "array",
                                "items": { "type": "integer" },
                                "description": "要旋转的页码列表，用于 rotate 操作"
                            },
                            "angle": {
                                "type": "integer",
                                "description": "旋转角度，用于 rotate 操作"
                            },
                            "userPassword": {
                                "type": "string",
                                "description": "用户密码，用于 encrypt 操作"
                            },
                            "ownerPassword": {
                                "type": "string",
                                "description": "所有者密码，用于 encrypt 操作"
                            },
                            "outputPath": {
                                "type": "string",
                                "description": "输出文件路径（可选），用于 PDF 高级操作"
                            }
                        },
                        "required": ["type"]
                    },
                    "description": "修改操作列表。支持 Word/Excel/PPT/PDF 各类操作，具体参数根据操作类型而定"
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
            "pdf" => "pdf",
            "md" | "markdown" => "md",
            _ => "docx",
        };

        // 对 operations 数组中的路径字段进行解析（将相对路径转为绝对路径）
        let resolved_operations = resolve_operation_paths(&params["operations"], workspace_root);

        let sidecar_params = json!({
            "path": resolved_path,
            "operations": resolved_operations,
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
