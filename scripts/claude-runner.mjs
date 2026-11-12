// Runs the Claude Code CLI (`claude -p`) for one request and normalizes its JSON answer.
//
// Safety: the email text is untrusted, so Claude runs with NO tools, no MCP servers, no
// settings/hooks/CLAUDE.md, in an empty folder. The prompt goes in through stdin (never a shell).
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const SANDBOX = path.join(os.tmpdir(), 'n8n-claude-bridge');

/** Build a `runClaude(request)` function from the claude CLI settings. */
export function createClaudeRunner({ bin, binArgs = [], defaultModel, timeoutMs }) {
  fs.mkdirSync(SANDBOX, { recursive: true });
  return function runClaude({ system, prompt, schema, model, effort }) {
    const args = [
      ...binArgs,
      '-p',
      '--output-format', 'json',
      '--model', model || defaultModel,
      '--tools', '',              // no tools at all: no files, no shell, no web
      '--strict-mcp-config',      // no MCP servers
      '--setting-sources', '',    // ignore user/project settings, hooks, CLAUDE.md
      '--no-session-persistence',
    ];
    if (system) args.push('--system-prompt', system);
    if (schema) args.push('--json-schema', JSON.stringify(schema));
    if (effort) args.push('--effort', effort);

    return new Promise((resolve) => {
      const child = spawn(bin, args, { cwd: SANDBOX, windowsHide: true, env: process.env });
      let stdout = '';
      let stderr = '';
      const timer = setTimeout(() => child.kill(), timeoutMs);
      child.stdout.on('data', (d) => (stdout += d));
      child.stderr.on('data', (d) => (stderr += d));
      child.on('error', (err) => {
        clearTimeout(timer);
        resolve({ ok: false, error: `Could not start ${bin}: ${err.message}` });
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
  };
}
