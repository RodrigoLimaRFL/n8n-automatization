// Claude bridge: a tiny local HTTP server that lets n8n use your Claude Pro/Max
// plan. n8n's HTTP Request node POSTs a prompt here; the bridge runs the official
// Claude Code CLI in non-interactive mode (`claude -p`) and returns its JSON.
//
//   POST http://127.0.0.1:5681/ask   { system, prompt, schema?, model?, effort? }
//   ->  200 { ok: true, output, result, usage, duration_ms }
//   ->  502 { ok: false, error }
//   GET  http://127.0.0.1:5681/health
//
// Safety: the email text is untrusted, so Claude runs with NO tools, no MCP
// servers, no settings/hooks/CLAUDE.md, in an empty folder. The prompt goes in
// through stdin (never a shell). Only localhost can connect, and browser requests
// (which carry an Origin header) are refused.
import http from 'node:http';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const PORT = Number(process.env.CLAUDE_BRIDGE_PORT || 5681);
const CLAUDE_BIN = process.env.CLAUDE_BIN || 'claude';
// Only for tests: extra args put before ours, e.g. ["fake-claude.mjs"] with CLAUDE_BIN=node
const CLAUDE_BIN_ARGS = JSON.parse(process.env.CLAUDE_BIN_ARGS || '[]');
const DEFAULT_MODEL = process.env.CLAUDE_MODEL || 'claude-sonnet-5-5';
const TIMEOUT_MS = Number(process.env.CLAUDE_BRIDGE_TIMEOUT_MS || 240_000);
const MAX_BODY = 2 * 1024 * 1024;
const SANDBOX = path.join(os.tmpdir(), 'n8n-claude-bridge');
fs.mkdirSync(SANDBOX, { recursive: true });

const log = (...a) => console.log(new Date().toISOString(), ...a);

function runClaude({ system, prompt, schema, model, effort }) {
  const args = [
    ...CLAUDE_BIN_ARGS,
    '-p',
    '--output-format', 'json',
    '--model', model || DEFAULT_MODEL,
    '--tools', '',              // no tools at all: no files, no shell, no web
    '--strict-mcp-config',      // no MCP servers
    '--setting-sources', '',    // ignore user/project settings, hooks, CLAUDE.md
    '--no-session-persistence',
  ];
  if (system) args.push('--system-prompt', system);
  if (schema) args.push('--json-schema', JSON.stringify(schema));
  if (effort) args.push('--effort', effort);

  return new Promise((resolve) => {
    const child = spawn(CLAUDE_BIN, args, { cwd: SANDBOX, windowsHide: true, env: process.env });
    let stdout = '';
    let stderr = '';
    const timer = setTimeout(() => child.kill(), TIMEOUT_MS);
    child.stdout.on('data', (d) => (stdout += d));
    child.stderr.on('data', (d) => (stderr += d));
    child.on('error', (err) => {
      clearTimeout(timer);
      resolve({ ok: false, error: `Could not start ${CLAUDE_BIN}: ${err.message}` });
    });
    child.on('close', (code) => {
      clearTimeout(timer);
      let r;
      try {
        r = JSON.parse(stdout);
      } catch {
        return resolve({ ok: false, error: `claude exited with ${code}: ${(stderr || stdout).slice(0, 2000)}` });
      }
      if (r.is_error) return resolve({ ok: false, error: String(r.result || r.subtype || 'claude error') });
      let output = r.structured_output;
      if (output === undefined && schema) {
        try { output = JSON.parse(r.result); } catch { /* handled below */ }
      }
      if (schema && (output === undefined || output === null)) {
        return resolve({ ok: false, error: `No structured output. Raw result: ${String(r.result).slice(0, 2000)}` });
      }
      resolve({ ok: true, output, result: r.result, usage: r.usage, duration_ms: r.duration_ms });
    });
    child.stdin.end(prompt);
  });
}

// One Claude call at a time, in arrival order.
let queue = Promise.resolve();
const enqueue = (job) => (queue = queue.then(job, job));

const send = (res, status, body) => {
  res.writeHead(status, { 'content-type': 'application/json' });
  res.end(JSON.stringify(body));
};

const server = http.createServer((req, res) => {
  if (req.headers.origin) return send(res, 403, { ok: false, error: 'Browser requests are not allowed' });
  if (req.method === 'GET' && req.url === '/health') return send(res, 200, { ok: true });
  if (req.method !== 'POST' || req.url !== '/ask') return send(res, 404, { ok: false, error: 'Not found' });
  if (!String(req.headers['content-type'] || '').includes('application/json')) {
    return send(res, 415, { ok: false, error: 'Content-Type must be application/json' });
  }
  let raw = '';
  req.on('data', (chunk) => {
    raw += chunk;
    if (raw.length > MAX_BODY) req.destroy();
  });
  req.on('end', () => {
    let body;
    try { body = JSON.parse(raw); } catch { return send(res, 400, { ok: false, error: 'Invalid JSON' }); }
    if (typeof body.prompt !== 'string' || !body.prompt.trim()) {
      return send(res, 400, { ok: false, error: '"prompt" is required' });
    }
    enqueue(async () => {
      const started = Date.now();
      const r = await runClaude(body);
      const u = r.usage || {};
      const tokens = (u.input_tokens || 0) + (u.cache_creation_input_tokens || 0) + (u.cache_read_input_tokens || 0);
      log(r.ok ? 'ok' : 'error', `${Date.now() - started}ms`, r.ok ? `tokens in=${tokens} out=${u.output_tokens || 0}` : r.error.slice(0, 300));
      send(res, r.ok ? 200 : 502, r);
    });
  });
});

server.listen(PORT, '127.0.0.1', () => log(`Claude bridge listening on http://127.0.0.1:${PORT} (bin: ${CLAUDE_BIN})`));
