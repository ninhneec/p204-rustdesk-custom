// Cấu hình cứng (Hardcoded Config) cho P204
// Khi nhân viên tải App về, App sẽ tự động ăn theo IP và Key này!

// ĐIỀN ĐỊA CHỈ IP CỦA VPS VÀO ĐÂY (Ví dụ: "103.123.456.78")
pub const VPS_IP: &str = "3.112.213.199"; 

// ĐIỀN PUBLIC KEY CỦA HỆ THỐNG RUSTDESK (hbbs) VÀO ĐÂY
pub const VPS_KEY: &str = "&str = "Giu6bdpzBCOWsXPkgvVS6TbG6QGh6zbdQiRvsZDJat4=";

// Cấu hình WebSocket của Node.js
pub fn get_ws_url() -> String {
    if VPS_IP == "YOUR_VPS_IP" {
        return "ws://127.0.0.1:3000/socket.io/?EIO=4&transport=websocket".to_string();
    }
    format!("ws://{}:3000/socket.io/?EIO=4&transport=websocket", VPS_IP)
}
