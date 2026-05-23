#[cfg(desktop)]
use crate::errors::{
    CommandError, UPDATE_CHECK_FAILED, UPDATE_DOWNLOAD_FAILED, UPDATE_INSTALL_FAILED,
    UPDATE_NO_UPDATE_AVAILABLE,
};
#[cfg(desktop)]
use serde::Serialize;
#[cfg(desktop)]
use tauri_plugin_updater::UpdaterExt;

/// 更新信息
#[cfg(desktop)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// 新版本号
    pub version: String,
    /// 发布日期
    pub date: Option<String>,
    /// 更新说明
    pub body: Option<String>,
}

/// 下载进度事件
#[cfg(desktop)]
#[derive(Clone, Serialize)]
#[serde(tag = "event", content = "data")]
#[serde(rename_all = "camelCase")]
pub enum DownloadEvent {
    /// 下载进度
    Progress {
        downloaded: u64,
        content_length: Option<u64>,
    },
    /// 下载完成
    Finished,
}

/// 检查更新
#[cfg(desktop)]
#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, CommandError> {
    let updater = app
        .updater()
        .map_err(|e| CommandError::update(UPDATE_CHECK_FAILED, e.to_string()))?;

    let update = updater.check().await.map_err(|e| {
        log::warn!("更新检查失败: {}", e);
        CommandError::update(UPDATE_CHECK_FAILED, e.to_string())
    })?;

    match update {
        Some(update) => {
            log::info!("发现新版本: {}", update.version);
            Ok(Some(UpdateInfo {
                version: update.version,
                date: update.date.map(|d| d.to_string()),
                body: update.body,
            }))
        }
        None => {
            log::info!("当前已是最新版本");
            Ok(None)
        }
    }
}

/// 下载并安装更新（通过 Channel 推送进度）
#[cfg(desktop)]
#[tauri::command]
pub async fn download_and_install_update(
    app: tauri::AppHandle,
    on_event: tauri::ipc::Channel<DownloadEvent>,
) -> Result<(), CommandError> {
    let updater = app
        .updater()
        .map_err(|e| CommandError::update(UPDATE_DOWNLOAD_FAILED, e.to_string()))?;

    let update = updater.check().await.map_err(|e| {
        CommandError::update(UPDATE_DOWNLOAD_FAILED, e.to_string())
    })?;

    let update = update.ok_or_else(|| {
        CommandError::update(UPDATE_NO_UPDATE_AVAILABLE, "没有可用的更新")
    })?;

    let mut downloaded: u64 = 0;
    let mut content_length: Option<u64> = None;

    update
        .download_and_install(
            |chunk_length, content_len| {
                downloaded += chunk_length as u64;
                content_length = content_len;
                let _ = on_event.send(DownloadEvent::Progress {
                    downloaded,
                    content_length: content_len,
                });
            },
            || {
                let _ = on_event.send(DownloadEvent::Finished);
            },
        )
        .await
        .map_err(|e| {
            log::error!("更新安装失败: {}", e);
            CommandError::update(UPDATE_INSTALL_FAILED, e.to_string())
        })?;

    log::info!("更新安装完成，准备重启");
    Ok(())
}
