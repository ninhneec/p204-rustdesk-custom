const express = require('express');
const { createServer } = require('http');
const { Server } = require('socket.io');
const cors = require('cors');
const jwt = require('jsonwebtoken');
const path = require('path');
const crypto = require('crypto');
const fs = require('fs');

const app = express();
const httpServer = createServer(app);
const io = new Server(httpServer, {
  cors: {
    origin: "*",
    methods: ["GET", "POST"]
  }
});

// Middleware
app.use(cors());
app.use(express.json());
// Serve dashboard — VPS: cùng thư mục, Repo: ../dashboard/
const dashFile = path.join(__dirname, 'dashboard.html');
const dashFileAlt = path.join(__dirname, '../dashboard/dashboard.html');
const DASHBOARD_FILE = fs.existsSync(dashFile) ? dashFile : dashFileAlt;
app.use(express.static(path.dirname(DASHBOARD_FILE)));

const DB_FILE = path.join(__dirname, 'p204_data.json');

// In-memory Database mock to avoid native module compilation errors on Windows
const db = {
  keys: [],
  machines: [],
  chat_messages: [],
  keyIdCounter: 1,
  msgIdCounter: 1,
  
  prepare: function(sql) {
    return {
      run: function(...args) {
        if (sql.includes('INSERT INTO keys')) {
          const seat_id = args[0];
          const token_key = args[1];
          db.keys.push({ id: db.keyIdCounter++, seat_id, token_key, status: 'unused', used_by_id: null });
        } else if (sql.includes('UPDATE keys SET status')) {
          const rustdesk_id = args[0];
          const id = args[1];
          const k = db.keys.find(x => x.id === id);
          if (k) { k.status = 'used'; k.used_by_id = rustdesk_id; }
        } else if (sql.includes('DELETE FROM keys')) {
          const token_key = args[0];
          db.keys = db.keys.filter(x => x.token_key !== token_key);
        } else if (sql.includes('INSERT INTO machines')) {
          const params = args[0];
          let m = db.machines.find(x => x.seat_id === params.seat_id);
          if (m) {
            m.rustdesk_id = params.rustdesk_id;
            m.hostname = params.hostname;
            m.last_seen = new Date().toISOString();
          } else {
            db.machines.push({ ...params, status: 'offline', last_seen: new Date().toISOString() });
          }
        } else if (sql.includes('UPDATE machines SET socket_id')) {
          const params = args[0];
          let m = db.machines.find(x => x.seat_id === params.seat_id);
          if (m) { 
              m.socket_id = params.socket_id; 
              m.status = params.status;
              if (params.rustdesk_id) m.rustdesk_id = params.rustdesk_id;
              if (params.rustdesk_pass) m.rustdesk_pass = params.rustdesk_pass;  // FIX: lưu password
              if (params.hostname) m.hostname = params.hostname;
              m.last_seen = new Date().toISOString(); 
          } else {
              db.machines.push({ ...params, status: 'online', last_seen: new Date().toISOString() });
          }
        } else if (sql.includes('UPDATE machines SET rustdesk_id')) {
          const params = args[0];
          let m = db.machines.find(x => x.seat_id === params.seat_id);
          if (m) {
            if (params.rustdesk_id) m.rustdesk_id = params.rustdesk_id;
            if (params.rustdesk_pass) m.rustdesk_pass = params.rustdesk_pass;
            m.status = 'online';
            m.last_seen = new Date().toISOString();
            if (params.hostname) m.hostname = params.hostname;
          } else {
            db.machines.push({ ...params, status: 'online', last_seen: new Date().toISOString() });
          }
        } else if (sql.includes('INSERT INTO chat_messages')) {
          db.chat_messages.push({ id: db.msgIdCounter++, sender: args[0], message: args[1], timestamp: new Date().toISOString() });
        }
        
        // Auto-save to JSON after any mutation
        try {
          fs.writeFileSync(DB_FILE, JSON.stringify({
            keys: db.keys, machines: db.machines, chat_messages: db.chat_messages,
            keyIdCounter: db.keyIdCounter, msgIdCounter: db.msgIdCounter
          }, null, 2));
        } catch(e) {}
      },
      get: function(param) {
        if (sql.includes('token_key = ?')) return db.keys.find(x => x.token_key === param);
        if (sql.includes('seat_id = ?')) return db.machines.find(x => x.seat_id === param);
        return null;
      },
      all: function() {
        if (sql.includes('FROM machines')) return db.machines;
        if (sql.includes('FROM keys')) return db.keys.slice().reverse();
        if (sql.includes('FROM chat_messages')) return db.chat_messages;
        return [];
      }
    };
  }
};

