// Claude bridge: a tiny local HTTP server that lets n8n use your Claude Pro/Max
// plan. n8n's HTTP Request node POSTs a prompt here; the bridge runs the official
// Claude Code CLI in non-interactive mode (`claude -p`) and returns its JSON.
//
//   POST http://127.0.0.1:5681/ask   { system, prompt, schema?, model?, effort? }
//   ->  200 { ok: true, output, result, usage, duration_ms }
//   ->  502 { ok: false, error }
//   GET  http://127.0.0.1:5681/health
//
// It also hosts the control API used by the desktop app and the n8n workflows
// (GET /status, POST /run, POST /event), documented in app/API.md. The state
// machine lives in run-status.mjs, the Claude CLI call in claude-runner.mjs; this file is only
// HTTP routing and wiring.
//
// Safety: the email text is untrusted, so Claude runs with NO tools, no MCP
// servers, no settings/hooks/CLAUDE.md, in an empty folder. The prompt goes in
// through stdin (never a shell). Only localhost can connect, and browser requests
// (which carry an Origin header) are refused.
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createClaudeRunner } from './claude-runner.mjs';
import { createRunStatus, isValidDays } from './run-status.mjs';
import { createN8nClient } from './n8n-client.mjs';
import { createSnapshotWriter, loadSnapshot } from './status-store.mjs';

const MAX_BODY = 2 * 1024 * 1024;

const defaultLog = (...a) => console.log(new Date().toISOString(), ...a);

const send = (res, status, body) => {
  res.writeHead(status, { 'content-type': 'application/json' });
  res.end(JSON.stringify(body));
};

const isJson = (req) => String(req.headers['content-type'] || '').includes('application/json');

/** Resolve with the parsed JSON body; null when it is not valid JSON. */
function readJson(req) {
  return new Promise((resolve) => {
    let raw = '';
    req.on('data', (chunk) => {
      raw += chunk;
      if (raw.length > MAX_BODY) req.destroy();
    });
    req.on('end', () => {
      try { resolve(JSON.parse(raw)); } catch { resolve(null); }
    });
  });
}

/**
 * Create (but do not start) the bridge server.
 * deps: { runClaude, status, n8n, log? }
 */
export function createServer({ runClaude, status, n8n, log = defaultLog }) {
  // One Claude call at a time, in arrival order.
  let queue = Promise.resolve();
  const enqueue = (job) => (queue = queue.then(job, job));
  let submitting = false; // a POST /run is waiting for n8n

  async function ask(res, body) {
    if (typeof body.prompt !== 'string' || !body.prompt.trim()) {
      return send(res, 400, { ok: false, error: '"prompt" is required' });
    }
    enqueue(async () => {
      const started = Date.now();
      const r = await runClaude(body);
      const u = r.usage || {};
      const tokens = (u.input_tokens || 0) + (u.cache_creation_input_tokens || 0) + (u.cache_read_input_tokens || 0);
      log(r.ok ? 'ok' : 'error', `${Date.now() - started}ms`, r.ok ? `tokens in=${tokens} out=${u.output_tokens || 0}` : r.error.slice(0, 300));
      if (!r.ok) status.noteClaudeError(r.error);
      send(res, r.ok ? 200 : 502, r);
    });
  }

  async function run(res, body) {
    if (!isValidDays(body?.days)) return send(res, 400, { ok: false, error: 'invalid_days' });
    if (submitting || status.isRunning()) return send(res, 409, { ok: false, error: 'already_running' });
    submitting = true;
    try {
      if (!(await n8n.submitRun(body.days))) return send(res, 502, { ok: false, error: 'n8n_unreachable' });
      status.startManual(body.days);
      send(res, 200, { ok: true });
    } finally {
      submitting = false;
    }
  }

  async function currentStatus(res, url) {
    const after = Number(url.searchParams.get('after')) || 0;
    const n8nUp = await n8n.isHealthy();
    send(res, 200, { ...status.view(after), services: { bridge: true, n8n: n8nUp } });
  }

  const POST_ROUTES = {
    '/ask': ask,
    '/run': run,
    '/event': (res, body) => (status.applyEvent(body)
      ? send(res, 200, { ok: true })
      : send(res, 400, { ok: false, error: 'invalid_event' })),
  };

  return http.createServer(async (req, res) => {
    if (req.headers.origin) return send(res, 403, { ok: false, error: 'Browser requests are not allowed' });
    const url = new URL(req.url, 'http://127.0.0.1');
    if (req.method === 'GET' && url.pathname === '/health') return send(res, 200, { ok: true });
    if (req.method === 'GET' && url.pathname === '/status') return currentStatus(res, url);
    const route = req.method === 'POST' ? POST_ROUTES[url.pathname] : undefined;
    if (!route) return send(res, 404, { ok: false, error: 'Not found' });
    if (!isJson(req)) return send(res, 415, { ok: false, error: 'Content-Type must be application/json' });
    const body = await readJson(req);
    if (body === null || typeof body !== 'object') return send(res, 400, { ok: false, error: 'Invalid JSON' });
    return route(res, body);
  });
}

function main() {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  // RUN_STATUS_FILE: only for tests, so they never touch the real logs/run-status.json.
  const statusFile = process.env.RUN_STATUS_FILE || path.join(root, 'logs', 'run-status.json');
  const port = Number(process.env.CLAUDE_BRIDGE_PORT || 5681);
  const bin = process.env.CLAUDE_BIN || 'claude';

  const runClaude = createClaudeRunner({
    bin,
    // Only for tests: extra args put before ours, e.g. ["fake-claude.mjs"] with CLAUDE_BIN=node
    binArgs: JSON.parse(process.env.CLAUDE_BIN_ARGS || '[]'),
    defaultModel: process.env.CLAUDE_MODEL || 'claude-sonnet-5-5',
    timeoutMs: Number(process.env.CLAUDE_BRIDGE_TIMEOUT_MS || 240_000),
  });
  const status = createRunStatus({
    now: () => new Date(),
    persist: createSnapshotWriter(statusFile),
    snapshot: loadSnapshot(statusFile),
  });
  const n8n = createN8nClient({
    fetch,
    baseUrl: process.env.N8N_URL || undefined,
    formId: process.env.FORM_ID || undefined,
  });

  createServer({ runClaude, status, n8n }).listen(port, '127.0.0.1', () =>
    defaultLog(`Claude bridge listening on http://127.0.0.1:${port} (bin: ${bin})`));
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
