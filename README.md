# n8n automatization

A learning project: a self-hosted [n8n](https://n8n.io) instance (installed with npm) that will use **Claude** to manage **Google Calendar** from my **personal** and **university** Gmail inboxes.

**Status:** both Gmail accounts are read every morning, or on demand through a form. Claude (through your Claude plan) turns new emails into Google Calendar events and Google Tasks, updates them when plans change, and marks cancellations.

## Quick start

Requires **Node.js 24+**.

```bash
npm install
cp .env.example .env      # then set N8N_ENCRYPTION_KEY (command is in the file)
npm start                 # editor at http://localhost:5678
npm run import            # load workflows/*.json into n8n (first time)
```

On first launch n8n asks you to create a local owner account. It lives only in `.n8n/` on this machine.

Then follow **[docs/02-connect-gmail.md](docs/02-connect-gmail.md)** to connect both Gmail accounts.

## Scripts

| Command | What it does |
|---|---|
| `npm start` | Runs n8n with `.env` loaded |
| `npm run export` | Writes every workflow from n8n's DB to `workflows/` (one JSON file each). Run before committing. |
| `npm run import` | Loads every `workflows/*.json` into n8n |
| `npm run stop` | Stops n8n (port 5678) and the Claude bridge (port 5681) |
| `npm run bridge` | Runs the Claude bridge in this terminal (normally started with n8n) |
| `npm run link` | Links workflow files to your credentials by name, then imports them |
| `npm run autostart:install` / `autostart:remove` | Start n8n hidden at Windows logon, or stop doing so ([docs/04](docs/04-autostart.md)) |

> Export file names follow the workflow's internal id. Rename them if you like; import doesn't care.

## Layout

```
.env / .env.example   config (env vars); .env holds the encryption key and is gitignored
.n8n/                 n8n's live data: SQLite DB, encrypted credentials, executions (gitignored)
workflows/            workflow JSON, the version-controlled source of truth
scripts/              start/stop/autostart, the Claude bridge, credential linking
docs/                 learning notes
```

## Learn

1. [How n8n works](docs/01-how-n8n-works.md): processes, workflows, items, expressions, credentials
2. [Connect two Gmail accounts](docs/02-connect-gmail.md): Google Cloud OAuth setup
3. [Daily schedule + on-demand form](docs/03-schedule-and-form.md): multiple triggers, Gmail search, publishing
4. [Start n8n automatically with Windows](docs/04-autostart.md): Task Scheduler, background logs
5. [Claude turns emails into tasks](docs/05-email-to-tasks.md): sub-workflows, Data Tables, structured output, the Claude bridge, **setup steps**

## Roadmap

- [x] n8n running locally via npm
- [x] Read mail from both Gmail accounts
- [x] Daily run (last 3 days) + on-demand form (last N days / all)
- [x] Missed 07:00 run is caught up when n8n starts
- [x] n8n starts automatically at Windows logon
- [x] Claude reads each new email once and creates/updates/cancels Calendar events and Google Tasks
- [x] Senders that aren't people and never send tasks are skipped

## Notes

- n8n is pinned to **2.47.4** in `package.json`. At startup it warns that running outside Docker is *deprecated* and future versions will require the official Docker image. The pinned version keeps working, but read the release notes before upgrading.
- **Back up `N8N_ENCRYPTION_KEY`.** Without it, the saved credentials in `.n8n/` can't be decrypted.
