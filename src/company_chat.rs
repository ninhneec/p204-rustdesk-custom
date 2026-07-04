use hbb_common::log;

pub fn handle_incoming_chat(sender: &str, message: &str) {
    let now = chrono::Local::now();
    let time_str = now.format("%H:%M").to_string();
    
    // We will push a global event to Flutter UI.
    // The event will be named "company_chat"
    log::info!("Received company chat from {}: {}", sender, message);
    
    let event = serde_json::json!({
        "name": "company_chat",
        "sender": sender,
        "message": message,
        "time": time_str
    });
    
    // We send it to the UI via global event channel (like other UI events).
    if let Ok(json_str) = serde_json::to_string(&event) {
        crate::flutter::push_global_event(crate::flutter::APP_TYPE_MAIN, json_str);
    }
}
