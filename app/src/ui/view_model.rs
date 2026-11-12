//! Decisão pura do que cada controle mostra, a partir do status da ponte.
//! Não conhece a janela: é testável sem criar nenhum controle.

use crate::format::{self, MSG_SEARCHING};
use crate::model::{State, Status};

pub const MSG_IDLE: &str = "Nenhuma execução em andamento";
pub const MSG_INTERRUPTED: &str = "Execução interrompida";
pub const MSG_OFFLINE: &str = "Sem conexão com a ponte do Claude";
pub const MSG_N8N_DOWN: &str = "O n8n não está respondendo";
pub const MSG_CONNECTING: &str = "Conectando à ponte do Claude...";
pub const TOOLTIP_OFFLINE: &str = "Assistente de Emails: sem conexão";

/// Tudo o que a janela e a bandeja exibem; comparável para só mexer nos controles que mudaram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewModel {
    /// "Data e hora: ...", "Período analisado: ...", "Emails lidos: ...".
    pub last_run_lines: [String; 3],
    /// Texto centralizado abaixo da barra de progresso.
    pub progress_label: String,
    /// 0..=100.
    pub progress_percent: u32,
    /// Habilita o botão "Processar agora" (e o item do menu da bandeja).
    pub can_run: bool,
    /// Há uma execução em andamento.
    pub running: bool,
    pub tray_tooltip: String,
}

/// `status`: último status recebido (`None` se a ponte nunca respondeu); `offline`: a última consulta falhou.
pub fn build(status: Option<&Status>, offline: bool) -> ViewModel {
    let last_run_lines = last_run_lines(status);

    let Some(st) = status else {
        return ViewModel {
            last_run_lines,
            progress_label: if offline { MSG_OFFLINE } else { MSG_CONNECTING }.into(),
            progress_percent: 0,
            can_run: false,
            running: false,
            tray_tooltip: if offline {
                TOOLTIP_OFFLINE.into()
            } else {
                format::tray_tooltip(None)
            },
        };
    };

    if offline {
        return ViewModel {
            last_run_lines,
            progress_label: MSG_OFFLINE.into(),
            progress_percent: 0,
            can_run: false,
            running: false,
            tray_tooltip: TOOLTIP_OFFLINE.into(),
        };
    }

    let (progress_label, progress_percent, running) = match (st.state, st.run.as_ref()) {
        (State::Running, Some(run)) => (
            format::progress_text(run),
            format::progress_percent(run).min(100),
            true,
        ),
        (State::Running, None) => (MSG_SEARCHING.to_string(), 0, true),
        _ if !st.services.n8n => (MSG_N8N_DOWN.to_string(), 0, false),
        (State::Interrupted, _) => (MSG_INTERRUPTED.to_string(), 0, false),
        (State::Idle, _) => (MSG_IDLE.to_string(), 0, false),
    };

    ViewModel {
        last_run_lines,
        progress_label,
        progress_percent,
        can_run: !running,
        running,
        tray_tooltip: format::tray_tooltip(Some(st)),
    }
}

