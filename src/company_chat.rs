use hbb_common::log;
use std::sync::Mutex;

lazy_static::lazy_static! {
    /// Ring buffer lưu 100 message gần nhất — không mất khi dialog đóng
    static ref CHAT_HISTORY: Mutex<Vec<serde_json::Value>> = Mutex::new(Vec::new());
}

pub fn handle_incoming_chat(sender: &str, message: &str) {
    let now = chrono::Local::now();
    let time_str = now.format("%H:%M").to_string();

    log::info!("Received company chat from {}: {}", sender, message);

    let event = serde_json::json!({
        "name": "company_chat",
        "sender": sender,
        "message": message,
        "time": time_str
    });

    // Lưu vào buffer
    if let Ok(mut hist) = CHAT_HISTORY.lock() {
        hist.push(event.clone());
        if hist.len() > 100 {
            hist.remove(0);
        }
    }

    // Push real-time đến Flutter UI (nếu dialog đang mở sẽ nhận được)
    if let Ok(json_str) = serde_json::to_string(&event) {
        crate::flutter::push_global_event(crate::flutter::APP_TYPE_MAIN, json_str);
    }
}

/// Trả về toàn bộ lịch sử chat (gọi từ Flutter khi mở dialog)
pub fn get_chat_history() -> Vec<serde_json::Value> {
    CHAT_HISTORY.lock().map(|h| h.clone()).unwrap_or_default()
}
