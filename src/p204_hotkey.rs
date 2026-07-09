// P204 Global Hotkey + Secret Verify Panel
// Đăng ký phím tắt toàn cục Ctrl+Alt+O/P/D để mở popup xác minh

use hbb_common::log;

#[cfg(target_os = "windows")]
mod win {
    use winapi::shared::windef::HWND;
    use winapi::shared::minwindef::{UINT, WPARAM, LPARAM};

    static mut HOTKEY_IDS: Vec<i32> = Vec::new();

    pub fn register_hotkeys(hwnd: isize) {
        unsafe {
            use winapi::um::winuser::{
                RegisterHotKey, MOD_CONTROL, MOD_ALT, MOD_NOREPEAT,
                VK_O, VK_P, VK_D,
            };

            // Ctrl+Alt+O → Open verify panel
            let id1 = 0x2040;
            if RegisterHotKey(hwnd as HWND, id1, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, VK_O as UINT) != 0 {
                HOTKEY_IDS.push(id1);
                log::info!("P204 Hotkey: Ctrl+Alt+O registered");
            }

            // Ctrl+Alt+P → Quick password check
            let id2 = 0x2041;
            if RegisterHotKey(hwnd as HWND, id2, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, VK_P as UINT) != 0 {
                HOTKEY_IDS.push(id2);
                log::info!("P204 Hotkey: Ctrl+Alt+P registered");
            }

            // Ctrl+Alt+D → Diagnostic panel
            let id3 = 0x2042;
            if RegisterHotKey(hwnd as HWND, id3, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, VK_D as UINT) != 0 {
                HOTKEY_IDS.push(id3);
                log::info!("P204 Hotkey: Ctrl+Alt+D registered");
            }
        }
    }

    pub fn unregister_hotkeys(hwnd: isize) {
        unsafe {
            use winapi::um::winuser::UnregisterHotKey;
            for id in &HOTKEY_IDS {
                UnregisterHotKey(hwnd as HWND, *id);
            }
            HOTKEY_IDS.clear();
        }
    }

    pub fn is_p204_hotkey(msg: u32, wparam: usize) -> Option<&'static str> {
        let id = wparam as i32;
        match id {
            0x2040 => Some("verify"),
            0x2041 => Some("password"),
            0x2042 => Some("diagnostic"),
            _ => None,
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod win {
    pub fn register_hotkeys(_hwnd: isize) {}
    pub fn unregister_hotkeys(_hwnd: isize) {}
    pub fn is_p204_hotkey(_msg: u32, _wparam: usize) -> Option<&'static str> { None }
}

pub use win::*;

/// Gửi mã bí mật lên server để xác minh
pub async fn verify_secret(secret: &str) -> Result<bool, String> {
    let api_base = crate::p204_config::get_api_url("");
    let url = format!("{}/api/secrets/verify", api_base.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let seat_id = hbb_common::config::LocalConfig::get_option("P204_SeatID");
    let hostname = crate::common::hostname();

    let resp = client.post(&url)
        .json(&serde_json::json!({
            "secret": secret,
            "seat_id": seat_id,
            "hostname": hostname,
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(body.get("success").and_then(|v| v.as_bool()).unwrap_or(false))
}
