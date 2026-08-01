use std::path::PathBuf;

use base64::Engine;
use serde_json::json;
use tauri::State;

use crate::errors::{
    CommandError, DOC_FILE_NOT_FOUND, DOC_FORMAT_UNSUPPORTED, FS_ALREADY_EXISTS, FS_PATH_NOT_FOUND,
};
use crate::models::document::PreviewContent;
use crate::AppState;

/// 预览文档内容
/// 支持文本/源码文件直接读取，Office/PDF 通过 Sidecar 解析，
/// 未知扩展名通过内容检测（NUL 字节）兜底区分二进制
#[tauri::command]
pub async fn preview_document(
    workspace_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<PreviewContent, CommandError> {
    log::info!(
        "preview_document: 预览文档, workspace_id={}, path={}",
        workspace_id,
        path
    );
    let config = state.config.lock().await;
    let ws_config = config.load_workspaces()?;

    let workspace = ws_config
        .workspaces
        .iter()
        .find(|w| w.id == workspace_id)
        .ok_or_else(|| {
            log::error!("preview_document: 工作区 '{}' 不存在", workspace_id);
            CommandError::fs(
                crate::errors::FS_PATH_NOT_FOUND,
                format!("工作区 '{}' 不存在", workspace_id),
            )
        })?;

    let file_path = PathBuf::from(&workspace.path).join(&path);
    if !file_path.exists() {
        log::error!("preview_document: 文件不存在: {}", path);
        return Err(CommandError::doc(
            DOC_FILE_NOT_FOUND,
            format!("文件不存在: {}", path),
        ));
    }

    let extension = file_path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    // 按扩展名分类文件
    let kind = classify_extension(&extension);
    log::debug!("preview_document: 扩展名={}, 分类={:?}", extension, kind);

    // Office/PDF 文档：通过 Sidecar 解析
    if let Some(FileKind::Sidecar(file_type)) = kind {
        // 释放配置锁后再调用 Sidecar（避免长时间持锁）
        drop(config);

        let sidecar_params = json!({
            "input_path": file_path.to_string_lossy().to_string(),
            "options": {
                "include_formatting": false,
            },
        });
        let content = match state
            .doc_service
            .process("read", file_type, sidecar_params)
            .await
        {
            Ok(data) => serde_json::to_string_pretty(&data)
                .unwrap_or_else(|_| "[预览] 文档内容解析失败".to_string()),
            Err(e) => {
                log::warn!(
                    "preview_document: Sidecar 解析失败, 降级为占位提示: {}",
                    e.message
                );
                format!(
                    "[预览] {} 格式文件解析失败: {}",
                    extension.to_uppercase(),
                    e.message
                )
            }
        };

        log::info!("preview_document: 预览完成, file_type={}", file_type);
        return Ok(PreviewContent {
            path: path.clone(),
            file_type: file_type.to_string(),
            content,
            page_count: None,
            sheet_names: None,
            metadata: None,
        });
    }

    // 图片文件：不读取二进制内容，file_type 统一为 image 标识，由前端通过 asset 协议（convertFileSrc）直接渲染
    if matches!(kind, Some(FileKind::Image)) {
        log::info!(
            "preview_document: 图片预览, file_type=image, extension={}",
            extension
        );
        return Ok(PreviewContent {
            path: path.clone(),
            file_type: "image".to_string(),
            content: String::new(),
            page_count: None,
            sheet_names: None,
            metadata: None,
        });
    }

    // 文本/源码文件（含未知扩展名）：读取内容并做二进制检测
    // 已知二进制格式（如 exe、压缩包等）直接拒绝
    if matches!(kind, Some(FileKind::Binary)) {
        log::error!("preview_document: 不支持预览二进制文件: .{}", extension);
        return Err(CommandError::doc(
            DOC_FORMAT_UNSUPPORTED,
            format!("不支持预览二进制文件格式: .{}", extension),
        ));
    }

    let bytes = std::fs::read(&file_path)?;
    // 内容级二进制检测（处理未知扩展名或无扩展名文件）
    if is_binary_bytes(&bytes) {
        log::error!("preview_document: 文件内容为二进制, 不支持预览: {}", path);
        return Err(CommandError::doc(
            DOC_FORMAT_UNSUPPORTED,
            format!("不支持预览二进制文件: {}", path),
        ));
    }

    // 文本解码（UTF-8 优先，GBK 回退），file_type 保留原扩展名便于前端语法高亮
    let content = decode_text(&bytes);
    let file_type = if extension.is_empty() {
        "txt"
    } else {
        &extension
    };
    log::info!("preview_document: 文本预览完成, file_type={}", file_type);
    Ok(PreviewContent {
        path: path.clone(),
        file_type: file_type.to_string(),
        content,
        page_count: None,
        sheet_names: None,
        metadata: None,
    })
}

/// 预览文件分类
#[derive(Debug, Clone, PartialEq)]
enum FileKind {
    /// 通过 Sidecar 解析的二进制文档（docx/xlsx/pptx/pdf）
    Sidecar(&'static str),
    /// 图片文件（前端通过 asset 协议直接渲染）
    Image,
    /// 文本/源码文件（直接读取解码）
    Text,
    /// 已知二进制格式（拒绝预览）
    Binary,
}

/// 根据扩展名分类文件类型（大小写不敏感）；返回 None 表示未知扩展名，交由内容检测兜底
fn classify_extension(extension: &str) -> Option<FileKind> {
    let extension = extension.to_ascii_lowercase();
    match extension.as_str() {
        // Office/PDF 文档：通过 Sidecar 解析
        "docx" | "doc" => Some(FileKind::Sidecar("docx")),
        "xlsx" | "xls" => Some(FileKind::Sidecar("xlsx")),
        "pptx" | "ppt" => Some(FileKind::Sidecar("pptx")),
        "pdf" => Some(FileKind::Sidecar("pdf")),
        // 常用图片格式：前端通过 asset 协议直接渲染（少见的 ico/tiff/heic/psd 等仍归为二进制拒绝）
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" => Some(FileKind::Image),
        // 已知二进制格式：可执行文件、音视频、压缩包、字体、数据库等
        "exe" | "dll" | "so" | "dylib" | "bin" | "o" | "obj" | "lib" | "a" | "pdb" | "class"
        | "jar" | "war" | "pyc" | "pyo" | "pyd" | "whl" | "wasm" | "ico" | "avif" | "tiff"
        | "tif" | "heic" | "psd" | "mp3" | "mp4" | "avi" | "mkv" | "mov" | "wav" | "flac"
        | "ogg" | "oga" | "ogv" | "webm" | "aac" | "wma" | "m4a" | "m4v" | "wmv" | "mpg"
        | "mpeg" | "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" | "iso" | "dmg"
        | "cab" | "deb" | "rpm" | "apk" | "ipa" | "ttf" | "otf" | "woff" | "woff2" | "eot"
        | "db" | "sqlite" | "sqlite3" | "sdb" | "mdb" | "accdb" | "dmp" | "dat" | "pak" | "h5"
        | "hdf5" | "npy" | "npz" | "pkl" | "pickle" | "der" | "swf" | "flv" => {
            Some(FileKind::Binary)
        }
        // 文本/源码文件
        _ if is_text_extension(&extension) => Some(FileKind::Text),
        // 未知扩展名：交由内容检测兜底
        _ => None,
    }
}

/// 是否为已知的文本/源码扩展名（覆盖主流编程语言与配置文件）
fn is_text_extension(extension: &str) -> bool {
    matches!(
        extension,
        // Markdown 与纯文本
        "md" | "markdown" | "txt" | "text" | "log"
        // JavaScript / TypeScript 生态
        | "js" | "mjs" | "cjs" | "jsx" | "ts" | "mts" | "cts" | "tsx" | "vue" | "svelte"
        | "astro"
        // Python
        | "py" | "pyw" | "pyi" | "pyx" | "ipynb"
        // Rust / Go / Java / C# / C/C++
        | "rs" | "go" | "java" | "jsp" | "cs" | "csx" | "c" | "h" | "cc" | "cpp" | "cxx"
        | "hpp" | "hxx" | "hh" | "ino"
        // 脚本与 Shell
        | "php" | "phtml" | "rb" | "rake" | "gemspec" | "swift" | "kt" | "kts" | "scala"
        | "sc" | "dart" | "sh" | "bash" | "zsh" | "fish" | "ksh" | "ps1" | "psd1" | "psm1"
        | "bat" | "cmd" | "awk" | "sed" | "lua" | "r" | "rmd" | "pl" | "pm" | "tcl" | "erl"
        | "hrl" | "ex" | "exs" | "clj" | "cljs" | "edn" | "hs" | "lhs" | "jl" | "nim" | "ml"
        | "mli" | "fs" | "fsi" | "fsx" | "vb" | "vbs" | "groovy" | "sol" | "zig" | "v" | "pas"
        | "pp" | "f" | "f90" | "f95" | "cob" | "cbl" | "adb" | "ads" | "lisp" | "lsp" | "cl"
        | "scm" | "ss" | "m" | "mm" | "cr" | "elm" | "vim"
        // Web 前端
        | "html" | "htm" | "css" | "scss" | "sass" | "less" | "styl" | "xml"
        // 数据与配置文件
        | "json" | "jsonc" | "json5" | "yaml" | "yml" | "toml" | "ini" | "cfg" | "conf"
        | "properties" | "env" | "editorconfig" | "gitignore" | "gitattributes" | "lock"
        | "gradle" | "mk"
        // SQL 与表格数据
        | "sql" | "csv" | "tsv"
    )
}

/// 检测字节流是否为二进制：文本文件几乎不会包含 NUL 字节
/// 仅采样前 8KB，避免大文件全量扫描开销
fn is_binary_bytes(bytes: &[u8]) -> bool {
    let sample_len = bytes.len().min(8192);
    bytes[..sample_len].contains(&0u8)
}

/// 解码文本内容：优先 UTF-8，失败时回退 GBK（Windows 环境常见的中文编码）
fn decode_text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let (decoded, _, _) = encoding_rs::GBK.decode(bytes);
            decoded.into_owned()
        }
    }
}

