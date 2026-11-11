# n8n automatization

A learning project: a self-hosted [n8n](https://n8n.io) instance (installed with npm) that will use **Claude** to manage **Google Calendar** from my **personal** and **university** Gmail inboxes.

**Status:** step 1, both Gmail accounts connected and read by a starter workflow.

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

> Export file names follow the workflow's internal id. Rename them if you like; import doesn't care.

## Layout

```
.env / .env.example   config (env vars); .env holds the encryption key and is gitignored
.n8n/                 n8n's live data: SQLite DB, encrypted credentials, executions (gitignored)
workflows/            workflow JSON, the version-controlled source of truth
docs/                 learning notes
```

## Learn

1. [How n8n works](docs/01-how-n8n-works.md): processes, workflows, items, expressions, credentials
2. [Connect two Gmail accounts](docs/02-connect-gmail.md): Google Cloud OAuth setup

## Roadmap

- [x] n8n running locally via npm
- [x] Read unread mail from both Gmail accounts
- [ ] Google Calendar credentials (same Google Cloud app, Calendar API already enabled)
- [ ] Claude (Anthropic Chat Model + AI Agent) extracts events and deadlines from email
- [ ] Scheduled workflow that creates calendar events automatically

## Notes

- n8n is pinned to **2.47.4** in `package.json`. At startup it warns that running outside Docker is *deprecated* and future versions will require the official Docker image. The pinned version keeps working, but read the release notes before upgrading.
- **Back up `N8N_ENCRYPTION_KEY`.** Without it, the saved credentials in `.n8n/` can't be decrypted.
