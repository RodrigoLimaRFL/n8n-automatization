//! Ícone da bandeja e menu de contexto.

use native_windows_gui as nwg;

#[derive(Default)]
pub struct Tray {
    pub icon: nwg::TrayNotification,
    menu: nwg::Menu,
    pub open: nwg::MenuItem,
    pub run: nwg::MenuItem,
    separator: nwg::MenuSeparator,
    pub quit: nwg::MenuItem,
    /// Dica atualmente exibida (evita refazer o ícone sem necessidade).
    tip: std::cell::RefCell<String>,
}

impl Tray {
    pub fn build(parent: &nwg::Window, icon: &nwg::Icon, tip: &str) -> Result<Tray, nwg::NwgError> {
        let mut t = Tray::default();
        nwg::TrayNotification::builder()
            .parent(parent)
            .icon(Some(icon))
            .tip(Some(tip))
            .build(&mut t.icon)?;
        nwg::Menu::builder()
            .popup(true)
            .parent(parent)
            .build(&mut t.menu)?;
        nwg::MenuItem::builder()
            .text("Abrir")
            .parent(&t.menu)
            .build(&mut t.open)?;
        nwg::MenuItem::builder()
            .text("Processar agora")
            .parent(&t.menu)
            .build(&mut t.run)?;
        nwg::MenuSeparator::builder()
            .parent(&t.menu)
            .build(&mut t.separator)?;
        nwg::MenuItem::builder()
            .text("Sair")
            .parent(&t.menu)
            .build(&mut t.quit)?;
        *t.tip.borrow_mut() = tip.to_string();
        Ok(t)
    }

    /// Mostra o menu na posição do cursor.
    pub fn show_menu(&self) {
        let (x, y) = nwg::GlobalCursor::position();
        self.menu.popup(x, y);
    }

    pub fn set_tip(&self, tip: &str) {
        let mut cur = self.tip.borrow_mut();
        if *cur != tip {
            self.icon.set_tip(tip);
            *cur = tip.to_string();
        }
    }

    pub fn set_run_enabled(&self, enabled: bool) {
        self.run.set_enabled(enabled);
    }

    pub fn remove(&self) {
        self.icon.set_visibility(false);
    }
}
