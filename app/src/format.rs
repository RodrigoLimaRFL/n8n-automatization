//! Textos em português do Brasil. Funções puras (sem I/O), todas com testes unitários.
//! Datas chegam como `AAAA-MM-DDTHH:MM:SS...` em horário local; só os 19 primeiros caracteres importam.

use crate::model::{LastRun, LogLine, Run, State, Status};

/// Texto da barra de progresso enquanto o total de emails ainda não é conhecido.
pub const MSG_SEARCHING: &str = "Buscando emails...";
const APP_NAME: &str = "Assistente de Emails";
/// Limite de caracteres do texto de dica de um ícone da bandeja (sem o terminador).
const TOOLTIP_MAX_CHARS: usize = 127;

/// `true` se `s` tem só dígitos ASCII.
fn all_digits(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_digit())
}

/// "AAAA-MM-DD" (10 primeiros caracteres) vira "DD/MM/AAAA"; `None` se malformado.
fn date_br(iso: &str) -> Option<String> {
    let date = iso.get(..10)?;
    let (year, month, day) = (date.get(0..4)?, date.get(5..7)?, date.get(8..10)?);
    let separators_ok = date.get(4..5) == Some("-") && date.get(7..8) == Some("-");
    (separators_ok && all_digits(year) && all_digits(month) && all_digits(day))
        .then(|| format!("{day}/{month}/{year}"))
}

/// Hora de um timestamp ISO: `HH:MM` ou `HH:MM:SS` conforme `len` (5 ou 8); `None` se malformado.
fn time_part(iso: &str, len: usize) -> Option<&str> {
    let time = iso.get(11..11 + len)?;
    let separator = iso.get(10..11)?;
    let shape_ok = (separator == "T" || separator == " ")
        && time.get(2..3) == Some(":")
        && all_digits(time.get(0..2)?)
        && all_digits(time.get(3..5)?)
        && (len == 5 || (time.get(5..6) == Some(":") && all_digits(time.get(6..8)?)));
    shape_ok.then_some(time)
}

/// Data no formato brasileiro, ou o texto original se não der para interpretar.
fn date_or_raw(iso: &str) -> String {
    date_br(iso).unwrap_or_else(|| iso.to_string())
}

/// "12/11/2026 10:07" a partir de `finished_at`.
pub fn last_run_datetime(last: &LastRun) -> String {
    let ts = &last.finished_at;
    match (date_br(ts), time_part(ts, 5)) {
        (Some(date), Some(time)) => format!("{date} {time}"),
        _ => ts.clone(),
    }
}

/// "últimos 5 dias (07/11/2026 a 12/11/2026)", "último dia (12/11/2026)" ou "todos os emails (até 12/11/2026)".
pub fn period_text(last: &LastRun) -> String {
    let to = date_or_raw(&last.range_to);
    match (last.days, &last.range_from) {
        (0, _) => format!("todos os emails (até {to})"),
        (1, _) => format!("último dia ({to})"),
        (n, Some(from)) => format!("últimos {n} dias ({} a {to})", date_or_raw(from)),
        (n, None) => format!("últimos {n} dias (até {to})"),
    }
}

/// "68 emails" ou "1 email".
pub fn emails_read_text(n: u32) -> String {
    let noun = if n == 1 { "email" } else { "emails" };
    format!("{n} {noun}")
}

/// "12 de 78 emails lidos"; "Buscando emails..." quando `total` é desconhecido.
pub fn progress_text(run: &Run) -> String {
    match run.total {
        None => MSG_SEARCHING.to_string(),
        Some(total) => {
            let done = run.done.min(total);
            let noun = if total == 1 {
                "email lido"
            } else {
                "emails lidos"
            };
            format!("{done} de {total} {noun}")
        }
    }
}

/// 0..=100; `None` (total desconhecido) vira 0.
pub fn progress_percent(run: &Run) -> u32 {
    match run.total {
        Some(total) if total > 0 => {
            let percent = u64::from(run.done) * 100 / u64::from(total);
            u32::try_from(percent.min(100)).unwrap_or(100)
        }
        _ => 0,
    }
}

/// "[10:07:41] Execução concluída: 68 emails lidos"; avisos e erros ganham o prefixo "Aviso: " / "Erro: ".
pub fn log_line(line: &LogLine) -> String {
    let time = time_part(&line.ts, 8).unwrap_or(&line.ts);
    let prefix = match line.level.as_str() {
        "warn" => "Aviso: ",
        "error" => "Erro: ",
        _ => "",
    };
    format!("[{time}] {prefix}{}", line.msg)
}

