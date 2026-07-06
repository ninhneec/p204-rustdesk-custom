use lazy_static::lazy_static;
use std::sync::Mutex;
use serde_derive::{Deserialize, Serialize};

lazy_static! {
    static ref CLOUD_CONFIG: Mutex<Option<CloudConfigData>> = Mutex::new(None);
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CloudConfigData {
    #[serde(default)]
    pub rendezvous_server: String,
    #[serde(default)]
    pub api_server: String,
    #[serde(default)]
    pub ws_server: String,
    #[serde(default)]
    pub public_key: String,
}

// URLs để fetch cloud config
const CLOUD_CONFIG_URLS: &[&str] = &[
    "https://raw.githubusercontent.com/ninhneec/p204-rustdesk-custom/master/cloud_config.json",
];

impl CloudConfigData {
    /// Lấy bản sao của cloud config hiện tại
    pub fn get() -> Option<CloudConfigData> {
        CLOUD_CONFIG.lock().ok()?.clone()
    }

    /// Kiểm tra xem cloud config đã được tải thành công chưa
    pub fn is_loaded() -> bool {
        CLOUD_CONFIG.lock().ok().map(|c| {
            c.as_ref().map(|d| !d.api_server.is_empty()).unwrap_or(false)
        }).unwrap_or(false)
    }

    /// Tạo WebSocket URL từ config (có fallback)
    pub fn get_ws_url() -> String {
        let cfg = CLOUD_CONFIG.lock().ok().and_then(|c| c.clone());
        match cfg {
            Some(ref c) if !c.ws_server.is_empty() => {
                format!("{}/socket.io/?EIO=4&transport=websocket", c.ws_server.trim_end_matches('/'))
            }
            Some(ref c) if !c.api_server.is_empty() => {
                let base = c.api_server
                    .replace("https://", "wss://")
                    .replace("http://", "ws://");
                format!("{}/socket.io/?EIO=4&transport=websocket", base.trim_end_matches('/'))
            }
            _ => "ws://127.0.0.1:3000/socket.io/?EIO=4&transport=websocket".to_string(),
        }
    }

    /// Tạo API URL cho path cụ thể
    pub fn get_api_url(path: &str) -> String {
        let base = CLOUD_CONFIG.lock().ok()
            .and_then(|c| c.clone())
            .and_then(|c| if c.api_server.is_empty() { None } else { Some(c.api_server) })
            .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());
        format!("{}{}", base.trim_end_matches('/'), path)
    }

    /// Lấy rendezvous server
    pub fn get_rendezvous_server() -> String {
        CLOUD_CONFIG.lock().ok()
            .and_then(|c| c.clone())
            .and_then(|c| if c.rendezvous_server.is_empty() { None } else { Some(c.rendezvous_server) })
            .unwrap_or_default()
    }

    /// Lấy public key
    pub fn get_public_key() -> String {
        CLOUD_CONFIG.lock().ok()
            .and_then(|c| c.clone())
            .and_then(|c| if c.public_key.is_empty() { None } else { Some(c.public_key) })
            .unwrap_or_default()
    }
}

/// Gọi từ global_init - chạy trên thread riêng, dùng blocking HTTP
pub fn init() {
    std::thread::spawn(|| {
        // Đợi network sẵn sàng
        std::thread::sleep(std::time::Duration::from_secs(2));

        for url in CLOUD_CONFIG_URLS {
            log::info!("Fetching cloud config from {}", url);
            match reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .user_agent("P204-RustDesk/2.0")
                .build()
            {
                Ok(client) => {
                    match client.get(*url).send() {
                        Ok(resp) if resp.status().is_success() => {
                            match resp.json::<CloudConfigData>() {
                                Ok(data) => {
                                    log::info!("Cloud config loaded: {:?}", data);
                                    if let Ok(mut cfg) = CLOUD_CONFIG.lock() {
                                        *cfg = Some(data);
                                    }
                                    return;
                                }
                                Err(e) => log::error!("Failed to parse cloud config from {}: {}", url, e),
                            }
                        }
                        Ok(resp) => log::error!("Cloud config fetch failed: HTTP {} from {}", resp.status(), url),
                        Err(e) => log::error!("Failed to fetch cloud config from {}: {}", url, e),
                    }
                }
                Err(e) => log::error!("Failed to build HTTP client: {}", e),
            }
        }
        log::warn!("Could not load cloud config from any source, using fallbacks");
    });
}

