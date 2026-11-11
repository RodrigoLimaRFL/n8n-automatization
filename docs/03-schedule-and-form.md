# 03 · Daily schedule + on-demand form

The workflow **01 · Gmail – read both inboxes** now starts in three ways:

| Trigger | When | Range |
|---|---|---|
| **Every day at 07:00** (Schedule Trigger) | Automatically, once a day | last **3 days** |
| **When n8n starts or is published** (n8n Trigger) | Every time n8n starts, so a missed 07:00 run is caught up | last **3 days** (more if n8n was off longer) |
| **Read emails on demand** (Form Trigger) | When you submit a small web form | last **N days**, or **all emails** |

```
Every day at 07:00 ───────────────┐
                                  ├─► Daily run due? ─┐
When n8n starts or is published ──┘                   │
                                                      ├─► Build Gmail search ─┬─► Personal: emails ──► Tag as personal ───┐
Read emails on demand ─► On demand: chosen range ─────┘   (newer_than:Nd)     └─► University: emails ─► Tag as university ┴─► Combine inboxes ─► Record run + clean up
```

## Concepts this introduces

### A workflow can have several triggers
Each run starts from **exactly one** trigger; the other trigger's branch stays idle. Both branches end in the same node (*Build Gmail search*), so you write the Gmail logic once.

The trick is to **normalize early**: each trigger produces data in a different shape (the schedule gives a timestamp, the form gives `Range` and `Days`). One small *Edit Fields* node per trigger converts it to a common shape, `{ days, source }`, so everything downstream doesn't care which trigger fired.

### Catching up a missed 07:00 run
n8n doesn't replay schedules it missed while it was off. So the workflow does it itself:

1. **When n8n starts or is published** (*n8n Trigger*, events *Instance Started*, *Workflow Published* and *Published Workflow Updated*) fires on every startup.
2. **Daily run due?** (*Code* node) is the gatekeeper for both daily triggers. It returns no items (stopping the run) if:
   - it's **before 07:00**, because the schedule will run later, or
   - **today's run already happened**.

   Otherwise it outputs `{ days, source }`. If the last run was more than 3 days ago, `days` grows to cover the gap: off for a week means a look back of 8 days.
3. **Record run + clean up** (the last node) saves today's date as `lastDailyRun`. Because it's last, a run that **failed** (no internet at boot, Google error) is *not* marked done, and the next start tries again.

**Static data** is where `lastDailyRun` lives: a small JSON object per workflow that n8n stores in its database and keeps across restarts (`$getWorkflowStaticData('global')`). Two rules:
- it's only **saved for real runs of a published workflow**. Tests in the editor read it but don't save changes.
- form runs don't touch it, so reading emails on demand never "uses up" the daily run.

**Why the Gmail nodes have *Always Output Data* on** (node ⚙ Settings): if an inbox has no matching emails, a node outputs nothing and the nodes after it are skipped. If both inboxes were empty, *Record run* would never run, so the day would never be marked done. With the setting on, an empty inbox outputs one empty placeholder item. *Record run + clean up* then removes the placeholders (items without an email `id`).

The Gmail nodes also have **Retry On Fail** (5 tries, 5 s apart), which helps when n8n starts before the network is ready.

### Turning parameters into a Gmail search
*Build Gmail search* computes:

```js
{{ $json.days > 0 ? 'newer_than:' + $json.days + 'd' : '' }}
```

This uses Gmail's own search syntax, the same as typing in the Gmail search box: `newer_than:3d` means received in the last 3 days. An empty search means every email (spam and trash excluded). The Gmail nodes put this in **Filters → Search** and set **Read Status = both**, so read emails come back too. **Return All** is on, so n8n pages through every result instead of stopping at a limit.

### Testing vs. publishing
- **Test the form:** in the editor, open the *Execute workflow* dropdown, choose **Read emails on demand**, and click it. n8n opens a *test* form. Submit it and watch the run happen live on the canvas.
- **Test the daily branch:** pick **Every day at 07:00** in the same dropdown. It goes through *Daily run due?*, so before 07:00 it stops there (that's correct behavior). Editor tests don't save static data, so you can repeat them.
- **Go live:** click **Publish** (top right). Then:
  - the schedule fires every day at 07:00 (`America/Sao_Paulo`, set in *Workflow settings*)
  - publishing itself counts as a start: if it's after 07:00 and today hasn't run, the daily run happens **right away**
  - the form gets a permanent **production URL**. Open the form node to copy it and bookmark it.

> **"Starts" means n8n starts.** With [autostart](04-autostart.md) set up, that happens about 30 s after you log in to Windows. If the computer only **sleeps** through 07:00 with n8n still running, n8n never restarts, so there's no catch-up until the next start.

### Seeing what happened
Every run is listed under **Executions** with its trigger. Open one and click **Combine inboxes** to see the emails. Each has `account`, `from`, `subject`, `snippet`, `receivedAt` and `unread`.

## Cautions

- **"All emails" can be very large.** The Gmail node makes one API call per email to fetch its headers, so 10,000 emails means 10,000 calls: it can take many minutes, and the execution stored in `.n8n/database.sqlite` gets big. Try *Last N days* with a large N first.
- The daily run has a **3-day overlap**, so the same email appears on up to 3 consecutive days. That's fine while we're only reading. When Claude starts creating calendar events, we'll need to skip emails we already processed.
