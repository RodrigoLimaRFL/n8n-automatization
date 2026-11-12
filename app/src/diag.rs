//! Registro de diagnóstico em arquivo (`%LOCALAPPDATA%\AssistenteEmails\app.log`).
//!
//! O aplicativo não tem console e, quando falha na inicialização, antes não deixava rastro.
//! Este registro só guarda eventos de partida e falhas; é apagado quando passa de [`MAX_BYTES`].

use crate::clock;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Tamanho máximo do arquivo; ao passar disso ele recomeça vazio.
const MAX_BYTES: u64 = 64 * 1024;

/// Caminho do arquivo de registro, se a pasta de dados locais do usuário existir.
pub fn log_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(|dir| PathBuf::from(dir).join("AssistenteEmails").join("app.log"))
}

/// Acrescenta uma linha com data e hora. Falhas de escrita são ignoradas de propósito:
/// o registro nunca pode impedir o aplicativo de funcionar.
pub fn log(message: &str) {
    if let Some(path) = log_path() {
        append_line(&path, &format!("{} {message}", clock::local_timestamp()));
    }
}

fn append_line(path: &Path, line: &str) {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let too_big = fs::metadata(path).is_ok_and(|m| m.len() > MAX_BYTES);
    if too_big {
        let _ = fs::remove_file(path);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("assistente-diag-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("app.log")
    }

    #[test]
    fn appends_lines_and_creates_the_folder() {
        let path = temp_file("append");
        append_line(&path, "primeira");
        append_line(&path, "segunda");
        assert_eq!(
            fs::read_to_string(&path).unwrap_or_default(),
            "primeira\nsegunda\n"
        );
        let _ = fs::remove_dir_all(path.parent().unwrap_or(&path));
    }

    #[test]
    fn restarts_when_the_file_gets_too_big() {
        let path = temp_file("trim");
        append_line(&path, &"x".repeat(MAX_BYTES as usize + 10));
        append_line(&path, "nova");
        assert_eq!(fs::read_to_string(&path).unwrap_or_default(), "nova\n");
        let _ = fs::remove_dir_all(path.parent().unwrap_or(&path));
    }
}
