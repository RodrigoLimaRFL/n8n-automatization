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
01 · Gmail – read both inboxes                     02 · Emails → tasks (Claude)   ← once per BATCH of up to 8 emails
  … → Combine inboxes ─┬─► Only real emails           Batch from 01 → One item per email
                       │   Not processed yet (table)   ├─ Gmail: full email (right account) → Clean email
                       │   Sender not skipped (table)  ├─ Senders (table) → Drop skipped senders
                       │   Oldest first                ├─ Open tasks (table)
                       │   Make batches (8 each)       ├─ Build Claude request → Ask Claude ──► Claude bridge (localhost:5681)
                       │   Process batches (02) ─────► │                                         └─ claude -p (your Claude plan)
                       │     └ failed batch → retry    ├─ Plan actions → What to do ─┬─ Calendar: create / update / mark cancelled
                       │       as single emails        │                              ├─ Tasks: create / update / mark CANCELADA
                       └─► Record run + clean up       │                              └─ …then save it in n8n_email_tasks
                                                       └─ Count emails → Mark emails processed → Sender rows → Update senders
```

### New n8n concepts

- **Sub-workflows.** 01 calls 02 with the **Execute Workflow** node in *Run once for each item* mode. Each item is a **batch** of up to 8 emails (made by the *Make batches* Code node), and batches run one after another, oldest first, so a later batch can update a task an earlier one created. 02 starts with **When Executed by Another Workflow**.
  - **02 must be published.** When 01 runs for real, it runs the *published* version of 02. After changing 02, publish it again.
  - **Error output.** *Process batches (02)* has **On Error → Continue (using error output)**: a failing batch leaves through its second output, *Split failed batches* turns it into single-email batches, and *Retry as single emails (02)* runs them. One bad email can't block the other seven. An email that still fails isn't marked processed, so it's retried on the next run.
- **Batching.** Every `claude -p` call carries about 7–9K tokens of fixed overhead, so one call for 8 emails costs roughly 6× less than 8 calls (measured: 9,977 vs 63,462 tokens on 8 test emails, same decisions). Each email is cut to 6,000 characters, so a batch stays under about 12K tokens of email text. Claude returns one action per task, with `email_ids` listing the emails behind it (two emails about the same task become one action with the final state), plus `sender_is_person` per email.
- **Data Tables** (*Overview → Data tables*): small tables stored inside n8n, which you can view and edit:
  - `n8n_email_tasks`: every task, with `kind` (calendar/list), `status` (active/cancelled) and the Google ids used to update it later
  - `n8n_processed_emails`: Gmail ids already read. This is how no email is read twice.
  - `n8n_email_senders`: per sender: `is_person`, `emails_analyzed`, `emails_with_tasks`, `skip`

  Workflow **00 · Setup – data tables** creates them (run it once).
- **Filtering with tables.** *Not processed yet* and *Sender not skipped* use the Data Table operation **"If row does not exist"**: an item passes only if no matching row exists.
- **Structured output.** Claude must answer in a fixed JSON shape (a JSON Schema in *Build Claude request*), so *Plan actions* never has to guess. Claude only *decides*. The Code and Google nodes *act*, which keeps the logic visible and testable.
- **Switch in expression mode.** *What to do* sends each operation to one of 7 outputs using `["create_event", …].indexOf($json.op)`.
- **`$('Node name').item`.** After a Google node, `$json` is Google's response (e.g. the new event `id`). The task's own data is still available from the node that produced it: `$('What to do').item.json.title`.

### Progress events

So the desktop app can show "n of N emails read", the workflows report to the bridge (`POST http://127.0.0.1:5681/event`, contract in [app/API.md](../app/API.md)) with plain **HTTP Request** nodes (timeout 3 s, *On Error → Continue*, no retries: if the bridge is down they fail silently and the run is unchanged):

| node | workflow | sends | when |
|---|---|---|---|
| **Progress: start** | 01 | `{event:'start', days, source}` | right after *Build Gmail search* |
| **Progress: total emails** | 01 | `{event:'total', total:N}` | after *Oldest first*, before *Make batches*, only if there is at least one email to process |
| **Progress: emails read** | 02 | `{event:'progress', count:n}` | once per batch, right after *Merge fetched emails* |
| **Progress: finish** | 01 | `{event:'finish'}` | last, on every run, even with zero emails (fed by the *Run finished* Code node, which always returns one item) |

Order matters: n8n (execution order v1) runs the branches of one output one after another, in connection order, so *Progress: start* and *Progress: total emails* are connected first, and *Run finished* is the third (lowest) branch of *Combine inboxes*, so it runs after the processing branch. A batch that fails and is retried as single emails reports its count again, so `progress` can add up to more than `total`; the bridge caps `done`. If a run crashes in the middle there is no `finish`; the bridge marks the run interrupted after 20 minutes without events. After changing 01/02, import and publish them again. The test in `scripts/test/e2e/` checks all of this.

### Why a "Claude bridge"?

