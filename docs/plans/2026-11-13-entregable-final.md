# Entregable final — Plan de implementación

**Goal:** Que la app recuerde sus valores y arranque con Windows en la bandeja (historia 9), con instancia única, botón Restablecer, ejecutable identificable, documentación de seguridad para CS2/FACEIT y un instalador `Setup.exe` que deje el acceso directo en el escritorio.

**Architecture:** Un módulo puro `settings` lee y escribe `%APPDATA%\Ajustes de pantalla\ajustes.txt`. Un módulo `system` encapsula las llamadas a Windows (mutex y eventos con nombre para la instancia única; clave `Run` del registro para el arranque) con `windows-sys`. `Controller` suma `reset()` y `apply_saved()`. `main.rs` maneja `--minimizado`/`--salir`, carga y aplica al abrir, guarda al soltar sliders y conecta la casilla de arranque y Restablecer. `build.rs` embebe ícono y versión en el `.exe`. Inno Setup genera el instalador.

**Tech Stack:** Rust stable (MSVC), Slint 1.18.1, `windows-sys` 0.61, `winresource` 0.1 (build), Inno Setup 6, PowerShell 5.1.

**Spec:** `docs/specs/2026-11-13-entregable-final-design.md`

## Global Constraints

- Solo Windows x64, toolchain `stable-x86_64-pc-windows-msvc`.
- Archivo de ajustes: `%APPDATA%\Ajustes de pantalla\ajustes.txt`, líneas `saturacion=N`, `contraste=N`, `brillo=N`. Lectura tolerante (ignora vacías, claves desconocidas y no números; recorta a 0–400, 50–200, 50–200; lo que falta es 100).
- "Iniciar con Windows" no se guarda en el archivo: la fuente de verdad es `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, valor `Ajustes de pantalla`, dato `"<ruta del exe>" --minimizado`. La app y el instalador escriben exactamente ese formato.
- Se guarda al **soltar** un slider y al Restablecer. Al abrir, se aplica solo lo que difiere de 100.
- Instancia única con `Local\AjustesDePantalla.Instancia` (mutex), `Local\AjustesDePantalla.Mostrar` y `Local\AjustesDePantalla.Salir` (eventos). `--salir` espera hasta 5 s.
- Textos exactos: "No se pudieron guardar los ajustes.", "No se pudo cambiar el inicio con Windows.", "Iniciar con Windows", "Restablecer".
- Versión del paquete: `1.0.0`. Recurso de versión del `.exe`: ProductName "Ajustes de pantalla", FileDescription "Saturación, contraste y brillo de la pantalla". Sin datos personales.
- Instalador: por usuario, sin administrador, en `%LOCALAPPDATA%\Programs\Ajustes de pantalla`, exe `Ajustes de pantalla.exe`; acceso directo en escritorio y menú Inicio; tarea "Iniciar con Windows" marcada; corre `--salir` antes de instalar y desinstalar; al desinstalar borra la entrada `Run` y conserva los ajustes. Salida `dist/AjustesDePantalla-Setup-<versión>.exe`; `dist/` no se versiona.
- `unsafe` solo en `src/igcl/` y `src/system/`.
- Scripts `.ps1` con texto no ASCII se guardan en UTF-8 **con BOM** (PowerShell 5.1 lee mal los acentos sin BOM).
- Textos visibles al usuario en español. Identificadores y mensajes de commit en inglés, sin trailers `Co-Authored-By` ni menciones a herramientas de IA en archivos o commits.
- Fuera de alcance: firma de código, NVIDIA/AMD, restaurar al cerrar sesión.

## Estructura de archivos

| Archivo | Cambio |
|---|---|
| `src/settings.rs` (nuevo) | Ajustes guardados |
| `src/controller.rs` | `reset()`, `apply_saved()`, helpers de factores |
| `src/system/mod.rs`, `autostart.rs`, `single_instance.rs` (nuevos) | Integración con Windows |
| `src/lib.rs` | Declara `settings` y `system` |
| `Cargo.toml` | `windows-sys`, `winresource`, versión 1.0.0 |
| `ui/app.slint`, `ui/tray.slint` | Casilla, Restablecer, `released` |
| `src/main.rs` | Cableado completo |
| `ui/make-icon.ps1`, `ui/icon.ico` | Ícono `.ico` |
| `build.rs` | Recurso de Windows |
| `installer/setup.iss`, `scripts/build-installer.ps1` (nuevos), `scripts/install.ps1` (se borra), `.gitignore` | Instalador |
| `docs/SEGURIDAD.md`, `README.md` (nuevos) | Documentación |

---

### Task 1: Ajustes guardados

**Files:**
- Create: `src/settings.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `saturacion::controller::{MAX_PERCENT, MIN_CONTRAST_PERCENT, MAX_CONTRAST_PERCENT, MIN_BRIGHTNESS_PERCENT, MAX_BRIGHTNESS_PERCENT}` (existen).
- Produces (en `saturacion::settings`):
  - `pub struct Settings { pub saturation: u32, pub contrast: u32, pub brightness: u32 }` con `Debug, Clone, Copy, PartialEq, Eq` y `Default` = 100/100/100.
  - `pub fn parse(text: &str) -> Settings` (asociada: `Settings::parse`), `pub fn to_text(&self) -> String`, `pub fn load(path: &Path) -> Settings`, `pub fn save(&self, path: &Path) -> std::io::Result<()>`.
  - `pub fn default_path() -> Option<PathBuf>` (función del módulo).

- [ ] **Step 1: Escribir el módulo con los tests y sin implementación**

`src/settings.rs`:

```rust
//! Ajustes guardados entre sesiones: saturación, contraste y brillo en un archivo de texto.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::controller::{
    MAX_BRIGHTNESS_PERCENT, MAX_CONTRAST_PERCENT, MAX_PERCENT, MIN_BRIGHTNESS_PERCENT,
    MIN_CONTRAST_PERCENT,
};

/// Carpeta dentro de `%APPDATA%` y nombre del archivo de ajustes.
pub const APP_DIR: &str = "Ajustes de pantalla";
pub const FILE_NAME: &str = "ajustes.txt";

/// Valores de los tres sliders, en porcentaje (100 = sin cambios).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub saturation: u32,
    pub contrast: u32,
    pub brightness: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("ajustes-test-{}-{name}", std::process::id()))
            .join(FILE_NAME)
    }

    #[test]
    fn default_is_neutral() {
        assert_eq!(
            Settings::default(),
            Settings { saturation: 100, contrast: 100, brightness: 100 }
        );
    }

    #[test]
    fn parse_reads_all_keys() {
        let s = Settings::parse("saturacion=150\ncontraste=120\nbrillo=80\n");
        assert_eq!(s, Settings { saturation: 150, contrast: 120, brightness: 80 });
    }

    #[test]
    fn parse_ignores_blank_unknown_and_garbage_lines() {
        let text = "\n  \notra=5\nsaturacion=abc\nsin igual\nbrillo = 130 \ncontraste=-4\n";
        assert_eq!(
            Settings::parse(text),
            Settings { saturation: 100, contrast: 100, brightness: 130 }
        );
    }

    #[test]
    fn parse_clamps_values_to_their_ranges() {
        let s = Settings::parse("saturacion=900\ncontraste=10\nbrillo=999\n");
        assert_eq!(s, Settings { saturation: 400, contrast: 50, brightness: 200 });
    }

    #[test]
    fn missing_keys_stay_neutral() {
        assert_eq!(
            Settings::parse("contraste=150"),
            Settings { saturation: 100, contrast: 150, brightness: 100 }
        );
    }

    #[test]
    fn text_round_trip_keeps_values() {
        let s = Settings { saturation: 0, contrast: 200, brightness: 50 };
        assert_eq!(Settings::parse(&s.to_text()), s);
    }

    #[test]
    fn load_of_missing_file_is_neutral() {
        assert_eq!(Settings::load(&temp_path("missing")), Settings::default());
    }

    #[test]
    fn save_creates_folder_and_load_reads_it_back() {
        let path = temp_path("save");
        let s = Settings { saturation: 250, contrast: 110, brightness: 90 };
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn default_path_is_under_appdata() {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            assert_eq!(
                default_path(),
                Some(PathBuf::from(appdata).join(APP_DIR).join(FILE_NAME))
            );
        }
    }
}
```

