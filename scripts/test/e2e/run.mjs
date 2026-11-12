// End-to-end test of workflows 00 + 01 + 02 in a THROWAWAY n8n server (never touches your real n8n).
// Gmail / Calendar / Tasks nodes are replaced by Code stubs, triggers by webhooks, and a fake bridge
// (recorder.mjs) answers /ask with a fake `claude` and records the progress events from /event.
// Usage (from the repo root):  node scripts/test/e2e/run.mjs      See README.md.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync, spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { startRecorder } from './recorder.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const PROJECT = path.resolve(HERE, '../../..');
const DIR = fs.mkdtempSync(path.join(os.tmpdir(), 'n8n-e2e-'));
const PORT = 5698, BROKER = '5693', BRIDGE_PORT = 5691, DEAD_PORT = 5699; // 5699: nothing listens there
const LOG = path.join(DIR, 'fake.log');
const SERVER_LOG = path.join(DIR, 'server.log');
const N8N = path.join(PROJECT, 'node_modules/n8n/bin/n8n');
const env = { ...process.env, N8N_USER_FOLDER: DIR, N8N_ENCRYPTION_KEY: 'testkey123', N8N_RUNNERS_BROKER_PORT: BROKER, N8N_PORT: String(PORT), GENERIC_TIMEZONE: 'America/Sao_Paulo', N8N_DIAGNOSTICS_ENABLED: 'false' };

