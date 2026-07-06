// Cấu hình cứng (Hardcoded Config) cho P204
// Khi nhân viên tải App về, App sẽ tự động ăn theo IP và Key này!

// ĐIỀN ĐỊA CHỈ IP CỦA VPS VÀO ĐÂY (Ví dụ: "103.123.456.78")
pub const VPS_IP: &str = "ad.apndocs.site"; 

// ĐIỀN PUBLIC KEY CỦA HỆ THỐNG RUSTDESK (hbbs) VÀO ĐÂY
pub const VPS_KEY: &str = "sb+u5mEGKomZBM28fp6BfZdOmnrp8KPMrtNnGy291zc=";

// Cấu hình WebSocket của Node.js
pub fn get_ws_url() -> String {
    if VPS_IP == "YOUR_VPS_IP" {
        return "ws://127.0.0.1:3000/socket.io/?EIO=4&transport=websocket".to_string();
    }
    format!("ws://{}:3000/socket.io/?EIO=4&transport=websocket", VPS_IP)
}
