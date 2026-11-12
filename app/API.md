# Control API: Claude bridge ⇄ desktop app ⇄ n8n

The desktop app talks to **one** local server: the Claude bridge (`scripts/claude-bridge.mjs`,
`http://127.0.0.1:5681`). n8n workflows 01/02 report their progress to the same server, and the
bridge keeps the run state. The app never reads n8n's database and never talks to n8n directly.

```
 n8n workflow 01/02 ──POST /event──►  bridge  ◄──GET /status, POST /run──  desktop app
                                       │  ▲
                              POST form │  │ healthz
                                       ▼  │
                                      n8n (localhost:5678)
```

All bodies are JSON (`content-type: application/json`). Requests that carry an `Origin` header
(browsers) are refused with 403. All timestamps are **local time with offset**, e.g.
`2026-11-12T10:05:03-03:00`; the app shows the first 19 characters as local time.

## `GET /status[?after=<seq>]`

```json
{
  "state": "idle",                      // "idle" | "running" | "interrupted"
  "run": null,                          // present while state != "idle", see below
  "last_run": {                         // null until a run has finished
    "started_at":  "2026-11-12T10:05:03-03:00",
    "finished_at": "2026-11-12T10:07:41-03:00",
    "days": 5,                          // 0 = all emails
    "range_from": "2026-11-07",         // null when days = 0
    "range_to":   "2026-11-12",
    "emails_read": 68,
    "ok": true
  },
  "log": [ { "seq": 41, "ts": "2026-11-12T10:07:41-03:00", "level": "info", "msg": "Execução concluída: 68 emails lidos" } ],
  "services": { "bridge": true, "n8n": true },   // n8n = bridge could reach http://localhost:5678/healthz
  "server_time": "2026-11-12T10:08:00-03:00"
}
```

`run` while running (or interrupted):

```json
{ "source": "manual",                   // "manual" (POST /run) | "daily" (schedule/startup) | "form" (n8n form)
  "days": 5, "started_at": "...", "total": 78, "done": 12 }   // total = null until known
```

* `log` holds at most the last 200 lines. With `?after=N` only lines with `seq > N` are returned.
  `seq` increases by one per line and survives bridge restarts (persisted).
* `interrupted`: a run was `running` but no event arrived for 20 minutes (workflow crashed, n8n was
  stopped...). `POST /run` is allowed again.

## `POST /run`  body `{ "days": 5 }`  (`days: 0` = all emails; integer 0..3650)

Submits the n8n form (`http://localhost:5678/form/<webhookId>`, fields `field-0` = "Last N days" /
"All emails", `field-1` = days) and marks the run `running` with `source: "manual"`.

* 200 `{ "ok": true }`
* 400 `{ "ok": false, "error": "invalid_days" }`
* 409 `{ "ok": false, "error": "already_running" }`
* 502 `{ "ok": false, "error": "n8n_unreachable" }`

## `POST /event`  (sent by the n8n workflows; fire-and-forget)

| body | meaning |
|---|---|
| `{ "event": "start", "days": 5, "source": "schedule" \| "catch-up" \| "form" }` (`schedule` and `catch-up` both become `run.source: "daily"`) | a run began (workflow 01, after the date range is known). If the bridge already started it through `/run`, it keeps `source: "manual"`. |
| `{ "event": "total", "total": 78 }` | number of emails that will be processed this run |
| `{ "event": "progress", "count": 8 }` | `count` more emails were read (workflow 02, once per batch). `done` is capped at `total`. `count: 0` (a batch retried as single emails, already counted) is accepted and ignored: no change, no log line. |
| `{ "event": "finish" }` | the run ended: `last_run` is built from the run state, `state` becomes `idle` |

Response 200 `{ "ok": true }`; 400 on an unknown event or invalid fields. A `start` while a run is
already `running` replaces it. `progress`/`total`/`finish` without a run are ignored (200).

Every event also appends a Portuguese line to the log, e.g. `Início: últimos 5 dias`,
`78 emails para processar`, `Execução concluída: 68 emails lidos`.
Claude errors seen by the bridge (for example "You've hit your session limit") also add a `warn`
line: `Limite de uso do Claude atingido. As próximas tentativas ocorrem na próxima execução.`

## Persistence

`logs/run-status.json` (gitignored folder `logs/`): `last_run`, the log lines and the last `seq`.
`run` is not persisted: after a bridge restart `state` is `idle`.
