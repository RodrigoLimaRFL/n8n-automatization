# 04 · Start n8n automatically with Windows

Without this, n8n only runs while a terminal has `npm start` open. With it, n8n starts **hidden, about 30 s after you log in to Windows**. That startup fires the workflow's *When n8n starts* trigger, so a missed 07:00 run is caught up (see [03](03-schedule-and-form.md)).

## How it's set up

A **Windows Task Scheduler** task named **`n8n autostart`** runs as your user (no admin) at logon:

```
Task "n8n autostart" (at logon + 30 s)
  └─ powershell -WindowStyle Hidden -File scripts\start-n8n.ps1
       ├─ Claude bridge not running? → start it (port 5681)  >> logs\claude-bridge.log
       ├─ port 5678 already in use? → exit (never starts a second n8n)
       └─ node --env-file=.env node_modules\n8n\bin\n8n start  >> logs\n8n.log
```

Task settings worth knowing:
- **No time limit.** By default Windows kills tasks after 72 h; this one is unlimited.
- **Runs on battery** and isn't stopped when you unplug.
- **Restarts up to 3 times** (1 min apart) if n8n crashes at startup.

You can see and edit it in **Task Scheduler** (Start menu → "Task Scheduler" → *Task Scheduler Library* → *n8n autostart*).

## Commands

| Command | What it does |
|---|---|
| `npm run autostart:install` | Create or update the task (safe to re-run) |
| `npm run autostart:remove` | Delete the task (doesn't stop a running n8n) |
| `npm run stop` | Stop the n8n running on port 5678 (e.g. the hidden one) |
| `powershell -File scripts\start-n8n.ps1` | Start it in the background now, without logging out and in |

`npm start` still works for watching n8n's output live, but only one n8n can use port 5678: run `npm run stop` first. Background output goes to `logs\n8n.log`, which is rotated at 5 MB and kept out of git.

## Limits

- It starts at **logon**, not at boot: n8n waits until you sign in to Windows.
- **Sleep isn't a restart.** If the laptop sleeps through 07:00 with n8n running, nothing catches up until n8n restarts. Waking from sleep could be added as a second trigger on this task.
- `npm run stop` force-kills the process (Windows has no gentle "please stop" signal for it). That's fine for n8n's SQLite database in practice, but don't stop it mid-run if you can avoid it.
