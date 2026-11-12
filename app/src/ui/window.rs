//! Janela principal: criação dos controles (posições fixas numa grade de 8 px) e
//! atualização a partir de um `ViewModel`.

use super::view_model::ViewModel;
use super::WINDOW_TITLE;
use crate::format;
use crate::model::LogLine;
use native_windows_gui as nwg;
use std::cell::RefCell;
use std::collections::VecDeque;

/// Área cliente da janela.
const CLIENT_W: i32 = 520;
const CLIENT_H: i32 = 400;
/// Máximo de linhas mantidas na caixa de log.
pub const MAX_LOG_LINES: usize = 300;
/// Limite de caracteres da caixa de log (o padrão do Windows é só 32 KB).
const LOG_BOX_CHAR_LIMIT: usize = 200_000;
pub const MIN_DAYS: i64 = 1;
pub const MAX_DAYS: i64 = 365;
pub const DEFAULT_DAYS: i64 = 3;

#[derive(Default)]
pub struct MainWindow {
    pub window: nwg::Window,
    pub icon: nwg::Icon,

    last_title: nwg::Label,
    last_frame: nwg::Frame,
    last_lines: [nwg::Label; 3],

    run_title: nwg::Label,
    run_frame: nwg::Frame,
    days_label: nwg::Label,
    pub days: nwg::NumberSelect,
    days_unit: nwg::Label,
    pub all_emails: nwg::CheckBox,
    pub run_button: nwg::Button,

    progress: nwg::ProgressBar,
    progress_label: nwg::Label,

    pub tasks_button: nwg::Button,
    pub calendar_button: nwg::Button,
    pub autostart: nwg::CheckBox,

    log_title: nwg::Label,
    log_box: nwg::TextBox,

    /// Cópia das linhas da caixa de log (para aparar as mais antigas).
    log_lines: RefCell<VecDeque<String>>,
    /// Último modelo aplicado: só os controles que mudaram são tocados (evita piscar).
    shown: RefCell<Option<ViewModel>>,
}

fn label(
    parent: &nwg::ControlHandle,
    text: &str,
    pos: (i32, i32),
    size: (i32, i32),
    out: &mut nwg::Label,
) -> Result<(), nwg::NwgError> {
    nwg::Label::builder()
        .text(text)
        .position(pos)
        .size(size)
        .parent(parent)
        .build(out)
}

fn button(
    parent: &nwg::Window,
    text: &str,
    pos: (i32, i32),
    size: (i32, i32),
    out: &mut nwg::Button,
) -> Result<(), nwg::NwgError> {
    nwg::Button::builder()
        .text(text)
        .position(pos)
        .size(size)
        .parent(parent)
        .build(out)
}

/// Tamanho externo da janela para a área cliente desejada (borda e título dependem do Windows).
fn outer_size() -> (i32, i32) {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AdjustWindowRect, WS_CAPTION, WS_MINIMIZEBOX, WS_SYSMENU,
    };
    let mut r = RECT {
        left: 0,
        top: 0,
        right: CLIENT_W,
        bottom: CLIENT_H,
    };
    // SAFETY: `r` é um RECT válido e local.
    let ok = unsafe { AdjustWindowRect(&mut r, WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX, 0) };
    if ok == 0 {
        return (CLIENT_W + 16, CLIENT_H + 39);
    }
    (r.right - r.left, r.bottom - r.top)
}

/// Liga/desliga os controles internos de `parent`. O `NumberSelect` do nwg só desabilita a janela
/// externa, então o campo de texto e os botões continuariam com aparência de habilitados.
fn set_children_enabled(parent: &nwg::ControlHandle, enabled: bool) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindow, GW_CHILD, GW_HWNDNEXT};
    let Some(parent) = parent.hwnd() else { return };
    // SAFETY: `parent` é a janela viva de um controle nosso; `GetWindow` devolve nulo ao fim da
    // lista de filhos e `EnableWindow` só muda o estado dessas janelas.
    unsafe {
        let mut child: HWND = GetWindow(parent as HWND, GW_CHILD);
        while !child.is_null() {
            EnableWindow(child, i32::from(enabled));
            child = GetWindow(child, GW_HWNDNEXT);
        }
    }
}

