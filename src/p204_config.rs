// Cấu hình P204 - Single Source of Truth
// Ưu tiên: Cloud Config (GitHub) → Hardcoded Fallback

/// Fallback VPS address (dùng khi không tải được cloud config)
const FALLBACK_VPS: &str = "ad.apndocs.site";

/// Fallback Public Key
const FALLBACK_KEY: &str = "sb+u5mEGKomZBM28fp6BfZdOmnrp8KPMrtNnGy291zc=";

pub fn get_ws_url() -> String {
    if crate::cloud_config::CloudConfigData::is_loaded() {
        let url = crate::cloud_config::CloudConfigData::get_ws_url();
        if !url.contains("127.0.0.1") && !url.contains("localhost") {
            return url;
        }
    }
    format!("ws://{}:3000/socket.io/?EIO=4&transport=websocket", FALLBACK_VPS)
}

pub fn get_api_url(path: &str) -> String {
    if crate::cloud_config::CloudConfigData::is_loaded() {
        let url = crate::cloud_config::CloudConfigData::get_api_url(path);
        if !url.contains("127.0.0.1") && !url.contains("localhost") {
            return url;
        }
    }
    format!("http://{}:3000{}", FALLBACK_VPS, path)
}

pub fn get_rendezvous_server() -> String {
    let cloud = crate::cloud_config::CloudConfigData::get_rendezvous_server();
    if !cloud.is_empty() { cloud } else { FALLBACK_VPS.to_string() }
}

pub fn get_public_key() -> String {
    let cloud = crate::cloud_config::CloudConfigData::get_public_key();
    if !cloud.is_empty() { cloud } else { FALLBACK_KEY.to_string() }
}


