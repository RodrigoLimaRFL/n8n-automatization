//! Relógio local do Windows (a ponte também manda horários locais, sem depender do fuso do app).

use windows_sys::Win32::System::SystemInformation::GetLocalTime;

/// Horário local no formato `AAAA-MM-DDTHH:MM:SS` (sem fuso), como os da ponte.
pub fn local_timestamp() -> String {
    // SAFETY: `GetLocalTime` só escreve na struct SYSTEMTIME (apenas inteiros) que passamos.
    let t = unsafe {
        let mut t = std::mem::zeroed();
        GetLocalTime(&mut t);
        t
    };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_timestamp_has_iso_shape() {
        let ts = local_timestamp();
        assert_eq!(ts.len(), 19);
        assert_eq!(ts.as_bytes()[10], b'T');
        assert!(ts.starts_with("20"));
    }
}
