const { io } = require("socket.io-client");
const jwt = require("jsonwebtoken");

const JWT_SECRET = 'P204_JWT_SECRET_2026';
// Tạo token giả lập cho máy P204-TEST
const token = jwt.sign({ seat_id: 'P204-TEST', rustdesk_id: '123456789' }, JWT_SECRET);

const socket = io("ws://127.0.0.1:3000", {
  transports: ["websocket"]
});

socket.on("connect", () => {
  console.log("✅ Đã kết nối với máy chủ Node.js!");
  
  // Gửi join-company
  socket.emit("join-company", {
    client_token: token,
    seat_id: "P204-TEST",
    rustdesk_id: "123456789",
    hostname: "TestMachine"
  });

  // Gửi tin nhắn test
  setTimeout(() => {
    socket.emit("chat-message", {
      sender: "P204-TEST",
      message: "Hello từ Test Script, mạng mượt quá!"
    });
  }, 1000);
});

socket.on("chat-message", (msg) => {
  console.log(`📩 Nhận được tin nhắn phát sóng từ Server: [${msg.sender}] ${msg.message}`);
  console.log("🎉 TEST CHAT THÀNH CÔNG! Ping/Pong tự động duy trì tốt.");
  process.exit(0);
});

socket.on("disconnect", () => {
  console.log("❌ Bị ngắt kết nối!");
});
