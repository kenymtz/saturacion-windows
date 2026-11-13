//! Inicio con Windows mediante la clave `Run` del usuario actual.

use std::path::Path;
use std::ptr;

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
};

use super::wide;

/// Clave del registro que Windows lee al iniciar sesión.
pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// Nombre del valor (el instalador usa el mismo).
pub const VALUE_NAME: &str = "Ajustes de pantalla";
/// Argumento con el que Windows arranca la app minimizada en la bandeja.
pub const MINIMIZED_ARG: &str = "--minimizado";

/// Dato del registro para arrancar `exe` minimizado: `"<exe>" --minimizado`.
pub fn command_for(exe: &Path) -> String {
    format!("\"{}\" {MINIMIZED_ARG}", exe.display())
}

/// Si `command` arranca este ejecutable minimizado. Las rutas de Windows no distinguen
/// mayúsculas, así que la comparación tampoco.
pub fn points_to(command: &str, exe: &Path) -> bool {
    command.trim().to_lowercase() == command_for(exe).to_lowercase()
}

/// Dato actual del valor `Run`, si existe.
fn read() -> Option<String> {
    let key = wide(RUN_KEY);
    let name = wide(VALUE_NAME);
    let mut size: u32 = 0;
    // SAFETY: con búfer nulo, RegGetValueW solo escribe en `size` el tamaño en bytes.
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut size,
        )
    };
    if r != ERROR_SUCCESS || size == 0 {
        return None;
    }
    let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
    // SAFETY: `buffer` tiene al menos `size` bytes.
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if r != ERROR_SUCCESS {
        return None;
    }
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..len]))
}

/// Si el inicio con Windows está activo para este ejecutable.
pub fn is_enabled(exe: &Path) -> bool {
    read().is_some_and(|command| points_to(&command, exe))
}

/// Activa o desactiva el inicio con Windows para `exe`. El error es el código de Windows.
pub fn set_enabled(exe: &Path, enabled: bool) -> Result<(), u32> {
    let key = wide(RUN_KEY);
    let name = wide(VALUE_NAME);
    let r = if enabled {
        let data = wide(&command_for(exe));
        // SAFETY: `data` es UTF-16 terminado en cero; el tamaño va en bytes e incluye el cero.
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            )
        }
    } else {
        // SAFETY: punteros a textos terminados en cero, válidos durante la llamada.
        let r = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) };
        if r == ERROR_FILE_NOT_FOUND {
            ERROR_SUCCESS
        } else {
            r
        }
    };
    if r == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn exe() -> PathBuf {
        PathBuf::from(r"C:\Users\x\AppData\Local\Programs\Ajustes de pantalla\Ajustes de pantalla.exe")
    }

    #[test]
    fn command_quotes_the_path_and_adds_the_minimized_flag() {
        assert_eq!(
            command_for(&exe()),
            r#""C:\Users\x\AppData\Local\Programs\Ajustes de pantalla\Ajustes de pantalla.exe" --minimizado"#
        );
    }

    #[test]
    fn points_to_ignores_case_and_surrounding_spaces() {
        let command = format!("  {}  ", command_for(&exe()).to_uppercase());
        assert!(points_to(&command, &exe()));
    }

    #[test]
    fn points_to_rejects_another_executable_or_missing_flag() {
        assert!(!points_to(&command_for(&PathBuf::from(r"C:\otro.exe")), &exe()));
        assert!(!points_to(&format!("\"{}\"", exe().display()), &exe()));
    }
}
