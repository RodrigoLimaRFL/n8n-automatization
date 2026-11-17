//! Estado do shell do Windows (barra de tarefas), usado para não criar o ícone da bandeja cedo demais.

use std::ptr::null;
use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, FindWindowW, GetForegroundWindow, GetWindowThreadProcessId, IsIconic,
    SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
};

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

/// Mostra a janela `hwnd` (valor do HWND; 0 = nenhuma), restaurando-a se estiver minimizada, e a coloca
/// em primeiro plano com foco.
///
/// O Windows costuma recusar `SetForegroundWindow` a um processo que está em segundo plano (o
/// que acontece quando o clique vem da bandeja e outra janela está na frente). Contorno clássico:
/// ligar a fila de entrada da nossa thread à da thread da janela em primeiro plano durante a
/// chamada; assim o sistema trata o pedido como vindo do processo ativo. É a opção mínima e
/// confiável (não mexe na ordem Z com TOPMOST, que piscaria e não dá foco).
pub fn bring_to_front(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    let hwnd = hwnd as *mut std::ffi::c_void;
    // SAFETY: `hwnd` não é nulo; as funções de janela só falham (sem UB) se o handle deixou de
    // existir. `fg_thread` vem de `GetWindowThreadProcessId` da janela ativa e só é ligado
    // quando difere da nossa thread; a ligação é sempre desfeita antes de retornar.
    unsafe {
        let cmd = if IsIconic(hwnd) != 0 {
            SW_RESTORE
        } else {
            SW_SHOW
        };
        ShowWindow(hwnd, cmd);
        let this_thread = GetCurrentThreadId();
        let fg = GetForegroundWindow();
        let fg_thread = if fg.is_null() {
            0
        } else {
            GetWindowThreadProcessId(fg, std::ptr::null_mut())
        };
        let attached = fg_thread != 0
            && fg_thread != this_thread
            && AttachThreadInput(this_thread, fg_thread, 1) != 0;
        BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd);
        if attached {
            AttachThreadInput(this_thread, fg_thread, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_is_nul_terminated() {
        assert_eq!(wide("ab"), vec![97, 98, 0]);
    }

    #[test]
    fn bring_to_front_ignores_null_window() {
        bring_to_front(0);
    }

    #[test]
    fn taskbar_query_does_not_panic() {
        // O resultado depende da sessão (em CI sem desktop pode ser falso); só garante que roda.
        let _ = taskbar_ready();
    }
}