/// 解析工作区路径，返回工作区信息和绝对路径
async fn resolve_workspace_path(
    workspace_id: &str,
    relative_path: &str,
    state: &State<'_, AppState>,
) -> Result<(String, PathBuf), CommandError> {
    let config = state.config.lock().await;
    let ws_config = config.load_workspaces()?;

    let workspace = ws_config
        .workspaces
        .iter()
        .find(|w| w.id == workspace_id)
        .ok_or_else(|| {
            log::error!("工作区 '{}' 不存在", workspace_id);
            CommandError::fs(
                FS_PATH_NOT_FOUND,
                format!("工作区 '{}' 不存在", workspace_id),
            )
        })?;

    let abs_path = PathBuf::from(&workspace.path).join(relative_path);
    Ok((workspace.path.clone(), abs_path))
}

/// 创建空文件
#[tauri::command]
pub async fn create_file(
    workspace_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "create_file: 创建文件, workspace_id={}, path={}",
        workspace_id,
        path
    );
    let (_, abs_path) = resolve_workspace_path(&workspace_id, &path, &state).await?;

    if abs_path.exists() {
        log::error!("create_file: 文件已存在: {}", path);
        return Err(CommandError::fs(
            FS_ALREADY_EXISTS,
            format!("文件已存在: {}", path),
        ));
    }

    // 确保父目录存在
    if let Some(parent) = abs_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::File::create(&abs_path)?;
    log::info!("create_file: 文件创建成功, path={}", path);
    Ok(())
}