const JWT_SECRET = 'P204_JWT_SECRET_2026';
const ADMIN_PASS = 'Boss@2026';

// Load Database at startup
if (fs.existsSync(DB_FILE)) {
  try {
    const data = JSON.parse(fs.readFileSync(DB_FILE, 'utf8'));
    db.keys = data.keys || [];
    db.machines = data.machines || [];
    // Reset all machines to offline on startup
    db.machines.forEach(m => { m.status = 'offline'; m.socket_id = null; });
    db.chat_messages = data.chat_messages || [];
    db.keyIdCounter = data.keyIdCounter || 1;
    db.msgIdCounter = data.msgIdCounter || 1;
  } catch(e) { console.log('Lỗi đọc database file', e); }
}

// Khởi tạo server báo console
console.log('🚀 P204 Server đang khởi động...');

// REST Endpoints
app.get('/', (req, res) => {
  res.sendFile(DASHBOARD_FILE);
});

app.post('/login', (req, res) => {
  const { username, password } = req.body;
  if (username === 'admin' && password === ADMIN_PASS) {
    const token = jwt.sign({ role: 'admin' }, JWT_SECRET, { expiresIn: '30d' });
    res.json({ success: true, token });
  } else {
    res.status(401).json({ success: false, message: 'Sai tên đăng nhập hoặc mật khẩu' });
  }
});

const verifyToken = (req, res, next) => {
  const authHeader = req.headers.authorization;
  if (authHeader) {
    const token = authHeader.split(' ')[1];
    jwt.verify(token, JWT_SECRET, (err, user) => {
      if (err) return res.status(403).json({ success: false, message: 'Token không hợp lệ' });
      req.user = user;
      next();
    });
  } else {
    res.status(401).json({ success: false, message: 'Không tìm thấy token' });
  }
};

// --- Client Endpoints ---
app.post('/api/keys/verify', (req, res) => {
  const { enrollment_key, rustdesk_id, hostname } = req.body;
  if (!enrollment_key || !rustdesk_id) {
    return res.status(400).json({ success: false, message: 'Thiếu thông tin' });
  }

  const keyRow = db.prepare("SELECT * FROM keys WHERE token_key = ? AND status = 'unused'").get(enrollment_key);
  if (!keyRow) {
    return res.status(400).json({ success: false, message: 'Enrollment Key không hợp lệ hoặc đã được sử dụng' });
  }

  // Đánh dấu key đã sử dụng
  db.prepare("UPDATE keys SET status = 'used', used_by_id = ? WHERE id = ?").run(rustdesk_id, keyRow.id);

  // Đăng ký máy
  const stmt = db.prepare(`
    INSERT INTO machines (seat_id, rustdesk_id, hostname, status, last_seen)
    VALUES (@seat_id, @rustdesk_id, @hostname, 'offline', CURRENT_TIMESTAMP)
    ON CONFLICT(seat_id) DO UPDATE SET
      rustdesk_id = @rustdesk_id,
      hostname = @hostname,
      last_seen = CURRENT_TIMESTAMP
  `);
  stmt.run({ seat_id: keyRow.seat_id, rustdesk_id, hostname: hostname || 'Unknown' });

  // Tạo token cho client (sống lâu dài)
  const client_token = jwt.sign({ seat_id: keyRow.seat_id, rustdesk_id }, JWT_SECRET, { expiresIn: '10y' });

  res.json({ success: true, seat_id: keyRow.seat_id, client_token });
});

// --- Admin Endpoints ---
app.get('/api/machines', verifyToken, (req, res) => {
  const machines = db.prepare('SELECT * FROM machines').all();
  res.json(machines);
});

app.get('/api/admin/keys', verifyToken, (req, res) => {
  const keys = db.prepare('SELECT * FROM keys ORDER BY id DESC').all();
  res.json(keys);
});

app.post('/api/admin/keys/generate', verifyToken, (req, res) => {
  const { seat_id } = req.body;
  if (!seat_id) return res.status(400).json({ success: false, message: 'Thiếu seat_id' });

  const token_key = 'P204-' + crypto.randomBytes(4).toString('hex').toUpperCase();
  
  try {
    db.prepare('INSERT INTO keys (seat_id, token_key) VALUES (?, ?)').run(seat_id, token_key);
    res.json({ success: true, token_key });
  } catch (e) {
    res.status(500).json({ success: false, message: 'Lỗi tạo key' });
  }
});

