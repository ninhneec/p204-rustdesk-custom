#!/bin/bash
# 🚀 P204 VPS Auto-Installer cho Ubuntu
# Ưu tiên hiệu năng mạng cao & Hình ảnh mượt mà nhất

echo "==========================================="
echo "🏢 Cài đặt Hệ thống Quản Lý P204 lên VPS..."
echo "==========================================="

# 1. Cập nhật hệ thống và cài đặt công cụ cần thiết
sudo apt update && sudo apt upgrade -y
sudo apt install -y curl wget unzip git ufw

# 2. Cài đặt Node.js 20 & PM2
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt install -y nodejs
sudo npm install -g pm2

# 3. Mở khóa tường lửa (UFW) cho Tốc độ tối đa
echo "Mở cổng mạng cho Node.js và Rustdesk..."
sudo ufw allow 3000/tcp  # Node.js Server
sudo ufw allow 21115:21119/tcp # Rustdesk hbbs/hbbr TCP
sudo ufw allow 21116/udp # Rustdesk hbbs UDP (Rất quan trọng cho tốc độ P2P)
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp

# 3.5. Nhận diện IP VPS (Hỗ trợ NAT VPS)
echo "==========================================="
echo "Bạn có đang sử dụng NAT VPS không?"
echo "Nếu có, hãy nhập IP hoặc Tên miền Public của NAT VPS (Ví dụ: vn1.natvps.net hoặc 103.1.2.3)."
echo "Nếu dùng VPS IP tĩnh bình thường, cứ để trống và bấm Enter!"
read -p "Nhập IP/Tên miền (bỏ trống để tự động lấy IP): " USER_IP

if [ -z "$USER_IP" ]; then
    PUBLIC_IP=$(curl -s ifconfig.me)
else
    PUBLIC_IP=$USER_IP
fi
echo "=> Hệ thống sẽ sử dụng IP/Tên miền: $PUBLIC_IP"
echo "==========================================="

# 4. Tải và cài đặt RustDesk Server (hbbs / hbbr)
echo "Cài đặt RustDesk Server..."
mkdir -p ~/rustdesk-server
cd ~/rustdesk-server
wget https://github.com/rustdesk/rustdesk-server/releases/download/1.1.11-1/rustdesk-server-linux-amd64.zip
unzip -o rustdesk-server-linux-amd64.zip
mv amd64/* .
chmod +x hbbs hbbr

# 5. Khởi chạy RustDesk bằng PM2
# Tối ưu hóa hbbr (Relay) cho tốc độ cao và gắn với IP công khai vừa nhập
pm2 start ./hbbs --name "hbbs" -- -r $PUBLIC_IP -k _
pm2 start ./hbbr --name "hbbr" -- -k _
pm2 save
pm2 startup

# 6. Hiển thị Key
echo "==========================================="
echo "✅ ĐÃ CÀI ĐẶT THÀNH CÔNG RUSTDESK SERVER!"
echo "🔑 ĐÂY LÀ PUBLIC KEY CỦA BẠN (Cần copy dán vào code hoặc Github):"
cat ./id_ed25519.pub
echo ""
echo "🌐 ĐỊA CHỈ IP CỦA VPS NÀY LÀ: $(curl -s ifconfig.me)"
echo "==========================================="
echo "👉 BƯỚC TIẾP THEO:"
echo "Hãy đưa thư mục 'server' và 'dashboard' lên VPS này, sau đó chạy lệnh:"
echo "cd server && npm install && pm2 start server.js --name p204-admin"
echo "==========================================="
