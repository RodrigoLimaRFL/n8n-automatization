//! Garante uma única instância: um mutex nomeado. Se já existe uma, traz a janela da primeira para a frente.

use std::ptr::null;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
};

/// Nome do mutex da instância única.
const MUTEX_NAME: &str = r"Local\AssistenteEmails-single-instance";

/// Mantém o mutex enquanto estiver vivo.
pub struct Singleton {
    /// Handle do mutex; 0 se o Windows não conseguiu criá-lo (o app roda mesmo assim).
    handle: isize,
}

impl Drop for Singleton {
    fn drop(&mut self) {
        if self.handle != 0 {
            // SAFETY: `handle` é um handle de mutex válido criado por `CreateMutexW`
            // e pertence só a este valor; é fechado uma única vez, aqui.
            unsafe { CloseHandle(self.handle as _) };
        }
    }
}

/// Resultado de [`acquire`].
pub enum Acquire {
    /// Esta é a primeira instância.
    First(Singleton),
    /// Já havia outra instância (e a janela dela foi mostrada).
    AlreadyRunning,
}

/// `window_title` é o título exato da janela principal, usado para encontrar a primeira instância.
pub fn acquire(window_title: &str) -> Acquire {
    acquire_named(MUTEX_NAME, window_title)
}

/// Texto UTF-16 terminado em zero, como o Windows espera.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Versão de [`acquire`] com o nome do mutex parametrizado (para os testes).
fn acquire_named(mutex_name: &str, window_title: &str) -> Acquire {
    let name = wide(mutex_name);
    // SAFETY: `name` é uma string UTF-16 terminada em zero que vive durante a chamada;
    // atributos de segurança nulos são permitidos. `GetLastError` é lido logo em seguida,
    // na mesma thread, antes de qualquer outra chamada ao sistema.
    let (handle, already_exists) = unsafe {
        let handle = CreateMutexW(null(), 0, name.as_ptr());
        (handle as isize, GetLastError() == ERROR_ALREADY_EXISTS)
    };
    if already_exists {
        if handle != 0 {
            // SAFETY: handle válido devolvido por `CreateMutexW` acima, fechado uma vez.
            unsafe { CloseHandle(handle as _) };
        }
        show_window(window_title);
        Acquire::AlreadyRunning
    } else {
        Acquire::First(Singleton { handle })
    }
}

/// Procura a janela pelo título exato e a traz para a frente (se existir).
fn show_window(title: &str) {
    let title = wide(title);
    // SAFETY: `title` é UTF-16 terminado em zero e vive durante as chamadas; a classe nula
    // significa "qualquer classe". O `hwnd` é verificado antes de ser usado e as funções
    // de janela aceitam handles que deixaram de existir (apenas falham).
    unsafe {
        let hwnd = FindWindowW(null(), title.as_ptr());
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_SHOW);
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_instance_at_a_time() {
        let name = format!("Local\\AssistenteEmails-teste-{}", std::process::id());
        let title = "Janela inexistente do teste";
        let first = acquire_named(&name, title);
        assert!(matches!(first, Acquire::First(_)));
        assert!(matches!(
            acquire_named(&name, title),
            Acquire::AlreadyRunning
        ));
        drop(first);
        assert!(matches!(acquire_named(&name, title), Acquire::First(_)));
    }

    #[test]
    fn wide_is_nul_terminated() {
        assert_eq!(wide("aé"), vec![97, 233, 0]);
    }
}
