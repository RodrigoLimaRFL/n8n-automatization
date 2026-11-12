//! Cliente HTTP da ponte (ver `app/API.md`). Bloqueante; chamado só pela thread do `worker`.

use crate::model::Status;
use std::fmt;
use std::time::Duration;

/// Tempo máximo de `GET /status`.
const STATUS_TIMEOUT: Duration = Duration::from_secs(3);
/// Tempo máximo de `POST /run` (a ponte envia o formulário ao n8n).
const RUN_TIMEOUT: Duration = Duration::from_secs(15);

/// Falhas possíveis ao falar com a ponte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientError {
    /// A ponte não respondeu (parada ou porta errada).
    Unreachable,
    /// `POST /run` com 409.
    AlreadyRunning,
    /// O n8n está parado (502 `n8n_unreachable`).
    N8nUnreachable,
    /// Qualquer outra resposta inesperada.
    Other(String),
}

/// Mensagens em português para o log da janela.
impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreachable => f.write_str("A ponte do Claude não está respondendo"),
            Self::AlreadyRunning => f.write_str("Já existe uma execução em andamento"),
            Self::N8nUnreachable => f.write_str("O n8n não está respondendo"),
            Self::Other(msg) => write!(f, "Resposta inesperada da ponte: {msg}"),
        }
    }
}

impl std::error::Error for ClientError {}

/// Cliente da ponte do Claude.
#[derive(Debug, Clone)]
pub struct Client {
    base: String,
}

impl Client {
    /// `base` sem barra final, por exemplo "http://127.0.0.1:5681".
    pub fn new(base: impl Into<String>) -> Self {
        Self { base: base.into() }
    }

    /// Endereço base da ponte.
    pub fn base(&self) -> &str {
        &self.base
    }

    /// `GET /status?after=<after_seq>`. Timeout curto (3 s).
    pub fn status(&self, after_seq: u64) -> Result<Status, ClientError> {
        let url = format!("{}/status?after={after_seq}", self.base);
        let response = ureq::get(&url)
            .timeout(STATUS_TIMEOUT)
            .call()
            .map_err(|e| map_error(e, false))?;
        let body = response
            .into_string()
            .map_err(|e| ClientError::Other(format!("falha ao ler a resposta ({e})")))?;
        serde_json::from_str::<Status>(&body)
            .map_err(|e| ClientError::Other(format!("JSON inválido ({e})")))
    }

    /// `POST /run` com `{"days": days}` (0 = todos os emails).
    pub fn run(&self, days: u32) -> Result<(), ClientError> {
        let url = format!("{}/run", self.base);
        ureq::post(&url)
            .timeout(RUN_TIMEOUT)
            .set("content-type", "application/json")
            .send_string(&format!("{{\"days\":{days}}}"))
            .map(drop)
            .map_err(|e| map_error(e, true))
    }
}

/// Traduz um erro do `ureq`; `is_run` habilita os códigos específicos de `POST /run`.
fn map_error(err: ureq::Error, is_run: bool) -> ClientError {
    match err {
        ureq::Error::Transport(_) => ClientError::Unreachable,
        ureq::Error::Status(409, _) if is_run => ClientError::AlreadyRunning,
        ureq::Error::Status(502, response) if is_run => {
            let body = response.into_string().unwrap_or_default();
            if body.contains("n8n_unreachable") {
                ClientError::N8nUnreachable
            } else {
                ClientError::Other("HTTP 502".to_string())
            }
        }
        ureq::Error::Status(code, _) => ClientError::Other(format!("HTTP {code}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    /// Lê a requisição inteira (cabeçalhos + corpo) e devolve o texto.
    fn read_request(stream: &mut std::net::TcpStream) -> String {
        let mut data = Vec::new();
        let mut buf = [0u8; 1024];
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
                    return text;
                }
            } else if n == 0 {
                return text;
            }
        }
    }

    /// Servidor de uma conexão: responde `status`/`body` e devolve a requisição recebida.
    fn serve_once(status: &str, body: &str) -> (Client, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            stream.write_all(response.as_bytes()).unwrap();
            request
        });
        (Client::new(format!("http://127.0.0.1:{port}")), handle)
    }

    #[test]
    fn status_success() {
        let body = r#"{"state":"running","run":{"source":"manual","days":5,"started_at":"x","total":78,"done":12},"log":[{"seq":41,"ts":"t","level":"info","msg":"m"}],"services":{"bridge":true,"n8n":true}}"#;
        let (client, server) = serve_once("200 OK", body);
        let status = client.status(40).unwrap();
        assert_eq!(status.state, crate::model::State::Running);
        assert_eq!(status.run.map(|r| r.done), Some(12));
        assert_eq!(status.log[0].seq, 41);
        assert!(server.join().unwrap().starts_with("GET /status?after=40 "));
    }

    #[test]
    fn status_malformed_json() {
        let (client, _server) = serve_once("200 OK", "isto não é json");
        assert!(matches!(client.status(0), Err(ClientError::Other(_))));
    }

    #[test]
    fn status_http_error_is_other() {
        let (client, _server) = serve_once("500 Internal Server Error", "{}");
        assert_eq!(client.status(0), Err(ClientError::Other("HTTP 500".into())));
    }

    #[test]
    fn run_success_sends_days() {
        let (client, server) = serve_once("200 OK", r#"{"ok":true}"#);
        assert_eq!(client.run(5), Ok(()));
        let request = server.join().unwrap();
        assert!(request.starts_with("POST /run "));
        assert!(request.ends_with(r#"{"days":5}"#));
    }

    #[test]
    fn run_conflict() {
        let (client, _s) = serve_once("409 Conflict", r#"{"ok":false,"error":"already_running"}"#);
        assert_eq!(client.run(0), Err(ClientError::AlreadyRunning));
    }

    #[test]
    fn run_n8n_unreachable() {
        let (client, _s) = serve_once(
            "502 Bad Gateway",
            r#"{"ok":false,"error":"n8n_unreachable"}"#,
        );
        assert_eq!(client.run(1), Err(ClientError::N8nUnreachable));
    }

    #[test]
    fn run_bad_request() {
        let (client, _s) = serve_once("400 Bad Request", r#"{"ok":false,"error":"invalid_days"}"#);
        assert_eq!(client.run(1), Err(ClientError::Other("HTTP 400".into())));
    }

    #[test]
    fn connection_refused_is_unreachable() {
        let port = {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap().port()
        };
        let client = Client::new(format!("http://127.0.0.1:{port}"));
        assert_eq!(client.status(0), Err(ClientError::Unreachable));
        assert_eq!(client.run(1), Err(ClientError::Unreachable));
    }

    #[test]
    fn messages_are_portuguese() {
        assert_eq!(
            ClientError::Unreachable.to_string(),
            "A ponte do Claude não está respondendo"
        );
        assert_eq!(
            ClientError::AlreadyRunning.to_string(),
            "Já existe uma execução em andamento"
        );
        assert_eq!(
            ClientError::N8nUnreachable.to_string(),
            "O n8n não está respondendo"
        );
        assert!(ClientError::Other("x".into()).to_string().contains('x'));
        assert_eq!(Client::new("http://a").base(), "http://a");
    }
}