impl MainWindow {
    /// Cria a janela e todos os controles. `visible`: `false` para iniciar escondida na bandeja.
    pub fn build(visible: bool) -> Result<MainWindow, nwg::NwgError> {
        let mut w = MainWindow::default();

        // Tahoma 8 pt: a fonte clássica dos formulários do VB6.
        let mut font = nwg::Font::default();
        nwg::Font::builder()
            .family("Tahoma")
            .size(15)
            .build(&mut font)?;
        nwg::Font::set_global_default(Some(font));

        nwg::Icon::builder()
            .source_bin(Some(super::ICON_BYTES))
            .strict(true)
            .build(&mut w.icon)?;

        let mut flags = nwg::WindowFlags::WINDOW | nwg::WindowFlags::MINIMIZE_BOX;
        if visible {
            flags |= nwg::WindowFlags::VISIBLE;
        }
        nwg::Window::builder()
            .title(WINDOW_TITLE)
            .flags(flags)
            .size(outer_size())
            .center(true)
            .icon(Some(&w.icon))
            .build(&mut w.window)?;
        let win = &w.window;
        let wh = &win.handle;

        // 1. Última execução (moldura com título sobre a borda, como no VB6).
        nwg::Frame::builder()
            .flags(nwg::FrameFlags::VISIBLE | nwg::FrameFlags::BORDER)
            .position((8, 16))
            .size((504, 80))
            .parent(win)
            .build(&mut w.last_frame)?;
        for (i, l) in w.last_lines.iter_mut().enumerate() {
            label(
                &w.last_frame.handle,
                "",
                (8, 12 + 20 * i as i32),
                (488, 16),
                l,
            )?;
        }
        label(wh, "Última execução", (16, 8), (92, 16), &mut w.last_title)?;

        // 2. Processar agora.
        nwg::Frame::builder()
            .flags(nwg::FrameFlags::VISIBLE | nwg::FrameFlags::BORDER)
            .position((8, 112))
            .size((504, 80))
            .parent(win)
            .build(&mut w.run_frame)?;
        let rf = &w.run_frame.handle;
        label(
            rf,
            "Analisar os últimos",
            (8, 12),
            (112, 16),
            &mut w.days_label,
        )?;
        nwg::NumberSelect::builder()
            .value_int(DEFAULT_DAYS)
            .min_int(MIN_DAYS)
            .max_int(MAX_DAYS)
            .step_int(1)
            .position((124, 8))
            .size((64, 24))
            .parent(&w.run_frame)
            .build(&mut w.days)?;
        label(rf, "dias", (196, 12), (32, 16), &mut w.days_unit)?;
        nwg::CheckBox::builder()
            .text("Todos os emails")
            .position((248, 10))
            .size((136, 20))
            .parent(&w.run_frame)
            .build(&mut w.all_emails)?;
        nwg::Button::builder()
            .text("Processar agora")
            .position((8, 44))
            .size((128, 24))
            .parent(&w.run_frame)
            .build(&mut w.run_button)?;
        label(wh, "Processar agora", (16, 104), (92, 16), &mut w.run_title)?;

        // 3. Progresso.
        nwg::ProgressBar::builder()
            .range(0..100)
            .position((8, 204))
            .size((504, 20))
            .parent(win)
            .build(&mut w.progress)?;
        nwg::Label::builder()
            .text("")
            .h_align(nwg::HTextAlign::Center)
            .position((8, 228))
            .size((504, 16))
            .parent(win)
            .build(&mut w.progress_label)?;

        // 4 e 5. Navegador e inicialização.
        button(
            win,
            "Abrir Tarefas",
            (8, 256),
            (120, 24),
            &mut w.tasks_button,
        )?;
        button(
            win,
            "Abrir Calendário",
            (136, 256),
            (120, 24),
            &mut w.calendar_button,
        )?;
        nwg::CheckBox::builder()
            .text("Iniciar com o Windows")
            .position((280, 258))
            .size((176, 20))
            .parent(win)
            .build(&mut w.autostart)?;

        // 6. Registro.
        label(wh, "Registro", (8, 288), (120, 16), &mut w.log_title)?;
        nwg::TextBox::builder()
            .flags(
                nwg::TextBoxFlags::VISIBLE
                    | nwg::TextBoxFlags::VSCROLL
                    | nwg::TextBoxFlags::AUTOVSCROLL,
            )
            .readonly(true)
            .limit(LOG_BOX_CHAR_LIMIT)
            .position((8, 304))
            .size((304, 88))
            .parent(win)
            .build(&mut w.log_box)?;

        Ok(w)
    }

