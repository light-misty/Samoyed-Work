//! 代码解析器:基于 tree-sitter 解析代码语法树，提取符号信息

use crate::errors::CommandError;
use std::path::Path;
use tree_sitter::{Language, Node, Parser};

/// 编程语言枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProgrammingLanguage {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    C,
    Cpp,
    /// 未知/不支持的语言
    Unknown,
}

impl ProgrammingLanguage {
    /// 从文件扩展名推断语言
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "rs" => Self::Rust,
            "py" => Self::Python,
            "js" | "jsx" | "mjs" | "cjs" => Self::JavaScript,
            "ts" | "tsx" => Self::TypeScript,
            "go" => Self::Go,
            "java" => Self::Java,
            "c" | "h" => Self::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Self::Cpp,
            _ => Self::Unknown,
        }
    }

    /// 从文件路径推断语言
    pub fn from_path(path: &Path) -> Self {
        path.extension()
            .and_then(|e| e.to_str())
            .map(Self::from_extension)
            .unwrap_or(Self::Unknown)
    }

    /// 获取对应的 tree-sitter Language
    fn to_tree_sitter_language(self) -> Option<Language> {
        match self {
            Self::Rust => Some(tree_sitter_rust::language()),
            Self::Python => Some(tree_sitter_python::language()),
            Self::JavaScript => Some(tree_sitter_javascript::language()),
            Self::TypeScript => Some(tree_sitter_typescript::language_typescript()),
            Self::Go => Some(tree_sitter_go::language()),
            Self::Java => Some(tree_sitter_java::language()),
            Self::C => Some(tree_sitter_c::language()),
            Self::Cpp => Some(tree_sitter_cpp::language()),
            Self::Unknown => None,
        }
    }

    /// 是否为受支持的语言
    pub fn is_supported(&self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

/// 符号类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolType {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    TypeAlias,
    Macro,
    Variable,
    Constant,
    Module,
    /// 未知类型
    Unknown,
}

impl SymbolType {
    /// 从节点类型字符串推断符号类型
    /// 不同语言的节点类型名称不同
    fn from_node_kind(lang: ProgrammingLanguage, kind: &str) -> Self {
        match (lang, kind) {
            // Rust
            (ProgrammingLanguage::Rust, "function_item") => Self::Function,
            (ProgrammingLanguage::Rust, "struct_item") => Self::Struct,
            (ProgrammingLanguage::Rust, "enum_item") => Self::Enum,
            (ProgrammingLanguage::Rust, "trait_item") => Self::Trait,
            (ProgrammingLanguage::Rust, "macro_definition") => Self::Macro,
            (ProgrammingLanguage::Rust, "impl_item") => Self::Class, // impl 块作为容器
            // Python
            (ProgrammingLanguage::Python, "function_definition") => Self::Function,
            (ProgrammingLanguage::Python, "class_definition") => Self::Class,
            // JavaScript
            (ProgrammingLanguage::JavaScript, "function_declaration") => Self::Function,
            (ProgrammingLanguage::JavaScript, "class_declaration") => Self::Class,
            (ProgrammingLanguage::JavaScript, "method_definition") => Self::Method,
            // TypeScript
            (ProgrammingLanguage::TypeScript, "function_declaration") => Self::Function,
            (ProgrammingLanguage::TypeScript, "class_declaration") => Self::Class,
            (ProgrammingLanguage::TypeScript, "method_definition") => Self::Method,
            (ProgrammingLanguage::TypeScript, "interface_declaration") => Self::Interface,
            (ProgrammingLanguage::TypeScript, "type_alias_declaration") => Self::TypeAlias,
            // Go
            (ProgrammingLanguage::Go, "function_declaration") => Self::Function,
            (ProgrammingLanguage::Go, "type_declaration") => Self::Struct,
            // Java
            (ProgrammingLanguage::Java, "method_declaration") => Self::Method,
            (ProgrammingLanguage::Java, "class_declaration") => Self::Class,
            (ProgrammingLanguage::Java, "interface_declaration") => Self::Interface,
            // C
            (ProgrammingLanguage::C, "function_definition") => Self::Function,
            (ProgrammingLanguage::C, "struct_specifier") => Self::Struct,
            // C++
            (ProgrammingLanguage::Cpp, "function_definition") => Self::Function,
            (ProgrammingLanguage::Cpp, "class_specifier") => Self::Class,
            (ProgrammingLanguage::Cpp, "struct_specifier") => Self::Struct,
            _ => Self::Unknown,
        }
    }

    /// 转换为字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Interface => "interface",
            Self::Trait => "trait",
            Self::TypeAlias => "type_alias",
            Self::Macro => "macro",
            Self::Variable => "variable",
            Self::Constant => "constant",
            Self::Module => "module",
            Self::Unknown => "unknown",
        }
    }
}

/// 代码符号信息
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeSymbol {
    /// 符号名称
    pub name: String,
    /// 符号类型
    pub symbol_type: SymbolType,
    /// 起始行(0-based)
    pub start_line: usize,
    /// 结束行(0-based)
    pub end_line: usize,
    /// 起始列(0-based)
    pub start_col: usize,
    /// 文档注释(如果有)
    pub doc_comment: Option<String>,
}

