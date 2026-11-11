# 01 · How n8n works (as used in this project)

## What's actually running

`npm start` runs `node --env-file=.env node_modules/n8n/bin/n8n start`:

- `--env-file=.env` is a Node 24 feature that loads `.env` into environment variables. **n8n is configured entirely through env vars** (no config file to edit).
- n8n starts **one process** that contains:
  - the **editor** (the web UI at <http://localhost:5678>)
  - the **REST API** the editor talks to (and the OAuth callback, `/rest/oauth2-credential/callback`)
  - the **execution engine** that runs workflows
  - a **scheduler** for time-based triggers
  - a **task runner** that runs your JavaScript/Python in *Code* nodes in isolation
- Everything it stores goes into **`.n8n/`** (because `N8N_USER_FOLDER=.`):
  - `database.sqlite` — workflows, credentials (encrypted), execution history
  - `config` — instance settings

`.n8n/` and `.env` are gitignored. **Git holds workflow JSON files, the database holds your live state.** `npm run export` / `npm run import` move workflows between the two.

## The core concepts

### Workflow
A graph of **nodes** connected by arrows. Stored as JSON — open `workflows/01-gmail-both-inboxes.json` to see: a `nodes` array and a `connections` object saying which node's output feeds which node's input.

### Nodes — three kinds
| Kind | Role | In our workflow |
|---|---|---|
| **Trigger** | Starts the workflow. Every workflow has at least one. | *Run manually* (Manual Trigger). Later: *Schedule Trigger*, *Gmail Trigger*. |
| **Action** | Calls a service. | *Gmail* (Get many messages). Later: *Google Calendar*. |
| **Core / logic** | Reshapes or routes data. | *Edit Fields (Set)*, *Merge*. Others: *IF*, *Switch*, *Code*, *Loop*. |

### Items — the most important idea
Data travels between nodes as a **list of items**; each item is a JSON object:

```json
[
  { "json": { "account": "personal", "subject": "Hi", "from": "…" } },
  { "json": { "account": "personal", "subject": "Invoice", "from": "…" } }
]
```

Most nodes **run once per input item**. The Gmail node gets 1 item from the trigger and outputs 5 (one per email); *Tag as personal* then runs 5 times, once per email. You rarely write loops — n8n loops for you.

### Expressions
Any parameter can be dynamic. Values starting with `=` and containing `{{ … }}` are JavaScript evaluated per item:

- `{{ $json.Subject }}` — field `Subject` of the **current** item
- `{{ $('Run manually').item.json }}` — data from an earlier, named node
- `{{ DateTime.fromMillis(Number($json.internalDate)).toISO() }}` — n8n ships **Luxon** as `DateTime` (handy for Calendar work)
- `{{ $now }}`, `{{ $today }}` — current time in `GENERIC_TIMEZONE`

In the editor, drag a field from the input panel into a parameter and n8n writes the expression for you.

### Credentials
Stored **separately** from workflows and encrypted with `N8N_ENCRYPTION_KEY`. A node only references a credential by id/name, which is why the workflow JSON is safe to commit. Two credentials of the same type (*Gmail – Personal*, *Gmail – University*) = two accounts.

### Executions
Every run is saved under **Executions** with each node's input and output. Your main debugging tool: open an execution and click through the nodes.

### Manual runs vs. published workflows
- **Execute workflow** in the editor = test run, right now.
- **Publish** (n8n 2.x; it used to be "Activate") = the triggers go live and run on their own, e.g. every morning at 7:00. Only published workflows react to schedules or new emails.

## How our starter workflow flows

```
Run manually ──┬──► Personal: unread emails ──► Tag as personal ────┐
               │    (credential: Gmail – Personal)                  ├──► Combine inboxes
               └──► University: unread emails ─► Tag as university ─┘    (Merge: append)
                    (credential: Gmail – University)
```

1. The trigger emits **1 empty item**, which goes to both branches.
2. Each Gmail node fetches up to 5 unread messages. *Simplify* is on, so headers like `From` and `Subject` come through as top-level fields.
3. *Edit Fields* keeps 5 clean fields and adds `account`, so after merging we still know where each email came from.
4. *Merge (Append)* waits for both inputs and outputs one combined list.

## Where Claude comes in (next steps)

n8n has AI nodes built in:

- **AI Agent** node: the "brain" loop
- **Anthropic Chat Model** sub-node: plugs Claude in (credential = Anthropic API key)
- **Tools** sub-nodes: e.g. *Google Calendar Tool*, *Gmail Tool*. The agent decides when to call them

The plan: a Schedule Trigger reads both inboxes, Claude extracts deadlines, classes and meetings, and creates or updates Google Calendar events.