/// Texto do ícone da bandeja (máx. 127 caracteres): "Assistente de Emails: ocioso" / "...: 12 de 78 emails".
pub fn tray_tooltip(status: Option<&Status>) -> String {
    let detail = match status {
        None => "sem conexão com a ponte".to_string(),
        Some(s) => match (s.state, &s.run) {
            (State::Idle, _) => "ocioso".to_string(),
            (State::Interrupted, _) => "execução interrompida".to_string(),
            (State::Running, Some(run)) => match run.total {
                Some(total) => format!("{} de {total} emails", run.done.min(total)),
                None => "buscando emails...".to_string(),
            },
            (State::Running, None) => "em execução".to_string(),
        },
    };
    format!("{APP_NAME}: {detail}")
        .chars()
        .take(TOOLTIP_MAX_CHARS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn last(days: u32, from: Option<&str>) -> LastRun {
        LastRun {
            started_at: "2026-11-12T10:05:03-03:00".into(),
            finished_at: "2026-11-12T10:07:41-03:00".into(),
            days,
            range_from: from.map(String::from),
            range_to: "2026-11-12".into(),
            emails_read: 68,
            ok: true,
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

    fn status(state: State, run: Option<Run>) -> Status {
        Status {
            state,
            run,
            ..Status::default()
        }
    }

    fn line(level: &str, ts: &str) -> LogLine {
        LogLine {
            seq: 1,
            ts: ts.into(),
            level: level.into(),
            msg: "Texto".into(),
        }
    }

    #[test]
    fn datetime_formats_and_falls_back() {
        assert_eq!(last_run_datetime(&last(5, None)), "12/11/2026 10:07");
        let mut l = last(5, None);
        l.finished_at = "ontem".into();
        assert_eq!(last_run_datetime(&l), "ontem");
        l.finished_at = "2026-11-12".into();
        assert_eq!(last_run_datetime(&l), "2026-11-12");
        l.finished_at = "é".repeat(12);
        assert_eq!(last_run_datetime(&l), l.finished_at);
        l.finished_at = String::new();
        assert_eq!(last_run_datetime(&l), "");
    }

    #[test]
    fn period_variants() {
        assert_eq!(
            period_text(&last(5, Some("2026-11-07"))),
            "últimos 5 dias (07/11/2026 a 12/11/2026)"
        );
        assert_eq!(
            period_text(&last(1, Some("2026-11-12"))),
            "último dia (12/11/2026)"
        );
        assert_eq!(
            period_text(&last(0, None)),
            "todos os emails (até 12/11/2026)"
        );
        assert_eq!(
            period_text(&last(3, None)),
            "últimos 3 dias (até 12/11/2026)"
        );
        let mut l = last(2, Some("xx"));
        l.range_to = "?".into();
        assert_eq!(period_text(&l), "últimos 2 dias (xx a ?)");
    }

    #[test]
    fn emails_plural() {
        assert_eq!(emails_read_text(0), "0 emails");
        assert_eq!(emails_read_text(1), "1 email");
        assert_eq!(emails_read_text(68), "68 emails");
    }

    #[test]
    fn progress_text_cases() {
        assert_eq!(progress_text(&run(Some(78), 12)), "12 de 78 emails lidos");
        assert_eq!(progress_text(&run(None, 0)), "Buscando emails...");
        assert_eq!(progress_text(&run(Some(10), 15)), "10 de 10 emails lidos");
        assert_eq!(progress_text(&run(Some(0), 0)), "0 de 0 emails lidos");
        assert_eq!(progress_text(&run(Some(1), 1)), "1 de 1 email lido");
    }

    #[test]
    fn progress_percent_cases() {
        assert_eq!(progress_percent(&run(Some(78), 39)), 50);
        assert_eq!(progress_percent(&run(Some(10), 15)), 100);
        assert_eq!(progress_percent(&run(Some(0), 3)), 0);
        assert_eq!(progress_percent(&run(None, 3)), 0);
        assert_eq!(progress_percent(&run(Some(u32::MAX), u32::MAX)), 100);
    }

    #[test]
    fn log_line_prefixes_and_fallback() {
        let ts = "2026-11-12T10:07:41-03:00";
        assert_eq!(log_line(&line("info", ts)), "[10:07:41] Texto");
        assert_eq!(log_line(&line("warn", ts)), "[10:07:41] Aviso: Texto");
        assert_eq!(log_line(&line("error", ts)), "[10:07:41] Erro: Texto");
        assert_eq!(log_line(&line("info", "agora")), "[agora] Texto");
        assert_eq!(
            log_line(&line("info", "2026-11-12T10:07")),
            "[2026-11-12T10:07] Texto"
        );
    }

    #[test]
    fn tooltip_variants() {
        assert_eq!(
            tray_tooltip(None),
            "Assistente de Emails: sem conexão com a ponte"
        );
        assert_eq!(
            tray_tooltip(Some(&status(State::Idle, None))),
            "Assistente de Emails: ocioso"
        );
        assert_eq!(
            tray_tooltip(Some(&status(State::Running, Some(run(Some(78), 12))))),
            "Assistente de Emails: 12 de 78 emails"
        );
        assert_eq!(
            tray_tooltip(Some(&status(State::Running, Some(run(None, 0))))),
            "Assistente de Emails: buscando emails..."
        );
        assert_eq!(
            tray_tooltip(Some(&status(State::Running, None))),
            "Assistente de Emails: em execução"
        );
        assert_eq!(
            tray_tooltip(Some(&status(State::Interrupted, None))),
            "Assistente de Emails: execução interrompida"
        );
    }

    #[test]
    fn tooltip_is_bounded() {
        let huge = run(Some(u32::MAX), u32::MAX);
        let text = tray_tooltip(Some(&status(State::Running, Some(huge))));
        assert!(text.chars().count() <= 127);
    }
}
