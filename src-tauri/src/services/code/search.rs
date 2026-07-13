//! 代码搜索器:基于 LanguageParser 在目录/文件中搜索代码符号
//! 支持按符号类型、名称通配符、文件扩展名过滤

use crate::errors::{self, CommandError};
use crate::services::code::parser::{CodeSymbol, LanguageParser, ProgrammingLanguage};
use serde::{Deserialize, Serialize};
use std::path::Path;
use wildmatch::WildMatch;

/// 搜索查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    /// 搜索目录(绝对路径或相对 workspace_root 的路径)
    pub directory: String,
    /// 符号名称通配符(如 "get_*"、"*_handler"),为 None 时不过滤
    pub symbol_name: Option<String>,
    /// 符号类型过滤(如 "function"、"class"、"struct"),为 None 时不过滤
    pub symbol_type: Option<String>,
    /// 文件扩展名过滤(如 ["rs", "py"]),为 None 时按受支持语言过滤
    pub extensions: Option<Vec<String>>,
    /// 是否递归搜索子目录
    pub recursive: bool,
    /// 最大结果数
    pub max_results: usize,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            directory: String::new(),
            symbol_name: None,
            symbol_type: None,
            extensions: None,
            recursive: true,
            max_results: 100,
        }
    }
}

/// 单条搜索结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    /// 文件绝对路径
    pub file_path: String,
    /// 符号信息
    pub symbol: CodeSymbol,
    /// 符号起始行内容(去除首尾空白)
    pub line_content: String,
}

/// 代码搜索器:封装 LanguageParser 提供目录级别的符号搜索能力
pub struct SourceCodeSearcher {
    parser: LanguageParser,
}

impl SourceCodeSearcher {
    /// 创建新的搜索器
    pub fn new() -> Result<Self, CommandError> {
        Ok(Self {
            parser: LanguageParser::new()?,
        })
    }

    /// 执行搜索
    /// 按目录递归(可选)遍历文件,解析每个受支持语言的文件,过滤符合条件的符号
    pub fn search(&mut self, query: &SearchQuery) -> Result<Vec<SearchResult>, CommandError> {
        let dir = Path::new(&query.directory);
        if !dir.exists() {
            return Err(CommandError::fs(
                errors::FS_PATH_NOT_FOUND,
                format!("目录不存在: {}", query.directory),
            ));
        }
        if !dir.is_dir() {
            return Err(CommandError::tool(
                errors::TOOL_INVALID_PARAMS,
                format!("路径不是目录: {}", query.directory),
            ));
        }

        let mut results = Vec::new();
        self.search_dir(dir, query, &mut results)?;

        // 限制结果数量(防止意外过大)
        results.truncate(query.max_results);
        Ok(results)
    }

    /// 递归搜索目录
    fn search_dir(
        &mut self,
        dir: &Path,
        query: &SearchQuery,
        results: &mut Vec<SearchResult>,
    ) -> Result<(), CommandError> {
        let entries = std::fs::read_dir(dir).map_err(|e| {
            CommandError::fs(
                errors::FS_IO_ERROR,
                format!("读取目录失败 {}: {}", dir.display(), e),
            )
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                CommandError::fs(errors::FS_IO_ERROR, format!("读取目录项失败: {}", e))
            })?;
            let path = entry.path();

            if path.is_dir() {
                if !query.recursive {
                    continue;
                }
                // 跳过忽略目录(常见构建产物/依赖目录,避免误入)
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if IGNORED_DIRS.contains(&name) {
                        continue;
                    }
                }
                self.search_dir(&path, query, results)?;
            } else if path.is_file() {
                // 检查扩展名是否匹配
                if !self.is_matching_extension(&path, &query.extensions) {
                    continue;
                }

                // 搜索文件
                self.search_file(&path, query, results)?;

                // 达到最大结果数停止
                if results.len() >= query.max_results {
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    /// 搜索单个文件
    fn search_file(
        &mut self,
        path: &Path,
        query: &SearchQuery,
        results: &mut Vec<SearchResult>,
    ) -> Result<(), CommandError> {
        let language = ProgrammingLanguage::from_path(path);
        if !language.is_supported() {
            return Ok(());
        }

        // 读取文件,跳过无法读取的文件(如二进制文件)
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };

        let symbols = self.parser.parse_symbols(&source, language)?;
        let source_lines: Vec<&str> = source.lines().collect();

        for symbol in symbols {
            if !self.is_matching_symbol(&symbol, query) {
                continue;
            }

            // 提取符号起始行内容
            let line_content = if symbol.start_line < source_lines.len() {
                source_lines[symbol.start_line].trim().to_string()
            } else {
                String::new()
            };

            results.push(SearchResult {
                file_path: path.to_string_lossy().to_string(),
                symbol,
                line_content,
            });

            if results.len() >= query.max_results {
                return Ok(());
            }
        }
        Ok(())
    }

    /// 检查文件扩展名是否匹配
    /// 未指定 extensions 时,按受支持语言过滤(避免解析非代码文件)
    fn is_matching_extension(&self, path: &Path, extensions: &Option<Vec<String>>) -> bool {
        match extensions {
            None => ProgrammingLanguage::from_path(path).is_supported(),
            Some(exts) => {
                let file_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                exts.iter().any(|e| e.eq_ignore_ascii_case(file_ext))
            }
        }
    }

    /// 检查符号是否匹配查询条件(类型 + 名称通配符)
    fn is_matching_symbol(&self, symbol: &CodeSymbol, query: &SearchQuery) -> bool {
        // 检查符号类型
        if let Some(type_filter) = &query.symbol_type {
            if !symbol
                .symbol_type
                .as_str()
                .eq_ignore_ascii_case(type_filter)
            {
                return false;
            }
        }

        // 检查符号名称(通配符匹配,WildMatch 默认大小写敏感)
        if let Some(name_pattern) = &query.symbol_name {
            let matcher = WildMatch::new(name_pattern);
            if !matcher.matches(&symbol.name) {
                return false;
            }
        }

        true
    }

    /// 解析代码并返回符号(供外部调用,如 list_symbols 操作)
    pub fn parse_symbols(
        &mut self,
        source: &str,
        language: ProgrammingLanguage,
    ) -> Result<Vec<CodeSymbol>, CommandError> {
        self.parser.parse_symbols(source, language)
    }
}

/// 被忽略的目录名(常见构建产物/依赖/缓存目录)
const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    "__pycache__",
    ".next",
    ".nuxt",
    "vendor",
    ".venv",
    "venv",
    "env",
];
