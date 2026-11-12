# Assistente de Emails (desktop app)

A small native Windows app (Rust, `native-windows-gui`, classic "Visual Basic" look) that shows
what the email automation (n8n workflows 01/02 + Claude) is doing and lets you trigger it. All UI
strings are in Brazilian Portuguese. The release exe is about 0.8 MB and idles at about 18 MB of RAM.

## Features

* **Última execução**: date/time, analysed period and number of emails read in the last run.
* **Processar agora**: "Analisar os últimos N dias" (1 to 365) or "Todos os emails" (sends `days = 0`).
* **Progress bar** with "n de N emails lidos" below it (never shows more than N).
* **Abrir Tarefas / Abrir Calendário**: opens Google Tasks / Calendar in the default browser.
* **Registro**: small log box (bottom left) with the last 300 lines, from the bridge and from the app.
* **Iniciar com o Windows** checkbox: a Task Scheduler logon task named `Assistente de Emails` (10 s after logon, runs `<exe> --minimizado`). It is not the `Run` registry key, see *Install* below.
* Closing the window hides it to the **tray**; the tray menu has Abrir, Processar agora, Sair.
* Single instance: starting it again just brings the first window to the front.
* Shows "Sem conexão com a ponte do Claude" when the bridge is down and recovers by itself.

## Architecture

```
 n8n workflow 01/02 --POST /event--> Claude bridge <--GET /status, POST /run-- this app
                                      (scripts/claude-bridge.mjs, :5681)
                                          |   ^
                              POST form   |   |  /healthz
                                          v   |
                                          n8n (:5678)
```

The app only talks to the bridge (contract: [`API.md`](API.md)). Layers of the Rust library; each
one only depends on the ones above it in this list:

```
 model        types mirroring GET /status                         (pure)
 format       pt-BR texts, dates, progress                        (pure)
 client       blocking HTTP client of the bridge (ureq)           (I/O)
 clock        local time (GetLocalTime)                           (Windows utility)
 worker       poll thread + commands; shares state with the UI    (thread, testable with a fake bridge)
 ui           window / tray / options; wiring only
   ui/view_model   status -> what each control shows              (pure)
   ui/window       controls and updating them from a ViewModel
   ui/tray         tray icon and menu
   ui/options      command line parsing                           (pure)
 autostart (Task Scheduler), browser, shell (taskbar check), diag (app.log), singleton   independent Windows utilities
```

The worker polls every 1 s while a run is active and every 3 s otherwise, and wakes the UI thread
with an `nwg::Notice` only when something visible changed.

## Build

The MSVC target is not available on this machine, so the project uses the GNU toolchain with the
MSYS2 mingw64 `gcc` first on `PATH`:

```
app\build.cmd                 # cargo build --release  ->  app\target\release\assistente-emails.exe
```

Git bash equivalent: `export PATH="/c/msys64/mingw64/bin:$HOME/.cargo/bin:$PATH"; cd app; cargo build --release`.

The `native-windows-gui` features are the minimum that works: `image-decoder` is needed to load the
`.ico` at run time even though the code compiles without it. Regenerate the icon with
`node app/tools/make-icon.mjs`.

## Install

`app\install.cmd` builds, copies the exe to **`%USERPROFILE%\AssistenteEmails`**, registers the logon
task (the app does it itself with `--ativar-inicializacao`) and starts the app through the task. Run it
again to update.

Two things to know, learned the hard way:

* **Do not install into `%LOCALAPPDATA%`, and do not rely on `HKCU\...\Run`, when installing from
  inside a packaged app** (the Claude desktop app is an MSIX package). What its shell writes to those
  places lands in a private copy under `%LOCALAPPDATA%\Packages\<package>\LocalCache`, invisible to
  Windows: the Run entry never fired, Task Manager did not list it and the scheduler answered "file
  not found". The Task Scheduler is a system service, so its registrations are real from any context,
  and `%USERPROFILE%\AssistenteEmails` is not redirected.
* If the app does not appear after logon, read `%LOCALAPPDATA%\AssistenteEmails\app.log` (written by
  the app when Windows starts it normally). It records each start, the wait for the taskbar and any
  failure to create the window or tray icon. No new line after a boot means Windows did not start it:
  check the task *Assistente de Emails* in Task Scheduler.

## Run

```
assistente-emails.exe                       # normal start
assistente-emails.exe --minimizado          # start hidden in the tray (what autostart uses)
assistente-emails.exe --ativar-inicializacao   # also turn "Iniciar com o Windows" on, once
set ASSISTENTE_BRIDGE_URL=http://127.0.0.1:5682   # bridge address (default http://127.0.0.1:5681)
```

For development without touching the real services, run the mock bridge and point the app at it:
`node app/tools/mock-bridge.mjs 5682` then `ASSISTENTE_BRIDGE_URL=http://127.0.0.1:5682`.

## Tests

| what | command |
|---|---|
| Bridge unit + HTTP tests (state machine, n8n client, store, API) | `npm test` (repo root) |
| Rust unit tests (format, view model, worker, client, autostart, singleton, browser, options) | `cd app; cargo test` |
| Rust integration test: the real bridge (Node child process) + the real `Client`, with a fake n8n and no `claude`; skipped with a message if `node` is missing | included in `cargo test` (`app/tests/bridge_integration.rs`) |
| Lints and format | `cargo fmt --check; cargo clippy --all-targets -- -D warnings` |
| Workflows 00/01/02 end to end in a throwaway n8n (about 5 minutes, ports 5698/5693/5691/5699) | `node scripts/test/e2e/run.mjs` |
| Look at the UI with fake data | mock bridge (above) |

The tests never use ports 5678/5681 and never touch the Google accounts. The `autostart` test registers a
throwaway scheduled task with a unique name and removes it.

## Known limitations

* Polling (no push): the progress updates up to 1 s late; idle polling costs about 1% of one core.
* A run started outside the app (schedule, n8n form) is shown too, but the app cannot stop a run.
* `last_run.ok` is always `true`: the bridge does not yet know about failed runs (a failed email
  is retried on the next run by the workflows).
* "Emails lidos" counts the emails fetched from Gmail, not only those that produced tasks.
* A run with no event for 20 minutes is shown as "Execução interrompida" and a new one is allowed.
* The tray menu cannot be driven by automated UI tests here; it was checked by reading the code.
* Only the classic Windows look (no dark mode, fixed 8 px grid layout, fixed window size, no DPI scaling).
