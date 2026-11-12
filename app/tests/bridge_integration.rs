//! Teste de integração com as peças reais: a ponte de verdade (`scripts/claude-bridge.mjs`, em
//! Node) rodando como processo filho + o `Client` do app. O n8n é simulado aqui mesmo
//! (`/healthz` e `POST /form/<id>`), e o `claude` nunca é executado (`CLAUDE_BIN=node`).
//!
//! Hermético: portas aleatórias em 127.0.0.1 (nunca 5678/5681) e o arquivo de estado da ponte
//! numa pasta temporária. Se o `node` não estiver instalado, o teste avisa e termina sem falhar.

use assistente_emails::client::{Client, ClientError};
use assistente_emails::model::State;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .expect("porta livre")
}

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Lê uma requisição HTTP inteira (cabeçalhos + corpo): devolve (linha de requisição, corpo).
fn read_request(stream: &mut TcpStream) -> (String, String) {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = stream.read(&mut buf).unwrap_or(0);
        data.extend_from_slice(&buf[..n]);
        let text = String::from_utf8_lossy(&data).to_string();
        if let Some(pos) = text.find("\r\n\r\n") {
            let length = text
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse().unwrap_or(0))
                })
                .unwrap_or(0usize);
            if data.len() >= pos + 4 + length || n == 0 {
                let line = text.lines().next().unwrap_or_default().to_string();
                return (line, text[pos + 4..].to_string());
            }
        } else if n == 0 {
            return (String::new(), text);
        }
    }
}

/// n8n falso: guarda o corpo de cada `POST /form/...` e responde 500 quando `down` está ligado.
struct FakeN8n {
    port: u16,
    forms: Arc<Mutex<Vec<String>>>,
    down: Arc<AtomicBool>,
}

impl FakeN8n {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let forms = Arc::new(Mutex::new(Vec::new()));
        let down = Arc::new(AtomicBool::new(false));
        let (shared_forms, shared_down) = (Arc::clone(&forms), Arc::clone(&down));
        // A thread fica bloqueada em `accept` até o processo de teste terminar.
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let (line, body) = read_request(&mut stream);
                let status = if shared_down.load(Ordering::SeqCst) {
                    "500 Internal Server Error"
                } else {
                    if line.starts_with("POST /form/") {
                        shared_forms.lock().expect("lock").push(body);
                    }
                    "200 OK"
                };
                let reply = format!(
                    "HTTP/1.1 {status}\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok"
                );
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        Self { port, forms, down }
    }

    fn form_count(&self) -> usize {
        self.forms.lock().expect("lock").len()
    }

    fn last_form(&self) -> String {
        self.forms
            .lock()
            .expect("lock")
            .last()
            .cloned()
            .unwrap_or_default()
    }
}

/// Ponte real em Node; o processo é encerrado ao sair do escopo.
struct Bridge {
    child: Child,
    url: String,
    dir: PathBuf,
}

