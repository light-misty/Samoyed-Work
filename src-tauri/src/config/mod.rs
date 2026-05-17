pub mod llm_config;
pub mod app_settings;
pub mod workspace_config;

use std::path::{Path, PathBuf};

use crate::errors::CommandError;

/// 配置管理器，统一管理所有配置文件的读写
pub struct ConfigManager {
    /// 应用数据目录，所有配置文件存储在此目录的 config/ 子目录下
    data_dir: PathBuf,
}

impl ConfigManager {
    /// 创建配置管理器实例
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            data_dir: app_data_dir,
        }
    }

    /// 获取数据目录路径
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    // ================================================================
    // LLM 配置
    // ================================================================

    /// 加载 LLM 配置
    pub fn load_llm_config(&self) -> Result<llm_config::LlmConfig, CommandError> {
        llm_config::load_llm_config(&self.data_dir)
    }

    /// 保存 LLM 配置
    pub fn save_llm_config(&self, config: &llm_config::LlmConfig) -> Result<(), CommandError> {
        llm_config::save_llm_config(&self.data_dir, config)
    }

    /// 获取默认 Provider
    pub fn get_default_provider<'a>(
        &self,
        config: &'a llm_config::LlmConfig,
    ) -> Option<&'a llm_config::LlmProvider> {
        llm_config::get_default_provider(config)
    }

    /// 添加 Provider
    pub fn add_provider(
        &self,
        config: &mut llm_config::LlmConfig,
        provider: llm_config::LlmProvider,
    ) -> Result<(), CommandError> {
        llm_config::add_provider(config, provider)
    }

    /// 更新 Provider
    pub fn update_provider(
        &self,
        config: &mut llm_config::LlmConfig,
        id: &str,
        provider: llm_config::LlmProvider,
    ) -> Result<(), CommandError> {
        llm_config::update_provider(config, id, provider)
    }

    /// 删除 Provider
    pub fn delete_provider(
        &self,
        config: &mut llm_config::LlmConfig,
        id: &str,
    ) -> Result<(), CommandError> {
        llm_config::delete_provider(config, id)
    }

    /// 设置默认 Provider
    pub fn set_default_provider(
        &self,
        config: &mut llm_config::LlmConfig,
        id: &str,
    ) -> Result<(), CommandError> {
        llm_config::set_default_provider(config, id)
    }

    // ================================================================
    // 应用设置
    // ================================================================

    /// 加载应用设置
    pub fn load_app_settings(&self) -> Result<app_settings::AppSettings, CommandError> {
        app_settings::load_app_settings(&self.data_dir)
    }

    /// 保存应用设置
    pub fn save_app_settings(
        &self,
        settings: &app_settings::AppSettings,
    ) -> Result<(), CommandError> {
        app_settings::save_app_settings(&self.data_dir, settings)
    }

    // ================================================================
    // 工作区配置
    // ================================================================

    /// 加载工作区配置
    pub fn load_workspaces(&self) -> Result<workspace_config::WorkspacesConfig, CommandError> {
        workspace_config::load_workspaces(&self.data_dir)
    }

    /// 保存工作区配置
    pub fn save_workspaces(
        &self,
        config: &workspace_config::WorkspacesConfig,
    ) -> Result<(), CommandError> {
        workspace_config::save_workspaces(&self.data_dir, config)
    }

    /// 添加工作区
    pub fn add_workspace(
        &self,
        config: &mut workspace_config::WorkspacesConfig,
        path: &str,
        name: &str,
    ) -> Result<workspace_config::WorkspaceEntry, CommandError> {
        workspace_config::add_workspace(config, path, name)
    }

    /// 移除工作区
    pub fn remove_workspace(
        &self,
        config: &mut workspace_config::WorkspacesConfig,
        id: &str,
    ) -> Result<(), CommandError> {
        workspace_config::remove_workspace(config, id)
    }
}
