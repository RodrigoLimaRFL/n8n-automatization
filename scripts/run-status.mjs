// Run-status state machine for the control API (see app/API.md).
//
// Pure module: no HTTP, no file system, no clock. The caller injects
//   now()              -> Date, the current time
//   persist(snapshot)  -> called after every change with { last_run, log, seq }
//   snapshot           -> the value previously given to persist (restores state after a restart)

export const LOG_LIMIT = 200;
export const INTERRUPTED_AFTER_MS = 20 * 60 * 1000;
export const MAX_DAYS = 3650;
export const SESSION_LIMIT_MESSAGE =
  'Limite de uso do Claude atingido. As próximas tentativas ocorrem na próxima execução.';

const EVENT_SOURCES = { schedule: 'daily', 'catch-up': 'daily', form: 'form' };
// Handler result for a valid event that changed nothing (no run to apply it to).
const IGNORED = 'ignored';
const pad = (n) => String(n).padStart(2, '0');

// ---- time helpers (local time with numeric offset) --------------------------

/** Minutes east of UTC (e.g. -180) -> "-03:00". */
export function formatOffset(minutesEast) {
  const sign = minutesEast < 0 ? '-' : '+';
  const abs = Math.abs(minutesEast);
  return `${sign}${pad(Math.floor(abs / 60))}:${pad(abs % 60)}`;
}

/** Local calendar date, "2026-11-12". */
export function formatLocalDate(date) {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** Local time with offset, "2026-11-12T10:05:03-03:00". */
export function formatLocalTimestamp(date) {
  const time = `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
  return `${formatLocalDate(date)}T${time}${formatOffset(-date.getTimezoneOffset())}`;
}

// ---- Portuguese phrases -----------------------------------------------------

export const emailCount = (n) => `${n} ${n === 1 ? 'email' : 'emails'}`;

export const emailsRead = (n) => `${n} ${n === 1 ? 'email lido' : 'emails lidos'}`;

export function describeDays(days) {
  if (days === 0) return 'todos os emails';
  return days === 1 ? 'último dia' : `últimos ${days} dias`;
}

// ---- validation -------------------------------------------------------------

export const isValidDays = (v) => Number.isInteger(v) && v >= 0 && v <= MAX_DAYS;
const isCount = (v) => Number.isInteger(v) && v >= 0;

// ---- state machine ----------------------------------------------------------

export function createRunStatus({
  now,
  persist = () => {},
  snapshot = null,
  logLimit = LOG_LIMIT,
  interruptedAfterMs = INTERRUPTED_AFTER_MS,
}) {
  let lastRun = snapshot?.last_run ?? null;
  let log = Array.isArray(snapshot?.log) ? snapshot.log.slice(-logLimit) : [];
  let seq = Math.max(Number(snapshot?.seq) || 0, log.at(-1)?.seq ?? 0);
  let run = null;          // { source, days, started_at, total, done }
  let lastActivity = 0;    // ms timestamp of the last event for `run`

  const addLog = (level, msg) => {
    log.push({ seq: ++seq, ts: formatLocalTimestamp(now()), level, msg });
    if (log.length > logLimit) log = log.slice(-logLimit);
  };
  const save = () => persist({ last_run: lastRun, log, seq });
  const touch = () => { lastActivity = now().getTime(); };

  const state = () => {
    if (!run) return 'idle';
    return now().getTime() - lastActivity >= interruptedAfterMs ? 'interrupted' : 'running';
  };

  const begin = (source, days) => {
    run = { source, days, started_at: formatLocalTimestamp(now()), total: null, done: 0 };
    touch();
  };

  const finishRun = () => {
    const end = now();
    const to = formatLocalDate(end);
    const from = run.days > 0
      ? formatLocalDate(new Date(end.getFullYear(), end.getMonth(), end.getDate() - run.days))
      : null;
    lastRun = {
      started_at: run.started_at,
      finished_at: formatLocalTimestamp(end),
      days: run.days,
      range_from: from,
      range_to: to,
      emails_read: run.done,
      ok: true,
    };
    addLog('info', `Execução concluída: ${emailsRead(run.done)}`);
    run = null;
  };

  const handlers = {
    start(e) {
      if (!isValidDays(e.days)) return false;
      if (e.source !== undefined && !(e.source in EVENT_SOURCES)) return false;
      // A run the bridge started through /run keeps source "manual".
      const keepManual = state() === 'running' && run.source === 'manual';
      begin(keepManual ? 'manual' : EVENT_SOURCES[e.source ?? 'schedule'], e.days);
      addLog('info', `Início: ${describeDays(e.days)}`);
      return true;
    },
    total(e) {
      if (!isCount(e.total)) return false;
      if (!run) return IGNORED;
      run.total = e.total;
      run.done = Math.min(run.done, e.total);
      touch();
      addLog('info', `${emailCount(e.total)} para processar`);
      return true;
    },
    progress(e) {
      if (!isCount(e.count)) return false;
      // count 0: a batch retried as single emails is not counted twice (workflow 02).
      if (!run || e.count === 0) return IGNORED;
      run.done += e.count;
      if (run.total !== null) run.done = Math.min(run.done, run.total);
      touch();
      addLog('info', emailsRead(e.count));
      return true;
    },
    finish() {
      if (!run) return IGNORED;
      finishRun();
      return true;
    },
  };

  return {
    /** True while a run is active; an interrupted run does not block a new one. */
    isRunning: () => state() === 'running',

    /** The bridge submitted the n8n form for a manual run. */
    startManual(days) {
      if (state() === 'running') {
        // n8n's own "start" event beat us to it: just claim the run.
        run.source = 'manual';
      } else {
        begin('manual', days);
        addLog('info', `Execução manual solicitada: ${describeDays(days)}`);
      }
      save();
    },

    /** Apply a POST /event body. Returns false for an unknown event or bad fields. */
    applyEvent(body) {
      const handler = Object.hasOwn(handlers, body?.event) ? handlers[body.event] : null;
      const result = handler?.(body);
      if (!result) return false;
      if (result !== IGNORED) save();
      return true;
    },

    /** Record a Claude error seen by the bridge; adds a warning for usage limits. */
    noteClaudeError(text) {
      if (!/session limit/i.test(String(text))) return;
      addLog('warn', SESSION_LIMIT_MESSAGE);
      save();
    },

    /** Body of GET /status without the `services` field. */
    view(after = 0) {
      const s = state();
      return {
        state: s,
        run: run ? { ...run } : null,
        last_run: lastRun,
        log: log.filter((l) => l.seq > after),
        server_time: formatLocalTimestamp(now()),
      };
    },
  };
}
