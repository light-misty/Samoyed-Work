/// 构建脚本：检测内置 Provider 配置文件是否存在
/// 如果项目根目录下存在 builtin_provider.json，则启用 builtin_provider cfg 标志
fn main() {
    // 声明 builtin_provider 为合法的 cfg 条件，消除 unexpected_cfgs 警告
    println!("cargo::rustc-check-cfg=cfg(builtin_provider)");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let project_root = std::path::Path::new(&manifest_dir)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let builtin_config = project_root.join("builtin_provider.json");

    if builtin_config.exists() {
        println!("cargo:rustc-cfg=builtin_provider");
        println!("cargo:warning=内置 Provider 配置文件已检测到，启用内置模型");
    }

    // tauri-build 需要在 build.rs 中调用
    tauri_build::build();
}