    /// Aplica o modelo. Mexe só no que mudou desde a última chamada.
    pub fn apply(&self, vm: &ViewModel) {
        let mut shown = self.shown.borrow_mut();
        let prev = shown.as_ref();
        for (i, l) in self.last_lines.iter().enumerate() {
            if prev.is_none_or(|p| p.last_run_lines[i] != vm.last_run_lines[i]) {
                l.set_text(&vm.last_run_lines[i]);
            }
        }
        if prev.is_none_or(|p| p.progress_label != vm.progress_label) {
            self.progress_label.set_text(&vm.progress_label);
        }
        if prev.is_none_or(|p| p.progress_percent != vm.progress_percent) {
            self.progress.set_pos(vm.progress_percent);
        }
        if prev.is_none_or(|p| p.can_run != vm.can_run) {
            self.run_button.set_enabled(vm.can_run);
        }
        *shown = Some(vm.clone());
    }

    /// Dias escolhidos: 0 quando "Todos os emails" está marcado.
    pub fn selected_days(&self) -> u32 {
        if self.all_emails.check_state() == nwg::CheckBoxState::Checked {
            return 0;
        }
        match self.days.data() {
            nwg::NumberSelectData::Int { value, .. } => value.clamp(MIN_DAYS, MAX_DAYS) as u32,
            _ => DEFAULT_DAYS as u32,
        }
    }

    /// "Todos os emails" desliga a caixa numérica (inclusive o campo e os botões + e -).
    pub fn sync_days_enabled(&self) {
        let all = self.all_emails.check_state() == nwg::CheckBoxState::Checked;
        self.days.set_enabled(!all);
        set_children_enabled(&self.days.handle, !all);
    }

    pub fn is_run_enabled(&self) -> bool {
        self.shown.borrow().as_ref().is_some_and(|vm| vm.can_run)
    }

    /// Trava o botão logo após o clique (antes de o worker confirmar a execução).
    pub fn mark_run_requested(&self) {
        self.run_button.set_enabled(false);
        if let Some(vm) = self.shown.borrow_mut().as_mut() {
            vm.can_run = false;
        }
    }

    /// Esquece o último modelo aplicado: a próxima `apply` atualiza todos os controles.
    pub fn forget_shown(&self) {
        *self.shown.borrow_mut() = None;
    }

    pub fn show(&self) {
        self.window.set_visible(true);
        self.window.restore();
        self.window.set_focus();
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    /// Acrescenta linhas ao registro, mantendo só as últimas `MAX_LOG_LINES`, e rola até o fim.
    pub fn append_log(&self, lines: &[LogLine]) {
        if lines.is_empty() {
            return;
        }
        let mut buf = self.log_lines.borrow_mut();
        let mut added = String::new();
        for l in lines {
            let text = format::log_line(l);
            added.push_str(&text);
            added.push_str("\r\n");
            buf.push_back(text);
        }
        if buf.len() > MAX_LOG_LINES {
            while buf.len() > MAX_LOG_LINES {
                buf.pop_front();
            }
            let mut all = String::new();
            for l in buf.iter() {
                all.push_str(l);
                all.push_str("\r\n");
            }
            self.log_box.set_text(&all);
        } else {
            self.log_box.append(&added);
        }
        self.log_box.scroll_lastline();
    }
}
