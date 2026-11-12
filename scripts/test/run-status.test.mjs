import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  createRunStatus, formatLocalTimestamp, formatLocalDate, formatOffset, describeDays, emailCount,
  SESSION_LIMIT_MESSAGE, INTERRUPTED_AFTER_MS,
} from '../run-status.mjs';

// Fake clock in local time so assertions do not depend on the machine's time zone.
function setup(snapshot = null, start = new Date(2026, 10, 12, 10, 5, 3)) {
  const clock = { t: start.getTime() };
  const saved = [];
  const status = createRunStatus({ now: () => new Date(clock.t), persist: (s) => saved.push(structuredClone(s)), snapshot });
  const advance = (ms) => { clock.t += ms; };
  return { status, saved, advance, clock };
}
const ev = (status, body) => assert.equal(status.applyEvent(body), true);
const msgs = (status) => status.view().log.map((l) => l.msg);

test('formatOffset', () => {
  assert.equal(formatOffset(-180), '-03:00');
  assert.equal(formatOffset(0), '+00:00');
  assert.equal(formatOffset(330), '+05:30');
  assert.equal(formatOffset(-570), '-09:30');
});

test('formatLocalTimestamp / formatLocalDate use local fields plus offset', () => {
  const d = new Date(2026, 10, 2, 9, 5, 3);
  assert.equal(formatLocalDate(d), '2026-11-02');
  const ts = formatLocalTimestamp(d);
  assert.match(ts, /^2026-11-02T09:05:03[+-]\d\d:\d\d$/);
  assert.equal(ts.slice(19), formatOffset(-d.getTimezoneOffset()));
});

test('Portuguese phrases and plurals', () => {
  assert.equal(describeDays(0), 'todos os emails');
  assert.equal(describeDays(1), 'último dia');
  assert.equal(describeDays(5), 'últimos 5 dias');
  assert.equal(emailCount(1), '1 email');
  assert.equal(emailCount(2), '2 emails');
});

test('starts idle', () => {
  const { status } = setup();
  const v = status.view();
  assert.equal(v.state, 'idle');
  assert.equal(v.run, null);
  assert.equal(v.last_run, null);
  assert.deepEqual(v.log, []);
  assert.match(v.server_time, /^2026-11-12T10:05:03/);
});

test('startup catch-up start is accepted and counts as a daily run', () => {
  const { status } = setup();
  ev(status, { event: 'start', days: 3, source: 'catch-up' });
  assert.equal(status.view().state, 'running');
  assert.equal(status.view().run.source, 'daily');
});

test('full run: start, total, progress, finish', () => {
  const { status, advance } = setup();
  ev(status, { event: 'start', days: 5, source: 'schedule' });
  let v = status.view();
  assert.equal(v.state, 'running');
  assert.deepEqual(v.run, { source: 'daily', days: 5, started_at: v.run.started_at, total: null, done: 0 });

  ev(status, { event: 'total', total: 78 });
  ev(status, { event: 'progress', count: 8 });
  ev(status, { event: 'progress', count: 1 });
  v = status.view();
  assert.equal(v.run.total, 78);
  assert.equal(v.run.done, 9);

  advance(158_000);
  ev(status, { event: 'finish' });
  v = status.view();
  assert.equal(v.state, 'idle');
  assert.equal(v.run, null);
  assert.equal(v.last_run.days, 5);
  assert.equal(v.last_run.emails_read, 9);
  assert.equal(v.last_run.ok, true);
  assert.match(v.last_run.started_at, /^2026-11-12T10:05:03/);
  assert.match(v.last_run.finished_at, /^2026-11-12T10:07:41/);
  assert.deepEqual(msgs(status), [
    'Início: últimos 5 dias',
    '78 emails para processar',
    '8 emails lidos',
    '1 email lido',
    'Execução concluída: 9 emails lidos',
  ]);
  assert.deepEqual(v.log.map((l) => l.seq), [1, 2, 3, 4, 5]);
  assert.ok(v.log.every((l) => l.level === 'info'));
});

test('form source and singular day/email wording', () => {
  const { status } = setup();
  ev(status, { event: 'start', days: 1, source: 'form' });
  assert.equal(status.view().run.source, 'form');
  ev(status, { event: 'total', total: 1 });
  ev(status, { event: 'progress', count: 1 });
  ev(status, { event: 'finish' });
  assert.deepEqual(msgs(status), ['Início: último dia', '1 email para processar', '1 email lido', 'Execução concluída: 1 email lido']);
});