/// 创建目录
#[tauri::command]
pub async fn create_directory(
    workspace_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "create_directory: 创建目录, workspace_id={}, path={}",
        workspace_id,
        path
    );
    let (_, abs_path) = resolve_workspace_path(&workspace_id, &path, &state).await?;

    if abs_path.exists() {
        log::error!("create_directory: 目录已存在: {}", path);
        return Err(CommandError::fs(
            FS_ALREADY_EXISTS,
            format!("目录已存在: {}", path),
        ));
    }

    std::fs::create_dir_all(&abs_path)?;
    log::info!("create_directory: 目录创建成功, path={}", path);
    Ok(())
}

/// 重命名文件或目录
#[tauri::command]
pub async fn rename_file(
    workspace_id: String,
    old_path: String,
    new_path: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "rename_file: 重命名, workspace_id={}, old_path={}, new_path={}",
        workspace_id,
        old_path,
        new_path
    );
    let (_, abs_old) = resolve_workspace_path(&workspace_id, &old_path, &state).await?;
    let (_, abs_new) = resolve_workspace_path(&workspace_id, &new_path, &state).await?;

    if !abs_old.exists() {
        log::error!("rename_file: 源路径不存在: {}", old_path);
        return Err(CommandError::fs(
            FS_PATH_NOT_FOUND,
            format!("源路径不存在: {}", old_path),
        ));
    }

    if abs_new.exists() {
        log::error!("rename_file: 目标路径已存在: {}", new_path);
        return Err(CommandError::fs(
            FS_ALREADY_EXISTS,
            format!("目标路径已存在: {}", new_path),
        ));
    }

    // 确保目标父目录存在
    if let Some(parent) = abs_new.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::rename(&abs_old, &abs_new)?;
    log::info!(
        "rename_file: 重命名成功, old_path={} -> new_path={}",
        old_path,
        new_path
    );
    Ok(())
}

