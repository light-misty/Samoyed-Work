use std::sync::Arc;
use std::time::Duration;

use tauri::{Emitter, Runtime};
use tokio::sync::RwLock;
use tokio::time::sleep;

use crate::events::emitter::AgentEmitter;
use crate::events::types::{NetworkChangePayload, SYSTEM_NETWORK_CHANGE};
use crate::services::llm::router::LlmRouter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkStatus {
    Online,
    Offline,
}

impl NetworkStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            NetworkStatus::Online => "online",
            NetworkStatus::Offline => "offline",
        }
    }
}

pub struct NetworkMonitor<R: Runtime> {
    status: Arc<RwLock<NetworkStatus>>,
    router: Arc<RwLock<Arc<LlmRouter>>>,
    emitter: AgentEmitter<R>,
    offline_count: Arc<RwLock<u32>>,
}

impl<R: Runtime> NetworkMonitor<R> {
    pub fn new(router: Arc<RwLock<Arc<LlmRouter>>>, emitter: AgentEmitter<R>) -> Self {
        Self {
            status: Arc::new(RwLock::new(NetworkStatus::Online)),
            router,
            emitter,
            offline_count: Arc::new(RwLock::new(0)),
        }
    }

    pub fn start(&self) {
        let status = self.status.clone();
        let router = self.router.clone();
        let emitter = self.emitter.clone();
        let offline_count = self.offline_count.clone();

        tokio::spawn(async move {
            loop {
                let current_status = status.read().await.clone();
                let is_online = Self::check_network().await;

                if is_online {
                    let mut count = offline_count.write().await;
                    *count = 0;

                    if current_status == NetworkStatus::Offline {
                        log::info!("网络已恢复，触发Provider恢复和连接池重建");
                        *status.write().await = NetworkStatus::Online;

                        // 发射网络恢复事件
                        let payload = NetworkChangePayload {
                            status: NetworkStatus::Online.as_str().to_string(),
                            previous_status: NetworkStatus::Offline.as_str().to_string(),
                        };
                        let _ = emitter
                            .app_handle_ref()
                            .emit(SYSTEM_NETWORK_CHANGE, &payload);

                        // 触发Provider恢复和连接池重建
                        let router_snap = router.read().await.clone();
                        router_snap.force_recover_all().await;
                        router_snap.rebuild_all_clients().await;
                    }
                } else {
                    let mut count = offline_count.write().await;
                    *count += 1;

                    // 连续2次检测失败才判定为离线，避免误报
                    if *count >= 2 && current_status == NetworkStatus::Online {
                        log::warn!("网络连接断开");
                        *status.write().await = NetworkStatus::Offline;

                        let payload = NetworkChangePayload {
                            status: NetworkStatus::Offline.as_str().to_string(),
                            previous_status: NetworkStatus::Online.as_str().to_string(),
                        };
                        let _ = emitter
                            .app_handle_ref()
                            .emit(SYSTEM_NETWORK_CHANGE, &payload);
                    }
                }

                sleep(Duration::from_secs(10)).await;
            }
        });
    }

    async fn check_network() -> bool {
        let dns_check = Self::check_dns("dns.google").await;
        if dns_check {
            return true;
        }

        let dns_check2 = Self::check_dns("www.baidu.com").await;
        if dns_check2 {
            return true;
        }

        Self::check_tcp("8.8.8.8", 53).await
    }

    async fn check_dns(host: &str) -> bool {
        match tokio::net::lookup_host(format!("{}:80", host)).await {
            Ok(_) => true,
            Err(e) => {
                log::debug!("DNS解析失败 {}: {}", host, e);
                false
            }
        }
    }

    async fn check_tcp(ip: &str, port: u16) -> bool {
        match tokio::net::TcpStream::connect((ip, port)).await {
            Ok(_) => true,
            Err(e) => {
                log::debug!("TCP连接失败 {}:{}: {}", ip, port, e);
                false
            }
        }
    }

    pub async fn get_status(&self) -> NetworkStatus {
        self.status.read().await.clone()
    }
}