/// 代码解析器
pub struct LanguageParser {
    /// 解析器
    parser: Parser,
}

impl LanguageParser {
    /// 创建新的解析器
    pub fn new() -> Result<Self, CommandError> {
        let parser = Parser::new();
        Ok(Self { parser })
    }

    /// 解析代码，提取所有符号
    /// source_code: 源代码文本
    /// language: 编程语言
    pub fn parse_symbols(
        &mut self,
        source_code: &str,
        language: ProgrammingLanguage,
    ) -> Result<Vec<CodeSymbol>, CommandError> {
        let lang = language
            .to_tree_sitter_language()
            .ok_or_else(|| CommandError::tool(9003, format!("不支持的语言: {:?}", language)))?;

        self.parser
            .set_language(&lang)
            .map_err(|e| CommandError::tool(9003, format!("设置语言失败: {}", e)))?;

        let tree = self
            .parser
            .parse(source_code, None)
            .ok_or_else(|| CommandError::tool(9003, "解析代码失败".to_string()))?;

        let root_node = tree.root_node();
        let mut symbols = Vec::new();

        // 遍历语法树，提取符号
        self.extract_symbols(&root_node, source_code, language, &mut symbols);

        Ok(symbols)
    }

    /// 递归提取符号
    fn extract_symbols(
        &self,
        node: &Node,
        source_code: &str,
        language: ProgrammingLanguage,
        symbols: &mut Vec<CodeSymbol>,
    ) {
        let kind = node.kind();
        let symbol_type = SymbolType::from_node_kind(language, kind);

        if !matches!(symbol_type, SymbolType::Unknown) {
            // 这是一个符号节点
            if let Some(symbol) = self.node_to_symbol(node, source_code, language, symbol_type) {
                symbols.push(symbol);

                // 对于类/结构体/trait/impl 块，递归提取内部方法
                if matches!(
                    symbol_type,
                    SymbolType::Class | SymbolType::Struct | SymbolType::Trait
                ) {
                    for i in 0..node.child_count() {
                        if let Some(child) = node.child(i) {
                            self.extract_symbols(&child, source_code, language, symbols);
                        }
                    }
                }
                return;
            }
        }

        // 递归遍历子节点
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                self.extract_symbols(&child, source_code, language, symbols);
            }
        }
    }

    /// 将语法树节点转换为符号信息
    fn node_to_symbol(
        &self,
        node: &Node,
        source_code: &str,
        language: ProgrammingLanguage,
        symbol_type: SymbolType,
    ) -> Option<CodeSymbol> {
        // 提取符号名称(需要源代码字节切片用于 utf8_text)
        let source_bytes = source_code.as_bytes();
        let name = self.extract_name(node, source_bytes, language, symbol_type)?;

        let start_pos = node.start_position();
        let end_pos = node.end_position();

        // 提取文档注释
        let doc_comment = self.extract_doc_comment(node, source_code, language);

        Some(CodeSymbol {
            name,
            symbol_type,
            start_line: start_pos.row,
            end_line: end_pos.row,
            start_col: start_pos.column,
            doc_comment,
        })
    }

    /// 提取符号名称
    /// 不同语言、不同符号类型的名称节点位置不同
    /// 通过查找名为 "identifier"、"type_identifier" 或 "name" 的直接子节点获取符号名
    fn extract_name(
        &self,
        node: &Node,
        source_bytes: &[u8],
        _language: ProgrammingLanguage,
        _symbol_type: SymbolType,
    ) -> Option<String> {
        // 通用查找:寻找名为 "identifier" 或 "type_identifier" 的子节点
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                let child_kind = child.kind();
                if child_kind == "identifier"
                    || child_kind == "type_identifier"
                    || child_kind == "name"
                {
                    // 使用 utf8_text 需要传入源代码字节切片
                    return Some(child.utf8_text(source_bytes).ok()?.to_string());
                }
            }
        }
        None
    }

    /// 提取文档注释
    /// 查找前一个兄弟节点是否为注释节点
    fn extract_doc_comment(
        &self,
        node: &Node,
        source_code: &str,
        language: ProgrammingLanguage,
    ) -> Option<String> {
        // 查找前一个兄弟节点是否为注释
        let prev = node.prev_sibling()?;
        let kind = prev.kind();

        // 不同语言的注释节点类型
        let is_comment = match language {
            ProgrammingLanguage::Rust | ProgrammingLanguage::Go => {
                kind == "line_comment" || kind == "block_comment"
            }
            ProgrammingLanguage::Python => kind == "comment",
            ProgrammingLanguage::JavaScript
            | ProgrammingLanguage::TypeScript
            | ProgrammingLanguage::Java
            | ProgrammingLanguage::C
            | ProgrammingLanguage::Cpp => kind == "comment",
            ProgrammingLanguage::Unknown => false,
        };

        if is_comment {
            let start = prev.start_byte();
            let end = prev.end_byte();
            if end <= source_code.len() {
                return Some(source_code[start..end].to_string());
            }
        }
        None
    }
}

impl Default for LanguageParser {
    fn default() -> Self {
        Self::new().expect("创建 LanguageParser 失败")
    }
}
