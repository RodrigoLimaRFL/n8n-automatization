//! Estado do shell do Windows (barra de tarefas), usado para não criar o ícone da bandeja cedo demais.

use std::ptr::null;
use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;

/// Nome da classe da janela da barra de tarefas, onde fica a bandeja.
const TASKBAR_CLASS: &str = "Shell_TrayWnd";

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// `true` quando a barra de tarefas já existe (logo depois do logon ela pode ainda não existir,
/// e criar o ícone da bandeja nessa hora falha).
pub fn taskbar_ready() -> bool {
    let class = wide(TASKBAR_CLASS);
    // SAFETY: `class` é uma string UTF-16 terminada em zero que vive durante a chamada.
    let hwnd = unsafe { FindWindowW(class.as_ptr(), null()) };
    !hwnd.is_null()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_is_nul_terminated() {
        assert_eq!(wide("ab"), vec![97, 98, 0]);
    }

    #[test]
    fn taskbar_query_does_not_panic() {
        // O resultado depende da sessão (em CI sem desktop pode ser falso); só garante que roda.
        let _ = taskbar_ready();
    }
}