test('start without source defaults to daily; all emails wording', () => {
  const { status } = setup();
  ev(status, { event: 'start', days: 0 });
  assert.equal(status.view().run.source, 'daily');
  assert.deepEqual(msgs(status), ['Início: todos os emails']);
});

test('progress is capped at total', () => {
  const { status } = setup();
  ev(status, { event: 'start', days: 2, source: 'form' });
  ev(status, { event: 'total', total: 10 });
  ev(status, { event: 'progress', count: 8 });
  ev(status, { event: 'progress', count: 8 });
  assert.equal(status.view().run.done, 10);
});

test('progress with count 0 (retried batch) changes nothing and adds no log line', () => {
  const { status } = setup();
  ev(status, { event: 'start', days: 2, source: 'form' });
  ev(status, { event: 'total', total: 10 });
  ev(status, { event: 'progress', count: 0 });
  assert.equal(status.view().run.done, 0);
  assert.deepEqual(msgs(status), ['Início: últimos 2 dias', '10 emails para processar']);
});

test('progress before total is kept, then total caps it', () => {
  const { status } = setup();
  ev(status, { event: 'start', days: 2, source: 'form' });
  ev(status, { event: 'progress', count: 8 });
  assert.equal(status.view().run.done, 8);
  ev(status, { event: 'total', total: 5 });
  assert.equal(status.view().run.done, 5);
});

test('start replaces a running run', () => {
  const { status } = setup();
  ev(status, { event: 'start', days: 5, source: 'form' });
  ev(status, { event: 'total', total: 10 });
  ev(status, { event: 'progress', count: 4 });
  ev(status, { event: 'start', days: 3, source: 'schedule' });
  const { run } = status.view();
  assert.equal(run.days, 3);
  assert.equal(run.source, 'daily');
  assert.equal(run.total, null);
  assert.equal(run.done, 0);
});

test('source stays "manual" for runs started through /run', () => {
  const { status } = setup();
  status.startManual(5);
  assert.equal(status.view().run.source, 'manual');
  ev(status, { event: 'start', days: 5, source: 'form' });
  assert.equal(status.view().run.source, 'manual');
  ev(status, { event: 'total', total: 3 });
  assert.equal(status.view().run.source, 'manual');
});

test('startManual claims a run n8n already announced', () => {
  const { status } = setup();
  ev(status, { event: 'start', days: 5, source: 'form' });
  ev(status, { event: 'total', total: 3 });
  status.startManual(5);
  const { run } = status.view();
  assert.equal(run.source, 'manual');
  assert.equal(run.total, 3);
  assert.equal(msgs(status).length, 2);
});

test('a new manual run is logged in Portuguese', () => {
  const { status } = setup();
  status.startManual(0);
  assert.deepEqual(msgs(status), ['Execução manual solicitada: todos os emails']);
});

test('progress/total/finish without a run are ignored but accepted', () => {
  const { status, saved } = setup();
  ev(status, { event: 'total', total: 5 });
  ev(status, { event: 'progress', count: 2 });
  ev(status, { event: 'finish' });
  const v = status.view();
  assert.equal(v.state, 'idle');
  assert.equal(v.last_run, null);
  assert.deepEqual(v.log, []);
  assert.deepEqual(saved, []);
});

test('invalid events are rejected and change nothing', () => {
  const { status, saved } = setup();
  const bad = [
    null, {}, { event: 'nope' }, { event: 'toString' },
    { event: 'start' }, { event: 'start', days: -1 }, { event: 'start', days: 3651 },
    { event: 'start', days: 1.5 }, { event: 'start', days: '5' }, { event: 'start', days: 5, source: 'cron' },
    { event: 'total' }, { event: 'total', total: -1 }, { event: 'progress', count: 'x' },
  ];
  for (const b of bad) assert.equal(status.applyEvent(b), false, JSON.stringify(b));
  assert.equal(status.view().state, 'idle');
  assert.deepEqual(saved, []);
});

test('interrupted after 20 minutes without events; activity resets the timer', () => {
  const { status, advance } = setup();
  ev(status, { event: 'start', days: 5, source: 'form' });
  advance(INTERRUPTED_AFTER_MS - 1000);
  assert.equal(status.view().state, 'running');
  ev(status, { event: 'progress', count: 1 });
  advance(INTERRUPTED_AFTER_MS - 1000);
  assert.equal(status.view().state, 'running');
  advance(1000);
  let v = status.view();
  assert.equal(v.state, 'interrupted');
  assert.equal(v.run.days, 5);
  assert.equal(status.isRunning(), false);

  // a new manual run is allowed again and replaces the interrupted one
  status.startManual(2);
  v = status.view();
  assert.equal(v.state, 'running');
  assert.equal(v.run.days, 2);
  assert.equal(v.run.done, 0);
});

