use hbb_common::{log, tokio};
use futures_util::{StreamExt, SinkExt};
use serde_json::json;
use std::sync::Mutex;
use std::time::Duration;
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

/// Tính thời gian retry với exponential backoff (5s → 300s max)
fn backoff_delay(attempt: u32) -> Duration {
    let secs = (5u64 * 2u64.saturating_pow(attempt)).min(300);
    Duration::from_secs(secs)
}

pub async fn start_company_agent() {
    log::info!("P204 Company Agent starting...");

    // Tạo kênh chat PERSISTENT — không reset khi reconnect
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    *OUTGOING_CHAT.lock().unwrap() = Some(tx);

    let mut attempt: u32 = 0;

    loop {
        // Đợi trước khi kết nối (dùng backoff)
        let delay = backoff_delay(attempt);
        if attempt > 0 {
            log::info!("Reconnect attempt #{} in {}s...", attempt, delay.as_secs());
        }
        tokio::time::sleep(delay).await;

        // Đọc config từ registry/local storage
        let (seat_id, client_token) = match get_config_from_registry() {
            Some(cfg) => cfg,
            None => {
                log::debug!("P204: Not yet configured. Waiting...");
                attempt = 0;
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }
        };

        log::info!("P204: Seat={}, connecting...", seat_id);

        // Đảm bảo autostart
        ensure_autostart_registry(true);

        let hostname = crate::common::hostname();
        let ws_url = crate::p204_config::get_ws_url();

        log::info!("P204 WS URL: {}", ws_url);

        match connect_async(&ws_url).await {
            Ok((ws_stream, _)) => {
                log::info!("P204: WebSocket connected");
                attempt = 0; // Reset backoff khi thành công

                let (mut write, mut read) = ws_stream.split();

                // Engine.IO handshake
                if let Some(Ok(Message::Text(text))) = read.next().await {
                    if text.starts_with('0') {
                        log::debug!("EIO handshake OK");
                        let _ = write.send(Message::Text("40".to_string())).await;
                    }
                }

                let rustdesk_id = hbb_common::config::Config::get_id();
                let rustdesk_pass = hbb_common::password_security::temporary_password();

                // Gửi join-company (gồm password 1 lần duy nhất)
                let join_payload = json!([
                    "join-company",
                    {
                        "client_token": client_token,
                        "seat_id": &seat_id,
                        "rustdesk_id": rustdesk_id,
                        "rustdesk_pass": rustdesk_pass,
                        "hostname": &hostname
                    }
                ]);
                let join_msg = format!("42{}", join_payload.to_string());
                if write.send(Message::Text(join_msg)).await.is_err() {
                    continue;
                }

                // ── Main loop ──────────────────
                let mut heartbeat = tokio::time::interval(Duration::from_secs(25));
                let mut eio_ping = tokio::time::interval(Duration::from_secs(20));

                loop {
                    tokio::select! {
                        // Heartbeat: chỉ gửi seat_id, KHÔNG gửi password
                        _ = heartbeat.tick() => {
                            let hb = json!(["heartbeat", {"seat_id": &seat_id}]);
                            let msg = format!("42{}", hb.to_string());
                            if write.send(Message::Text(msg)).await.is_err() {
                                log::warn!("P204: Heartbeat send failed");
                                break;
                            }
                        }

                        // Engine.IO ping/pong
                        _ = eio_ping.tick() => {
                            if write.send(Message::Text("2".to_string())).await.is_err() {
                                break;
                            }
                        }

                        // Outgoing chat
                        out_msg = rx.recv() => {
                            if let Some(text) = out_msg {
                                let payload = json!(["chat-message", {
                                    "sender": &seat_id,
                                    "message": text
                                }]);
                                let msg = format!("42{}", payload.to_string());
                                if write.send(Message::Text(msg)).await.is_err() {
                                    break;
                                }
                            }
                        }

                        // Incoming messages
                        msg = read.next() => {
                            match msg {
                                Some(Ok(Message::Text(ref text))) => {
                                    if text == "2" || text.starts_with('2') && text.len() == 1 {
                                        // EIO ping → pong
                                        let _ = write.send(Message::Text("3".to_string())).await;
                                    } else if text.starts_with("42") {
                                        let json_str = &text[2..];
                                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                                            if let Some(arr) = val.as_array() {
                                                if arr.len() >= 2 {
                                                    match arr[0].as_str() {
                                                        Some("chat-message") => {
                                                            let payload = &arr[1];
                                                            if let (Some(s), Some(m)) = (
                                                                payload.get("sender").and_then(|v| v.as_str()),
                                                                payload.get("message").and_then(|v| v.as_str())
                                                            ) {
                                                                crate::company_chat::handle_incoming_chat(s, m);
                                                            }
                                                        }
                                                        Some("revoke-key") => {
                                                            log::error!("P204: Key revoked by admin!");
                                                            hbb_common::config::LocalConfig::set_option(
                                                                "P204_SeatID".to_string(), "".to_string()
                                                            );
                                                            hbb_common::config::LocalConfig::set_option(
                                                                "P204_Token".to_string(), "".to_string()
                                                            );
                                                            ensure_autostart_registry(false);
                                                            break;
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Some(Err(e)) => {
                                    log::error!("P204 WS read error: {}", e);
                                    break;
                                }
                                None => {
                                    log::warn!("P204: WS closed by server");
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("P204: WS connect failed: {}", e);
            }
        }

        attempt += 1;
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

#[cfg(target_os = "windows")]
fn ensure_autostart_registry(enable: bool) {
    use winreg::enums::*;
    if let Ok(hkcu) = winreg::RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            KEY_WRITE,
        )
    {
        let key_name = "P204_RustDesk_Agent";
        if enable {
            let exe_path = std::env::current_exe().unwrap_or_default();
            let cmd = format!("\"{}\" --tray", exe_path.display());
            let _ = hkcu.set_value(key_name, &cmd);
        } else {
            let _ = hkcu.delete_value(key_name);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn ensure_autostart_registry(_enable: bool) {}

