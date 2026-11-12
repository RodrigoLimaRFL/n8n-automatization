//! Assistente de Emails: acompanha e aciona o processamento de emails (n8n + Claude).
//!
//! Camadas (de baixo para cima), cada uma só conhece as de baixo:
//! `model` (dados) → `format` (textos em pt-BR) → `client` (HTTP) → `worker` (thread de
//! consulta) → `ui` (janela e bandeja). `autostart`, `browser`, `clock`, `diag`, `shell` e `singleton` são utilitários
//! do Windows, sem dependência de interface.

pub mod autostart;
pub mod browser;
pub mod client;
pub mod clock;
pub mod diag;
pub mod format;
pub mod model;
pub mod shell;
pub mod singleton;
pub mod ui;
pub mod worker;
