// Ponte simulada para testar a interface (contrato em app/API.md).
// Uso: node app/tools/mock-bridge.mjs [porta=5682] [--sem-n8n]
import http from 'node:http';

const port = Number(process.argv.find((a, i) => i >= 2 && /^\d+$/.test(a)) ?? 5682);
if (port === 5678 || port === 5681) { console.error('porta reservada aos serviços reais'); process.exit(1); }
const n8nUp = !process.argv.includes('--sem-n8n');

const pad = (n) => String(n).padStart(2, '0');
const isoLocal = (d = new Date()) => {
  const off = -d.getTimezoneOffset();
  const s = off >= 0 ? '+' : '-';
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}${s}${pad(Math.floor(Math.abs(off) / 60))}:${pad(Math.abs(off) % 60)}`;
};
const dateOnly = (d) => isoLocal(d).slice(0, 10);

let seq = 0;
const log = [];
const addLog = (level, msg) => { log.push({ seq: ++seq, ts: isoLocal(), level, msg }); if (log.length > 200) log.shift(); };

let state = 'idle';
let run = null;
let lastRun = {
  started_at: '2026-11-12T10:05:03-03:00', finished_at: '2026-11-12T10:07:41-03:00',
  days: 5, range_from: '2026-11-07', range_to: '2026-11-12', emails_read: 68, ok: true,
};
addLog('info', 'Início: últimos 5 dias');
addLog('info', '78 emails para processar');
addLog('info', 'Execução concluída: 68 emails lidos');
let timer = null;

function startRun(days) {
  const now = new Date();
  run = { source: 'manual', days, started_at: isoLocal(now), total: null, done: 0 };
  state = 'running';
  addLog('info', days === 0 ? 'Início: todos os emails' : `Início: últimos ${days} dias`);
  let tick = 0;
  timer = setInterval(() => {
    tick++;
    if (tick === 2) { run.total = 40; addLog('info', '40 emails para processar'); }
    if (run.total) run.done = Math.min(run.total, run.done + 3);
    if (tick === 8) addLog('warn', 'Limite de uso do Claude atingido. As próximas tentativas ocorrem na próxima execução.');
    if (tick >= 15) {
      clearInterval(timer);
      const end = new Date();
      const from = new Date(end); from.setDate(from.getDate() - days);
      lastRun = { started_at: run.started_at, finished_at: isoLocal(end), days,
        range_from: days === 0 ? null : dateOnly(from), range_to: dateOnly(end), emails_read: run.done, ok: true };
      addLog('info', `Execução concluída: ${run.done} emails lidos`);
      state = 'idle'; run = null;
    }
  }, 1000);
}

const send = (res, code, obj) => {
  res.writeHead(code, { 'content-type': 'application/json' });
  res.end(JSON.stringify(obj));
};

http.createServer((req, res) => {
  const url = new URL(req.url, 'http://x');
  if (req.method === 'GET' && url.pathname === '/status') {
    const after = Number(url.searchParams.get('after') ?? 0);
    return send(res, 200, { state, run, last_run: lastRun, log: log.filter((l) => l.seq > after),
      services: { bridge: true, n8n: n8nUp }, server_time: isoLocal() });
  }
  if (req.method === 'POST' && url.pathname === '/run') {
    let body = '';
    req.on('data', (c) => (body += c));
    req.on('end', () => {
      let days; try { days = JSON.parse(body).days; } catch { /* inválido */ }
      if (!Number.isInteger(days) || days < 0 || days > 3650) return send(res, 400, { ok: false, error: 'invalid_days' });
      if (state === 'running') return send(res, 409, { ok: false, error: 'already_running' });
      if (!n8nUp) return send(res, 502, { ok: false, error: 'n8n_unreachable' });
      startRun(days);
      send(res, 200, { ok: true });
    });
    return;
  }
  send(res, 404, { ok: false, error: 'not_found' });
}).listen(port, '127.0.0.1', () => console.log(`ponte simulada em http://127.0.0.1:${port}`));
