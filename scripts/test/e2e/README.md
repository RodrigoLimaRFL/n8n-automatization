# End-to-end test of workflows 00 / 01 / 02 (progress events)

```
node scripts/test/e2e/run.mjs        # from the repo root; takes about 5 minutes (mostly n8n start-up and imports)
```

It starts a **throwaway** n8n server (own user folder inside the OS temp dir, port 5698, runners
broker 5693) and a fake bridge on port 5691. It never touches your real n8n (5678) or the real
bridge (5681), and deletes its temp folder at the end. Exit code is non-zero if a check fails.
Stop your own test copies first if something already listens on 5691, 5693 or 5698.

What is faked, using test copies of the real workflow files in `workflows/`:

* Gmail, Google Calendar and Google Tasks nodes become Code stubs; the schedule/startup/form
  triggers become a webhook (`/webhook/run01`).
* `127.0.0.1:5681` (the bridge) in the HTTP nodes is rewritten to the fake bridge `5691`.
  `recorder.mjs` answers `/ask` with `fake-claude.mjs` (a fake `claude -p`; markers like
  `FAKE:CREATE_EVENT` / `FAKE:FAIL` in the email subject decide the answer) and records every
  `POST /event` body in order.
* A second pair of copies sends its events to port 5699, where nothing listens (scenario d).

Scenarios (each prints PASS/FAIL):

| | what | expected events |
|---|---|---|
| a | 11 mixed emails (2 batches) | `start, total(11), progress(8), progress(3), finish` |
| b | zero emails | `start, finish` |
| b2 | only emails that were already processed | `start, finish` |
| c | one failing email in a batch of 4 (falls back to single emails) | `start, total(4), progress(4), progress(0) x4, finish` |
| d | bridge unreachable | run completes, tables are written, no events |

Scenario (c) note: the failed batch already reported `progress(4)` (the Gmail fetch happened). Workflow 01
marks the retried single emails with `retry: true`, and workflow 02 then reports `progress(0)` for them,
so progress adds up to the total. The bridge ignores `count: 0`.

Files: `run.mjs` (harness), `recorder.mjs` (fake bridge + event recorder), `fake-claude.mjs`
(fake `claude`).
