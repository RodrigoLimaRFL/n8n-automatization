# 05 · Claude turns emails into tasks

Every email that workflow **01** reads goes to Claude, which decides whether it contains tasks:

| Claude finds… | Where it goes |
|---|---|
| a task **with a date** | **Google Calendar** (personal), colored by type |
| a task **without a date** | **Google Tasks** (personal), default list: the "general list" |
| a change to a task already registered (new date, room, requirements) | that event or task is **updated** |
| a cancellation | the event turns **red** with title "CANCELADA – …"; a list task gets "CANCELADA – …" |
| a list task that now has a date | it moves from Google Tasks to the Calendar |

Colors (Tomato/red is reserved for cancellations, Flamingo isn't used because it looks red):

| Type | Color | colorId |
|---|---|---|
| prova (exams, tests) | Grape (purple) | 3 |
| entrega (assignments, reports, readings) | Tangerine (orange) | 6 |
| evento (classes, meetings, talks) | Peacock (blue) | 7 |
| burocracia (enrollment, documents, payments) | Banana (yellow) | 5 |
| pessoal | Basil (green) | 10 |
| outro | Graphite (grey) | 8 |
| *cancelled* | Tomato (red) | 11 |

## How it fits together

```
01 · Gmail – read both inboxes                       02 · Email → tasks (Claude)   ← once per new email
  … → Combine inboxes ─┬─► Only real emails             Email from 01
                       │   Not processed yet  (table)   ├─ Gmail: full email (right account)
                       │   Sender not skipped (table)   ├─ Clean email
                       │   Oldest first                 ├─ Sender history, Open tasks  (tables)
                       │   Process each email (02) ───► ├─ Build Claude request → Ask Claude ──► Claude bridge (localhost:5681)
                       │                                │                                         └─ claude -p (your Claude plan)
                       └─► Record run + clean up        ├─ Plan actions → What to do ─┬─ Calendar: create / update / mark cancelled
                                                        │                              ├─ Tasks: create / update / mark CANCELADA
                                                        │                              └─ …then save it in n8n_email_tasks
                                                        └─ Count sender stats → Mark email processed → Update sender
```

### New n8n concepts

- **Sub-workflows.** 01 calls 02 with the **Execute Workflow** node in *Run once for each item* mode: one email at a time, oldest first, each finishing before the next starts. That matters because email #2 may update a task email #1 just created. 02 starts with **When Executed by Another Workflow**.
  - **02 must be published.** When 01 runs for real, it runs the *published* version of 02. After changing 02, publish it again.
  - 01's Execute Workflow node has **On Error → Continue**: one failing email doesn't stop the others. It isn't marked processed, so it's retried next run.
- **Data Tables** (*Overview → Data tables*): small tables stored inside n8n, which you can view and edit:
  - `n8n_email_tasks`: every task, with `kind` (calendar/list), `status` (active/cancelled) and the Google ids used to update it later
  - `n8n_processed_emails`: Gmail ids already read. This is how no email is read twice.
  - `n8n_email_senders`: per sender: `is_person`, `emails_analyzed`, `emails_with_tasks`, `skip`

  Workflow **00 · Setup – data tables** creates them (run it once).
- **Filtering with tables.** *Not processed yet* and *Sender not skipped* use the Data Table operation **"If row does not exist"**: an item passes only if no matching row exists.
- **Structured output.** Claude must answer in a fixed JSON shape (a JSON Schema in *Build Claude request*), so *Plan actions* never has to guess. Claude only *decides*. The Code and Google nodes *act*, which keeps the logic visible and testable.
- **Switch in expression mode.** *What to do* sends each operation to one of 7 outputs using `["create_event", …].indexOf($json.op)`.
- **`$('Node name').item`.** After a Google node, `$json` is Google's response (e.g. the new event `id`). The task's own data is still available from the node that produced it: `$('What to do').item.json.title`.

### Why a "Claude bridge"?

Your Claude **Pro** plan can't be used through an API key: the API is billed separately. What Pro *does* include is **Claude Code**, which has a non-interactive mode, `claude -p`. [scripts/claude-bridge.mjs](../scripts/claude-bridge.mjs) is a tiny local server that n8n calls with its **HTTP Request** node, and that runs `claude -p` for each email.

It's a separate process (started with n8n, see [04](04-autostart.md)) because n8n's Code nodes run in a sandbox that can't start programs, and making them able to would weaken that sandbox for every workflow.

**Safety.** Email content is untrusted: anyone can send you an email that says "ignore your instructions and…". So:
- Claude runs with **no tools** (`--tools ""`): it can't read files, run commands or browse. No MCP servers, no settings or hooks (`--strict-mcp-config --setting-sources ""`), in an empty temp folder.
- The email goes in through **stdin**, never through a shell command line.
- The bridge only listens on `127.0.0.1` and refuses browser requests.
- The system prompt tells Claude the email is data, not instructions.

The worst a malicious email can do is make Claude invent a wrong task in your calendar.

**Usage.** Each new email is one Claude Sonnet 5.5 call (effort `medium`), counted against your Pro plan's limits. If you hit the limit, those emails fail and are retried on the next run. Emails from skipped senders and already-processed emails cost nothing.

### Skipping senders

After each email, *Count sender stats* updates the sender's row. A sender is skipped from then on when Claude says it **isn't a person** (no-reply, newsletter, store…) **and** its last **5** emails had **no tasks** (`SKIP_AFTER` in that node).

To un-skip someone: in `n8n_email_senders`, set `skip` to false **and** `emails_analyzed` to 0. Otherwise they're skipped again after the next email.

## Setup (once)

1. **Log the Claude CLI in.** It's separate from the desktop app. For unattended use, create a long-lived token in a terminal:
   ```bash
   claude setup-token
   ```
   Put the token it prints in `.env` as `CLAUDE_CODE_OAUTH_TOKEN=...`, then restart the bridge: `npm run stop`, then `powershell -File scripts\start-n8n.ps1`. Check it with:
   ```bash
   curl -s -X POST http://127.0.0.1:5681/ask -H "content-type: application/json" -d "{\"prompt\":\"Responda só: ok\"}"
   ```
2. **Google Cloud:** in the same project as [02](02-connect-gmail.md), *APIs & Services → Library* → enable **Google Tasks API** (the Calendar API is already enabled).
3. **n8n credentials**, both with the same Client ID/Secret as Gmail, signing in with your **personal** account:
   - **Google Calendar OAuth2 API**, named exactly `Google Calendar – Personal`
   - **Google Tasks OAuth2 API**, named exactly `Google Tasks – Personal`
4. `npm run link` writes the credential ids into the workflow files and re-imports them (see *Credentials and imports* below).
5. In the editor, open **00 · Setup – data tables** → **Execute workflow** (once).
6. **Publish 02**, then **publish 01** again. Importing a workflow unpublishes it.
7. Test: run 01 from the form (*Read emails on demand*) with *Last N days* = 2, then check **Executions**, your Calendar, Google Tasks and the Data Tables.

## Credentials and imports

Workflow files point to credentials by id, and ids are different on every n8n instance. [scripts/link-credentials.mjs](../scripts/link-credentials.mjs) (`npm run link`) looks up your credentials **by name** (it never reads their secrets), fills in the ids, and imports the changed workflows. Two consequences:

- **Importing replaces the workflow in n8n** with the file's version: export first (`npm run export`) if you edited it in the editor.
- **Importing unpublishes it**: publish again afterwards.

## Limits and known gaps

- Emails longer than 15,000 characters are cut before going to Claude (mostly newsletters).
- If a run fails halfway through an email (e.g. the Calendar event was created but saving it to the table failed), retrying that email can create a duplicate event.
- Only tasks that are still active, and only the 200 most recently changed ones, are shown to Claude for matching.
- Data Tables are looked up by **name**, and n8n matches names by "contains". Don't create other tables whose names contain these three names.
- Attachments aren't read, only the email text.
