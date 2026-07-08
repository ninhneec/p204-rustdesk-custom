// ═══════════════════════════════════════════════
// P204 Remote Management Server v2.0
// Node.js + Express + Socket.IO + SQLite
// ═══════════════════════════════════════════════

require('dotenv').config({ path: require('path').join(__dirname, '.env') });

const express = require('express');
const { createServer } = require('http');
const { Server } = require('socket.io');
const cors = require('cors');
const jwt = require('jsonwebtoken');
const path = require('path');
const crypto = require('crypto');
const fs = require('fs');
const rateLimit = require('express-rate-limit');

// ── Config từ biến môi trường ─────────────────
const JWT_SECRET = process.env.JWT_SECRET || crypto.randomBytes(32).toString('hex');
const ADMIN_USER = process.env.ADMIN_USER || 'admin';
const ADMIN_PASS = process.env.ADMIN_PASS || 'Boss@2026';
const PORT = parseInt(process.env.PORT, 10) || 3000;
const CORS_ORIGIN = process.env.CORS_ORIGIN || '*';
const DB_PATH = process.env.DB_PATH || path.join(__dirname, 'data', 'p204.db');
const JWT_EXPIRES = '24h';
const CLIENT_TOKEN_EXPIRES = '90d';

// ── Đảm bảo thư mục data tồn tại ──────────────
const dataDir = path.dirname(DB_PATH);
if (!fs.existsSync(dataDir)) fs.mkdirSync(dataDir, { recursive: true });

// ── Database SQLite ────────────────────────────
const Database = require('better-sqlite3');
const db = new Database(DB_PATH, { /* verbose: console.log */ });
db.pragma('journal_mode = WAL');
db.pragma('foreign_keys = ON');

db.exec(`
  CREATE TABLE IF NOT EXISTS keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    seat_id TEXT NOT NULL,
    token_key TEXT UNIQUE NOT NULL,
    status TEXT DEFAULT 'unused',
    used_by_id TEXT,
    created_at DATETIME DEFAULT (datetime('now', 'localtime'))
  );
  CREATE TABLE IF NOT EXISTS machines (
    seat_id TEXT PRIMARY KEY,
    rustdesk_id TEXT UNIQUE,
    rustdesk_pass TEXT,
    hostname TEXT,
    socket_id TEXT,
    status TEXT DEFAULT 'offline',
    last_seen DATETIME DEFAULT (datetime('now', 'localtime'))
  );
  CREATE TABLE IF NOT EXISTS chat_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sender TEXT NOT NULL,
    message TEXT NOT NULL,
    timestamp DATETIME DEFAULT (datetime('now', 'localtime'))
  );
  CREATE INDEX IF NOT EXISTS idx_keys_token ON keys(token_key);
  CREATE INDEX IF NOT EXISTS idx_machines_socket ON machines(socket_id);
`);

// ── Prepared Statements ────────────────────────
const stmts = {
  keyFindUnused: db.prepare("SELECT * FROM keys WHERE token_key = ? AND status = 'unused'"),
  keyFindAny: db.prepare("SELECT * FROM keys WHERE token_key = ?"),
  keyMarkUsed: db.prepare("UPDATE keys SET status = 'used', used_by_id = ? WHERE id = ?"),
  keyInsert: db.prepare("INSERT INTO keys (seat_id, token_key) VALUES (?, ?)"),
  keyDelete: db.prepare("DELETE FROM keys WHERE token_key = ?"),
  keyAll: db.prepare("SELECT * FROM keys ORDER BY id DESC"),

  machineUpsert: db.prepare(`
    INSERT INTO machines (seat_id, rustdesk_id, hostname, status, last_seen)
    VALUES (@seat_id, @rustdesk_id, @hostname, 'offline', datetime('now', 'localtime'))
    ON CONFLICT(seat_id) DO UPDATE SET
      rustdesk_id = COALESCE(@rustdesk_id, rustdesk_id),
      hostname = COALESCE(@hostname, hostname),
      last_seen = datetime('now', 'localtime')
  `),
  machineSetOnline: db.prepare(`
    UPDATE machines SET socket_id = ?, rustdesk_pass = ?, status = 'online',
    last_seen = datetime('now', 'localtime')
    WHERE seat_id = ? AND rustdesk_id = ?
  `),
  machineSetOffline: db.prepare("UPDATE machines SET status = 'offline', socket_id = NULL WHERE socket_id = ?"),
  machineFindBySocket: db.prepare("SELECT * FROM machines WHERE socket_id = ?"),
  machineFindBySeat: db.prepare("SELECT * FROM machines WHERE seat_id = ?"),
  machineAll: db.prepare("SELECT * FROM machines"),
  machineTouch: db.prepare("UPDATE machines SET last_seen = datetime('now', 'localtime') WHERE seat_id = ?"),

  chatInsert: db.prepare("INSERT INTO chat_messages (sender, message) VALUES (?, ?)"),
  chatRecent: db.prepare("SELECT * FROM chat_messages ORDER BY id DESC LIMIT 200"),
  chatCleanup: db.prepare("DELETE FROM chat_messages WHERE id NOT IN (SELECT id FROM chat_messages ORDER BY id DESC LIMIT 1000)"),
};

