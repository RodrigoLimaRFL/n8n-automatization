//! Tipos que espelham a resposta de `GET /status` (ver `app/API.md`).
//! Este arquivo é o contrato: não mude os campos sem mudar `API.md` e a ponte.

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum State {
    #[default]
    Idle,
    Running,
    Interrupted,
}

/// Execução em andamento (ou interrompida).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Run {
    /// "manual", "daily" ou "form".
    pub source: String,
    /// 0 = todos os emails.
    pub days: u32,
    pub started_at: String,
    /// `None` até o fluxo saber quantos emails serão processados.
    pub total: Option<u32>,
    pub done: u32,
}

/// Última execução concluída.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LastRun {
    pub started_at: String,
    pub finished_at: String,
    /// 0 = todos os emails.
    pub days: u32,
    /// `None` quando `days == 0`. Formato `AAAA-MM-DD`.
    pub range_from: Option<String>,
    pub range_to: String,
    pub emails_read: u32,
    pub ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LogLine {
    pub seq: u64,
    pub ts: String,
    /// "info", "warn" ou "error".
    pub level: String,
    pub msg: String,
}

impl LogLine {
    /// Linha gerada pelo próprio app (sem número de sequência da ponte).
    pub fn local(ts: String, level: &str, msg: &str) -> Self {
        Self {
            seq: 0,
            ts,
            level: level.to_string(),
            msg: msg.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
pub struct Services {
    pub bridge: bool,
    pub n8n: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct Status {
    #[serde(default)]
    pub state: State,
    #[serde(default)]
    pub run: Option<Run>,
    #[serde(default)]
    pub last_run: Option<LastRun>,
    #[serde(default)]
    pub log: Vec<LogLine>,
    #[serde(default)]
    pub services: Services,
    #[serde(default)]
    pub server_time: Option<String>,
}
