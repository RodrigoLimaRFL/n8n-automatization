//! Thread de consulta: pergunta o status à ponte e executa comandos da janela.
//! Não conhece a interface: avisa a janela por meio do callback `notify`.

use crate::client::{Client, ClientError};
use crate::clock::local_timestamp;
use crate::model::{LogLine, State, Status};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

/// Intervalo entre consultas com uma execução em andamento.
const POLL_RUNNING: Duration = Duration::from_secs(1);
/// Intervalo entre consultas quando ocioso ou sem conexão.
const POLL_IDLE: Duration = Duration::from_secs(3);

/// Pedidos da janela para a thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Inicia o processamento (0 = todos os emails).
    Run { days: u32 },
    /// Encerra a thread.
    Shutdown,
}

/// Estado compartilhado, lido pela janela depois de cada `notify`.
#[derive(Debug, Default)]
pub struct Shared {
    /// Último status recebido; `None` enquanto a ponte nunca respondeu.
    /// Seu campo `log` fica sempre vazio: as linhas novas vão para `new_log`.
    pub status: Option<Status>,
    /// `true` quando a última consulta falhou (ponte parada).
    pub offline: bool,
    /// Linhas de log novas (da ponte e do próprio app); a janela esvazia este vetor.
    pub new_log: Vec<LogLine>,
}

/// O que o worker precisa da ponte (implementado por [`Client`]; os testes usam um falso).
trait Bridge {
    fn status(&self, after_seq: u64) -> Result<Status, ClientError>;
    fn run(&self, days: u32) -> Result<(), ClientError>;
}

impl Bridge for Client {
    fn status(&self, after_seq: u64) -> Result<Status, ClientError> {
        Client::status(self, after_seq)
    }

    fn run(&self, days: u32) -> Result<(), ClientError> {
        Client::run(self, days)
    }
}

/// Alça da thread de consulta; ao ser descartada, a thread é avisada para encerrar (sem esperar:
/// uma requisição em andamento pode levar até alguns segundos, e a janela não deve travar ao sair).
pub struct Worker {
    tx: Sender<Command>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    /// Inicia a thread. Consulta a cada ~1 s com execução em andamento e a cada ~3 s ocioso.
    /// Chama `notify` (fora do lock de `shared`) sempre que `shared` mudou.
    pub fn spawn(
        client: Client,
        shared: Arc<Mutex<Shared>>,
        notify: Box<dyn Fn() + Send>,
    ) -> Worker {
        let (tx, rx) = mpsc::channel();
        let core = Core::new(client, shared, notify, local_timestamp);
        let thread = std::thread::spawn(move || run_loop(core, &rx));
        Worker {
            tx,
            thread: Some(thread),
        }
    }

    /// Envia um comando para a thread (ignorado se ela já terminou).
    pub fn send(&self, cmd: Command) {
        let _ = self.tx.send(cmd);
    }

    /// Encerra a thread e espera ela terminar (usado nos testes; a interface só descarta a alça).
    pub fn join(mut self) {
        self.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.send(Command::Shutdown);
    }
}