// ── Express App ────────────────────────────────
const app = express();
const httpServer = createServer(app);
const io = new Server(httpServer, {
  cors: { origin: CORS_ORIGIN, methods: ["GET", "POST"] }
});

app.use(cors({ origin: CORS_ORIGIN }));
app.use(express.json({ limit: '1mb' }));
const dashboardPath = fs.existsSync(path.join(__dirname, 'dashboard'))
  ? path.join(__dirname, 'dashboard')
  : path.join(__dirname, '..', 'dashboard');
app.use(express.static(dashboardPath));

// ── Rate Limiter ───────────────────────────────
const apiLimiter = rateLimit({
  windowMs: 60 * 1000,     // 1 phút
  max: 60,                  // 60 req/phút
  standardHeaders: true,
  message: { success: false, message: 'Quá nhiều yêu cầu. Vui lòng thử lại sau.' }
});
const loginLimiter = rateLimit({
  windowMs: 60 * 1000,
  max: 10,
  message: { success: false, message: 'Quá nhiều lần đăng nhập. Thử lại sau 1 phút.' }
});
app.use('/api/', apiLimiter);

// ── Auth Middleware ─────────────────────────────
const verifyToken = (req, res, next) => {
  const authHeader = req.headers.authorization;
  if (!authHeader) return res.status(401).json({ success: false, message: 'Không tìm thấy token' });
  const token = authHeader.startsWith('Bearer ') ? authHeader.slice(7) : authHeader;
  jwt.verify(token, JWT_SECRET, (err, user) => {
    if (err) return res.status(403).json({ success: false, message: 'Token không hợp lệ hoặc đã hết hạn' });
    req.user = user;
    next();
  });
};

// ── Helper: sanitize string ────────────────────
const sanitize = (s) => (typeof s === 'string') ? s.replace(/[<>]/g, '').trim().slice(0, 128) : '';

// ═══════════════════════════════════════════════
// REST API ENDPOINTS
// ═══════════════════════════════════════════════

// ── Login ──────────────────────────────────────
app.post('/login', loginLimiter, (req, res) => {
  const { username, password } = req.body;
  if (username === ADMIN_USER && password === ADMIN_PASS) {
    const token = jwt.sign({ role: 'admin', user: ADMIN_USER }, JWT_SECRET, { expiresIn: JWT_EXPIRES });
    return res.json({ success: true, token, expiresIn: JWT_EXPIRES });
  }
  res.status(401).json({ success: false, message: 'Sai tên đăng nhập hoặc mật khẩu' });
});

// ── Health check ───────────────────────────────
app.get('/api/health', (req, res) => {
  res.json({ status: 'ok', uptime: process.uptime(), machines: stmts.machineAll.all().length });
});

