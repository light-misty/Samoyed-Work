use serde::{Deserialize, Serialize};

/// 文档预览内容
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PreviewContent {
    pub path: String,
    /// "docx" | "xlsx" | "pptx" | "pdf" | "md"
    pub file_type: String,
    /// 预览文本内容
    pub content: String,
    /// 页数（适用于 PDF/DOCX）
    pub page_count: Option<u32>,
    /// 工作表名称（适用于 XLSX）
    pub sheet_names: Option<Vec<String>>,
    pub metadata: Option<DocumentMetadata>,
}

/// 文档元数据
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DocumentMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub word_count: Option<u32>,
}