/// Laço da thread: consulta periodicamente e atende comandos assim que chegam.
fn run_loop<B: Bridge>(mut core: Core<B>, rx: &Receiver<Command>) {
    core.poll();
    loop {
        match rx.recv_timeout(core.interval()) {
            Ok(cmd) => {
                if !core.handle(cmd) {
                    break;
                }
            }
            Err(RecvTimeoutError::Timeout) => core.poll(),
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// Lógica do worker, sem thread nem relógio fixo (para poder testar).
struct Core<B> {
    bridge: B,
    shared: Arc<Mutex<Shared>>,
    notify: Box<dyn Fn() + Send>,
    clock: fn() -> String,
    last_seq: u64,
    offline: bool,
    running: bool,
}

impl<B: Bridge> Core<B> {
    fn new(
        bridge: B,
        shared: Arc<Mutex<Shared>>,
        notify: Box<dyn Fn() + Send>,
        clock: fn() -> String,
    ) -> Self {
        Self {
            bridge,
            shared,
            notify,
            clock,
            last_seq: 0,
            offline: false,
            running: false,
        }
    }

    /// Tempo até a próxima consulta.
    fn interval(&self) -> Duration {
        if self.running && !self.offline {
            POLL_RUNNING
        } else {
            POLL_IDLE
        }
    }

    /// Atende um comando; `false` quando a thread deve terminar.
    fn handle(&mut self, cmd: Command) -> bool {
        match cmd {
            Command::Shutdown => false,
            Command::Run { days } => {
                match self.bridge.run(days) {
                    Ok(()) => self.poll(),
                    Err(e) => self.push_local("warn", e.to_string()),
                }
                true
            }
        }
    }

    /// Consulta o status uma vez e atualiza `shared`.
    fn poll(&mut self) {
        match self.bridge.status(self.last_seq) {
            Ok(status) => self.apply_status(status),
            Err(_) => self.apply_failure(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn local_line(&self, level: &str, msg: &str) -> LogLine {
        LogLine::local((self.clock)(), level, msg)
    }

    /// Acrescenta uma linha do próprio app ao log e avisa a janela.
    fn push_local(&self, level: &str, msg: String) {
        let line = self.local_line(level, &msg);
        self.lock().new_log.push(line);
        (self.notify)();
    }

    fn apply_status(&mut self, mut status: Status) {
        let mut lines = std::mem::take(&mut status.log);
        if let Some(last) = lines.last() {
            self.last_seq = self.last_seq.max(last.seq);
        }
        self.running = status.state == State::Running;
        let restored = std::mem::replace(&mut self.offline, false);
        let restored_line = restored.then(|| self.local_line("info", "Conexão restabelecida"));
        let changed = {
            let mut shared = self.lock();
            let changed = restored
                || !lines.is_empty()
                || shared
                    .status
                    .as_ref()
                    .is_none_or(|old| !same_view(old, &status));
            shared.status = Some(status);
            shared.offline = false;
            shared.new_log.extend(restored_line);
            shared.new_log.append(&mut lines);
            changed
        };
        if changed {
            (self.notify)();
        }
    }

    fn apply_failure(&mut self) {
        self.running = false;
        if self.offline {
            return;
        }
        self.offline = true;
        let line = self.local_line("warn", "Conexão com a ponte perdida");
        {
            let mut shared = self.lock();
            shared.offline = true;
            shared.new_log.push(line);
        }
        (self.notify)();
    }
}

/// Dois status são "iguais" para a janela se só `server_time` (e o log) diferem.
fn same_view(a: &Status, b: &Status) -> bool {
    a.state == b.state && a.run == b.run && a.last_run == b.last_run && a.services == b.services
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Ponte falsa: respostas de `status` em fila (a última se repete) e resultado fixo de `run`.
    struct Fake {
        statuses: RefCell<VecDeque<Result<Status, ClientError>>>,
        run_result: Result<(), ClientError>,
        seqs_asked: RefCell<Vec<u64>>,
        runs: RefCell<Vec<u32>>,
    }

    impl Fake {
        fn new(
            statuses: Vec<Result<Status, ClientError>>,
            run_result: Result<(), ClientError>,
        ) -> Self {
            Self {
                statuses: RefCell::new(statuses.into()),
                run_result,
                seqs_asked: RefCell::default(),
                runs: RefCell::default(),
            }
        }
    }

    impl Bridge for &Fake {
        fn status(&self, after_seq: u64) -> Result<Status, ClientError> {
            self.seqs_asked.borrow_mut().push(after_seq);
            let mut queue = self.statuses.borrow_mut();
            if queue.len() > 1 {
                queue.pop_front().unwrap_or(Err(ClientError::Unreachable))
            } else {
                queue
                    .front()
                    .cloned()
                    .unwrap_or(Err(ClientError::Unreachable))
            }
        }

        fn run(&self, days: u32) -> Result<(), ClientError> {
            self.runs.borrow_mut().push(days);
            self.run_result.clone()
        }
    }

    fn status(state: State, seqs: &[u64], time: &str) -> Status {
        Status {
            state,
            log: seqs
                .iter()
                .map(|&seq| LogLine {
                    seq,
                    ts: "t".into(),
                    level: "info".into(),
                    msg: format!("linha {seq}"),
                })
                .collect(),
            server_time: Some(time.into()),
            ..Status::default()
        }
    }

    fn make(fake: &Fake) -> (Core<&Fake>, Arc<Mutex<Shared>>, Arc<AtomicUsize>) {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let count = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&count);
        let notify = Box::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        let core = Core::new(fake, Arc::clone(&shared), notify, || {
            "2026-11-12T10:00:00".to_string()
        });
        (core, shared, count)
    }

    fn drain(shared: &Arc<Mutex<Shared>>) -> Vec<LogLine> {
        std::mem::take(&mut shared.lock().unwrap().new_log)
    }

    #[test]
    fn first_status_notifies_and_tracks_seq() {
        let fake = Fake::new(vec![Ok(status(State::Idle, &[3, 4], "a"))], Ok(()));
        let (mut core, shared, count) = make(&fake);
        core.poll();
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(drain(&shared).len(), 2);
        let guard = shared.lock().unwrap();
        assert!(guard.status.as_ref().is_some_and(|s| s.log.is_empty()));
        drop(guard);
        core.poll();
        assert_eq!(*fake.seqs_asked.borrow(), vec![0, 4]);
    }

    #[test]
    fn unchanged_status_does_not_notify() {
        let fake = Fake::new(
            vec![
                Ok(status(State::Idle, &[], "a")),
                Ok(status(State::Idle, &[], "b")),
                Ok(status(State::Running, &[], "c")),
            ],
            Ok(()),
        );
        let (mut core, shared, count) = make(&fake);
        core.poll();
        core.poll();
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(core.interval(), POLL_IDLE);
        core.poll();
        assert_eq!(count.load(Ordering::SeqCst), 2);
        assert_eq!(core.interval(), POLL_RUNNING);
        let server_time = shared
            .lock()
            .unwrap()
            .status
            .as_ref()
            .and_then(|s| s.server_time.clone());
        assert_eq!(server_time.as_deref(), Some("c"));
    }

    #[test]
    fn offline_transitions_log_once() {
        let fake = Fake::new(
            vec![
                Err(ClientError::Unreachable),
                Err(ClientError::Unreachable),
                Ok(status(State::Idle, &[], "a")),
                Ok(status(State::Idle, &[], "a")),
            ],
            Ok(()),
        );
        let (mut core, shared, count) = make(&fake);
        core.poll();
        core.poll();
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(shared.lock().unwrap().offline);
        let lines = drain(&shared);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].msg, "Conexão com a ponte perdida");
        assert_eq!((lines[0].seq, lines[0].level.as_str()), (0, "warn"));
        assert_eq!(lines[0].ts, "2026-11-12T10:00:00");
        core.poll();
        core.poll();
        assert_eq!(count.load(Ordering::SeqCst), 2);
        assert!(!shared.lock().unwrap().offline);
        let lines = drain(&shared);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].msg, "Conexão restabelecida");
    }

    #[test]
    fn run_error_adds_local_warning() {
        let fake = Fake::new(
            vec![Ok(status(State::Idle, &[], "a"))],
            Err(ClientError::AlreadyRunning),
        );
        let (mut core, shared, count) = make(&fake);
        assert!(core.handle(Command::Run { days: 5 }));
        assert_eq!(*fake.runs.borrow(), vec![5]);
        assert!(fake.seqs_asked.borrow().is_empty());
        let lines = drain(&shared);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].msg, "Já existe uma execução em andamento");
        assert_eq!((lines[0].seq, lines[0].level.as_str()), (0, "warn"));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn run_success_polls_immediately() {
        let fake = Fake::new(vec![Ok(status(State::Running, &[1], "a"))], Ok(()));
        let (mut core, shared, _count) = make(&fake);
        assert!(core.handle(Command::Run { days: 0 }));
        assert_eq!(fake.seqs_asked.borrow().len(), 1);
        assert_eq!(drain(&shared).len(), 1);
        assert_eq!(core.interval(), POLL_RUNNING);
    }

    #[test]
    fn shutdown_stops() {
        let fake = Fake::new(vec![], Ok(()));
        let (mut core, _shared, count) = make(&fake);
        assert!(!core.handle(Command::Shutdown));
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn loop_ends_on_shutdown_and_disconnect() {
        let fake = Fake::new(vec![Ok(status(State::Idle, &[], "a"))], Ok(()));
        let (core, _shared, count) = make(&fake);
        let (tx, rx) = mpsc::channel();
        tx.send(Command::Shutdown).unwrap();
        run_loop(core, &rx);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        let (core, _shared, _count) = make(&fake);
        let (tx, rx) = mpsc::channel::<Command>();
        drop(tx);
        run_loop(core, &rx);
    }

    #[test]
    fn worker_spawns_and_stops_on_drop() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let notified = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&notified);
        // Porta fechada: a primeira consulta falha de imediato e o app fica "offline".
        let port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        let worker = Worker::spawn(
            Client::new(format!("http://127.0.0.1:{port}")),
            Arc::clone(&shared),
            Box::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }),
        );
        worker.join();
        assert!(shared.lock().unwrap().offline);
        assert_eq!(notified.load(Ordering::SeqCst), 1);
    }
}