const n8n = (...args) => execFileSync(process.execPath, [N8N, ...args], { env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
const read = (f) => JSON.parse(fs.readFileSync(path.join(PROJECT, 'workflows', f), 'utf8'));
const save = (name, wf) => { const p = path.join(DIR, name); fs.writeFileSync(p, JSON.stringify(wf)); return p; };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const stub = (n, jsCode, mode = 'runOnceForEachItem') => { n.type = 'n8n-nodes-base.code'; n.typeVersion = 2; n.parameters = { mode, jsCode }; delete n.credentials; };
const webhook = (p, position) => ({ parameters: { httpMethod: 'POST', path: p, responseMode: 'lastNode', options: {} }, id: 'wh-' + p, name: 'Webhook', type: 'n8n-nodes-base.webhook', typeVersion: 2, position, webhookId: crypto.randomUUID() });
const post = async (p, body = {}) => { const r = await fetch(`http://127.0.0.1:${PORT}/webhook/${p}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) }); return { status: r.status, body: await r.text() }; };

// Test copies of 02 and 01 (one pair per event port, so scenario (d) can point at a dead port).
function build(tag, eventPort) {
  const url = (n) => { if (n.parameters?.url) n.parameters.url = n.parameters.url.replace('127.0.0.1:5681/event', `127.0.0.1:${eventPort}/event`).replace('127.0.0.1:5681/ask', `127.0.0.1:${BRIDGE_PORT}/ask`); };
  const wf2 = read('02-email-to-tasks.json');
  wf2.id = 'emailToTasks' + tag; wf2.name += ' ' + tag;
  for (const n of wf2.nodes) {
    if (n.type === 'n8n-nodes-base.gmail') stub(n, "return { json: { id: $json.id, subject: $json.subject, text: 'Corpo do email: ' + $json.subject, from: { value: [{ address: $json.sender, name: 'Remetente' }] }, date: $json.receivedAt, headers: /LIST/.test($json.subject) ? { 'list-id': 'grad-l' } : {} } };");
    if (n.type === 'n8n-nodes-base.googleCalendar') stub(n, `return { json: { id: 'evt-' + Math.random().toString(36).slice(2, 8), node: ${JSON.stringify(n.name)}, got: $('What to do').item.json } };`);
    if (n.type === 'n8n-nodes-base.googleTasks') stub(n, `return { json: { id: 'gt-' + Math.random().toString(36).slice(2, 8), node: ${JSON.stringify(n.name)} } };`);
    url(n);
  }
  const wf1 = read('01-gmail-both-inboxes.json');
  wf1.id = 'testParent' + tag; wf1.name += ' ' + tag;
  wf1.nodes.push(webhook('run01' + tag, [0, 600]));
  wf1.connections.Webhook = { main: [[{ node: 'On demand: chosen range', type: 'main', index: 0 }]] };
  const triggers = ['Every day at 07:00', 'When n8n starts or is published', 'Read emails on demand'];
  for (const n of wf1.nodes) {
    if (n.name === 'Personal: emails') stub(n, "return $('Webhook').first().json.body.emails.map((json) => ({ json }));", 'runOnceForAllItems');
    if (n.name === 'University: emails') stub(n, "return ($('Webhook').first().json.body.universityEmails || []).map((json) => ({ json }));", 'runOnceForAllItems');
    if (n.type === 'n8n-nodes-base.executeWorkflow') n.parameters.workflowId.value = wf2.id;
    url(n);
  }
  for (const t of triggers) delete wf1.connections[t];
  wf1.nodes = wf1.nodes.filter((n) => !triggers.includes(n.name));
  return { wf1, wf2 };
}

const setup = read('00-setup-data-tables.json');
setup.nodes = setup.nodes.filter((n) => n.type !== 'n8n-nodes-base.manualTrigger');
setup.nodes.push(webhook('setup', [0, 0]));
setup.connections = { Webhook: setup.connections['Run once to set up'], ...Object.fromEntries(Object.entries(setup.connections).filter(([k]) => k !== 'Run once to set up')) };

const dtNode = (id, name, table, x) => ({ parameters: { resource: 'row', operation: 'get', dataTableId: { __rl: true, mode: 'name', value: table }, returnAll: true }, id, name, type: 'n8n-nodes-base.dataTable', typeVersion: 1.1, position: [x, 0], executeOnce: true, alwaysOutputData: true });
const reader = { id: 'readTables000001', name: 'read', nodes: [webhook('read', [0, 0]), dtNode('r1', 'T', 'n8n_email_tasks', 200), dtNode('r2', 'P', 'n8n_processed_emails', 400), dtNode('r3', 'S', 'n8n_email_senders', 600),
  { parameters: { mode: 'runOnceForAllItems', jsCode: "const g = (n) => $(n).all().map(i => i.json).filter(x => x.id); return [{ json: { tasks: g('T'), processed: g('P'), senders: g('S') } }];" }, id: 'r4', name: 'All', type: 'n8n-nodes-base.code', typeVersion: 2, position: [800, 0] }],
  connections: { Webhook: { main: [[{ node: 'T', type: 'main', index: 0 }]] }, T: { main: [[{ node: 'P', type: 'main', index: 0 }]] }, P: { main: [[{ node: 'S', type: 'main', index: 0 }]] }, S: { main: [[{ node: 'All', type: 'main', index: 0 }]] } }, settings: { executionOrder: 'v1' } };

// ── assertions ──
let failures = 0;
const check = (name, ok, detail = '') => { console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${ok ? '' : '  ' + detail}`); if (!ok) failures++; };
const fmt = (evs) => evs.map((e) => e.event === 'start' ? `start(days=${e.days},source=${e.source})` : e.event === 'total' ? `total(${e.total})` : e.event === 'progress' ? `progress(${e.count})` : e.event).join(', ');
const mk = (list) => list.map(([id, addr, subj], k) => ({ id, From: `Fulano <${addr}>`, Subject: subj, snippet: '', internalDate: String(Date.parse('2026-11-10T12:00:00Z') + k * 60000), labels: [] }));
const callRun = (tag, emails) => post('run01' + tag, { emails: mk(emails.filter((e) => !e[0].startsWith('u'))), universityEmails: mk(emails.filter((e) => e[0].startsWith('u'))) });

const taskWf = [];
for (const [tag, port] of [['', BRIDGE_PORT], ['Down', DEAD_PORT]]) { const { wf1, wf2 } = build(tag, port); taskWf.push([`02${tag}.json`, wf2], [`01${tag}.json`, wf1]); }

const killTree = (p) => { try { if (process.platform === 'win32') execFileSync('taskkill', ['/PID', String(p.pid), '/T', '/F'], { stdio: 'ignore' }); else p.kill(); } catch {} };
let server, recorder;
try {
  for (const [f, wf] of [['00.json', setup], ...taskWf, ['reader.json', reader]]) {
    n8n('import:workflow', `--input=${save(f, wf)}`);
    n8n('publish:workflow', `--id=${wf.id}`);
  }
  recorder = await startRecorder({ port: BRIDGE_PORT, logFile: LOG });
  const fd = fs.openSync(SERVER_LOG, 'w');
  server = spawn(process.execPath, [N8N, 'start'], { env, stdio: ['ignore', fd, fd] });
  for (let i = 0; i < 90; i++) { try { if ((await fetch(`http://127.0.0.1:${PORT}/healthz`)).ok) break; } catch {} await sleep(1000); }
  for (let i = 0; i < 90; i++) { await sleep(2000); const s = fs.readFileSync(SERVER_LOG, 'utf8'); if ((await post('nonexistent-probe').catch(() => ({ status: 0 }))).status === 404 && s.includes('Editor is now accessible')) break; }
  for (let i = 0; i < 60; i++) { if (fs.readFileSync(SERVER_LOG, 'utf8').includes('Finished building workflow dependency index')) break; await sleep(2000); }
  await sleep(3000);
  await post('setup'); await post('setup');

  const scenario = async (title, tag, emails) => {
    const from = recorder.events.length;
    const res = await callRun(tag, emails);
    const evs = recorder.events.slice(from);
    console.log(`\n[${title}] HTTP ${res.status}; events: ${fmt(evs)}`);
    return { res, evs };
  };
  const tables = async () => { const r = JSON.parse((await post('read')).body); return Array.isArray(r) ? r[0] : r; };

  // (a) 11 mixed emails -> 2 batches (8 + 3)
  const a = await scenario('a: 11 emails, 2 batches', '', [['e1', 'prof@uni.br', 'Prova FAKE:CREATE_EVENT'], ['u2', 'prof@uni.br', 'Leitura FAKE:CREATE_LIST'], ['u3', 'secretaria@uni.br', 'Matricula FAKE:CREATE_ALLDAY'], ['n1', 'news@loja.com', 'Oferta FAKE:NONE'], ['n2', 'news@loja.com', 'Oferta FAKE:NONE'], ['n3', 'news@loja.com', 'Oferta FAKE:NONE'], ['e4', 'camila@uni.br', 'Apresentacao FAKE:SAME'], ['u5', 'camila@uni.br', 'Apresentacao corrigida FAKE:SAME'], ['n4', 'news@loja.com', 'Oferta FAKE:NONE'], ['e6', 'grad@uni.br', 'LIST Divulgacao FAKE:NONE'], ['e7', 'colega@gmail.com', 'Oi FAKE:NONE']]);
  check('a: HTTP 200', a.res.status === 200, a.res.body.slice(0, 200));
  check('a: sequence start, total(11), progress(8), progress(3), finish', fmt(a.evs) === 'start(days=3,source=form), total(11), progress(8), progress(3), finish', fmt(a.evs));
  check('a: progress sums to total', a.evs.filter((e) => e.event === 'progress').reduce((s, e) => s + e.count, 0) === 11);
  check('a: finish is the last event, after all progress', a.evs.at(-1)?.event === 'finish');

  // (b) zero emails
  const b = await scenario('b: zero emails', '', []);
  check('b: HTTP 200', b.res.status === 200, b.res.body.slice(0, 200));
  check('b: sequence is start, finish (no total/progress)', fmt(b.evs) === 'start(days=3,source=form), finish', fmt(b.evs));

  // (b2) only already-processed emails: nothing to do, so no total either
  const b2 = await scenario('b2: only emails already processed', '', [['e1', 'prof@uni.br', 'Prova FAKE:CREATE_EVENT']]);
  check('b2: sequence is start, finish (no total/progress)', fmt(b2.evs) === 'start(days=3,source=form), finish', fmt(b2.evs));

  // (c) failing batch -> retried as single emails
  const c = await scenario('c: failing batch falls back to single emails', '', [['e8', 'prof@uni.br', 'Remarcada FAKE:NONE'], ['e9', 'prof@uni.br', 'Prazo FAKE:NONE'], ['e10', 'prof@uni.br', 'Cancelada FAKE:NONE'], ['x1', 'estranho@x.com', 'Quebra FAKE:FAIL']]);
  check('c: HTTP 200', c.res.status === 200, c.res.body.slice(0, 200));
  check('c: sequence start, total(4), progress(4) for the batch, then progress(0) x4 (retries are not counted twice), finish', fmt(c.evs) === 'start(days=3,source=form), total(4), progress(4), progress(0), progress(0), progress(0), progress(0), finish', fmt(c.evs));
  const cSum = c.evs.filter((e) => e.event === 'progress').reduce((s, e) => s + e.count, 0);
  check('c: progress adds up to the total (4), not more', cSum === 4, String(cSum));

  // (d) bridge unreachable (event URL -> closed port)
  const before = recorder.events.length;
  const t0 = Date.now();
  const d = await callRun('Down', [['d1', 'prof@uni.br', 'Prova D FAKE:CREATE_EVENT'], ['ud2', 'prof@uni.br', 'Leitura D FAKE:CREATE_LIST']]);
  console.log(`\n[d: events go to a closed port] HTTP ${d.status}; ${Date.now() - t0} ms; events received by the recorder: ${recorder.events.length - before}`);
  check('d: workflow still completes (HTTP 200)', d.status === 200, d.body.slice(0, 200));
  const t = await tables();
  check('d: emails processed and tasks written despite the dead event port', ['d1', 'ud2'].every((id) => t.processed.some((p) => p.message_id === id)) && t.tasks.some((x) => x.title === 'Prova de Cálculo') && t.tasks.some((x) => x.title === 'Ler capítulo 3'), JSON.stringify(t.processed.map((p) => p.message_id)));
  check('d: no events were recorded for that run', recorder.events.length === before);

  // sanity: workflows (a)-(c) also wrote their tables
  check('tables: all emails of (a) and (c) recorded as processed', ['e1', 'e7', 'u5', 'e8'].every((id) => t.processed.some((p) => p.message_id === id)));
  const errs = fs.readFileSync(SERVER_LOG, 'utf8').split('\n').filter((l) => /error|problem/i.test(l) && !/N8N_|deprecat|Python|simulated failure|AxiosError|Bad Gateway|ECONNREFUSED|5699|Failed to|connect/i.test(l));
  if (errs.length) console.log('\nserver log (suspicious lines):\n' + errs.slice(0, 8).join('\n'));
} catch (e) {
  failures++; console.log('FAIL  harness error:', e?.stack || e);
} finally {
  if (server) killTree(server);
  recorder?.close();
  await sleep(1500);
  for (let i = 0; i < 5; i++) { try { fs.rmSync(DIR, { recursive: true, force: true }); break; } catch { await sleep(1500); } }
  console.log(failures ? `\n${failures} check(s) FAILED` : '\nAll checks passed');
  process.exit(failures ? 1 : 0);
}
