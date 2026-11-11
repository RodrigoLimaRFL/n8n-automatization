# 02 · Connect two Gmail accounts (personal + university)

n8n talks to Gmail through **OAuth2**: you never give n8n your Google password. Instead:

1. You create an "app" in Google Cloud (a *Client ID* + *Client Secret*). This identifies *your n8n* to Google.
2. In n8n you create a **credential** with that Client ID/Secret and click *Sign in with Google*.
3. Google asks *you* to approve, then hands n8n a token for that one account.
4. n8n encrypts the token (with `N8N_ENCRYPTION_KEY`) and stores it in `.n8n/database.sqlite`.

**One Google Cloud app can serve both accounts.** You create *two n8n credentials* from the same Client ID/Secret, signing in with a different Google account each time. Which credential a Gmail node uses decides which inbox it reads.

---

## Part A — Google Cloud (one-time, ~10 min)

Do this logged in with your **personal** Google account.

1. Go to <https://console.cloud.google.com/> → project picker (top left) → **New project** → name it `n8n-learning` → Create, then select it.
2. **Enable APIs**: *APIs & Services → Library*, search and **Enable**:
   - **Gmail API**
   - **Google Calendar API** (not used yet, but we'll need it next and it saves a trip)
3. **Consent screen**: *Google Auth Platform* (or *APIs & Services → OAuth consent screen*) → **Get started**
   - App name: `n8n learning`; support email: your personal address
   - Audience: **External**
   - Contact email: your personal address → Finish
4. **Test users**: *Google Auth Platform → Audience → Test users → Add users* — add **both** addresses (personal and university). While the app is in *Testing*, only listed users can sign in.
5. **Client**: *Google Auth Platform → Clients → Create client*
   - Application type: **Web application**
   - Name: `n8n local`
   - **Authorized JavaScript origins** → leave **empty**. (Pasting the URL below here gives *"Invalid Origin: URIs must not contain a path"*.)
   - **Authorized redirect URIs** (the second section, further down) → **+ Add URI**:
     ```
     http://localhost:5678/rest/oauth2-credential/callback
     ```
     (n8n shows this exact URL in the credential dialog as "OAuth Redirect URL" — copy it from there if it differs.)
   - Create → copy the **Client ID** and **Client secret**. Keep them out of git; you'll paste them only into n8n.

## Part B — n8n credentials (one per account)

With n8n running (`npm start`) at <http://localhost:5678>:

1. Left sidebar → **Credentials** (or *Overview → Credentials*) → **Create credential** → search **Gmail OAuth2 API**.
2. Paste Client ID and Client Secret.
3. Click **Sign in with Google** → pick your **personal** account → approve.
   - You'll see *"Google hasn't verified this app"*. That's expected: it's your own app. Click **Advanced → Go to n8n learning (unsafe)**.
4. Rename the credential (click its title) to exactly **`Gmail – Personal`** → Save.
5. Repeat steps 1–4 with the **same** Client ID/Secret, but sign in with your **university** account, and name it **`Gmail – University`**.

The names just help you pick the right one in each Gmail node later.

## Part C — Run the starter workflow

1. **Overview → Workflows → "01 · Gmail – read both inboxes"** (already imported by `npm run import`; or use *⋯ → Import from file* on `workflows/01-gmail-both-inboxes.json`).
2. Open each Gmail node and **select its credential** in *Credential to connect with* (*Gmail – Personal* / *Gmail – University*), then save (Ctrl+S). Workflow files in git don't carry credentials: n8n links nodes to credentials by an internal id that exists only in your local database.
3. Click **Execute workflow**. Click **Combine inboxes** to see up to 10 items, each tagged `account: personal` or `account: university`.

---

## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `Error 403: access_denied` | That address isn't in **Test users** (Part A step 4). |
| *Invalid Origin* when adding the URL in Google Cloud | You pasted it under **Authorized JavaScript origins**. Put it under **Authorized redirect URIs** instead and leave origins empty. |
| *Node "…" uses invalid credential* | The node points at a credential id that doesn't exist in this n8n. Open the node and pick the credential from the dropdown again. |
| `redirect_uri_mismatch` | Redirect URI in Google must match n8n's exactly — `http`, `localhost`, port `5678`, no trailing slash. |
| University login: *"Access blocked: [university] has not granted access"* / `admin_policy_enforced` | Your university's Google Workspace admin blocks unapproved third-party apps. Options: (a) ask IT to allow it; (b) if your university account is allowed to create Cloud projects, repeat Part A *with the university account* and choose **Internal** audience — no test users and no 7-day expiry; (c) auto-forward university mail to your personal Gmail and read it there. |
| University email isn't Google at all (Outlook/Microsoft 365) | Use n8n's **Microsoft Outlook** node + *Microsoft Outlook OAuth2* credential instead (Azure app registration). |
| Works, then fails a week later with `invalid_grant` | In *Testing* mode Google expires refresh tokens after **7 days**. Open the credential and click *Sign in with Google* again. Publishing the app to *In production* avoids this, but Gmail scopes are "restricted", so you'll keep seeing the unverified-app warning. For a learning project, reconnecting weekly is fine. |