// ── Client: Verify enrollment key ──────────────
app.post('/api/keys/verify', (req, res) => {
  try {
    const enrollment_key = sanitize(req.body.enrollment_key);
    const rustdesk_id = sanitize(req.body.rustdesk_id);
    const hostname = sanitize(req.body.hostname) || 'Unknown';

    if (!enrollment_key || !rustdesk_id) {
      return res.status(400).json({ success: false, message: 'Thiếu thông tin enrollment_key hoặc rustdesk_id' });
    }
    if (!/^P204-[A-F0-9]{8}$/.test(enrollment_key)) {
      return res.status(400).json({ success: false, message: 'Định dạng key không hợp lệ' });
    }

    const keyRow = stmts.keyFindUnused.get(enrollment_key);
    if (!keyRow) {
      return res.status(400).json({ success: false, message: 'Key không hợp lệ hoặc đã được sử dụng' });
    }

    stmts.keyMarkUsed.run(rustdesk_id, keyRow.id);
    stmts.machineUpsert.run({ seat_id: keyRow.seat_id, rustdesk_id, hostname });

    const client_token = jwt.sign(
      { seat_id: keyRow.seat_id, rustdesk_id },
      JWT_SECRET,
      { expiresIn: CLIENT_TOKEN_EXPIRES }
    );

    res.json({ success: true, seat_id: keyRow.seat_id, client_token });
  } catch (e) {
    console.error('verify error:', e.message);
    res.status(500).json({ success: false, message: 'Lỗi máy chủ' });
  }
});

// ── Admin: Get machines ────────────────────────
app.get('/api/machines', verifyToken, (req, res) => {
  res.json(stmts.machineAll.all());
});

// ── Admin: Get keys ────────────────────────────
app.get('/api/admin/keys', verifyToken, (req, res) => {
  res.json(stmts.keyAll.all());
});

// ── Admin: Generate key ────────────────────────
app.post('/api/admin/keys/generate', verifyToken, (req, res) => {
  let seat_id = sanitize(req.body.seat_id);
  if (!seat_id) return res.status(400).json({ success: false, message: 'Thiếu seat_id' });

  // Auto-format: "5" → "M05"
  if (/^\d+$/.test(seat_id)) {
    seat_id = 'M' + seat_id.padStart(2, '0');
  }

  const token_key = 'P204-' + crypto.randomBytes(4).toString('hex').toUpperCase();
  try {
    stmts.keyInsert.run(seat_id, token_key);
    res.json({ success: true, token_key, seat_id });
  } catch (e) {
    if (e.message.includes('UNIQUE')) {
      return res.status(409).json({ success: false, message: 'Key đã tồn tại, vui lòng thử lại' });
    }
    res.status(500).json({ success: false, message: 'Lỗi tạo key' });
  }
});

// ── Admin: Delete/Revoke key ───────────────────
app.delete('/api/admin/keys/:token_key', verifyToken, (req, res) => {
  const { token_key } = req.params;
  try {
    const keyRow = stmts.keyFindAny.get(token_key);
    if (!keyRow) return res.status(404).json({ success: false, message: 'Key không tồn tại' });

    stmts.keyDelete.run(token_key);

    // Ngắt kết nối máy đang dùng key này
    if (keyRow.seat_id) {
      const machine = stmts.machineFindBySeat.get(keyRow.seat_id);
      if (machine && machine.socket_id) {
        io.to(machine.socket_id).emit('revoke-key', { message: 'Key của bạn đã bị quản trị viên thu hồi.' });
        setTimeout(() => {
          const sock = io.sockets.sockets.get(machine.socket_id);
          if (sock) sock.disconnect(true);
        }, 1000);
      }
    }
    res.json({ success: true, message: 'Đã thu hồi key thành công' });
  } catch (e) {
    console.error('delete key error:', e.message);
    res.status(500).json({ success: false, message: 'Lỗi xóa key' });
  }
});

// ═══════════════════════════════════════════════
// SOCKET.IO
// ═══════════════════════════════════════════════

// Heartbeat timeout: nếu 60s không ping → coi là offline
const HEARTBEAT_TIMEOUT = 60 * 1000;
const heartbeats = new Map(); // socketId → timeout

function clearHeartbeat(socketId) {
  if (heartbeats.has(socketId)) {
    clearTimeout(heartbeats.get(socketId));
    heartbeats.delete(socketId);
  }
}

function handleOffline(socketId) {
  clearHeartbeat(socketId);
  const machine = stmts.machineFindBySocket.get(socketId);
  if (machine) {
    stmts.machineSetOffline.run(socketId);
    io.emit('machine-status-change', {
      seat_id: machine.seat_id,
      rustdesk_id: machine.rustdesk_id,
      hostname: machine.hostname,
      status: 'offline'
    });
    console.log(`❌ ${machine.seat_id} offline (timeout/disconnect)`);
  }
}

