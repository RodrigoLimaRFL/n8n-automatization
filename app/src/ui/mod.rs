//! Interface: janela principal no estilo de um formulário clássico do Visual Basic e ícone na bandeja.

mod options;
mod tray;
mod view_model;
mod window;

use crate::client::Client;
use crate::clock::local_timestamp;
use crate::model::LogLine;
use crate::singleton::{self, Acquire};
use crate::worker::{Command, Shared, Worker};
use crate::{autostart, browser, diag, shell};
use native_windows_gui as nwg;
use options::{parse_args, Options};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tray::Tray;
use window::MainWindow;

pub use view_model::ViewModel;

/// Título exato da janela principal (também usado para achar a instância já aberta).
pub const WINDOW_TITLE: &str = "Assistente de Emails";

const ICON_BYTES: &[u8] = include_bytes!("../../assets/icon.ico");
const DEFAULT_BRIDGE_URL: &str = "http://127.0.0.1:5681";
const BRIDGE_URL_ENV: &str = "ASSISTENTE_BRIDGE_URL";
/// Tentativas de criar a janela e a bandeja: logo depois do logon o shell pode não estar pronto.
const START_ATTEMPTS: u32 = 40;
/// Pausa entre as tentativas de `START_ATTEMPTS`.
const START_RETRY_DELAY: Duration = Duration::from_secs(3);
/// Espera máxima pela barra de tarefas numa partida automática (`--minimizado`).
const TASKBAR_WAIT: Duration = Duration::from_secs(120);
/// Tempo máximo que o botão fica travado depois de um clique sem resposta do worker.
const RUN_LOCK_MS: u64 = 5000;

struct App {
    w: MainWindow,
    tray: Tray,
    notice: nwg::Notice,
    lock_timer: nwg::AnimationTimer,
    shared: Arc<Mutex<Shared>>,
    worker: Worker,
}

impl App {
    /// Copia o que precisa de `Shared`, solta o lock e só então mexe nos controles.
    fn refresh(&self) {
        let (vm, lines) = {
            let mut s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            (
                view_model::build(s.status.as_ref(), s.offline),
                std::mem::take(&mut s.new_log),
            )
        };
        self.w.apply(&vm);
        self.tray.set_tip(&vm.tray_tooltip);
        self.tray.set_run_enabled(vm.can_run);
        self.w.append_log(&lines);
    }

    /// Registra uma linha do próprio app na mesma fila das linhas do worker (mantém a ordem).
    fn log(&self, level: &str, msg: impl AsRef<str>) {
        let line = LogLine::local(local_timestamp(), level, msg.as_ref());
        self.shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .new_log
            .push(line);
        self.refresh();
    }

    fn run_now(&self) {
        if !self.w.is_run_enabled() {
            return;
        }
        self.worker.send(Command::Run {
            days: self.w.selected_days(),
        });
        self.w.mark_run_requested();
        self.tray.set_run_enabled(false);
        self.lock_timer.start();
    }

    /// Fim da trava depois do clique: reaplica o estado real.
    fn unlock_run(&self) {
        self.lock_timer.stop();
        self.w.forget_shown();
        self.refresh();
    }

    fn toggle_autostart(&self) {
        let want = self.w.autostart.check_state() == nwg::CheckBoxState::Checked;
        if let Err(e) = autostart::set_enabled(want) {
            self.w.autostart.set_check_state(if want {
                nwg::CheckBoxState::Unchecked
            } else {
                nwg::CheckBoxState::Checked
            });
            self.log(
                "error",
                format!("Não foi possível alterar a inicialização com o Windows: {e}"),
            );
        }
    }

    fn open_page(&self, what: &str, result: std::io::Result<()>) {
        if let Err(e) = result {
            self.log("error", format!("Não foi possível abrir {what}: {e}"));
        }
    }

    fn quit(&self) {
        self.worker.send(Command::Shutdown);
        self.tray.remove();
        nwg::stop_thread_dispatch();
    }

    fn on_event(&self, evt: nwg::Event, data: &nwg::EventData, handle: nwg::ControlHandle) {
        use nwg::Event as E;
        match evt {
            E::OnWindowClose if handle == self.w.window.handle => {
                // Fechar esconde na bandeja; só "Sair" encerra.
                if let nwg::EventData::OnWindowClose(d) = data {
                    d.close(false);
                }
                self.w.hide();
            }
            E::OnNotice if handle == self.notice.handle => self.refresh(),
            E::OnTimerTick if handle == self.lock_timer.handle => self.unlock_run(),
            E::OnButtonClick => {
                if handle == self.w.run_button.handle {
                    self.run_now();
                } else if handle == self.w.all_emails.handle {
                    self.w.sync_days_enabled();
                } else if handle == self.w.autostart.handle {
                    self.toggle_autostart();
                } else if handle == self.w.tasks_button.handle {
                    self.open_page("o Google Tarefas", browser::open_tasks());
                } else if handle == self.w.calendar_button.handle {
                    self.open_page("o Google Calendário", browser::open_calendar());
                }
            }
            E::OnMousePress(nwg::MousePressEvent::MousePressLeftUp)
                if handle == self.tray.icon.handle =>
            {
                self.w.show()
            }
            E::OnContextMenu if handle == self.tray.icon.handle => self.tray.show_menu(),
            E::OnMenuItemSelected => {
                if handle == self.tray.open.handle {
                    self.w.show();
                } else if handle == self.tray.run.handle {
                    self.run_now();
                } else if handle == self.tray.quit.handle {
                    self.quit();
                }
            }
            _ => {}
        }
    }
}

