// Fake Claude bridge for the e2e test: records POST /event bodies (in order) and answers POST /ask
// with the fake `claude` (fake-claude.mjs), like scripts/claude-bridge.mjs would with the real one.
import http from 'node:http';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));

const askFakeClaude = (req, logFile) => new Promise((resolve) => {
  const child = spawn(process.execPath, [path.join(HERE, 'fake-claude.mjs'), '--effort', String(req.effort || '')], {
    env: { ...process.env, FAKE_LOG: logFile }, stdio: ['pipe', 'pipe', 'ignore'],
  });
  let out = '';
  child.stdout.on('data', (d) => (out += d));
  child.on('close', () => {
    try {
      const r = JSON.parse(out);
      if (r.is_error) return resolve({ status: 500, body: { ok: false, error: String(r.result) } });
      resolve({ status: 200, body: { ok: true, output: r.structured_output, result: r.result, usage: r.usage, duration_ms: r.duration_ms } });
    } catch { resolve({ status: 500, body: { ok: false, error: 'bad fake claude output' } }); }
  });
  child.stdin.end(String(req.system || '') + '\n' + String(req.prompt || ''));
});

export function startRecorder({ port, logFile }) {
  const events = [];
  const server = http.createServer((req, res) => {
    let raw = '';
    req.on('data', (d) => (raw += d));
    req.on('end', async () => {
      const send = (status, body) => { res.writeHead(status, { 'content-type': 'application/json' }); res.end(JSON.stringify(body)); };
      let body = {};
      try { body = JSON.parse(raw || '{}'); } catch { return send(400, { ok: false }); }
      if (req.method === 'POST' && req.url === '/event') { events.push({ ...body, _at: Date.now() }); return send(200, { ok: true }); }
      if (req.method === 'POST' && req.url === '/ask') { const r = await askFakeClaude(body, logFile); return send(r.status, r.body); }
      send(404, { ok: false });
    });
  });
  return new Promise((resolve) => server.listen(port, '127.0.0.1', () => resolve({ events, close: () => server.close() })));
}