io.on('connection', (socket) => {
  console.log(`📡 Kết nối: ${socket.id}`);

  // ── Client join-company ────────────────────
  socket.on('join-company', (payload) => {
    try {
      const { client_token, seat_id, rustdesk_id, hostname } = payload || {};
      const decoded = jwt.verify(client_token, JWT_SECRET);
      if (decoded.seat_id !== seat_id || decoded.rustdesk_id !== rustdesk_id) {
        throw new Error('Token mismatch');
      }

      stmts.machineSetOnline.run(socket.id, payload.rustdesk_pass || null, seat_id, rustdesk_id);

      // Reset heartbeat timer
      clearHeartbeat(socket.id);
      heartbeats.set(socket.id, setTimeout(() => handleOffline(socket.id), HEARTBEAT_TIMEOUT));

      console.log(`✅ ${seat_id} (${sanitize(hostname)}) online`);

      io.emit('machine-status-change', {
        seat_id, rustdesk_id,
        rustdesk_pass: payload.rustdesk_pass || null,
        hostname: sanitize(hostname),
        status: 'online'
      });

      socket.emit('all-machines', stmts.machineAll.all());
    } catch (e) {
      console.log(`⚠️ join-company rejected: ${e.message}`);
      socket.emit('error-msg', 'Xác thực thất bại. Vui lòng đăng ký lại.');
    }
  });

  // ── Heartbeat ─────────────────────────────
  socket.on('heartbeat', (payload) => {
    const { seat_id } = payload || {};
    if (seat_id) {
      stmts.machineTouch.run(sanitize(seat_id));
      // Reset timeout
      clearHeartbeat(socket.id);
      heartbeats.set(socket.id, setTimeout(() => handleOffline(socket.id), HEARTBEAT_TIMEOUT));
    }
  });

  // ── Admin join ────────────────────────────
  socket.on('join-admin', (payload) => {
    try {
      const { token } = payload || {};
      jwt.verify(token, JWT_SECRET);
      socket.join('admin_room');
      socket.emit('all-machines', stmts.machineAll.all());
      socket.emit('chat-history', stmts.chatRecent.all().reverse());
      console.log(`👨‍💻 Admin vào dashboard (${socket.id})`);
    } catch (e) {
      console.log('⚠️ Admin join rejected');
    }
  });

  // ── Chat message ──────────────────────────
  socket.on('chat-message', (payload) => {
    const { sender, message } = payload || {};
    const cleanSender = sanitize(sender) || 'unknown';
    const cleanMsg = sanitize(message)?.slice(0, 500) || '';
    if (!cleanMsg) return;

    stmts.chatInsert.run(cleanSender, cleanMsg);
    // Dọn dẹp chat cũ (giữ 1000 messages)
    try { stmts.chatCleanup.run(); } catch (e) { /* ignore */ }

    io.emit('chat-message', {
      sender: cleanSender,
      message: cleanMsg,
      timestamp: new Date().toISOString()
    });
  });

  // ── Disconnect ────────────────────────────
  socket.on('disconnect', () => {
    handleOffline(socket.id);
  });
});

// ── Định kỳ dọn chat cũ ───────────────────────
setInterval(() => {
  try { stmts.chatCleanup.run(); } catch (e) { /* ignore */ }
}, 3600_000); // mỗi giờ

// ═══════════════════════════════════════════════
// START SERVER
// ═══════════════════════════════════════════════
httpServer.listen(PORT, '0.0.0.0', () => {
  console.log('═══════════════════════════════════════');
  console.log(`🔥 P204 Server v2.0 - Port ${PORT}`);
  console.log(`📂 Database: ${DB_PATH}`);
  console.log(`👤 Admin: ${ADMIN_USER}`);
  console.log('═══════════════════════════════════════');
});

// Graceful shutdown
process.on('SIGTERM', () => {
  console.log('🛑 Shutting down...');
  db.close();
  process.exit(0);
});
process.on('SIGINT', () => {
  console.log('🛑 Shutting down...');
  db.close();
  process.exit(0);
});