app.delete('/api/admin/keys/:token_key', verifyToken, (req, res) => {
  const { token_key } = req.params;
  
  try {
    const keyRow = db.prepare('SELECT * FROM keys WHERE token_key = ?').get(token_key);
    db.prepare('DELETE FROM keys WHERE token_key = ?').run(token_key);
    
    if (keyRow && keyRow.seat_id) {
        const machine = db.prepare('SELECT * FROM machines WHERE seat_id = ?').get(keyRow.seat_id);
        if (machine && machine.socket_id) {
            io.to(machine.socket_id).emit('revoke-key', { message: 'Token của bạn đã bị quản trị viên thu hồi.' });
            setTimeout(() => {
                const socket = io.sockets.sockets.get(machine.socket_id);
                if (socket) socket.disconnect(true);
            }, 1000);
        }
    }
    res.json({ success: true, message: 'Đã xóa key và thu hồi quyền truy cập' });
  } catch (e) {
    res.status(500).json({ success: false, message: 'Lỗi xóa key' });
  }
});

// ── OTA Version ────────────────────────────────
app.get('/api/version/latest', (req, res) => {
  const latest = db._otaVersion || { version: '1.4.8', download_url: '', changelog: '', mandatory: false };
  res.json(latest);
});

app.post('/api/admin/version', verifyToken, (req, res) => {
  const { version, download_url, changelog, mandatory } = req.body;
  if (!version || !download_url) return res.status(400).json({ success: false, message: 'Thiếu version hoặc download_url' });
  db._otaVersion = { version, download_url, changelog: changelog || '', mandatory: !!mandatory };
  try { fs.writeFileSync(DB_FILE, JSON.stringify(db, null, 2)); } catch(e) {}
  res.json({ success: true });
});

// ── Secrets ─────────────────────────────────────
app.post('/api/secrets/verify', (req, res) => {
  const { secret, seat_id, hostname } = req.body;
  const row = (db._secrets || []).find(s => s.code === secret);
  if (row) {
    if (!db._secretLogs) db._secretLogs = [];
    db._secretLogs.push({ secret, seat_id, hostname, used_at: new Date().toISOString() });
    res.json({ success: true, verified: true, label: row.label || '' });
  } else {
    res.json({ success: false, verified: false, message: 'Mã bí mật không hợp lệ' });
  }
});

app.post('/api/admin/secrets/generate', verifyToken, (req, res) => {
  if (!db._secrets) db._secrets = [];
  const code = 'SEC-' + crypto.randomBytes(6).toString('hex').toUpperCase();
  db._secrets.push({ code, label: req.body.label || '', created_at: new Date().toISOString() });
  try { fs.writeFileSync(DB_FILE, JSON.stringify(db, null, 2)); } catch(e) {}
  res.json({ success: true, code });
});

app.get('/api/admin/secrets', verifyToken, (req, res) => {
  res.json(db._secrets || []);
});

app.delete('/api/admin/secrets/:code', verifyToken, (req, res) => {
  if (!db._secrets) db._secrets = [];
  db._secrets = db._secrets.filter(s => s.code !== req.params.code);
  try { fs.writeFileSync(DB_FILE, JSON.stringify(db, null, 2)); } catch(e) {}
  res.json({ success: true });
});

// ── Remote Command ──────────────────────────────
app.post('/api/admin/command', verifyToken, (req, res) => {
  const { seat_id, command, payload } = req.body;
  const machine = db.machines.find(m => m.seat_id === seat_id);
  if (!machine || !machine.socket_id) {
    return res.status(400).json({ success: false, message: 'Máy đang offline' });
  }
  io.to(machine.socket_id).emit(command, payload || {});
  res.json({ success: true, message: `Đã gửi lệnh ${command}` });
});