impl Bridge {
    fn start(n8n_port: u16) -> Self {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/claude-bridge.mjs");
        let port = free_port();
        let dir = std::env::temp_dir().join(format!("assistente-it-{}-{port}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("pasta temporária");
        let child = Command::new("node")
            .arg(&script)
            .env("CLAUDE_BRIDGE_PORT", port.to_string())
            .env("N8N_URL", format!("http://127.0.0.1:{n8n_port}"))
            .env("RUN_STATUS_FILE", dir.join("run-status.json"))
            .env("CLAUDE_BIN", "node") // o claude de verdade nunca roda
            .env("CLAUDE_BIN_ARGS", r#"["-e","process.exit(1)"]"#)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("iniciar a ponte");
        let url = format!("http://127.0.0.1:{port}");
        let bridge = Self { child, url, dir };
        let deadline = Instant::now() + Duration::from_secs(15);
        while ureq::get(&format!("{}/health", bridge.url)).call().is_err() {
            assert!(Instant::now() < deadline, "a ponte não subiu a tempo");
            std::thread::sleep(Duration::from_millis(100));
        }
        bridge
    }

    fn event(&self, body: &str) {
        ureq::post(&format!("{}/event", self.url))
            .set("content-type", "application/json")
            .send_string(body)
            .expect("POST /event");
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn client_against_the_real_bridge() {
    if !node_available() {
        eprintln!("PULADO: o `node` não está instalado; teste de integração não executado.");
        return;
    }
    let n8n = FakeN8n::start();
    let bridge = Bridge::start(n8n.port);
    let client = Client::new(bridge.url.clone());

    // Ocioso, sem execuções anteriores, n8n acessível.
    let st = client.status(0).expect("status");
    assert_eq!(st.state, State::Idle);
    assert!(st.run.is_none() && st.last_run.is_none());
    assert!(st.services.bridge && st.services.n8n);
    assert!(st.server_time.is_some());

    // Sequência de eventos dos fluxos 01/02.
    bridge.event(r#"{"event":"start","days":5,"source":"form"}"#);
    bridge.event(r#"{"event":"total","total":10}"#);
    bridge.event(r#"{"event":"progress","count":4}"#);
    let st = client.status(0).expect("status");
    let run = st.run.expect("run");
    assert_eq!(st.state, State::Running);
    assert_eq!((run.source.as_str(), run.days), ("form", 5));
    assert_eq!((run.total, run.done), (Some(10), 4));

    // `done` nunca passa de `total`, mesmo com contagens a mais.
    bridge.event(r#"{"event":"progress","count":4}"#);
    bridge.event(r#"{"event":"progress","count":4}"#);
    let st = client.status(0).expect("status");
    assert_eq!(st.run.map(|r| (r.total, r.done)), Some((Some(10), 10)));
    let last_seq = st.log.last().expect("log").seq;

    bridge.event(r#"{"event":"finish"}"#);
    let st = client.status(last_seq).expect("status");
    assert_eq!(st.state, State::Idle);
    assert!(st.run.is_none());
    let last = st.last_run.expect("last_run");
    assert_eq!((last.days, last.emails_read, last.ok), (5, 10, true));
    assert!(last.range_from.is_some() && last.range_to.len() == 10);
    // `after` filtra: só chegou a linha do `finish`.
    assert_eq!(st.log.len(), 1);
    assert!(st.log[0].msg.starts_with("Execução concluída"));
    assert!(st.log[0].seq > last_seq);

    // "Processar agora" com 3 dias: formulário do n8n + execução manual.
    assert_eq!(client.run(3), Ok(()));
    assert_eq!(n8n.form_count(), 1);
    let form = n8n.last_form();
    assert!(form.contains("Last N days") && form.contains("field-1"));
    let st = client.status(0).expect("status");
    assert_eq!(st.state, State::Running);
    assert_eq!(
        st.run.map(|r| (r.source, r.days)),
        Some(("manual".into(), 3))
    );

    // Já em andamento: 409; o n8n não recebe um segundo formulário.
    assert_eq!(client.run(3), Err(ClientError::AlreadyRunning));
    assert_eq!(n8n.form_count(), 1);

    // O evento `start` do fluxo (source "schedule") não troca a origem manual.
    bridge.event(r#"{"event":"start","days":3,"source":"schedule"}"#);
    let st = client.status(0).expect("status");
    assert_eq!(st.run.map(|r| r.source), Some("manual".to_string()));
    bridge.event(r#"{"event":"finish"}"#);

    // "Todos os emails" (0 dias) não envia o campo de dias.
    assert_eq!(client.run(0), Ok(()));
    let form = n8n.last_form();
    assert!(form.contains("All emails") && !form.contains("field-1"));
    bridge.event(r#"{"event":"finish"}"#);

    // Dias inválidos: 400 direto na API (o Client só aceita u32 válidos).
    let err = ureq::post(&format!("{}/run", bridge.url))
        .set("content-type", "application/json")
        .send_string(r#"{"days":99999}"#)
        .expect_err("400");
    assert!(matches!(err, ureq::Error::Status(400, _)));

    // n8n fora do ar: `/status` mostra n8n=false e `/run` devolve 502.
    n8n.down.store(true, Ordering::SeqCst);
    assert!(!client.status(0).expect("status").services.n8n);
    assert_eq!(client.run(2), Err(ClientError::N8nUnreachable));
    assert_eq!(client.status(0).expect("status").state, State::Idle);
}

#[test]
fn client_reports_unreachable_when_the_bridge_stops() {
    if !node_available() {
        eprintln!("PULADO: o `node` não está instalado; teste de integração não executado.");
        return;
    }
    let n8n = FakeN8n::start();
    let mut bridge = Bridge::start(n8n.port);
    let client = Client::new(bridge.url.clone());
    assert!(client.status(0).is_ok());
    let _ = bridge.child.kill();
    let _ = bridge.child.wait();
    assert_eq!(client.status(0), Err(ClientError::Unreachable));
    assert_eq!(client.run(1), Err(ClientError::Unreachable));
}
