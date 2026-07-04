use hbb_common::{log, ResultType, tokio};
use futures_util::{StreamExt, SinkExt};
use serde_json::json;
use std::time::Duration;
use std::sync::Mutex;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

lazy_static::lazy_static! {
    static ref OUTGOING_CHAT: Mutex<Option<mpsc::UnboundedSender<String>>> = Mutex::new(None);
}

pub fn send_chat_to_server(message: String) {
    if let Some(tx) = OUTGOING_CHAT.lock().unwrap().as_ref() {
        let _ = tx.send(message);
    }
}


pub async fn start_company_agent() {
    log::info!("Starting Company Agent...");
    
    let mut connect_interval = tokio::time::interval(Duration::from_secs(5));
    
    loop {
        connect_interval.tick().await;

        // Try reading the machine's assigned seat and token from the Windows Registry
        let (seat_id, client_token) = match get_config_from_registry() {
            Some(cfg) => cfg,
            None => {
                log::debug!("P204Remote not yet configured. Waiting for user enrollment...");
                continue;
            }
        };

        log::info!("P204 Configured - Seat ID: {}", seat_id);
        
        // Cài đặt auto-start registry
        ensure_autostart_registry(true);

        let hostname = crate::common::hostname();
        
        let ws_url = "ws://127.0.0.1:3000/socket.io/?EIO=4&transport=websocket";
        log::info!("Connecting to P204 Management Server: {}", ws_url);
        
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        *OUTGOING_CHAT.lock().unwrap() = Some(tx);
        
        match connect_async(ws_url).await {
            Ok((ws_stream, _)) => {
                log::info!("Connected to P204 Management Server!");
                let (mut write, mut read) = ws_stream.split();
                
                // Wait for Engine.IO handshake '0'
                if let Some(msg) = read.next().await {
                    if let Ok(Message::Text(text)) = msg {
                        if text.starts_with('0') {
                            log::debug!("Engine.IO Handshake OK: {}", text);
                            // Send Connect '40'
                            if let Err(e) = write.send(Message::Text("40".to_string())).await {
                                log::error!("Failed to send connect packet: {}", e);
                                continue;
                            }
                        }
                    }
                }
                
                let rustdesk_id = hbb_common::config::Config::get_id();
                let rustdesk_pass = hbb_common::password_security::temporary_password();

                // Send join-company event
                let join_payload = json!([
                    "join-company",
                    {
                        "client_token": client_token,
                        "seat_id": seat_id,
                        "rustdesk_id": rustdesk_id,
                        "rustdesk_pass": rustdesk_pass,
                        "hostname": hostname
                    }
                ]);
                let join_msg = format!("42{}", join_payload.to_string());
                if let Err(e) = write.send(Message::Text(join_msg)).await {
                    log::error!("Failed to send join-company: {}", e);
                    continue;
                }

                // Heartbeat Loop & Message Reader
                let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(25));
                let mut socket_io_ping_interval = tokio::time::interval(Duration::from_secs(20)); // Engine.IO ping '2'
                
                loop {
                    tokio::select! {
                        _ = socket_io_ping_interval.tick() => {
                            if let Err(e) = write.send(Message::Text("2".to_string())).await {
                                log::error!("Ping error: {}", e);
                                break;
                            }
                        }
                        _ = heartbeat_interval.tick() => {
                            let rustdesk_id = hbb_common::config::Config::get_id();
                            let rustdesk_pass = hbb_common::password_security::temporary_password();
                            let hb_payload = json!([
                                "heartbeat",
                                {
                                    "seat_id": &seat_id,
                                    "rustdesk_id": rustdesk_id,
                                    "rustdesk_pass": rustdesk_pass,
                                    "hostname": &hostname
                                }
                            ]);
                            let hb_msg = format!("42{}", hb_payload.to_string());
                            if let Err(e) = write.send(Message::Text(hb_msg)).await {
                                log::error!("Heartbeat error: {}", e);
                                break;
                            }
                        }
                        out_msg = rx.recv() => {
                            if let Some(msg_text) = out_msg {
                                let rustdesk_id = hbb_common::config::Config::get_id();
                                let chat_payload = json!([
                                    "chat-message",
                                    {
                                        "sender": seat_id,
                                        "message": msg_text
                                    }
                                ]);
                                let chat_msg = format!("42{}", chat_payload.to_string());
                                if let Err(e) = write.send(Message::Text(chat_msg)).await {
                                    log::error!("Chat send error: {}", e);
                                    break;
                                }
                            }
                        }
                        msg = read.next() => {
                            match msg {
                                Some(Ok(Message::Text(text))) => {
                                    if text.starts_with('3') {
                                        // Engine.IO pong '3'
                                        log::trace!("Pong received");
                                    } else if text.starts_with("42") {
                                        // Socket.IO event
                                        let json_str = &text[2..];
                                        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(json_str) {
                                            if let Some(arr) = json_val.as_array() {
                                                if arr.len() >= 2 {
                                                    if let Some(event_name) = arr[0].as_str() {
                                                        if event_name == "chat-message" {
                                                            let payload = &arr[1];
                                                            if let (Some(sender), Some(message)) = (
                                                                payload.get("sender").and_then(|v| v.as_str()),
                                                                payload.get("message").and_then(|v| v.as_str())
                                                            ) {
                                                                // Forward chat message to UI via company_chat module
                                                                crate::company_chat::handle_incoming_chat(sender, message);
                                                            }
                                                        } else if event_name == "revoke-key" {
                                                            log::error!("Token revoked by Admin! Deleting local config...");
                                                            hbb_common::config::LocalConfig::set_option("P204_SeatID".to_string(), "".to_string());
                                                            hbb_common::config::LocalConfig::set_option("P204_Token".to_string(), "".to_string());
                                                            ensure_autostart_registry(false);
                                                            // Ngắt kết nối ngay lập tức bằng cách thoát vòng lặp
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Some(Err(e)) => {
                                    log::error!("WebSocket read error: {}", e);
                                    break;
                                }
                                None => {
                                    log::warn!("WebSocket connection closed by server");
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to connect to P204 server: {}", e);
            }
        }
    }
}

fn get_config_from_registry() -> Option<(String, String)> {
    let seat_id = hbb_common::config::LocalConfig::get_option("P204_SeatID");
    let token = hbb_common::config::LocalConfig::get_option("P204_Token");
    if !seat_id.is_empty() && !token.is_empty() {
        return Some((seat_id, token));
    }
    None
}

#[allow(unused_variables)]
fn ensure_autostart_registry(enable: bool) {
    #[cfg(windows)]
    {
        if let Ok(hkcu) = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            winreg::enums::KEY_WRITE,
        ) {
            let key_name = "P204_RustDesk_Agent";
            if enable {
                let exe_path = std::env::current_exe().unwrap_or_default();
                let cmd = format!("\"{}\" --silent-agent", exe_path.display());
                let _ = hkcu.set_value(key_name, &cmd);
            } else {
                let _ = hkcu.delete_value(key_name);
            }
        }
    }
}