/// 删除文件或目录（永久删除）
#[tauri::command]
pub async fn delete_file(
    workspace_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "delete_file: 删除, workspace_id={}, path={}",
        workspace_id,
        path
    );
    let (_, abs_path) = resolve_workspace_path(&workspace_id, &path, &state).await?;

    if !abs_path.exists() {
        log::error!("delete_file: 路径不存在: {}", path);
        return Err(CommandError::fs(
            FS_PATH_NOT_FOUND,
            format!("路径不存在: {}", path),
        ));
    }

    if abs_path.is_dir() {
        std::fs::remove_dir_all(&abs_path)?;
    } else {
        std::fs::remove_file(&abs_path)?;
    }

    log::info!("delete_file: 删除成功, path={}", path);
    Ok(())
}

/// 在系统文件管理器中显示文件或目录
#[tauri::command]
pub async fn show_in_file_manager(
    workspace_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "show_in_file_manager: 在文件管理器中显示, workspace_id={}, path={}",
        workspace_id,
        path
    );
    let (_, abs_path) = resolve_workspace_path(&workspace_id, &path, &state).await?;

    if !abs_path.exists() {
        log::error!("show_in_file_manager: 路径不存在: {}", path);
        return Err(CommandError::fs(
            FS_PATH_NOT_FOUND,
            format!("路径不存在: {}", path),
        ));
    }

    #[cfg(target_os = "windows")]
    {
        // Windows: 使用 explorer /select,"path" 选中并定位到文件
        // 必须使用 raw_arg 避免 Command::arg 对双引号进行转义，
        // 否则 explorer 无法识别 /select 标志，只会打开默认页面
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        std::process::Command::new("explorer")
            .raw_arg(format!("/select,\"{}\"", abs_path.to_string_lossy()))
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: 使用 open -R 在 Finder 中显示
        std::process::Command::new("open")
            .arg("-R")
            .arg(&abs_path)
            .spawn()?;
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: 打开文件所在目录
        let dir = if abs_path.is_dir() {
            abs_path.clone()
        } else {
            abs_path.parent().unwrap_or(&abs_path).to_path_buf()
        };
        std::process::Command::new("xdg-open").arg(&dir).spawn()?;
    }

    log::info!("show_in_file_manager: 已打开文件管理器, path={}", path);
    Ok(())
}

