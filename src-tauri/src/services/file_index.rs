use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// 索引文件条目（仅文件名与相对路径，构建时无需 stat，速度快）
#[derive(Debug, Clone)]
pub struct IndexedFile {
    /// 文件名
    pub name: String,
    /// 文件名小写（用于大小写不敏感匹配）
    pub name_lower: String,
    /// 相对工作区根目录的路径
    pub path: String,
    /// 文件扩展名（小写，用于扩展名过滤）
    pub extension: String,
}

/// 单个工作区的文件名索引
#[derive(Debug, Clone)]
pub struct FileIndex {
    /// 工作区根目录（用于判断索引归属是否正确）
    pub root: PathBuf,
    /// 全部文件条目
    pub files: Vec<IndexedFile>,
}

impl FileIndex {
    /// 按文件名匹配搜索（大小写不敏感、包含匹配），最多返回 max_results 条
    pub fn search(&self, query: &str, max_results: usize) -> Vec<&IndexedFile> {
        let query_lower = query.to_lowercase();
        self.files
            .iter()
            .filter(|f| f.name_lower.contains(&query_lower))
            .take(max_results)
            .collect()
    }
}

/// 递归收集目录下所有文件（跳过隐藏文件与目录，保留 .agent 以便智能体感知工作区 Skill）
fn collect_files(dir: &Path, root: &Path, files: &mut Vec<IndexedFile>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') && name != ".agent" {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, root, files);
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        files.push(IndexedFile {
            name: name.clone(),
            name_lower: name.to_lowercase(),
            path: relative,
            extension,
        });
    }
}

/// 构建工作区文件名索引（根目录不存在时返回空索引）
pub fn build_file_index(root: &Path) -> FileIndex {
    let mut files = Vec::new();
    if root.is_dir() {
        collect_files(root, root, &mut files);
    }
    FileIndex {
        root: root.to_path_buf(),
        files,
    }
}

/// 文件索引缓存：按工作区 ID 缓存索引，文件变更时由 FsWatcher 失效，下次搜索自动重建
pub struct FileIndexCache {
    map: Mutex<HashMap<String, Arc<FileIndex>>>,
}

impl FileIndexCache {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            map: Mutex::new(HashMap::new()),
        })
    }

    /// 获取工作区索引；不存在或根目录变化时自动构建并缓存
    pub fn get_or_build(&self, workspace_id: &str, root: &PathBuf) -> Arc<FileIndex> {
        {
            let map = self.map.lock().unwrap();
            if let Some(index) = map.get(workspace_id) {
                if index.root == *root {
                    return Arc::clone(index);
                }
            }
        }
        let built = Arc::new(build_file_index(root));
        self.map
            .lock()
            .unwrap()
            .insert(workspace_id.to_string(), Arc::clone(&built));
        built
    }

    /// 使工作区索引失效（文件变更时调用）
    pub fn invalidate(&self, workspace_id: &str) {
        self.map.lock().unwrap().remove(workspace_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 创建临时目录结构：a/root.txt、a/b/mid.txt、a/b/c/deep.txt、.git/hidden.txt、.agent/skill.md
    fn build_temp_tree() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "samoyed_file_index_test_{}_{}",
            std::process::id(),
            std::thread::current()
                .name()
                .unwrap_or("main")
                .replace(':', "_")
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("a/b/c")).expect("创建临时目录失败");
        fs::create_dir_all(dir.join(".git")).expect("创建临时目录失败");
        fs::create_dir_all(dir.join(".agent")).expect("创建临时目录失败");
        fs::write(dir.join("a/root.txt"), "root").expect("写入临时文件失败");
        fs::write(dir.join("a/b/mid.txt"), "mid").expect("写入临时文件失败");
        fs::write(dir.join("a/b/c/deep.txt"), "deep").expect("写入临时文件失败");
        fs::write(dir.join(".git/hidden.txt"), "hidden").expect("写入临时文件失败");
        fs::write(dir.join(".agent/skill.md"), "skill").expect("写入临时文件失败");
        dir
    }

    /// 索引包含全部文件（隐藏目录除外，.agent 保留），且无需 stat
    #[test]
    fn test_build_index_collects_all_files() {
        let dir = build_temp_tree();
        let index = build_file_index(&dir);
        assert_eq!(index.files.len(), 4);
        assert!(index
            .files
            .iter()
            .any(|f| Path::new(&f.path) == Path::new("a/b/c/deep.txt")));
        assert!(index.files.iter().all(|f| !f.path.starts_with(".git")));
        assert!(index
            .files
            .iter()
            .any(|f| Path::new(&f.path) == Path::new(".agent/skill.md")));
        let _ = fs::remove_dir_all(&dir);
    }

    /// 搜索大小写不敏感、包含匹配、结果数受限
    #[test]
    fn test_index_search() {
        let dir = build_temp_tree();
        let index = build_file_index(&dir);
        assert_eq!(index.search("deep", 10).len(), 1);
        assert_eq!(index.search("DEEP", 10).len(), 1);
        assert_eq!(index.search("t", 2).len(), 2);
        assert_eq!(index.search("不存在", 10).len(), 0);
        let _ = fs::remove_dir_all(&dir);
    }

    /// 缓存命中与失效重建
    #[test]
    fn test_cache_get_or_build_and_invalidate() {
        let dir = build_temp_tree();
        let cache = FileIndexCache::new();
        let first = cache.get_or_build("ws1", &dir);
        assert_eq!(first.files.len(), 4);
        // 再次获取命中缓存（同一 Arc）
        let second = cache.get_or_build("ws1", &dir);
        assert!(Arc::ptr_eq(&first, &second));
        // 失效后重新构建
        cache.invalidate("ws1");
        let third = cache.get_or_build("ws1", &dir);
        assert!(!Arc::ptr_eq(&first, &third));
        assert_eq!(third.files.len(), 4);
        let _ = fs::remove_dir_all(&dir);
    }
}
