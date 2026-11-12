//! "Iniciar com o Windows": tarefa agendada de logon no Agendador de Tarefas.
//!
//! Por que não a chave `Run` do registro: quando o programa é registrado a partir de um processo
//! empacotado (MSIX, como o app do Claude), as escritas em `HKCU` e em `%LOCALAPPDATA%` vão para uma
//! cópia privada do pacote, que o Windows não enxerga na hora do logon. O Agendador de Tarefas é um
//! serviço do sistema, então o registro dele vale em qualquer contexto.

use std::io;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
use winreg::RegKey;

/// Nome da tarefa agendada.
pub const TASK_NAME: &str = "Assistente de Emails";
/// Argumento passado na inicialização do Windows: abre direto na bandeja.
pub const MINIMIZED_ARG: &str = "--minimizado";

/// Valor que versões antigas gravavam em `HKCU\...\Run` (removido ao ligar ou desligar).
const LEGACY_RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const LEGACY_VALUE_NAME: &str = "AssistenteEmails";

/// Atraso depois do logon, para o Windows terminar de montar a área de trabalho.
const LOGON_DELAY: &str = "PT10S";
/// `CREATE_NO_WINDOW`: não piscar janela de console ao chamar o PowerShell.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `true` se o programa está registrado para iniciar com o Windows.
pub fn is_enabled() -> bool {
    task_exists(TASK_NAME)
}

/// Liga/desliga. Ao ligar registra a tarefa para o executável atual (`<exe> --minimizado`).
pub fn set_enabled(enabled: bool) -> io::Result<()> {
    remove_legacy_run_value();
    if enabled {
        register_task(TASK_NAME, &std::env::current_exe()?)
    } else {
        delete_task(TASK_NAME)
    }
}

/// Existe a tarefa `name`?
fn task_exists(name: &str) -> bool {
    Command::new("schtasks")
        .args(["/Query", "/TN", name])
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Cria (ou substitui) a tarefa de logon `name` para `exe`.
fn register_task(name: &str, exe: &Path) -> io::Result<()> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
        ])
        .arg(register_script(name, exe))
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ))
    }
}

/// Apaga a tarefa `name`; não é erro se ela já não existir.
fn delete_task(name: &str) -> io::Result<()> {
    if !task_exists(name) {
        return Ok(());
    }
    let status = Command::new("schtasks")
        .args(["/Delete", "/TN", name, "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            "não foi possível remover a tarefa agendada",
        ))
    }
}

/// Texto de `s` dentro de aspas simples do PowerShell (aspas simples viram duas).
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Script do PowerShell que registra a tarefa. Não usa aspas duplas, para passar intacto
/// como argumento de linha de comando.
fn register_script(name: &str, exe: &Path) -> String {
    let exe_text = exe.display().to_string();
    [
        "$ErrorActionPreference = 'Stop'".to_string(),
        format!("$exe = {}", ps_quote(&exe_text)),
        "$user = [Security.Principal.WindowsIdentity]::GetCurrent().Name".to_string(),
        format!(
            "$action = New-ScheduledTaskAction -Execute $exe -Argument {} -WorkingDirectory (Split-Path $exe)",
            ps_quote(MINIMIZED_ARG)
        ),
        format!(
            "$trigger = New-ScheduledTaskTrigger -AtLogOn -User $user; $trigger.Delay = {}",
            ps_quote(LOGON_DELAY)
        ),
        "$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances IgnoreNew".to_string(),
        "$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited".to_string(),
        format!(
            "Register-ScheduledTask -TaskName {} -Action $action -Trigger $trigger -Settings $settings -Principal $principal -Force | Out-Null",
            ps_quote(name)
        ),
    ]
    .join("; ")
}

/// Apaga o valor `Run` que versões antigas gravavam. Falhas são ignoradas: é só limpeza.
fn remove_legacy_run_value() {
    let key =
        RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(LEGACY_RUN_KEY, KEY_SET_VALUE);
    if let Ok(key) = key {
        let _ = key.delete_value(LEGACY_VALUE_NAME);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Remove a tarefa de teste ao sair, mesmo se o teste falhar.
    struct Cleanup(String);

    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = delete_task(&self.0);
        }
    }

    #[test]
    fn quoting_doubles_single_quotes() {
        assert_eq!(ps_quote("C:\\a b\\x.exe"), "'C:\\a b\\x.exe'");
        assert_eq!(ps_quote("it's"), "'it''s'");
    }

    #[test]
    fn script_targets_the_exe_and_has_no_double_quotes() {
        let script = register_script("Tarefa", Path::new(r"C:\Pasta d'Teste\app.exe"));
        assert!(script.contains(r"'C:\Pasta d''Teste\app.exe'"));
        assert!(script.contains("'--minimizado'"));
        assert!(script.contains("-AtLogOn"));
        assert!(!script.contains('"'));
    }

    #[test]
    fn round_trip_with_the_real_scheduler() {
        let name = format!("Assistente de Emails (teste {})", std::process::id());
        let _cleanup = Cleanup(name.clone());
        assert!(!task_exists(&name));
        register_task(&name, Path::new(r"C:\Windows\System32\notepad.exe")).unwrap();
        assert!(task_exists(&name));
        delete_task(&name).unwrap();
        assert!(!task_exists(&name));
        delete_task(&name).unwrap();
    }
}
