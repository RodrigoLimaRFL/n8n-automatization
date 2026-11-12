//! "Iniciar com o Windows": valor em HKCU\Software\Microsoft\Windows\CurrentVersion\Run.

use std::io;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
use winreg::RegKey;

/// Nome do valor na chave `Run`.
pub const VALUE_NAME: &str = "AssistenteEmails";
/// Argumento passado na inicialização do Windows: abre direto na bandeja.
pub const MINIMIZED_ARG: &str = "--minimizado";

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// `true` se o programa está registrado para iniciar com o Windows.
pub fn is_enabled() -> bool {
    value_exists(VALUE_NAME)
}

/// Liga/desliga. Ao ligar grava `"<caminho do exe>" --minimizado`.
pub fn set_enabled(enabled: bool) -> io::Result<()> {
    if enabled {
        write_value(VALUE_NAME, &startup_command()?)
    } else {
        remove_value(VALUE_NAME)
    }
}

/// Linha de comando gravada no registro para o executável atual.
fn startup_command() -> io::Result<String> {
    let exe = std::env::current_exe()?;
    Ok(format!("\"{}\" {MINIMIZED_ARG}", exe.display()))
}

/// Existe o valor `name` na chave `Run`?
fn value_exists(name: &str) -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY, KEY_READ)
        .and_then(|key| key.get_value::<String, _>(name))
        .is_ok()
}

/// Grava `name` = `command` na chave `Run`.
fn write_value(name: &str, command: &str) -> io::Result<()> {
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN_KEY)?;
    key.set_value(name, &command)
}

/// Apaga `name` da chave `Run`; não é erro se ele já não existir.
fn remove_value(name: &str) -> io::Result<()> {
    let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)?;
    match key.delete_value(name) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Remove o valor de teste ao sair, mesmo se o teste falhar.
    struct Cleanup(&'static str);

    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = remove_value(self.0);
        }
    }

    #[test]
    fn round_trip() {
        const NAME: &str = "AssistenteEmailsTeste-round-trip";
        let _cleanup = Cleanup(NAME);
        assert!(!value_exists(NAME));
        write_value(NAME, "\"C:\\x.exe\" --minimizado").unwrap();
        assert!(value_exists(NAME));
        remove_value(NAME).unwrap();
        assert!(!value_exists(NAME));
        remove_value(NAME).unwrap();
    }

    #[test]
    fn command_quotes_exe_and_adds_argument() {
        let command = startup_command().unwrap();
        assert!(command.starts_with('"'));
        assert!(command.ends_with("\" --minimizado"));
    }
}
