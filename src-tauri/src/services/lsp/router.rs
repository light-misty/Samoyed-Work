//! 语言路由:根据文件扩展名自动选择对应的 LSP 服务器

use std::path::Path;

/// 语言路由器
pub struct LanguageRouter {
    /// 扩展名到语言名称的映射
    extension_map: std::collections::HashMap<String, String>,
}

impl Default for LanguageRouter {
    fn default() -> Self {
        let mut map = std::collections::HashMap::new();

        // Rust
        map.insert("rs".to_string(), "rust".to_string());

        // Python
        map.insert("py".to_string(), "python".to_string());

        // TypeScript / JavaScript
        map.insert("ts".to_string(), "typescript".to_string());
        map.insert("tsx".to_string(), "typescript".to_string());
        map.insert("js".to_string(), "javascript".to_string());
        map.insert("jsx".to_string(), "javascript".to_string());

        // Go
        map.insert("go".to_string(), "go".to_string());

        // Java
        map.insert("java".to_string(), "java".to_string());

        // C / C++
        map.insert("c".to_string(), "c".to_string());
        map.insert("h".to_string(), "c".to_string());
        map.insert("cpp".to_string(), "cpp".to_string());
        map.insert("cxx".to_string(), "cpp".to_string());
        map.insert("cc".to_string(), "cpp".to_string());
        map.insert("hpp".to_string(), "cpp".to_string());

        Self { extension_map: map }
    }
}

impl LanguageRouter {
    pub fn new() -> Self {
        Self::default()
    }

    /// 根据文件路径推断语言
    pub fn detect_language(&self, file_path: &Path) -> Option<String> {
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())?;

        self.extension_map.get(&ext).cloned()
    }

    /// 获取语言的 language_id(用于 LSP didOpen)
    pub fn get_language_id(&self, language: &str) -> &str {
        match language {
            "rust" => "rust",
            "python" => "python",
            "typescript" => "typescript",
            "javascript" => "javascript",
            "go" => "go",
            "java" => "java",
            "c" => "c",
            "cpp" => "cpp",
            _ => "plaintext",
        }
    }

    /// 添加自定义扩展名映射
    pub fn add_extension(&mut self, ext: &str, language: &str) {
        self.extension_map
            .insert(ext.to_lowercase(), language.to_string());
    }
}
