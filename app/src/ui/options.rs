//! Opções da linha de comando.

use crate::autostart;

/// Argumento que liga "Iniciar com o Windows" uma vez e segue normalmente.
pub const ENABLE_AUTOSTART_ARG: &str = "--ativar-inicializacao";

/// Opções reconhecidas; argumentos desconhecidos são ignorados.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Options {
    /// Inicia escondido na bandeja.
    pub minimized: bool,
    /// Liga "Iniciar com o Windows" na partida.
    pub enable_autostart: bool,
}

/// Interpreta os argumentos (sem o nome do programa).
pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Options {
    let mut o = Options::default();
    for a in args {
        if a == autostart::MINIMIZED_ARG {
            o.minimized = true;
        } else if a == ENABLE_AUTOSTART_ARG {
            o.enable_autostart = true;
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_args() {
        assert_eq!(parse_args(args(&[])), Options::default());
    }

    #[test]
    fn minimized_and_autostart() {
        let o = parse_args(args(&["--minimizado", "--ativar-inicializacao", "--outro"]));
        assert!(o.minimized && o.enable_autostart);
    }

    #[test]
    fn only_minimized() {
        let o = parse_args(args(&["--minimizado"]));
        assert!(o.minimized && !o.enable_autostart);
    }
}