Your Claude **Pro** plan can't be used through an API key: the API is billed separately. What Pro *does* include is **Claude Code**, which has a non-interactive mode, `claude -p`. [scripts/claude-bridge.mjs](../scripts/claude-bridge.mjs) is a tiny local server that n8n calls with its **HTTP Request** node, and that runs `claude -p` for each email.

It's a separate process (started with n8n, see [04](04-autostart.md)) because n8n's Code nodes run in a sandbox that can't start programs, and making them able to would weaken that sandbox for every workflow.

**Safety.** Email content is untrusted: anyone can send you an email that says "ignore your instructions and…". So:
- Claude runs with **no tools** (`--tools ""`): it can't read files, run commands or browse. No MCP servers, no settings or hooks (`--strict-mcp-config --setting-sources ""`), in an empty temp folder.
- The email goes in through **stdin**, never through a shell command line.
- The bridge only listens on `127.0.0.1` and refuses browser requests.
- The system prompt tells Claude the email is data, not instructions.

The worst a malicious email can do is make Claude invent a wrong task in your calendar.

**Usage.** Each batch of up to 8 emails is one Claude Sonnet 5.5 call (effort `low`), counted against your Pro plan's limits. Measured on 68 real emails: **10 calls, about 2,300 tokens per email, about 5 s per call**. The one-email-per-call design needed roughly 12,000 tokens per email (an estimate from the fixed overhead, not a measurement). The bridge log (`logs/claude-bridge.log`) now shows the tokens of every call. Other savings: Gmail category filters, skipping bulk senders after 3 task-less emails, at most 40 relevant open tasks per call, emails cut to 6,000 characters. If you hit the limit, those batches fail and are retried on the next run (a failed batch is retried once as single emails first, which costs extra calls while the limit is active).

### Skipping senders

After each batch, *Sender rows* updates one row per sender. A sender is skipped from then on when Claude says it **isn't a person** (no-reply, newsletter, store…) **and** its first **3** emails analysed had **no tasks** (`SKIP_AFTER` in that node). The check happens twice: 01 drops emails of already-skipped senders, and 02 (*Drop skipped senders*) drops them again after reading the table, so a sender that gets skipped in the middle of a big run stops costing tokens right away. (The first version only checked in 01, before the run, and analysed one noisy sender 28 times.)

Gmail searches also exclude the **Promotions, Social and Forums** categories, so those emails never reach n8n.

To un-skip someone: in `n8n_email_senders`, set `skip` to false **and** `emails_analyzed` to 0. Otherwise they're skipped again after the next email.

### What counts as a task

Claude is told to create tasks only for **obligations and commitments**: deadlines in courses and projects you're already in, meetings and events you were personally invited to, personal deadlines (library returns, payments, appointments), selection processes you've already applied to, and **automatic deadline alerts** from institutional systems (e-Disciplinas, library, GIP/FAPESP). **Generic invitations and announcements are ignored**, even with a deadline: open registrations for programs, workshops, talks, contests, surveys. A "mailing list / bulk mail" hint (from the `List-Id`, `List-Unsubscribe` and `Precedence` headers) is passed with each email. The rule is in the system prompt in *Build Claude request*; edit it there to change what counts. "When in doubt" cuts both ways on purpose: doubtful *generic invitations* are ignored, doubtful *deadlines addressed to you* are created. (A first version said "when in doubt, don't create" for everything, and Claude skipped a real FAPESP deadline alert. Testing found it.)

### Testing changes on real emails without touching your Calendar

Changing the prompt or the flow and "seeing what happens" would put wrong events in your Calendar, so changes were tested with a **shadow run**: a second n8n on a *copy* of the database (`VACUUM INTO` makes a consistent copy while n8n runs), with the same encryption key so the real Gmail credentials work (read-only), the **Calendar and Tasks nodes replaced by stubs**, and the processed-emails and senders tables emptied. Starting with an *empty* task list matters: otherwise Claude correctly says "already registered, nothing changed" and you can't tell whether the relevance rule works. Comparing what the shadow run *would create* with what the previous version had created is how the missing FAPESP alert was found. Everything stays in a throwaway folder.

## Setup (once)

1. **Log the Claude CLI in.** It's separate from the desktop app. For unattended use, create a long-lived token in a terminal:
   ```bash
   claude setup-token
   ```
   Put the token it prints in `.env` as `CLAUDE_CODE_OAUTH_TOKEN=...`, then restart n8n and the bridge with `npm run restart`. Check it with:
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

- Emails longer than 6,000 characters are cut before going to Claude (mostly newsletters).
- If a run fails halfway through an email (e.g. the Calendar event was created but saving it to the table failed), retrying that email can create a duplicate event.
- Only active tasks are shown to Claude for matching: the 100 most recently changed, then only undated ones or those dated in the last 60 days or the future, at most 40. A very old task can therefore be duplicated if an email mentions it again.
- An email that always makes Claude fail costs about 4 calls per run (2 HTTP tries for its batch, 2 more as a single email) until it ages out of the date window.
- Data Tables are looked up by **name**, and n8n matches names by "contains". Don't create other tables whose names contain these three names.
- Attachments aren't read, only the email text.