/// Ponto de entrada da interface: lê os argumentos (`--minimizado`), garante instância única,
/// cria janela + bandeja + worker e roda o laço de mensagens até "Sair".
pub fn run() {
    let opts = parse_args(std::env::args().skip(1));
    diag::log(&format!("Iniciando (minimizado: {}).", opts.minimized));
    let _single = match singleton::acquire(WINDOW_TITLE) {
        Acquire::AlreadyRunning => {
            diag::log("Já existe outra instância em execução; encerrando.");
            return;
        }
        Acquire::First(s) => s,
    };

    // Controles clássicos (sem estilos visuais), como num formulário do VB6.
    if let Err(e) = nwg::init_common_controls() {
        diag::log(&format!("Falha ao iniciar os controles do Windows: {e}"));
        return;
    }
    if opts.minimized {
        wait_for_taskbar();
    }
    let Some(app) = build_with_retry(opts) else {
        diag::log("Não foi possível criar a janela e a bandeja; encerrando.");
        return;
    };
    let app = Rc::new(app);

    let weak = Rc::downgrade(&app);
    let handler = nwg::full_bind_event_handler(&app.w.window.handle, move |evt, data, handle| {
        if let Some(app) = weak.upgrade() {
            app.on_event(evt, &data, handle);
        }
    });

    app.log("info", "Aplicativo iniciado");
    if opts.enable_autostart {
        if let Err(e) = autostart::set_enabled(true) {
            app.log(
                "error",
                format!("Não foi possível ativar a inicialização com o Windows: {e}"),
            );
        }
    }
    let enabled = autostart::is_enabled();
    app.w.autostart.set_check_state(if enabled {
        nwg::CheckBoxState::Checked
    } else {
        nwg::CheckBoxState::Unchecked
    });
    app.refresh();

    nwg::dispatch_thread_events();

    nwg::unbind_event_handler(&handler);
}

/// Espera a barra de tarefas existir (até `TASKBAR_WAIT`); sem ela o ícone da bandeja não pode ser criado.
fn wait_for_taskbar() {
    let start = Instant::now();
    while !shell::taskbar_ready() {
        if start.elapsed() >= TASKBAR_WAIT {
            diag::log("A barra de tarefas não apareceu a tempo; tentando mesmo assim.");
            return;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    let waited = start.elapsed();
    if waited >= Duration::from_secs(1) {
        diag::log(&format!(
            "Barra de tarefas pronta após {} s.",
            waited.as_secs()
        ));
    }
}

/// Cria a janela e a bandeja, tentando de novo se o Windows ainda não estiver pronto.
fn build_with_retry(opts: Options) -> Option<App> {
    for attempt in 1..=START_ATTEMPTS {
        match build_app(opts) {
            Ok(app) => {
                diag::log(&format!("Janela e bandeja criadas (tentativa {attempt})."));
                return Some(app);
            }
            Err(e) => {
                diag::log(&format!(
                    "Falha ao criar a janela/bandeja (tentativa {attempt}/{START_ATTEMPTS}): {e}"
                ));
                std::thread::sleep(START_RETRY_DELAY);
            }
        }
    }
    None
}

fn build_app(opts: Options) -> Result<App, nwg::NwgError> {
    let w = MainWindow::build(!opts.minimized)?;
    let initial = view_model::build(None, false);
    let tray = Tray::build(&w.window, &w.icon, &initial.tray_tooltip)?;

    let mut notice = nwg::Notice::default();
    nwg::Notice::builder()
        .parent(&w.window)
        .build(&mut notice)?;
    let mut lock_timer = nwg::AnimationTimer::default();
    nwg::AnimationTimer::builder()
        .parent(&w.window)
        .interval(Duration::from_millis(RUN_LOCK_MS))
        .max_tick(Some(1))
        .active(false)
        .build(&mut lock_timer)?;

    let url = std::env::var(BRIDGE_URL_ENV).unwrap_or_else(|_| DEFAULT_BRIDGE_URL.to_string());
    let shared = Arc::new(Mutex::new(Shared::default()));
    let sender = notice.sender();
    let worker = Worker::spawn(
        Client::new(url.trim_end_matches('/')),
        shared.clone(),
        Box::new(move || sender.notice()),
    );

    Ok(App {
        w,
        tray,
        notice,
        lock_timer,
        shared,
        worker,
    })
}
