use std::sync::{Arc, Mutex};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudConfigData {
    pub rendezvous_server: Option<String>,
    pub api_server: Option<String>,
    pub public_key: Option<String>,
}

lazy_static! {
    pub static ref CLOUD_CONFIG: Arc<Mutex<Option<CloudConfigData>>> = Arc::new(Mutex::new(None));
}

pub fn init() {
    std::thread::spawn(|| {
        let url = "https://raw.githubusercontent.com/ninhneec/p204-rustdesk-custom/master/cloud_config.json";
        log::info!("Fetching cloud config from {}", url);
        match reqwest::blocking::get(url) {
            Ok(resp) => {
                if resp.status().is_success() {
                    if let Ok(text) = resp.text() {
                        match serde_json::from_str::<CloudConfigData>(&text) {
                            Ok(data) => {
                                *CLOUD_CONFIG.lock().unwrap() = Some(data.clone());
                                log::info!("Cloud config loaded successfully: {:?}", data);
                            }
                            Err(e) => {
                                log::error!("Failed to parse cloud config JSON: {}", e);
                            }
                        }
                    }
                } else {
                    log::error!("Cloud config fetch failed with status: {}", resp.status());
                }
            }
            Err(e) => {
                log::error!("Failed to fetch cloud config: {}", e);
            }
        }
    });
}
