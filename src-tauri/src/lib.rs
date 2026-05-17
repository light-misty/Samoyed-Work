use std::sync::Arc;

use tauri::Manager;

pub mod commands;
pub mod config;
pub mod db;
pub mod errors;
pub mod events;
pub mod models;
pub mod services;
pub mod utils;

/// 应用全局状态，通过 tauri::State 在命令中共享
pub struct AppState {
    pub db: Arc<crate::db::Database>,
    pub config: Arc<tokio::sync::Mutex<crate::config::ConfigManager>>,
    pub active_agents: Arc<tokio::sync::Mutex<std::collections::HashMap<String, bool>>>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // 初始化应用数据目录
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("无法获取应用数据目录");
            std::fs::create_dir_all(&app_data_dir).expect("无法创建应用数据目录");

            // 初始化数据库
            let db_path = app_data_dir.join("docagent.db");
            let database = crate::db::Database::new(&db_path)
                .expect("数据库初始化失败");

            // 初始化配置管理器
            let config_manager = crate::config::ConfigManager::new(app_data_dir);

            // 创建应用状态
            let state = AppState {
                db: Arc::new(database),
                config: Arc::new(tokio::sync::Mutex::new(config_manager)),
                active_agents: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            };

            app.manage(state);

            // 初始化日志
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
                .format_timestamp_millis()
                .init();

            log::info!("DocAgent 应用初始化完成");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // LLM 命令
            commands::llm::test_connection,
            commands::llm::list_providers,
            commands::llm::add_provider,
            commands::llm::update_provider,
            commands::llm::delete_provider,
            commands::llm::set_default_provider,
            // 会话命令
            commands::session::create_session,
            commands::session::list_sessions,
            commands::session::get_session,
            commands::session::delete_session,
            commands::session::update_session_title,
            // 工作区命令
            commands::workspace::list_workspaces,
            commands::workspace::add_workspace,
            commands::workspace::remove_workspace,
            commands::workspace::set_active_workspace,
            commands::workspace::get_file_tree,
            commands::workspace::search_files,
            // 文档命令
            commands::document::preview_document,
            commands::document::get_document_versions,
            commands::document::rollback_version,
            // Skill 命令
            commands::skill::list_skills,
            commands::skill::toggle_skill,
            commands::skill::add_custom_skill,
            commands::skill::delete_custom_skill,
            // 设置命令
            commands::settings::get_settings,
            commands::settings::update_settings,
            // Agent 命令
            commands::agent::start_agent,
            commands::agent::stop_agent,
            commands::agent::confirm_operation,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