En `src/lib.rs`, agregar al final:

```rust
pub mod settings;
```

- [ ] **Step 2: Ver que fallan**

Run: `cargo test --lib settings`
Expected: error de compilación (falta `Default`, `parse`, `to_text`, `load`, `save`, `default_path`).

- [ ] **Step 3: Implementar**

En `src/settings.rs`, entre el struct y `#[cfg(test)]`:

```rust
impl Default for Settings {
    fn default() -> Self {
        Self { saturation: 100, contrast: 100, brightness: 100 }
    }
}

impl Settings {
    /// Lee el texto de forma tolerante: ignora líneas vacías, claves desconocidas y valores
    /// que no son números, y recorta cada valor a su rango. Lo que falta queda en 100.
    pub fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let Ok(value) = value.trim().parse::<u32>() else { continue };
            match key.trim() {
                "saturacion" => settings.saturation = value.min(MAX_PERCENT),
                "contraste" => {
                    settings.contrast = value.clamp(MIN_CONTRAST_PERCENT, MAX_CONTRAST_PERCENT)
                }
                "brillo" => {
                    settings.brightness =
                        value.clamp(MIN_BRIGHTNESS_PERCENT, MAX_BRIGHTNESS_PERCENT)
                }
                _ => {}
            }
        }
        settings
    }

    /// Texto que se guarda en el archivo.
    pub fn to_text(&self) -> String {
        format!(
            "saturacion={}\ncontraste={}\nbrillo={}\n",
            self.saturation, self.contrast, self.brightness
        )
    }

    /// Lee el archivo; si no existe o no se puede leer, devuelve los valores por defecto.
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path).map(|text| Self::parse(&text)).unwrap_or_default()
    }

    /// Guarda en el archivo, creando la carpeta si hace falta.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, self.to_text())
    }
}

/// `%APPDATA%\Ajustes de pantalla\ajustes.txt`, o `None` si `APPDATA` no está definida.
pub fn default_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join(APP_DIR).join(FILE_NAME))
}
```

- [ ] **Step 4: Ver que pasan**

Run: `cargo test --lib settings`
Expected: `test result: ok. 9 passed`.

Run: `cargo test`
Expected: `test result: ok. 50 passed` (41 anteriores + 9), sin advertencias.

- [ ] **Step 5: Commit**

```bash
git add src/settings.rs src/lib.rs
git commit -m "feat: add saved settings file"
```

---

### Task 2: Restablecer y aplicar valores guardados en el controlador

**Files:**
- Modify: `src/controller.rs`

**Interfaces:**
- Consumes: nada nuevo.
- Produces (en `saturacion::controller::Controller`):
  - `pub fn reset(&mut self) -> Result<(), BackendError>` — factores a 1.0 y restaura si hubo escrituras (como `shutdown`).
  - `pub fn apply_saved(&mut self, saturation: u32, contrast: u32, brightness: u32) -> Result<(), BackendError>` — escribe solo lo que difiere de 100; una sola curva combinada; nada de curva si la placa no la admite.

- [ ] **Step 1: Tests que fallan**

Agregar al final de `mod tests` en `src/controller.rs` (antes de su `}`; `controller()`, `FakeBackend`, `Call`, `assert_last_curve` y `saturation_matrix` ya existen ahí):

```rust
    #[test]
    fn reset_restores_and_returns_factors_to_neutral() {
        let (mut c, calls) = controller(None);
        c.set_brightness(50).unwrap();
        c.reset().unwrap();
        assert_eq!(calls.borrow().last(), Some(&Call::Restore));
        c.set_contrast(200).unwrap();
        assert_last_curve(&calls, [0.0, 0.0, 0.5, 1.0, 1.0]);
    }

    #[test]
    fn reset_without_writes_does_not_touch_the_display() {
        let (mut c, calls) = controller(None);
        c.reset().unwrap();
        assert!(calls.borrow().is_empty());
    }

    #[test]
    fn apply_saved_at_neutral_values_writes_nothing() {
        let (mut c, calls) = controller(None);
        c.apply_saved(100, 100, 100).unwrap();
        assert!(calls.borrow().is_empty());
    }

    #[test]
    fn apply_saved_with_only_saturation_writes_only_the_matrix() {
        let (mut c, calls) = controller(None);
        c.apply_saved(150, 100, 100).unwrap();
        assert_eq!(*calls.borrow(), vec![Call::Apply(saturation_matrix(1.5))]);
    }

    #[test]
    fn apply_saved_writes_matrix_and_one_combined_curve() {
        let (mut c, calls) = controller(None);
        c.apply_saved(250, 200, 50).unwrap();
        assert_eq!(calls.borrow().len(), 2);
        assert_eq!(calls.borrow()[0], Call::Apply(saturation_matrix(2.5)));
        assert_last_curve(&calls, [0.0, 0.0, 0.25, 1.0, 1.0]);
    }

    #[test]
    fn apply_saved_with_only_brightness_writes_only_the_curve() {
        let (mut c, calls) = controller(None);
        c.apply_saved(100, 100, 50).unwrap();
        assert_eq!(calls.borrow().len(), 1);
        assert_last_curve(&calls, [0.0, 0.0625, 0.25, 0.5625, 1.0]);
    }

    #[test]
    fn apply_saved_skips_the_curve_when_unsupported() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let backend =
            FakeBackend { calls: Rc::clone(&calls), fail_with: None, curve_supported: false };
        let mut c = Controller::new(backend);
        c.apply_saved(150, 200, 100).unwrap();
        assert_eq!(*calls.borrow(), vec![Call::Apply(saturation_matrix(1.5))]);
    }
```

