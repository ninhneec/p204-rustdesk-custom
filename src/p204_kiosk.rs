// P204 Kiosk Mode - Strict lockdown for managed machines
// Khi đã đăng ký P204 → khóa toàn bộ cài đặt, không cho thoát app

use hbb_common::log;

lazy_static::lazy_static! {
    /// Admin password để mở khóa (hash SHA256)
    static ref ADMIN_UNLOCK_HASH: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
    /// Trạng thái: đang bị khóa?
    static ref LOCKED: std::sync::Mutex<bool> = std::sync::Mutex::new(false);
}

/// Hash SHA256 từ password
fn hash_password(pass: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(pass.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Kiểm tra máy đã đăng ký P204 chưa → nếu có thì bật kiosk
pub fn is_registered() -> bool {
    let seat_id = hbb_common::config::LocalConfig::get_option("P204_SeatID");
    !seat_id.is_empty()
}

/// Bật chế độ khóa
pub fn lock(admin_password: &str) {
    if let Ok(mut hash) = ADMIN_UNLOCK_HASH.lock() {
        *hash = hash_password(admin_password);
    }
    if let Ok(mut locked) = LOCKED.lock() {
        *locked = true;
    }
    log::info!("P204 Kiosk: Machine LOCKED");
}

/// Mở khóa với admin password
pub fn unlock(password: &str) -> bool {
    let hash = ADMIN_UNLOCK_HASH.lock().unwrap_or_else(|e| e.into_inner());
    if hash.is_empty() || hash_password(password) == *hash {
        if let Ok(mut locked) = LOCKED.lock() {
            *locked = false;
        }
        log::info!("P204 Kiosk: Machine UNLOCKED");
        return true;
    }
    log::warn!("P204 Kiosk: Unlock attempt with wrong password");
    false
}

/// Đang trong trạng thái khóa?
pub fn is_locked() -> bool {
    LOCKED.lock().map(|l| *l).unwrap_or(false)
}

/// Khóa từ xa (từ server gửi lệnh)
pub fn remote_lock(admin_password_hash: &str) {
    if let Ok(mut hash) = ADMIN_UNLOCK_HASH.lock() {
        *hash = admin_password_hash.to_string();
    }
    if let Ok(mut locked) = LOCKED.lock() {
        *locked = true;
    }
    log::info!("P204 Kiosk: Remote LOCK activated");
}

/// Mở khóa từ xa
pub fn remote_unlock() {
    if let Ok(mut locked) = LOCKED.lock() {
        *locked = false;
    }
    if let Ok(mut hash) = ADMIN_UNLOCK_HASH.lock() {
        *hash = String::new();
    }
    log::info!("P204 Kiosk: Remote UNLOCK");
}

/// Kiểm tra có được phép quit không
pub fn can_quit() -> bool {
    // Nếu chưa đăng ký P204 → cho phép quit bình thường
    if !is_registered() {
        return true;
    }
    // Nếu đã đăng ký nhưng chưa khóa → vẫn cho quit (chờ admin khóa)
    // Nếu đã khóa → phải unlock mới được quit
    !is_locked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_password() {
        let h = hash_password("admin123");
        assert_eq!(h, hash_password("admin123"));
        assert_ne!(h, hash_password("wrong"));
    }
}