fn last_run_lines(status: Option<&Status>) -> [String; 3] {
    match status.and_then(|s| s.last_run.as_ref()) {
        None => [
            "Data e hora: nenhuma execução ainda".into(),
            "Período analisado: -".into(),
            "Emails lidos: -".into(),
        ],
        Some(last) => {
            let suffix = if last.ok { "" } else { " (com erros)" };
            [
                format!("Data e hora: {}{suffix}", format::last_run_datetime(last)),
                format!("Período analisado: {}", format::period_text(last)),
                format!(
                    "Emails lidos: {}{suffix}",
                    format::emails_read_text(last.emails_read)
                ),
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LastRun, Run, Services};

    fn last(ok: bool) -> LastRun {
        LastRun {
            started_at: "2026-11-12T10:05:03-03:00".into(),
            finished_at: "2026-11-12T10:07:41-03:00".into(),
            days: 5,
            range_from: Some("2026-11-07".into()),
            range_to: "2026-11-12".into(),
            emails_read: 68,
            ok,
        }
    }

    fn run(total: Option<u32>, done: u32) -> Run {
        Run {
            source: "manual".into(),
            days: 5,
            started_at: "2026-11-12T10:05:03-03:00".into(),
            total,
            done,
        }
    }

    fn status(state: State, run: Option<Run>, last_run: Option<LastRun>, n8n: bool) -> Status {
        Status {
            state,
            run,
            last_run,
            log: vec![],
            services: Services { bridge: true, n8n },
            server_time: None,
        }
    }

    #[test]
    fn idle_with_last_run() {
        let st = status(State::Idle, None, Some(last(true)), true);
        let vm = build(Some(&st), false);
        assert_eq!(vm.last_run_lines[0], "Data e hora: 12/11/2026 10:07");
        assert!(vm.last_run_lines[1].starts_with("Período analisado: "));
        assert_eq!(vm.last_run_lines[2], "Emails lidos: 68 emails");
        assert_eq!(vm.progress_label, MSG_IDLE);
        assert_eq!(vm.progress_percent, 0);
        assert!(vm.can_run);
        assert!(!vm.running);
    }

    #[test]
    fn last_run_with_errors_gets_suffix() {
        let st = status(State::Idle, None, Some(last(false)), true);
        let vm = build(Some(&st), false);
        assert!(vm.last_run_lines[0].ends_with(" (com erros)"));
        assert!(vm.last_run_lines[2].ends_with(" (com erros)"));
    }

    #[test]
    fn no_last_run() {
        let st = status(State::Idle, None, None, true);
        let vm = build(Some(&st), false);
        assert_eq!(vm.last_run_lines[0], "Data e hora: nenhuma execução ainda");
        assert!(vm.can_run);
    }

    #[test]
    fn running_with_total() {
        let st = status(State::Running, Some(run(Some(78), 12)), None, true);
        let vm = build(Some(&st), false);
        assert_eq!(vm.progress_label, "12 de 78 emails lidos");
        assert_eq!(vm.progress_percent, 15);
        assert!(vm.running);
        assert!(!vm.can_run);
    }

    #[test]
    fn running_without_total() {
        let st = status(State::Running, Some(run(None, 0)), None, true);
        let vm = build(Some(&st), false);
        assert_eq!(vm.progress_label, MSG_SEARCHING);
        assert_eq!(vm.progress_percent, 0);
        assert!(!vm.can_run);
    }

    #[test]
    fn running_without_run_details() {
        let st = status(State::Running, None, None, true);
        let vm = build(Some(&st), false);
        assert_eq!(vm.progress_label, MSG_SEARCHING);
        assert!(!vm.can_run);
    }

    #[test]
    fn interrupted_allows_new_run() {
        let st = status(State::Interrupted, Some(run(Some(78), 12)), None, true);
        let vm = build(Some(&st), false);
        assert_eq!(vm.progress_label, MSG_INTERRUPTED);
        assert_eq!(vm.progress_percent, 0);
        assert!(vm.can_run);
    }

    #[test]
    fn n8n_down_message_but_button_stays_enabled() {
        let st = status(State::Idle, None, Some(last(true)), false);
        let vm = build(Some(&st), false);
        assert_eq!(vm.progress_label, MSG_N8N_DOWN);
        assert!(vm.can_run);
    }

    #[test]
    fn offline_keeps_last_known_run_and_blocks_button() {
        let st = status(
            State::Running,
            Some(run(Some(78), 12)),
            Some(last(true)),
            true,
        );
        let vm = build(Some(&st), true);
        assert_eq!(vm.progress_label, MSG_OFFLINE);
        assert_eq!(vm.progress_percent, 0);
        assert!(!vm.can_run);
        assert!(!vm.running);
        assert_eq!(vm.last_run_lines[0], "Data e hora: 12/11/2026 10:07");
        assert_eq!(vm.tray_tooltip, TOOLTIP_OFFLINE);
    }

    #[test]
    fn never_connected() {
        let vm = build(None, false);
        assert_eq!(vm.progress_label, MSG_CONNECTING);
        assert!(!vm.can_run);
        let vm = build(None, true);
        assert_eq!(vm.progress_label, MSG_OFFLINE);
        assert_eq!(vm.last_run_lines[0], "Data e hora: nenhuma execução ainda");
    }
}