- [ ] **Step 2: Ver que fallan**

Run: `cargo test --lib controller`
Expected: error de compilación `no method named 'reset'` / `apply_saved`.

- [ ] **Step 3: Implementar**

En `src/controller.rs`, agregar antes de `pub struct Controller`:

```rust
/// Factor de contraste (1.0 = sin cambios) para un porcentaje, recortado a su rango.
fn contrast_factor(percent: u32) -> f64 {
    f64::from(percent.clamp(MIN_CONTRAST_PERCENT, MAX_CONTRAST_PERCENT)) / 100.0
}

/// Factor de brillo (1.0 = sin cambios) para un porcentaje, recortado a su rango.
fn brightness_factor(percent: u32) -> f64 {
    f64::from(percent.clamp(MIN_BRIGHTNESS_PERCENT, MAX_BRIGHTNESS_PERCENT)) / 100.0
}
```

Reemplazar los cuerpos de `set_contrast` y `set_brightness` para usar los helpers (mismos comentarios de doc):

```rust
    pub fn set_contrast(&mut self, percent: u32) -> Result<(), BackendError> {
        self.contrast = contrast_factor(percent);
        self.write_tone_curve()
    }
```

```rust
    pub fn set_brightness(&mut self, percent: u32) -> Result<(), BackendError> {
        self.brightness = brightness_factor(percent);
        self.write_tone_curve()
    }
```

Agregar dentro del `impl`, después de `supports_tone_curve`:

```rust
    /// Aplica los valores guardados al abrir la app. Escribe solo lo que difiere de 100:
    /// la matriz si cambia la saturación y una sola curva si cambian contraste o brillo
    /// (si la placa la admite). Intenta ambas escrituras y devuelve el primer error.
    pub fn apply_saved(
        &mut self,
        saturation: u32,
        contrast: u32,
        brightness: u32,
    ) -> Result<(), BackendError> {
        let mut first_error = None;
        if saturation != 100 {
            if let Err(e) = self.set_saturation(saturation) {
                first_error.get_or_insert(e);
            }
        }
        if (contrast != 100 || brightness != 100) && self.supports_tone_curve() {
            self.contrast = contrast_factor(contrast);
            self.brightness = brightness_factor(brightness);
            if let Err(e) = self.write_tone_curve() {
                first_error.get_or_insert(e);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Vuelve contraste y brillo a 1.0 y devuelve la imagen a la normal si la app la
    /// había modificado. El controlador sigue listo para nuevas escrituras.
    pub fn reset(&mut self) -> Result<(), BackendError> {
        self.contrast = 1.0;
        self.brightness = 1.0;
        self.shutdown()
    }
```

- [ ] **Step 4: Ver que pasan**

Run: `cargo test`
Expected: `test result: ok. 57 passed` (50 + 7), sin advertencias.

- [ ] **Step 5: Commit**

```bash
git add src/controller.rs
git commit -m "feat: add reset and saved-value application to the controller"
```

---

### Task 3: Integración con Windows (instancia única y arranque)

