import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from '../claude-bridge.mjs';
import { createClaudeRunner } from '../claude-runner.mjs';
import { createRunStatus } from '../run-status.mjs';
import { createN8nClient } from '../n8n-client.mjs';

const FAKE_CLAUDE = path.join(path.dirname(fileURLToPath(import.meta.url)), 'fixtures', 'fake-claude.mjs');

let server;
let base;
let status;
let logLines;
let n8nState; // what the fake n8n does
let formCalls;

before(async () => {
  const fakeFetch = async (url, init) => {
    if (url.endsWith('/healthz')) return { ok: n8nState.healthy };
    formCalls.push({ url, init });
    if (n8nState.formError) throw new Error('ECONNREFUSED');
    return { ok: n8nState.formOk };
  };
  status = createRunStatus({ now: () => new Date() });
  logLines = [];
  server = createServer({
    runClaude: createClaudeRunner({ bin: 'node', binArgs: [FAKE_CLAUDE], defaultModel: 'm', timeoutMs: 10_000 }),
    status,
    n8n: createN8nClient({ fetch: fakeFetch }),
    log: (...a) => logLines.push(a.join(' ')),
  });
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  base = `http://127.0.0.1:${server.address().port}`;
});

after(() => new Promise((r) => server.close(r)));

const post = (p, body, headers = {}) => fetch(base + p, {
  method: 'POST',
  headers: { 'content-type': 'application/json', ...headers },
  body: typeof body === 'string' ? body : JSON.stringify(body),
});
const getStatus = async (q = '') => (await fetch(`${base}/status${q}`)).json();

async function reset() {
  n8nState = { healthy: true, formOk: true, formError: false };
  formCalls = [];
  logLines.length = 0;
  // finish any leftover run
  await post('/event', { event: 'finish' });
}

test('GET /health', async () => {
  const res = await fetch(`${base}/health`);
  assert.equal(res.status, 200);
  assert.deepEqual(await res.json(), { ok: true });
});

test('browser requests (Origin header) are refused with 403', async () => {
  for (const [p, init] of [
    ['/health', {}],
    ['/status', {}],
    ['/ask', { method: 'POST' }],
    ['/run', { method: 'POST' }],
    ['/event', { method: 'POST' }],
  ]) {
    const res = await fetch(base + p, { ...init, headers: { origin: 'http://evil.example' } });
    assert.equal(res.status, 403, p);
  }
});

test('unknown routes are 404, non-JSON posts are 415, bad JSON is 400', async () => {
  assert.equal((await fetch(`${base}/nope`)).status, 404);
  assert.equal((await fetch(`${base}/ask`)).status, 404);
  const text = await fetch(`${base}/run`, { method: 'POST', headers: { 'content-type': 'text/plain' }, body: '{}' });
  assert.equal(text.status, 415);
  assert.equal((await post('/run', '{nope')).status, 400);
  assert.equal((await post('/event', 'null')).status, 400);
});

test('/ask runs claude and logs token usage', async () => {
  await reset();
  const res = await post('/ask', { prompt: 'hello' });
  assert.equal(res.status, 200);
  const body = await res.json();
  assert.equal(body.ok, true);
  assert.equal(body.result, 'echo:hello');
  assert.equal(body.duration_ms, 12);
  assert.ok(logLines.some((l) => /^ok \d+ms tokens in=16 out=7$/.test(l)), logLines.join('|'));
  assert.deepEqual((await getStatus()).log, []);
});

test('/ask validates the prompt', async () => {
  assert.equal((await post('/ask', { prompt: '  ' })).status, 400);
  assert.equal((await post('/ask', {})).status, 400);
});

test('/ask answers requests one at a time, in order', async () => {
  await reset();
  const replies = await Promise.all(['a', 'b', 'c'].map((p) => post('/ask', { prompt: p }).then((r) => r.json())));
  assert.deepEqual(replies.map((r) => r.result), ['echo:a', 'echo:b', 'echo:c']);
});

test('/ask 502 on a claude session limit also adds a warn line to the status log', async () => {
  await reset();
  const res = await post('/ask', { prompt: 'LIMIT please' });
  assert.equal(res.status, 502);
  assert.match((await res.json()).error, /session limit/);
  assert.ok(logLines.some((l) => l.startsWith('error ')));
  const { log } = await getStatus();
  assert.equal(log.at(-1).level, 'warn');
  assert.match(log.at(-1).msg, /^Limite de uso do Claude atingido/);
});

