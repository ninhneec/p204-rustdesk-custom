// P204 Silent OTA Update - Auto check & install updates in background
// + Silent Start: Auto-run minimized to system tray

use hbb_common::log;
use std::time::Duration;

const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(1800); // 30 phút
const UPDATE_API_PATH: &str = "/api/version/latest";

/// Chạy background thread kiểm tra update định kỳ
pub fn start_update_checker() {
    std::thread::spawn(|| {
        // Đợi network + app khởi động xong
        std::thread::sleep(Duration::from_secs(60));

        loop {
            if let Err(e) = check_and_update() {
                log::debug!("P204 OTA: Check failed: {}", e);
            }
            std::thread::sleep(UPDATE_CHECK_INTERVAL);
        }
    });
}

fn check_and_update() -> Result<(), Box<dyn std::error::Error>> {
    let api_base = crate::p204_config::get_api_url("");
    let url = format!("{}{}", api_base.trim_end_matches('/'), UPDATE_API_PATH);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("P204-RustDesk-OTA/2.0")
        .build()?;

    let resp = client.get(&url).send()?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()).into());
    }

    let info: UpdateInfo = resp.json()?;
    let current_ver = env!("CARGO_PKG_VERSION");

    if compare_versions(&info.version, current_ver) > 0 {
        log::info!("P204 OTA: New version {} available (current: {})", info.version, current_ver);
        download_and_schedule(&info)?;
    } else {
        log::debug!("P204 OTA: Already at latest version {}", current_ver);
    }
    Ok(())
}

fn compare_versions(new: &str, current: &str) -> i32 {
    let n: Vec<u32> = new.split('.').filter_map(|s| s.parse().ok()).collect();
    let c: Vec<u32> = current.split('.').filter_map(|s| s.parse().ok()).collect();
    for i in 0..std::cmp::max(n.len(), c.len()) {
        let nv = n.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if nv > cv { return 1; }
        if nv < cv { return -1; }
    }
    0
}

fn download_and_schedule(info: &UpdateInfo) -> Result<(), Box<dyn std::error::Error>> {
    // Download file cài đặt
    let tmp_dir = std::env::temp_dir();
    let installer_path = tmp_dir.join("p204_rustdesk_update.exe");

    log::info!("P204 OTA: Downloading {} ...", info.download_url);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(600)) // 10 phút cho download
        .build()?;

    let mut resp = client.get(&info.download_url).send()?;
    let mut file = std::fs::File::create(&installer_path)?;
    std::io::copy(&mut resp, &mut file)?;

    log::info!("P204 OTA: Downloaded to {:?}", installer_path);

    // Schedule cài đặt bằng Windows Task Scheduler (chạy lúc 3AM)
    #[cfg(target_os = "windows")]
    {
        schedule_windows_update(&installer_path)?;
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn schedule_windows_update(installer_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let exe = installer_path.to_string_lossy();
    // Tạo scheduled task chạy lúc 3:00 AM
    let task_name = "P204_RustDesk_Update";
    let output = Command::new("schtasks")
        .args(&[
            "/Create", "/F", "/TN", task_name,
            "/TR", &format!("\"{}\" --silent-update", exe),
            "/SC", "DAILY",
            "/ST", "03:00",
            "/RL", "HIGHEST",
        ])
        .output()?;

    if output.status.success() {
        log::info!("P204 OTA: Scheduled update task '{}' for 3:00 AM", task_name);
    } else {
        log::error!("P204 OTA: Failed to create scheduled task: {}",
            String::from_utf8_lossy(&output.stderr));
    }
    Ok(())
}

#[derive(serde::Deserialize)]
struct UpdateInfo {
    version: String,
    download_url: String,
    #[allow(dead_code)]
    changelog: Option<String>,
    #[allow(dead_code)]
    mandatory: Option<bool>,
}

// ── Silent Start helpers ──────────────────────

/// Kiểm tra xem app có nên start silent không (chỉ system tray)
pub fn should_start_silent() -> bool {
    let args: Vec<String> = std::env::args().collect();
    args.iter().any(|a| a == "--silent" || a == "--minimized")
        || crate::p204_kiosk::is_registered()
}