**Files:**
- Modify: `Cargo.toml`
- Create: `src/system/mod.rs`, `src/system/autostart.rs`, `src/system/single_instance.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Produces (en `saturacion::system::autostart`):
  - `pub const MINIMIZED_ARG: &str = "--minimizado";`
  - `pub fn command_for(exe: &Path) -> String` → `"\"<exe>\" --minimizado"`.
  - `pub fn points_to(command: &str, exe: &Path) -> bool` (sin distinguir mayúsculas, ignora espacios en los extremos).
  - `pub fn is_enabled(exe: &Path) -> bool`, `pub fn set_enabled(exe: &Path, enabled: bool) -> Result<(), u32>`.
- Produces (en `saturacion::system::single_instance`):
  - `pub enum Command { Show, Quit }`, `pub enum Startup { Primary(Instance), Secondary }`, `pub struct Instance`.
  - `pub fn acquire() -> Startup`, `impl Instance { pub fn listen(&self, on_command: impl Fn(Command) + Send + 'static) }`, `pub fn send(command: Command) -> bool`, `pub fn wait_until_gone(timeout: Duration) -> bool`.

- [ ] **Step 1: Dependencia**

En `Cargo.toml`, sección `[dependencies]`, agregar:

```toml
windows-sys = { version = "0.61", features = ["Win32_Foundation", "Win32_Security", "Win32_System_Registry", "Win32_System_Threading"] }
```

(`windows-sys` 0.61.2 ya está en `Cargo.lock` por Slint; no se descarga nada nuevo.)

- [ ] **Step 2: Tests de la parte pura que fallan**

`src/system/mod.rs`:

```rust
//! Integración con Windows: instancia única y arranque con el sistema.
//! Junto con `src/igcl/`, es el único lugar con código `unsafe`.

pub mod autostart;
pub mod single_instance;

/// Texto en UTF-16 terminado en cero, como esperan las funciones `W` de Windows.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
```

`src/system/autostart.rs` (solo constantes y tests por ahora):

```rust
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
```

`src/system/single_instance.rs` (por ahora solo el encabezado, para que compile el módulo):

```rust
//! Instancia única: un mutex con nombre detecta otra copia y dos eventos con nombre
//! le pasan órdenes (mostrar la ventana o salir).
```

En `src/lib.rs`, agregar:

```rust
pub mod system;
```

Run: `cargo test --lib system`
Expected: error de compilación `cannot find function 'command_for'` / `points_to`.

- [ ] **Step 3: Implementar `autostart`**

En `src/system/autostart.rs`, entre las constantes y `#[cfg(test)]`:

```rust
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
```

Run: `cargo test --lib system`
Expected: `test result: ok. 3 passed`.

- [ ] **Step 4: Implementar `single_instance`**

Reemplazar `src/system/single_instance.rs` completo:

```rust
//! Instancia única: un mutex con nombre detecta otra copia y dos eventos con nombre
//! le pasan órdenes (mostrar la ventana o salir).

use std::ptr;
use std::thread;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, WAIT_OBJECT_0,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateMutexW, OpenEventW, OpenMutexW, SetEvent, WaitForMultipleObjects,
    EVENT_MODIFY_STATE, INFINITE, SYNCHRONIZATION_SYNCHRONIZE,
};

use super::wide;

const MUTEX_NAME: &str = r"Local\AjustesDePantalla.Instancia";
const SHOW_EVENT: &str = r"Local\AjustesDePantalla.Mostrar";
const QUIT_EVENT: &str = r"Local\AjustesDePantalla.Salir";

/// Orden que una copia nueva le manda a la que corre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Show,
    Quit,
}

/// Resultado de intentar ser la copia principal.
pub enum Startup {
    Primary(Instance),
    Secondary,
}

/// Mutex de la copia principal: mientras vive, las demás copias la detectan.
pub struct Instance {
    mutex: HANDLE,
}

/// Intenta ser la copia principal. Si el mutex no se puede crear, sigue como si fuera
/// la única copia.
pub fn acquire() -> Startup {
    let name = wide(MUTEX_NAME);
    // SAFETY: atributos nulos y nombre terminado en cero.
    let mutex = unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) };
    // SAFETY: se lee inmediatamente después de CreateMutexW.
    let already_running = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    if !mutex.is_null() && already_running {
        // SAFETY: `mutex` es un handle válido que no se usa después.
        unsafe { CloseHandle(mutex) };
        return Startup::Secondary;
    }
    Startup::Primary(Instance { mutex })
}

impl Instance {
    /// Atiende en un hilo propio las órdenes de otras copias. `on_command` corre en ese
    /// hilo: para tocar la interfaz debe reenviar la orden al hilo de Slint.
    pub fn listen(&self, on_command: impl Fn(Command) + Send + 'static) {
        let show = create_event(SHOW_EVENT);
        let quit = create_event(QUIT_EVENT);
        if show.is_null() || quit.is_null() {
            return;
        }
        // Los handles son punteros crudos (no `Send`); se pasan al hilo como números.
        let raw = [show as usize, quit as usize];
        thread::spawn(move || {
            let handles: [HANDLE; 2] = [raw[0] as HANDLE, raw[1] as HANDLE];
            loop {
                // SAFETY: dos eventos válidos que este hilo nunca cierra.
                let r = unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, INFINITE) };
                if r == WAIT_OBJECT_0 {
                    on_command(Command::Show);
                } else if r == WAIT_OBJECT_0 + 1 {
                    on_command(Command::Quit);
                } else {
                    return;
                }
            }
        });
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        if !self.mutex.is_null() {
            // SAFETY: handle válido de CreateMutexW que no se usa después.
            unsafe { CloseHandle(self.mutex) };
        }
    }
}

/// Evento de reinicio automático con nombre; nulo si falla.
fn create_event(name: &str) -> HANDLE {
    let name = wide(name);
    // SAFETY: atributos nulos y nombre terminado en cero.
    unsafe { CreateEventW(ptr::null(), 0, 0, name.as_ptr()) }
}

/// Le manda `command` a la copia que corre. `false` si no hay ninguna escuchando.
pub fn send(command: Command) -> bool {
    let name = wide(match command {
        Command::Show => SHOW_EVENT,
        Command::Quit => QUIT_EVENT,
    });
    // SAFETY: nombre terminado en cero.
    let event = unsafe { OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr()) };
    if event.is_null() {
        return false;
    }
    // SAFETY: `event` es válido hasta CloseHandle.
    let signaled = unsafe { SetEvent(event) } != 0;
    // SAFETY: handle válido que no se usa después.
    unsafe { CloseHandle(event) };
    signaled
}

/// Si hay una copia principal corriendo.
fn is_running() -> bool {
    let name = wide(MUTEX_NAME);
    // SAFETY: nombre terminado en cero.
    let mutex = unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr()) };
    if mutex.is_null() {
        return false;
    }
    // SAFETY: handle válido que no se usa después.
    unsafe { CloseHandle(mutex) };
    true
}

/// Espera hasta `timeout` a que no quede ninguna copia corriendo. `true` si terminó.
pub fn wait_until_gone(timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if !is_running() {
            return true;
        }
        thread::sleep(Duration::from_millis(100));
    }
    !is_running()
}
```

Si el compilador ubica alguna constante en otro módulo de `windows-sys` (por ejemplo `INFINITE` o `WAIT_OBJECT_0`), corregir solo el `use`; no cambiar comportamiento.

- [ ] **Step 5: Compilar y correr los tests**

Run: `cargo build`
Expected: `Finished` sin advertencias (los ítems nuevos son `pub` en la librería, así que no generan avisos de código sin usar).

Run: `cargo test`
Expected: `test result: ok. 60 passed` (57 + 3).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/lib.rs src/system
git commit -m "feat: add single-instance and autostart integration with Windows"
```

---

### Task 4: Cableado en la ventana, la bandeja y `main`

Sin tests unitarios nuevos (integración con la interfaz); se verifica compilando y en la Task 8.

**Files:**
- Modify: `ui/app.slint`
- Modify: `ui/tray.slint`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `Settings` y `settings::default_path` (Task 1); `Controller::{apply_saved, reset}` (Task 2); `system::autostart::{MINIMIZED_ARG, is_enabled, set_enabled}` y `system::single_instance::{acquire, send, wait_until_gone, Command, Startup}` (Task 3).
- Produces: `AppWindow` con `in-out property <bool> autostart`, `in property <bool> autostart-available`, `callback autostart-toggled(bool)`, `callback reset-requested()`, `callback settings-committed()`; `AppTray` con `callback reset()`.

- [ ] **Step 1: Ventana**

En `ui/app.slint`:

Cambiar la línea del import a:

```slint
import { Button, CheckBox, Slider } from "std-widgets.slint";
```

Después de `callback brightness-changed(float);` agregar:

```slint
    in-out property <bool> autostart: false;
    in property <bool> autostart-available: true;
    callback autostart-toggled(bool);
    callback reset-requested();
    callback settings-committed();
```

En cada uno de los tres `Slider`, después del bloque `changed(value) => { ... }`, agregar:

```slint
            released(value) => {
                root.settings-committed();
            }
```

Después del tercer `Slider` (el de brillo) y antes de `if root.tone-note != ""`, agregar:

```slint
        CheckBox {
            text: "Iniciar con Windows";
            enabled: root.autostart-available;
            checked <=> root.autostart;
            toggled => {
                root.autostart-toggled(self.checked);
            }
        }

        Button {
            text: "Restablecer";
            enabled: root.controls-enabled;
            clicked => {
                root.reset-requested();
            }
        }
```

- [ ] **Step 2: Bandeja**

En `ui/tray.slint`, después de `callback quit();` agregar `callback reset();`, y en el `Menu`, entre el `MenuItem` "Abrir" y el `MenuSeparator`, agregar:

```slint
        MenuItem {
            title: "Restablecer";
            activated => {
                root.reset();
            }
        }
```

- [ ] **Step 3: Reemplazar `src/main.rs` completo**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use saturacion::backend::BackendError;
use saturacion::controller::Controller;
use saturacion::igcl::IgclBackend;
use saturacion::settings::{self, Settings};
use saturacion::system::autostart;
use saturacion::system::single_instance::{self, Command, Startup};
use slint::CloseRequestResponse;

slint::include_modules!();

type SharedController = Rc<RefCell<Controller<IgclBackend>>>;

/// Argumento con el que el instalador pide cerrar la copia que corre.
const QUIT_ARG: &str = "--salir";
/// Cuánto espera `--salir` a que la otra copia termine.
const QUIT_TIMEOUT: Duration = Duration::from_secs(5);

const TRAY_UNAVAILABLE: &str =
    "No se pudo crear el ícono de la bandeja; cerrar la ventana termina la app.";
const SAVE_FAILED: &str = "No se pudieron guardar los ajustes.";
const AUTOSTART_FAILED: &str = "No se pudo cambiar el inicio con Windows.";

/// Muestra el resultado de una escritura: borra el error si salió bien.
fn show_result(ui: &AppWindow, result: Result<(), BackendError>) {
    let message = match result {
        Ok(()) => String::new(),
        Err(e) => e.to_string(),
    };
    ui.set_error_text(message.into());
}

/// Arma el manejador de un slider: pasa el valor redondeado a `apply` y muestra el resultado.
fn slider_handler(
    ui: &AppWindow,
    controller: &SharedController,
    apply: fn(&mut Controller<IgclBackend>, u32) -> Result<(), BackendError>,
) -> impl FnMut(f32) + 'static {
    let ui_weak = ui.as_weak();
    let controller = Rc::clone(controller);
    move |value| {
        let Some(ui) = ui_weak.upgrade() else { return };
        let percent = value.round().max(0.0) as u32;
        let result = apply(&mut *controller.borrow_mut(), percent);
        show_result(&ui, result);
    }
}

/// Valores que muestran los sliders, tal como se guardan.
fn current_settings(ui: &AppWindow) -> Settings {
    Settings {
        saturation: ui.get_saturation().round().max(0.0) as u32,
        contrast: ui.get_contrast().round().max(0.0) as u32,
        brightness: ui.get_brightness().round().max(0.0) as u32,
    }
}

/// Pone los sliders en `values` (no dispara `changed`: solo lo hace el usuario).
fn show_settings(ui: &AppWindow, values: Settings) {
    ui.set_saturation(values.saturation as f32);
    ui.set_contrast(values.contrast as f32);
    ui.set_brightness(values.brightness as f32);
}

/// Guarda los ajustes; si no se puede, lo muestra en la ventana.
fn save_settings(ui: &AppWindow, path: Option<&Path>, values: Settings) {
    let saved = path.is_some_and(|path| values.save(path).is_ok());
    if !saved {
        ui.set_error_text(SAVE_FAILED.into());
    }
}

/// Vuelve todo a 100: restaura la placa, mueve los sliders y guarda.
fn reset_all(ui: &AppWindow, controller: &SharedController, path: Option<&Path>) {
    let result = controller.borrow_mut().reset();
    let neutral = Settings::default();
    show_settings(ui, neutral);
    show_result(ui, result);
    save_settings(ui, path, neutral);
}

/// Crea y muestra el ícono de la bandeja. `None` si la plataforma no lo permite.
fn create_tray(ui: &AppWindow, on_reset: impl Fn() + 'static) -> Option<AppTray> {
    let tray = AppTray::new().ok()?;
    tray.show().ok()?;

    let ui_weak = ui.as_weak();
    tray.on_open(move || {
        if let Some(ui) = ui_weak.upgrade() {
            let _ = ui.show();
            ui.window().set_minimized(false);
        }
    }); // el clic izquierdo también llama a `open` (ver tray.slint)
    tray.on_reset(on_reset);
    tray.on_quit(|| {
        let _ = slint::quit_event_loop();
    });
    Some(tray)
}

fn main() -> Result<(), slint::PlatformError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let quit_requested = args.iter().any(|arg| arg == QUIT_ARG);
    let start_minimized = args.iter().any(|arg| arg == autostart::MINIMIZED_ARG);

    // Instancia única: si ya hay una copia, se le pasa la orden y esta termina.
    let instance = match single_instance::acquire() {
        Startup::Secondary => {
            if quit_requested {
                single_instance::send(Command::Quit);
                single_instance::wait_until_gone(QUIT_TIMEOUT);
            } else {
                single_instance::send(Command::Show);
            }
            return Ok(());
        }
        Startup::Primary(instance) => instance,
    };
    if quit_requested {
        return Ok(()); // no había otra copia corriendo
    }

    let ui = AppWindow::new()?;

    let settings_path = settings::default_path();
    let saved = settings_path.as_deref().map(Settings::load).unwrap_or_default();
    show_settings(&ui, saved);

    let controller = match IgclBackend::new() {
        Ok(backend) => {
            let mut controller = Controller::new(backend);
            if !controller.supports_tone_curve() {
                ui.set_tone_enabled(false);
                ui.set_tone_note(BackendError::ToneCurveUnsupported.to_string().into());
            }
            let applied =
                controller.apply_saved(saved.saturation, saved.contrast, saved.brightness);
            show_result(&ui, applied);
            Some(Rc::new(RefCell::new(controller)))
        }
        Err(e) => {
            ui.set_error_text(e.to_string().into());
            ui.set_controls_enabled(false);
            None
        }
    };

    if let Some(controller) = &controller {
        ui.on_saturation_changed(slider_handler(&ui, controller, Controller::set_saturation));
        ui.on_contrast_changed(slider_handler(&ui, controller, Controller::set_contrast));
        ui.on_brightness_changed(slider_handler(&ui, controller, Controller::set_brightness));

        let ui_weak = ui.as_weak();
        let path = settings_path.clone();
        ui.on_settings_committed(move || {
            if let Some(ui) = ui_weak.upgrade() {
                save_settings(&ui, path.as_deref(), current_settings(&ui));
            }
        });

        let ui_weak = ui.as_weak();
        let shared = Rc::clone(controller);
        let path = settings_path.clone();
        ui.on_reset_requested(move || {
            if let Some(ui) = ui_weak.upgrade() {
                reset_all(&ui, &shared, path.as_deref());
            }
        });
    }

    // Casilla "Iniciar con Windows": su estado sale del registro, no del archivo.
    match std::env::current_exe() {
        Ok(exe) => {
            ui.set_autostart(autostart::is_enabled(&exe));
            let ui_weak = ui.as_weak();
            ui.on_autostart_toggled(move |enabled| {
                let Some(ui) = ui_weak.upgrade() else { return };
                if autostart::set_enabled(&exe, enabled).is_err() {
                    ui.set_autostart(!enabled);
                    ui.set_error_text(AUTOSTART_FAILED.into());
                }
            });
        }
        Err(_) => ui.set_autostart_available(false),
    }

    // El ícono debe vivir hasta el final de `main`: al soltarlo desaparece de la bandeja.
    let tray_reset = {
        let ui_weak = ui.as_weak();
        let controller = controller.clone();
        let path = settings_path.clone();
        move || {
            if let (Some(ui), Some(controller)) = (ui_weak.upgrade(), &controller) {
                reset_all(&ui, controller, path.as_deref());
            }
        }
    };
    let tray = create_tray(&ui, tray_reset);
    if tray.is_none() && controller.is_some() {
        ui.set_error_text(TRAY_UNAVAILABLE.into());
    }

    // Órdenes de otras copias: llegan en otro hilo y se reenvían al de la interfaz.
    let ui_weak = ui.as_weak();
    instance.listen(move |command| match command {
        Command::Show => {
            let _ = ui_weak.upgrade_in_event_loop(|ui| {
                let _ = ui.show();
                ui.window().set_minimized(false);
            });
        }
        Command::Quit => {
            let _ = slint::quit_event_loop();
        }
    });

    // Con controlador e ícono, la X solo oculta la ventana (el ícono mantiene vivo el
    // bucle). Sin alguno de los dos, la X termina la app.
    let keep_running_in_tray = controller.is_some() && tray.is_some();
    ui.window().on_close_requested(move || {
        if !keep_running_in_tray {
            let _ = slint::quit_event_loop();
        }
        CloseRequestResponse::HideWindow
    });

    // Con `--minimizado` y bandeja disponible, arranca solo con el ícono.
    if !(start_minimized && tray.is_some()) {
        ui.show()?;
    }
    let run_result = slint::run_event_loop();

    // Restaurar aunque el bucle de eventos haya terminado con error.
    if let Some(controller) = controller {
        let _ = controller.borrow_mut().shutdown();
    }
    drop(tray);
    drop(instance);
    run_result
}
```

Notas para adaptar solo si el compilador lo exige, sin cambiar comportamiento:
- Si `ui_weak` (un `slint::Weak<AppWindow>`) no es `Send` para `instance.listen`, reenviar con `slint::invoke_from_event_loop` capturando un `slint::Weak` creado con `ui.as_weak()`; Slint documenta `upgrade_in_event_loop` justamente para este uso.
- Si `Option<&Path>::is_some_and` no existe en la versión de Rust, usar `match`.

- [ ] **Step 4: Compilar y correr los tests**

Run: `cargo build`
Expected: `Finished` sin advertencias en `src/` ni `ui/`.

Run: `cargo test`
Expected: `test result: ok. 60 passed`.

No correr la app: cambia los colores reales y escribe en el registro.

- [ ] **Step 5: Commit**

```bash
git add ui/app.slint ui/tray.slint src/main.rs
git commit -m "feat: remember values, start with Windows, single instance and reset"
```

---

### Task 5: Ejecutable identificable (ícono y versión)

**Files:**
- Modify: `ui/make-icon.ps1`
- Create: `ui/icon.ico` (generado)
- Modify: `build.rs`
- Modify: `Cargo.toml`

**Interfaces:**
- Produces: `ui/icon.ico` (usado por `build.rs` y por el instalador); `.exe` con ícono y recurso de versión; paquete en versión `1.0.0`.

- [ ] **Step 1: Generar también el `.ico`**

En `ui/make-icon.ps1`, al final (después de `Write-Output "Escrito $out"`), agregar:

```powershell

# Ícono .ico con el PNG embebido (formato admitido desde Windows Vista).
$pngBytes = [System.IO.File]::ReadAllBytes($out)
$ico = Join-Path $PSScriptRoot 'icon.ico'
$stream = New-Object System.IO.MemoryStream
$writer = New-Object System.IO.BinaryWriter $stream
$writer.Write([uint16]0)                # reservado
$writer.Write([uint16]1)                # tipo: ícono
$writer.Write([uint16]1)                # cantidad de imágenes
$writer.Write([byte]$size)              # ancho
$writer.Write([byte]$size)              # alto
$writer.Write([byte]0)                  # paleta
$writer.Write([byte]0)                  # reservado
$writer.Write([uint16]1)                # planos
$writer.Write([uint16]32)               # bits por pixel
$writer.Write([uint32]$pngBytes.Length) # tamaño de la imagen
$writer.Write([uint32]22)               # posición de la imagen
$writer.Write($pngBytes)
$writer.Flush()
[System.IO.File]::WriteAllBytes($ico, $stream.ToArray())
$writer.Dispose()
Write-Output "Escrito $ico"
```

Guardar el archivo en UTF-8 con BOM (Global Constraints). Desde PowerShell:

```powershell
$p = "ui/make-icon.ps1"; $t = [IO.File]::ReadAllText((Resolve-Path $p), [Text.Encoding]::UTF8); [IO.File]::WriteAllText((Resolve-Path $p), $t, (New-Object Text.UTF8Encoding $true))
```

Run: `powershell -ExecutionPolicy Bypass -File ui/make-icon.ps1`
Expected: dos líneas `Escrito ...icon.png` y `Escrito ...icon.ico`. `git diff --stat ui/icon.png` no muestra cambios (el PNG se regenera idéntico; si cambiara, revertirlo con `git checkout -- ui/icon.png` y reportarlo).

- [ ] **Step 2: Recurso de Windows en el build**

En `Cargo.toml`: cambiar `version = "0.1.0"` por `version = "1.0.0"`, y en `[build-dependencies]` agregar:

```toml
winresource = "0.1"
```

Reemplazar `build.rs` completo:

```rust
fn main() {
    slint_build::compile("ui/app.slint").expect("no se pudo compilar ui/app.slint");

    // Ícono y datos de producto en el .exe (versión tomada de Cargo.toml).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=ui/icon.ico");
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("ui/icon.ico")
            .set("ProductName", "Ajustes de pantalla")
            .set("FileDescription", "Saturación, contraste y brillo de la pantalla")
            .set("OriginalFilename", "Ajustes de pantalla.exe");
        resource.compile().expect("no se pudo compilar el recurso de Windows");
    }
}
```

Si `winresource` no encuentra el compilador de recursos (`rc.exe` del Windows SDK que instalan las Build Tools), reportar BLOCKED con el error exacto; no cambiar de crate sin avisar.

- [ ] **Step 3: Compilar y verificar**

Run: `cargo build --release`
Expected: `Finished` sin advertencias.

Run (PowerShell): `(Get-Item target\release\saturacion.exe).VersionInfo | Format-List ProductName, FileDescription, ProductVersion, FileVersion`
Expected: `ProductName : Ajustes de pantalla`, `FileDescription : Saturación, contraste y brillo de la pantalla`, versiones `1.0.0` (o `1.0.0.0`).

Run: `cargo test`
Expected: `test result: ok. 60 passed`.

- [ ] **Step 4: Commit**

```bash
git add ui/make-icon.ps1 ui/icon.ico build.rs Cargo.toml Cargo.lock
git commit -m "feat: embed icon and version information in the executable"
```

---

### Task 6: Instalador con Inno Setup

**Files:**
- Create: `installer/setup.iss`
- Create: `scripts/build-installer.ps1`
- Delete: `scripts/install.ps1`
- Modify: `.gitignore`

**Interfaces:**
- Consumes: `target\release\saturacion.exe` con recurso de versión, `ui/icon.ico` (Task 5); argumentos `--salir` y `--minimizado` (Task 4).
- Produces: `dist/AjustesDePantalla-Setup-1.0.0.exe`.

Prerrequisito (lo instala el controlador antes de esta tarea, con la aprobación ya dada por el usuario): Inno Setup 6 (`winget install --id JRSoftware.InnoSetup -e`).

- [ ] **Step 1: Script del instalador**

`installer/setup.iss`:

```ini
; Instalador de Ajustes de pantalla (Inno Setup 6).
; Se compila con scripts/build-installer.ps1, que pasa /DAppVersion=<versión de Cargo.toml>.

#define AppName "Ajustes de pantalla"
#define AppExe "Ajustes de pantalla.exe"
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{8D3F0C52-4B6E-4E8A-9F1A-6C2B7D5E3A91}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
DefaultDirName={localappdata}\Programs\{#AppName}
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=..\dist
OutputBaseFilename=AjustesDePantalla-Setup-{#AppVersion}
SetupIconFile=..\ui\icon.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=no
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "desktopicon"; Description: "Crear un acceso directo en el escritorio"; GroupDescription: "Accesos directos:"
Name: "autostart"; Description: "Iniciar con Windows (minimizada en la bandeja)"; GroupDescription: "Inicio:"

[Files]
Source: "..\target\release\saturacion.exe"; DestDir: "{app}"; DestName: "{#AppExe}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{userdesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "{#AppName}"; ValueData: """{app}\{#AppExe}"" --minimizado"; Tasks: autostart

[Run]
Filename: "{app}\{#AppExe}"; Description: "Abrir {#AppName}"; Flags: nowait postinstall skipifsilent

[Code]
{ Cierra la copia que corre pidiéndole que restaure la imagen (--salir). }
procedure CloseRunningApp();
var
  ResultCode: Integer;
  Exe: String;
begin
  Exe := ExpandConstant('{app}\{#AppExe}');
  if FileExists(Exe) then
    Exec(Exe, '--salir', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  CloseRunningApp();
  Result := '';
end;

function InitializeUninstall(): Boolean;
begin
  CloseRunningApp();
  Result := True;
end;

{ Al desinstalar borra la entrada de inicio aunque la haya creado la app y no el instalador. }
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', '{#AppName}');
end;
```

- [ ] **Step 2: Script de compilación**

`scripts/build-installer.ps1` (guardar en UTF-8 con BOM):

```powershell
# Compila la app en release y genera el instalador con Inno Setup en dist/.
# Requiere Inno Setup 6: winget install --id JRSoftware.InnoSetup -e
# Uso, desde la raíz del repo:
#   powershell -ExecutionPolicy Bypass -File scripts/build-installer.ps1
$ErrorActionPreference = 'Stop'

$repo = Split-Path -Parent $PSScriptRoot
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path $cargo)) { $cargo = 'cargo' }

& $cargo build --release --manifest-path (Join-Path $repo 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw "cargo build falló (código $LASTEXITCODE)" }

$versionLine = Select-String -Path (Join-Path $repo 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
$version = $versionLine.Matches[0].Groups[1].Value

$iscc = @(
    (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'),
    (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
    (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe')
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) { throw 'No se encontró Inno Setup 6. Instalalo con: winget install --id JRSoftware.InnoSetup -e' }

& $iscc "/DAppVersion=$version" (Join-Path $repo 'installer\setup.iss')
if ($LASTEXITCODE -ne 0) { throw "ISCC falló (código $LASTEXITCODE)" }

Write-Output "Instalador: $(Join-Path $repo "dist\AjustesDePantalla-Setup-$version.exe")"
```

Para guardarlo con BOM, después de escribirlo:

```powershell
$p = "scripts/build-installer.ps1"; $t = [IO.File]::ReadAllText((Resolve-Path $p), [Text.Encoding]::UTF8); [IO.File]::WriteAllText((Resolve-Path $p), $t, (New-Object Text.UTF8Encoding $true))
```

- [ ] **Step 3: Limpieza**

Borrar `scripts/install.ps1` (`git rm scripts/install.ps1`) y agregar al `.gitignore` una línea:

```
/dist
```

- [ ] **Step 4: Generar el instalador**

Run: `powershell -ExecutionPolicy Bypass -File scripts/build-installer.ps1`
Expected: la compilación de Inno Setup termina con `Successful compile` y la última línea es `Instalador: ...\dist\AjustesDePantalla-Setup-1.0.0.exe`; el archivo existe.

No ejecutar el instalador: lo hace el usuario en la Task 8.

- [ ] **Step 5: Commit**

```bash
git add installer/setup.iss scripts/build-installer.ps1 .gitignore
git commit -m "feat: add Inno Setup installer with desktop shortcut and autostart"
```

(`git rm` de `scripts/install.ps1` ya quedó en el índice y entra en este commit.)

---

### Task 7: Documentación de seguridad y README

**Files:**
- Create: `docs/SEGURIDAD.md`
- Create: `README.md`

- [ ] **Step 1: `docs/SEGURIDAD.md`**

```markdown
# Seguridad con CS2 y FACEIT

Solo Valve y FACEIT pueden garantizar que un programa no cause un ban. Este documento reúne qué hace la app, por qué el riesgo es bajo y qué queda abierto.

## Qué hace la app

- Escribe dos ajustes en el pipeline de pantalla de la placa Intel: una matriz de color 3×3 (saturación) y la curva 1D de salida (contraste y brillo).
- Lo hace con la API pública de Intel, Intel Graphics Control Library (`ControlLib.dll`), la misma que usa el panel de Intel. La DLL se carga solo desde `C:\Windows\System32`, donde la instala el driver.
- El cambio ocurre en la placa, después de que el juego entrega la imagen.

## Qué no hace

- No abre, lee ni escribe el proceso del juego.
- No inyecta código ni carga DLL en el juego.
- No instala drivers propios.
- No usa la red.

## Antecedentes

- **vibranceGUI** hace lo mismo con las APIs de NVIDIA y AMD (sube la saturación desde el driver). FACEIT lo incluye en su lista de programas que no causan ban: [What is deemed to be a cheat?](https://support.faceit.com/hc/en-us/articles/360015788779-What-is-deemed-to-be-a-cheat). Su autor indica que es seguro con VAC: [guía en Steam](https://steamcommunity.com/sharedfiles/filedetails/?id=752422230).
- **AMD Anti-Lag+ (2023)**: Valve baneó a quienes lo activaron en CS2 porque el driver desviaba funciones de las DLL del juego, y después revirtió esos bans: [PC Gamer](https://pcgamer.com/valve-warns-counter-strike-2-players-use-amds-anti-lag-feature-get-banned). Es el ejemplo de lo que esta app no hace.

## Transparencia

El código es público: <https://github.com/kenymtz/saturacion-windows>. Cualquiera puede comprobar lo anterior. El ejecutable lleva nombre de producto, descripción y versión en sus propiedades.

## Riesgo residual

- FACEIT puede avisar que un programa que no conoce no se puede usar con su anticheat activo. Según su política, es un aviso que pide cerrarlo, no un ban.
- El ejecutable y el instalador no están firmados digitalmente (requiere un certificado pago). Windows SmartScreen puede mostrar un aviso al instalar.

## Mensaje para soporte de FACEIT

> Hello, I use a small open-source Windows app, "Ajustes de pantalla" (https://github.com/kenymtz/saturacion-windows), to raise my screen saturation above the Intel control panel's limit, similar to what vibranceGUI does for NVIDIA and AMD. It only calls Intel's official Graphics Control Library (ControlLib.dll, installed with the driver) to set the display pipeline's colour matrix and output curve. It never opens, reads or injects into the game process, installs no drivers and uses no network. Could you confirm it is allowed while the FACEIT Anti-Cheat is running? Thank you.

## Prueba con el cliente de FACEIT

Pendiente de la verificación con el usuario.
```

- [ ] **Step 2: `README.md`**

~~~markdown
# Ajustes de pantalla

App de Windows para jugadores que sube la saturación de la pantalla por encima del tope de Windows y del panel de Intel, y ajusta contraste y brillo. Los ajustes se aplican en la placa de video, así que se mantienen dentro de los juegos en pantalla completa exclusiva.

## Qué hace

- **Saturación** de 0 a 400 (100 = sin cambios).
- **Contraste** y **brillo** de 50 a 200.
- Recuerda los valores y puede **iniciar con Windows**, minimizada en la bandeja.
- **Restablecer** vuelve todo a la imagen normal.
- Cerrar la ventana la deja en la bandeja; **Salir** desde el ícono restaura la imagen y cierra.

## Requisitos

- Windows 10 u 11 de 64 bits.
- Pantalla conectada a una placa **Intel** con su driver instalado (la app usa la API de Intel; NVIDIA y AMD no están soportadas todavía).

## Instalación

1. Ejecutar `AjustesDePantalla-Setup-<versión>.exe`. No pide permisos de administrador.
2. Si Windows SmartScreen muestra "Windows protegió su PC", elegir **Más información** → **Ejecutar de todas formas** (el instalador no está firmado).
3. El instalador crea el acceso directo en el escritorio, la entrada en el menú Inicio y, si se deja marcado, el inicio con Windows.

Para desinstalar: Configuración → Aplicaciones → "Ajustes de pantalla". Los valores guardados quedan en `%APPDATA%\Ajustes de pantalla`.

## Seguridad con CS2 y FACEIT

Ver [docs/SEGURIDAD.md](docs/SEGURIDAD.md).

## Compilar

Requisitos: Rust (toolchain `stable-x86_64-pc-windows-msvc`), Build Tools de C++ de Visual Studio e Inno Setup 6 para el instalador. Si Windows tiene Smart App Control activado, bloquea los ejecutables sin firma que genera Rust y hay que desactivarlo para compilar.

```bash
cargo test
```

```bash
powershell -ExecutionPolicy Bypass -File scripts/build-installer.ps1
```

El instalador queda en `dist/`.

## Documentación

Las especificaciones y planes de cada historia están en `docs/specs/` y `docs/plans/`.
~~~

- [ ] **Step 3: Commit**

```bash
git add docs/SEGURIDAD.md README.md
git commit -m "docs: add security notes and README"
```

---

### Task 8: Verificación de aceptación

**Necesita a una persona frente a la pantalla.** El controlador corre los comandos en la terminal del usuario.

**Files:** `docs/specs/2026-11-13-entregable-final-design.md`, `docs/SEGURIDAD.md` (resultados).

- [ ] **Step 1: Desinstalar copias manuales previas**

Si existe la instalación manual anterior (`%LOCALAPPDATA%\Programs\Ajustes de pantalla` creada por el script viejo, sin desinstalador) y el acceso directo viejo del escritorio, cerrar la app con "Salir" y dejar que el instalador los reemplace (misma carpeta y mismo nombre de acceso directo).

- [ ] **Step 2: Instalar**

Ejecutar `dist\AjustesDePantalla-Setup-1.0.0.exe`.
Expected: no pide administrador; ofrece "Crear un acceso directo en el escritorio" e "Iniciar con Windows" marcadas; al terminar ofrece abrir la app. Aparecen el acceso directo en el escritorio, la entrada en el menú Inicio y "Ajustes de pantalla" en Configuración → Aplicaciones con su ícono. Propiedades del `.exe` instalado → Detalles: nombre de producto, descripción y versión 1.0.0.

- [ ] **Step 3: Recordar valores**

Abrir la app, poner saturación 250, contraste 120, brillo 110 (soltando cada slider). "Salir" desde la bandeja. Volver a abrir desde el acceso directo.
Expected: los sliders vuelven en 250/120/110 y la imagen se ve con esos ajustes apenas abre.

- [ ] **Step 4: Instancia única**

Con la app abierta (o en la bandeja), abrir de nuevo el acceso directo.
Expected: no aparece una segunda ventana ni un segundo ícono; la ventana existente vuelve al frente.

- [ ] **Step 5: Inicio con Windows**

Comprobar que la casilla "Iniciar con Windows" está marcada. En la terminal: `reg query HKCU\Software\Microsoft\Windows\CurrentVersion\Run /v "Ajustes de pantalla"` muestra `"...\Ajustes de pantalla.exe" --minimizado`. Desmarcar la casilla: la consulta ya no encuentra el valor. Volver a marcarla.
Simular el arranque: "Salir" y ejecutar el comando del registro tal cual. Expected: la app arranca solo en la bandeja, sin ventana, con los valores aplicados.
Reinicio real (opcional, al final de la sesión): tras reiniciar, la app aparece sola en la bandeja con los valores aplicados.

- [ ] **Step 6: Restablecer**

Botón "Restablecer" en la ventana. Expected: sliders en 100, imagen normal. Mover un slider y usar "Restablecer" desde el menú de la bandeja: mismo resultado. Cerrar y abrir: los sliders vuelven en 100.

- [ ] **Step 7: Prueba con FACEIT**

Con valores aplicados y la app en la bandeja, abrir el cliente de FACEIT (si está instalado). Anotar si aparece algún aviso sobre la app. Si no está instalado, anotar que no se probó.

- [ ] **Step 8: Reinstalar con la app abierta y desinstalar**

Con valores aplicados y la app corriendo, ejecutar el instalador de nuevo. Expected: la app se cierra sola restaurando la imagen antes de instalar, y la instalación termina bien.
Desinstalar desde Configuración → Aplicaciones con la app abierta. Expected: la app se cierra restaurando la imagen; desaparecen el acceso directo, la entrada del menú Inicio y la entrada `Run` (la consulta de `reg query` ya no la encuentra); `%APPDATA%\Ajustes de pantalla\ajustes.txt` sigue existiendo.

- [ ] **Step 9: Reinstalar para dejar la app instalada**

Volver a instalar con el Setup para que el usuario se quede con la app.

- [ ] **Step 10: Registrar y commit**

- En `docs/SEGURIDAD.md`, reemplazar "Pendiente de la verificación con el usuario." por el resultado del Step 7 con la fecha.
- En la spec, cambiar `Estado:` a `Estado: implementada y verificada en hardware (plan: `docs/plans/2026-11-13-entregable-final.md`)` y agregar `## Resultado de la verificación de aceptación` con la fecha, el commit probado y lo observado en los Steps 2 a 8, marcando lo no verificado (reinicio real si no se hizo).

```bash
git add docs/SEGURIDAD.md docs/specs/2026-11-13-entregable-final-design.md
git commit -m "docs: record final deliverable acceptance results"
```