// Socket.IO
io.on('connection', (socket) => {
  console.log(`📡 Client kết nối: ${socket.id}`);

  // Event: Máy trạm kết nối vào mạng công ty
  socket.on('join-company', (payload) => {
    const { client_token, seat_id, rustdesk_id, hostname } = payload;
    
    try {
      const decoded = jwt.verify(client_token, JWT_SECRET);
      if (decoded.seat_id !== seat_id || decoded.rustdesk_id !== rustdesk_id) {
        throw new Error('Token mismatch');
      }
    } catch (e) {
      console.log(`⚠️ Từ chối kết nối từ ${seat_id} - Token không hợp lệ`);
      return;
    }

    // Cập nhật Database
    const stmt = db.prepare(`
      UPDATE machines SET
        socket_id = @socket_id,
        rustdesk_pass = @rustdesk_pass,
        status = 'online',
        last_seen = CURRENT_TIMESTAMP
      WHERE seat_id = @seat_id AND rustdesk_id = @rustdesk_id
    `);
    stmt.run({ socket_id: socket.id, rustdesk_pass: payload.rustdesk_pass || null, seat_id, rustdesk_id, hostname: hostname || 'Unknown' });

    console.log(`✅ Máy ${seat_id} (${hostname}) đã online.`);

    // Gửi thay đổi cho toàn mạng
    io.emit('machine-status-change', { seat_id, rustdesk_id, rustdesk_pass: payload.rustdesk_pass, hostname, status: 'online' });
    
    // Gửi danh sách cho chính máy đó (nếu cần)
    const machines = db.prepare('SELECT * FROM machines').all();
    socket.emit('all-machines', machines);
  });

  // Event: Heartbeat để giữ kết nối
  socket.on('heartbeat', (payload) => {
    const { seat_id, rustdesk_id, rustdesk_pass, hostname } = payload;
    const stmt = db.prepare(`
      UPDATE machines SET
        rustdesk_id = @rustdesk_id,
        rustdesk_pass = @rustdesk_pass,
        status = 'online',
        last_seen = CURRENT_TIMESTAMP
      WHERE seat_id = @seat_id
    `);
    stmt.run({ rustdesk_id: rustdesk_id || null, rustdesk_pass: rustdesk_pass || null, seat_id, hostname: hostname || 'Unknown' });
    
    // Broadcast thay đổi để giao diện Web cập nhật ID/Pass liên tục
    io.emit('machine-status-change', { seat_id, rustdesk_id, rustdesk_pass, hostname, status: 'online' });
  });

  // Event: Quản trị viên kết nối dashboard
  socket.on('join-admin', (payload) => {
    const { token } = payload;
    try {
      jwt.verify(token, JWT_SECRET);
      socket.join('admin_room');
      
      const machines = db.prepare('SELECT * FROM machines').all();
      socket.emit('all-machines', machines);
      
      const history = db.prepare('SELECT * FROM chat_messages ORDER BY id DESC LIMIT 100').all().reverse();
      socket.emit('chat-history', history);
      console.log(`👨‍💻 Admin đã kết nối dashboard (Socket: ${socket.id})`);
    } catch (e) {
      console.log(`⚠️ Admin kết nối thất bại (Sai token)`);
    }
  });

  // Event: Gửi tin nhắn chat
  socket.on('chat-message', (payload) => {
    const { sender, message } = payload;
    db.prepare('INSERT INTO chat_messages (sender, message) VALUES (?, ?)').run(sender, message);
    
    io.emit('chat-message', {
      sender,
      message,
      timestamp: new Date().toISOString()
    });
    console.log(`💬 Chat từ ${sender}: ${message}`);
  });

  socket.on('request-machines', () => {
    const machines = db.prepare('SELECT * FROM machines').all();
    socket.emit('all-machines', machines);
  });

  // Event: Ngắt kết nối
  socket.on('disconnect', () => {
    // Tìm máy bị ngắt kết nối
    const machine = db.prepare('SELECT * FROM machines WHERE socket_id = ?').get(socket.id);
    if (machine) {
      db.prepare('UPDATE machines SET status = ?, socket_id = NULL WHERE socket_id = ?').run('offline', socket.id);
      io.emit('machine-status-change', {
        seat_id: machine.seat_id,
        rustdesk_id: machine.rustdesk_id,
        hostname: machine.hostname,
        status: 'offline'
      });
      console.log(`❌ Máy ${machine.seat_id} đã ngắt kết nối (offline).`);
    } else {
      console.log(`📉 Client ${socket.id} ngắt kết nối.`);
    }
  });
});

const PORT = process.env.PORT || 3000;
httpServer.listen(PORT, () => {
  console.log(`🔥 P204 Server đang chạy tại http://localhost:${PORT}`);
});
