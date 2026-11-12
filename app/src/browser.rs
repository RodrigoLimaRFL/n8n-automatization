//! Abre páginas no navegador padrão, sem piscar janela de console.

use std::io;
use std::os::windows::process::CommandExt;
use std::process::Command;

/// Google Agenda.
pub const CALENDAR_URL: &str = "https://calendar.google.com/calendar/u/0/r";
/// Google Tarefas.
pub const TASKS_URL: &str = "https://tasks.google.com/tasks/";

/// `CREATE_NO_WINDOW` do Windows.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Abre o Google Agenda no navegador padrão.
pub fn open_calendar() -> io::Result<()> {
    open(CALENDAR_URL)
}

/// Abre o Google Tarefas no navegador padrão.
pub fn open_tasks() -> io::Result<()> {
    open(TASKS_URL)
}

/// Abre `url` (só `http://` ou `https://`) no navegador padrão, sem esperar por ele.
pub fn open(url: &str) -> io::Result<()> {
    validate(url)?;
    Command::new("rundll32")
        .arg("url.dll,FileProtocolHandler")
        .arg(url)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(drop)
}

/// Aceita só URLs http(s) sem espaços nem caracteres de controle.
fn validate(url: &str) -> io::Result<()> {
    let lower = url.to_ascii_lowercase();
    let scheme_ok = lower.starts_with("http://") || lower.starts_with("https://");
    let chars_ok = !url.chars().any(|c| c.is_control() || c.is_whitespace());
    if scheme_ok && chars_ok {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "só são aceitos endereços http(s) válidos",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_and_https() {
        assert!(validate("https://example.com/a?b=1").is_ok());
        assert!(validate("http://localhost:5678").is_ok());
        assert!(validate("HTTPS://EXAMPLE.COM").is_ok());
        assert!(validate(CALENDAR_URL).is_ok());
        assert!(validate(TASKS_URL).is_ok());
    }

    #[test]
    fn rejects_other_schemes_and_junk() {
        for url in [
            "",
            "file:///C:/Windows/notepad.exe",
            "calc.exe",
            "ftp://example.com",
            "javascript:alert(1)",
            "https://a.com b",
            "https://a.com\n",
        ] {
            let err = validate(url).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{url:?}");
        }
        assert_eq!(
            open("calc.exe").unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }
}
