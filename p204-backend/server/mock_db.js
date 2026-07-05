const fs = require('fs');

let c = fs.readFileSync('server.js', 'utf8');

const mockDbCode = `
const dbState = { keys: [], machines: [], chat_messages: [] };
let autoIncKeys = 1;
let autoIncChat = 1;
const db = {
  prepare: (query) => {
    return {
      run: (...args) => {
        if (query.startsWith('INSERT INTO keys')) {
           dbState.keys.unshift({ id: autoIncKeys++, seat_id: args[0], token_key: args[1], status: 'unused', created_at: new Date().toISOString() });
        } else if (query.startsWith('UPDATE keys SET status')) {
           const k = dbState.keys.find(k => k.id === args[1]);
           if (k) { k.status = 'used'; k.used_by_id = args[0]; }
        } else if (query.startsWith('INSERT INTO machines')) {
           const argObj = args[0];
           const seat_id = argObj.seat_id || args[0];
           const exists = dbState.machines.find(m => m.seat_id === seat_id);
           if (!exists) dbState.machines.push({ seat_id: argObj.seat_id, rustdesk_id: argObj.rustdesk_id, hostname: argObj.hostname, status: 'offline' });
        } else if (query.startsWith('UPDATE machines SET')) {
           const argObj = args[0];
           const m = dbState.machines.find(m => m.seat_id === argObj.seat_id);
           if (m) { m.socket_id = argObj.socket_id; m.status = 'online'; }
        } else if (query.startsWith('INSERT INTO chat_messages')) {
           dbState.chat_messages.unshift({ id: autoIncChat++, sender: args[0], message: args[1], timestamp: new Date().toISOString() });
        } else if (query.includes('UPDATE machines SET status = ?')) {
           const m = dbState.machines.find(m => m.socket_id === args[1]);
           if (m) { m.status = 'offline'; m.socket_id = null; }
        }
      },
      all: () => {
        if (query.includes('FROM keys')) return dbState.keys;
        if (query.includes('FROM machines')) return dbState.machines;
        if (query.includes('FROM chat_messages')) return [...dbState.chat_messages].reverse();
        return [];
      },
      get: (...args) => {
        if (query.includes('FROM keys WHERE token_key')) return dbState.keys.find(k => k.token_key === args[0] && k.status === 'unused');
        if (query.includes('FROM machines WHERE socket_id')) return dbState.machines.find(m => m.socket_id === args[0]);
        return null;
      }
    };
  },
  exec: () => {}
};
`;

c = c.replace(/const db = new Database\('p204\.db'\);/, mockDbCode);
c = c.replace("const Database = require('better-sqlite3');", "");
fs.writeFileSync('server_mock.js', c);