test('last_run range: days back from the finish date, null for all emails', () => {
  const a = setup(null, new Date(2026, 10, 12, 23, 59, 0));
  ev(a.status, { event: 'start', days: 5, source: 'form' });
  ev(a.status, { event: 'finish' });
  assert.equal(a.status.view().last_run.range_from, '2026-11-07');
  assert.equal(a.status.view().last_run.range_to, '2026-11-12');

  const b = setup(null, new Date(2026, 2, 3, 8, 0, 0));
  ev(b.status, { event: 'start', days: 5, source: 'form' });
  ev(b.status, { event: 'finish' });
  assert.equal(b.status.view().last_run.range_from, '2026-02-26');

  const c = setup(null, new Date(2026, 0, 2, 8, 0, 0));
  ev(c.status, { event: 'start', days: 0, source: 'form' });
  ev(c.status, { event: 'finish' });
  assert.equal(c.status.view().last_run.range_from, null);
  assert.equal(c.status.view().last_run.range_to, '2026-01-02');
});

test('range_to is the date of finish, even when the run crossed midnight', () => {
  const { status, advance } = setup(null, new Date(2026, 10, 12, 23, 50, 0));
  ev(status, { event: 'start', days: 1, source: 'form' });
  advance(20 * 60 * 1000 - 1000); // stay under the interrupted limit
  ev(status, { event: 'finish' });
  const lr = status.view().last_run;
  assert.equal(lr.range_to, '2026-11-13');
  assert.equal(lr.range_from, '2026-11-12');
});

test('log keeps the last 200 lines and `after` filters by seq', () => {
  const { status } = setup();
  ev(status, { event: 'start', days: 5, source: 'form' });
  for (let i = 0; i < 249; i++) ev(status, { event: 'progress', count: 1 });
  const log = status.view().log;
  assert.equal(log.length, 200);
  assert.equal(log[0].seq, 51);
  assert.equal(log.at(-1).seq, 250);
  assert.deepEqual(status.view(248).log.map((l) => l.seq), [249, 250]);
  assert.deepEqual(status.view(250).log, []);
  assert.equal(status.view(0).log.length, 200);
});

test('every change is persisted with last_run, log and seq', () => {
  const { status, saved } = setup();
  ev(status, { event: 'start', days: 5, source: 'form' });
  ev(status, { event: 'finish' });
  assert.equal(saved.length, 2);
  const last = saved.at(-1);
  assert.equal(last.seq, 2);
  assert.equal(last.log.length, 2);
  assert.equal(last.last_run.days, 5);
  assert.equal('run' in last, false);
});

test('seq, log and last_run survive a restart; run does not', () => {
  const first = setup();
  ev(first.status, { event: 'start', days: 5, source: 'form' });
  ev(first.status, { event: 'finish' });
  ev(first.status, { event: 'start', days: 5, source: 'form' });
  const snapshot = first.saved.at(-1);

  const second = setup(snapshot);
  let v = second.status.view();
  assert.equal(v.state, 'idle');
  assert.equal(v.run, null);
  assert.equal(v.last_run.days, 5);
  assert.equal(v.log.length, 3);
  ev(second.status, { event: 'start', days: 1, source: 'form' });
  assert.equal(second.status.view().log.at(-1).seq, 4);
});

test('restoring tolerates a snapshot whose seq is behind its log', () => {
  const { status } = setup({ last_run: null, seq: 0, log: [{ seq: 7, ts: 'x', level: 'info', msg: 'm' }] });
  ev(status, { event: 'start', days: 1, source: 'form' });
  assert.equal(status.view().log.at(-1).seq, 8);
});

test('session limit errors add one warn line; other errors do not', () => {
  const { status, saved } = setup();
  status.noteClaudeError('claude exited with 1: boom');
  assert.deepEqual(status.view().log, []);
  status.noteClaudeError("You've hit your Session Limit - resets 5pm");
  const [line] = status.view().log;
  assert.equal(line.level, 'warn');
  assert.equal(line.msg, SESSION_LIMIT_MESSAGE);
  assert.equal(saved.length, 1);
});
