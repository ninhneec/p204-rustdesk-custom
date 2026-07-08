#!/bin/bash
# ═══════════════════════════════════════════════
# P204 VPS Auto-Installer v2.0
# Ubuntu 20.04/22.04/24.04
# ═══════════════════════════════════════════════

set -e

echo "==========================================="
echo "🏢 P204 Remote Management - Cài đặt VPS"
echo "==========================================="

VPS_IP=$(curl -s ifconfig.me || echo "YOUR_VPS_IP")
RUSTDESK_VER="1.1.14"

# ── 1. System Update ──────────────────────────
echo "[1/6] Cập nhật hệ thống..."
sudo apt update && sudo apt upgrade -y
sudo apt install -y curl wget unzip git ufw python3 build-essential

# ── 2. Node.js 20 + PM2 ───────────────────────
echo "[2/6] Cài đặt Node.js 20 & PM2..."
if ! command -v node &> /dev/null; then
  curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
  sudo apt install -y nodejs
fi
sudo npm install -g pm2

# ── 3. Firewall ───────────────────────────────
echo "[3/6] Cấu hình tường lửa..."
sudo ufw allow 22/tcp
sudo ufw allow 3000/tcp
sudo ufw allow 21115:21119/tcp
sudo ufw allow 21116/udp
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw --force enable

# ── 4. RustDesk Server ────────────────────────
echo "[4/6] Cài đặt RustDesk Server v${RUSTDESK_VER}..."
mkdir -p /opt/rustdesk-server
cd /opt/rustdesk-server

ARCH="linux-amd64"
if [ "$(uname -m)" = "aarch64" ]; then ARCH="linux-arm64"; fi

wget -q "https://github.com/rustdesk/rustdesk-server/releases/download/${RUSTDESK_VER}/rustdesk-server-${ARCH}.zip" -O rd.zip
unzip -o rd.zip
find . -type f \( -name "hbbs" -o -name "hbbr" \) -exec mv {} . \;
chmod +x hbbs hbbr

# Lấy public key tự động
pm2 start ./hbbs --name "p204-hbbs" -- -r "$VPS_IP" -k _
pm2 start ./hbbr --name "p204-hbbr" -- -k _

# ── 5. P204 Management Server ─────────────────
echo "[5/6] Cài đặt P204 Management Server..."
mkdir -p /opt/p204-server
cp -r $HOME/p204-rustdesk-custom/server/* /opt/p204-server/
cp -r $HOME/p204-rustdesk-custom/dashboard /opt/p204-server/
cd /opt/p204-server

# Tạo .env nếu chưa có
if [ ! -f .env ]; then
  JWT_SECRET=$(node -e "console.log(require('crypto').randomBytes(32).toString('hex'))")
  cat > .env << EOF
JWT_SECRET=${JWT_SECRET}
ADMIN_USER=admin
ADMIN_PASS=Boss@2026
PORT=3000
CORS_ORIGIN=*
DB_PATH=./data/p204.db
EOF
  echo "✅ Đã tạo file .env với JWT_SECRET ngẫu nhiên"
  echo "⚠️  HÃY ĐỔI MẬT KHẨU ADMIN TRONG FILE .env NGAY!"
fi

npm install --production
pm2 start server.js --name "p204-admin"

# ── 6. PM2 autostart ──────────────────────────
pm2 save
pm2 startup

# ── Summary ───────────────────────────────────
echo ""
echo "==========================================="
echo "✅ CÀI ĐẶT THÀNH CÔNG!"
echo "==========================================="
echo "🌐 IP VPS:           $VPS_IP"
echo "📊 Dashboard:        http://${VPS_IP}:3000"
echo "📡 RustDesk Server:  ${VPS_IP}"
echo "🔑 Public Key:"
cat /opt/rustdesk-server/id_ed25519.pub 2>/dev/null || echo "  (chưa có - tạo bằng ./hbbs)"
echo ""
echo "📋 Các lệnh quản lý:"
echo "  pm2 status         - Xem trạng thái"
echo "  pm2 logs p204-admin - Xem log server"
echo "  pm2 restart all    - Khởi động lại tất cả"
echo ""
echo "⚠️  VIỆC CẦN LÀM NGAY:"
echo "  1. Đổi mật khẩu admin: nano /opt/p204-server/.env"
echo "  2. Cập nhật cloud_config.json trên GitHub với IP mới"
echo "  3. Cập nhật public key trên GitHub cloud_config.json"
echo "==========================================="