test('POST /run submits the n8n form and marks the run as manual', async () => {
  await reset();
  const res = await post('/run', { days: 5 });
  assert.equal(res.status, 200);
  assert.deepEqual(await res.json(), { ok: true });
  assert.equal(formCalls.length, 1);
  assert.equal(formCalls[0].init.body.get('field-0'), 'Last N days');
  assert.equal(formCalls[0].init.body.get('field-1'), '5');
  const s = await getStatus();
  assert.equal(s.state, 'running');
  assert.equal(s.run.source, 'manual');
  assert.equal(s.run.days, 5);
  assert.deepEqual(s.services, { bridge: true, n8n: true });
});

test('POST /run with days 0 submits "All emails"', async () => {
  await reset();
  assert.equal((await post('/run', { days: 0 })).status, 200);
  assert.equal(formCalls[0].init.body.get('field-0'), 'All emails');
  assert.equal(formCalls[0].init.body.has('field-1'), false);
});

test('POST /run 409 while a run is active, and nothing is submitted', async () => {
  await reset();
  await post('/event', { event: 'start', days: 2, source: 'schedule' });
  const res = await post('/run', { days: 5 });
  assert.equal(res.status, 409);
  assert.deepEqual(await res.json(), { ok: false, error: 'already_running' });
  assert.equal(formCalls.length, 0);
});

test('POST /run 400 for invalid days', async () => {
  await reset();
  for (const days of [-1, 3651, 1.5, '5', null, undefined]) {
    const res = await post('/run', days === undefined ? {} : { days });
    assert.equal(res.status, 400, String(days));
    assert.deepEqual(await res.json(), { ok: false, error: 'invalid_days' });
  }
  assert.equal(formCalls.length, 0);
});

test('POST /run 502 when n8n is unreachable or refuses, and the state stays idle', async () => {
  await reset();
  n8nState.formError = true;
  let res = await post('/run', { days: 5 });
  assert.equal(res.status, 502);
  assert.deepEqual(await res.json(), { ok: false, error: 'n8n_unreachable' });
  n8nState.formError = false;
  n8nState.formOk = false;
  res = await post('/run', { days: 5 });
  assert.equal(res.status, 502);
  assert.equal((await getStatus()).state, 'idle');
});

test('concurrent POST /run: only one submits', async () => {
  await reset();
  const [a, b] = await Promise.all([post('/run', { days: 1 }), post('/run', { days: 1 })]);
  assert.deepEqual([a.status, b.status].sort(), [200, 409]);
  assert.equal(formCalls.length, 1);
});

test('POST /event drives /status; unknown events are 400', async () => {
  await reset();
  assert.equal((await post('/event', { event: 'start', days: 3, source: 'form' })).status, 200);
  await post('/event', { event: 'total', total: 4 });
  await post('/event', { event: 'progress', count: 9 });
  let s = await getStatus();
  assert.equal(s.state, 'running');
  assert.equal(s.run.done, 4);
  const bad = await post('/event', { event: 'explode' });
  assert.equal(bad.status, 400);
  assert.equal((await bad.json()).ok, false);
  await post('/event', { event: 'finish' });
  s = await getStatus();
  assert.equal(s.state, 'idle');
  assert.equal(s.run, null);
  assert.equal(s.last_run.emails_read, 4);
  assert.equal(s.last_run.days, 3);
});

test('GET /status?after=N filters the log and reports n8n health', async () => {
  await reset();
  n8nState.healthy = false;
  await post('/event', { event: 'start', days: 1, source: 'form' });
  await post('/event', { event: 'total', total: 2 });
  const all = await getStatus();
  assert.deepEqual(all.services, { bridge: true, n8n: false });
  const lastSeq = all.log.at(-1).seq;
  const tail = await getStatus(`?after=${lastSeq - 1}`);
  assert.deepEqual(tail.log.map((l) => l.seq), [lastSeq]);
  assert.deepEqual((await getStatus(`?after=${lastSeq}`)).log, []);
  assert.match(all.server_time, /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d[+-]\d\d:\d\d$/);
});