/// 获取 PDF 文件的 base64 编码数据，用于前端 pdfjs-dist 渲染
#[tauri::command]
pub async fn get_pdf_data(
    workspace_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<String, CommandError> {
    log::info!(
        "get_pdf_data: 获取PDF数据, workspace_id={}, path={}",
        workspace_id,
        path
    );
    let (_, abs_path) = resolve_workspace_path(&workspace_id, &path, &state).await?;

    if !abs_path.exists() {
        log::error!("get_pdf_data: 文件不存在: {}", path);
        return Err(CommandError::doc(
            DOC_FILE_NOT_FOUND,
            format!("文件不存在: {}", path),
        ));
    }

    // 校验文件扩展名
    let extension = abs_path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if extension != "pdf" {
        log::warn!("get_pdf_data: 非 PDF 文件: .{}", extension);
        return Err(CommandError::doc(
            DOC_FORMAT_UNSUPPORTED,
            format!("仅支持 PDF 文件，当前文件格式: .{}", extension),
        ));
    }

    // 读取 PDF 文件二进制数据并编码为 base64
    let file_data = std::fs::read(&abs_path)?;
    let base64_str = base64::engine::general_purpose::STANDARD.encode(&file_data);

    log::info!("get_pdf_data: 读取完成, 文件大小={} 字节", file_data.len());
    Ok(base64_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 分类：Office/PDF 文档走 Sidecar
    #[test]
    fn classify_sidecar_extensions() {
        assert!(matches!(
            classify_extension("docx"),
            Some(FileKind::Sidecar("docx"))
        ));
        assert!(matches!(
            classify_extension("doc"),
            Some(FileKind::Sidecar("docx"))
        ));
        assert!(matches!(
            classify_extension("xlsx"),
            Some(FileKind::Sidecar("xlsx"))
        ));
        assert!(matches!(
            classify_extension("xls"),
            Some(FileKind::Sidecar("xlsx"))
        ));
        assert!(matches!(
            classify_extension("pptx"),
            Some(FileKind::Sidecar("pptx"))
        ));
        assert!(matches!(
            classify_extension("ppt"),
            Some(FileKind::Sidecar("pptx"))
        ));
        assert!(matches!(
            classify_extension("pdf"),
            Some(FileKind::Sidecar("pdf"))
        ));
    }

    /// 分类：常见源码扩展名视为文本
    #[test]
    fn classify_source_extensions() {
        for ext in [
            "js", "ts", "tsx", "jsx", "py", "rs", "go", "java", "c", "cpp", "h", "cs", "php", "rb",
            "swift", "kt", "scala", "dart", "sh", "ps1", "sql", "html", "css", "json", "yaml",
            "toml", "xml", "vue", "lua", "r", "md",
        ] {
            assert!(
                matches!(classify_extension(ext), Some(FileKind::Text)),
                "扩展名 .{} 应被分类为文本",
                ext
            );
        }
    }

    /// 分类：常用图片扩展名走 Image（前端 asset 协议直接渲染）
    #[test]
    fn classify_image_extensions() {
        for ext in ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "PNG"] {
            assert!(
                matches!(classify_extension(ext), Some(FileKind::Image)),
                "扩展名 .{} 应被分类为图片",
                ext
            );
        }
    }

    /// 分类：已知二进制扩展名（扩展名大小写不敏感）
    #[test]
    fn classify_binary_extensions() {
        for ext in [
            "exe", "dll", "zip", "mp4", "wasm", "pyc", "EXE", "ico", "tiff", "heic", "psd",
        ] {
            assert!(
                matches!(classify_extension(ext), Some(FileKind::Binary)),
                "扩展名 .{} 应被分类为二进制",
                ext
            );
        }
    }

    /// 分类：未知扩展名返回 None，交由内容检测兜底
    #[test]
    fn classify_unknown_extension() {
        assert_eq!(classify_extension("xyz"), None);
        assert_eq!(classify_extension(""), None);
    }

    /// 二进制检测：含 NUL 字节的文件视为二进制
    #[test]
    fn detect_binary_by_nul_byte() {
        assert!(is_binary_bytes(b"\x4d\x5a\x90\x00\x03\x00\x00\x00"));
        assert!(!is_binary_bytes(b"hello world"));
        assert!(!is_binary_bytes(b""));
    }

    /// 解码：UTF-8 内容原样返回
    #[test]
    fn decode_utf8_text() {
        let content = decode_text("你好，Rust！".as_bytes());
        assert_eq!(content, "你好，Rust！");
    }

    /// 解码：GBK 编码内容回退解码（Windows 常见中文源码）
    #[test]
    fn decode_gbk_text() {
        // "测试" 的 GBK 编码字节
        let content = decode_text(&[0xB2, 0xE2, 0xCA, 0xD4]);
        assert_eq!(content, "测试");
    }
}
