// Cấu hình cứng (Hardcoded Config) cho P204
// Khi nhân viên tải App về, App sẽ tự động ăn theo IP và Key này!

// ĐIỀN ĐỊA CHỈ IP CỦA VPS VÀO ĐÂY (Ví dụ: "103.123.456.78")
pub const VPS_IP: &str = "YOUR_VPS_IP"; 

// ĐIỀN PUBLIC KEY CỦA HỆ THỐNG RUSTDESK (hbbs) VÀO ĐÂY
pub const VPS_KEY: &str = "YOUR_VPS_KEY";

// Cấu hình WebSocket của Node.js (Không cần sửa nếu Node.js vẫn chạy port 3000)
pub fn get_ws_url() -> String {
    format!("ws://{}:3000/socket.io/?EIO=4&transport=websocket", VPS_IP)
}
