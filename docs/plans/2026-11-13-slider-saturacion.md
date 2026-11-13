# Slider de saturación (historia 1) — Plan de implementación

**Goal:** Una app de Windows con un slider de 0 a 300 que sube la saturación de las pantallas conectadas a la placa Intel por encima del tope del panel de Intel, y la restaura al cerrar.

**Architecture:** Un crate Rust con librería + binario. Una función pura calcula la matriz de saturación; un trait `ColorBackend` abstrae la placa; `IgclBackend` lo implementa cargando `ControlLib.dll` (IGCL, la API pública de Intel) y escribiendo la matriz en el bloque 3×3 del pipeline de pantalla; un `Controller` une slider y backend; la ventana es Slint.

**Tech Stack:** Rust stable (MSVC), Slint 1.x, libloading 0.8, bindgen-cli (una vez, para generar bindings), IGCL (`igcl_api.h`, commit `1db55c364b15649427a71a05d950ab9c1eaf9a07`).

**Spec:** `docs/specs/2026-11-13-slider-saturacion-design.md`

## Global Constraints

- Solo Windows x64, toolchain `stable-x86_64-pc-windows-msvc`.
- El slider va de **0 a 300** en pasos enteros; **100 es la imagen sin modificar**.
- `ControlLib.dll` **no se distribuye** con la app y se carga **solo desde System32** (`LOAD_LIBRARY_SEARCH_SYSTEM32`), nunca desde la carpeta de la app ni del directorio actual.
- La app no inyecta código ni lee memoria de otros procesos.
- No se usa el flag `CTL_PIXTX_PIPE_SET_CONFIG_FLAG_PERSIST_ACROSS_POWER_EVENTS`.
- Todo `unsafe` y todo lo específico de IGCL vive en `src/igcl/`.
- Textos visibles al usuario en español. Identificadores y mensajes de commit en inglés.
- Fuera de alcance: contraste, brillo, animaciones, persistencia, reset rápido, NVIDIA/AMD.

## Estructura de archivos

| Archivo | Responsabilidad |
|---|---|
| `Cargo.toml`, `.gitignore`, `build.rs` | Manifiesto, ignorar `target/`, compilar la UI Slint |
| `src/lib.rs` | Declara los módulos públicos |
| `src/color.rs` | `Matrix3`, `IDENTITY`, `LUMA`, `saturation_matrix` |
| `src/backend.rs` | Trait `ColorBackend`, enum `BackendError` con sus mensajes |
| `src/controller.rs` | `Controller<B>`: valor del slider → matriz → backend; restaurar al salir |
| `src/igcl/mod.rs` | `IgclBackend`: implementación de `ColorBackend` sobre IGCL |
| `src/igcl/bindings.rs` | Bindings generados por bindgen (commiteados, no se editan a mano) |
| `vendor/igcl/igcl_api.h`, `vendor/igcl/LICENSE` | Header oficial de Intel, fijado a un commit |
| `examples/probe.rs` | Prueba de riesgo de consola |
| `ui/app.slint` | Ventana: slider, valor, mensaje de error |
| `src/main.rs` | Arranque: crea backend y ventana, conecta callbacks, restaura al salir |

---

### Task 0: Prerrequisitos de desarrollo

**Requiere confirmación explícita del usuario antes de ejecutar**: instala software en la máquina (varios GB entre Build Tools y LLVM).

**Files:** ninguno.

**Interfaces:**
- Produces: `cargo`, `rustc` y `bindgen` disponibles en la terminal; `LIBCLANG_PATH` definido.

- [ ] **Step 1: Instalar Rust, Build Tools de C++ y LLVM**

Cada comando por separado, en PowerShell:

```powershell
winget install --id Rustlang.Rustup -e
```

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

```powershell
winget install --id LLVM.LLVM -e
```

Si winget ofrece una versión más nueva de las Build Tools (`winget search Microsoft.VisualStudio BuildTools`), sirve igual siempre que incluya el workload `VCTools`.

- [ ] **Step 2: Configurar toolchain y bindgen**

En una terminal nueva (para que tome el PATH):

```powershell
rustup default stable-x86_64-pc-windows-msvc
```

```powershell
[Environment]::SetEnvironmentVariable("LIBCLANG_PATH", "C:\Program Files\LLVM\bin", "User")
```

```powershell
cargo install bindgen-cli
```

- [ ] **Step 3: Verificar**

En otra terminal nueva:

```powershell
cargo --version; rustc --version; bindgen --version; $env:LIBCLANG_PATH
```

Expected: tres líneas de versión (`cargo 1.x`, `rustc 1.x`, `bindgen 0.x`) y la ruta `C:\Program Files\LLVM\bin`.

---

### Task 1: Esqueleto del crate y matriz de saturación

**Files:**
- Create: `Cargo.toml`
- Create: `.gitignore`
- Create: `src/lib.rs`
- Create: `src/color.rs`

**Interfaces:**
- Consumes: nada.
- Produces (en `saturacion::color`):
  - `pub type Matrix3 = [[f64; 3]; 3];` — indexado `[fila][columna]`, salida = M × vector columna RGB.
  - `pub const IDENTITY: Matrix3`
  - `pub const LUMA: [f64; 3] = [0.2126, 0.7152, 0.0722];`
  - `pub fn saturation_matrix(s: f64) -> Matrix3` — `s = 1.0` es identidad.

- [ ] **Step 1: Crear manifiesto, .gitignore y lib.rs**

`Cargo.toml`:

```toml
[package]
name = "saturacion"
version = "0.1.0"
edition = "2021"

[dependencies]
```

`.gitignore`:

```
/target
```

`src/lib.rs`:

```rust
pub mod color;
```

- [ ] **Step 2: Escribir los tests que fallan**

`src/color.rs`:

```rust
//! Matriz de saturación. Función pura, sin dependencias de placa ni de UI.

/// Matriz 3×3 indexada `[fila][columna]`; salida = M × vector columna RGB.
pub type Matrix3 = [[f64; 3]; 3];

pub const IDENTITY: Matrix3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// Pesos de luma BT.709.
pub const LUMA: [f64; 3] = [0.2126, 0.7152, 0.0722];

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: Matrix3, expected: Matrix3) {
        for i in 0..3 {
            for j in 0..3 {
                assert!(
                    (actual[i][j] - expected[i][j]).abs() < 1e-12,
                    "[{i}][{j}]: {} != {}",
                    actual[i][j],
                    expected[i][j]
                );
            }
        }
    }

    #[test]
    fn factor_one_is_identity() {
        assert_close(saturation_matrix(1.0), IDENTITY);
    }

    #[test]
    fn factor_zero_is_grayscale() {
        assert_close(saturation_matrix(0.0), [LUMA, LUMA, LUMA]);
    }

    #[test]
    fn rows_sum_to_one_so_grays_are_preserved() {
        for s in [0.0, 0.5, 1.0, 1.25, 3.0] {
            for row in saturation_matrix(s) {
                let sum: f64 = row.iter().sum();
                assert!((sum - 1.0).abs() < 1e-12, "s={s}: fila suma {sum}");
            }
        }
    }

    #[test]
    fn factor_three_boosts_diagonal_and_subtracts_other_channels() {
        let m = saturation_matrix(3.0);
        assert!((m[0][0] - 2.5748).abs() < 1e-12);
        assert!((m[0][1] - (-1.4304)).abs() < 1e-12);
        assert!((m[0][2] - (-0.1444)).abs() < 1e-12);
    }
}
```

- [ ] **Step 3: Correr los tests y ver que fallan**

Run: `cargo test --lib color`
Expected: error de compilación `cannot find function 'saturation_matrix' in this scope`.

- [ ] **Step 4: Implementar**

En `src/color.rs`, entre la constante `LUMA` y `#[cfg(test)]`:

```rust
/// `M = s·I + (1 − s)·[LUMA; LUMA; LUMA]`.
/// `s = 1.0` deja la imagen igual, `0.0` la pasa a grises, `> 1.0` satura.
pub fn saturation_matrix(s: f64) -> Matrix3 {
    let mut m = [[0.0; 3]; 3];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (1.0 - s) * LUMA[j] + if i == j { s } else { 0.0 };
        }
    }
    m
}
```

- [ ] **Step 5: Correr los tests y ver que pasan**

Run: `cargo test --lib color`
Expected: `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock .gitignore src/lib.rs src/color.rs
git commit -m "feat: add crate skeleton and saturation matrix"
```

---

### Task 2: Trait de backend y controlador

**Files:**
- Create: `src/backend.rs`
- Create: `src/controller.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `saturacion::color::{Matrix3, saturation_matrix}` (Task 1).
- Produces (en `saturacion::backend`):
  - `pub enum BackendError { DriverNotFound, NoActiveDisplay, MatrixUnsupported, ApplyFailed(u32) }` — deriva `Debug, Clone, PartialEq, Eq`, implementa `Display` (mensajes en español) y `std::error::Error`.
  - `pub trait ColorBackend { fn apply_matrix(&mut self, matrix: &Matrix3) -> Result<(), BackendError>; fn restore_default(&mut self) -> Result<(), BackendError>; }`
- Produces (en `saturacion::controller`):
  - `pub const MAX_PERCENT: u32 = 300;`
  - `pub struct Controller<B: ColorBackend>`
  - `pub fn new(backend: B) -> Self`
  - `pub fn set_saturation(&mut self, percent: u32) -> Result<(), BackendError>` — recorta a `MAX_PERCENT`.
  - `pub fn shutdown(&mut self) -> Result<(), BackendError>` — restaura solo si hubo alguna escritura.

- [ ] **Step 1: Crear backend.rs**

`src/backend.rs`:

```rust
//! Interfaz que separa la lógica de la app de la placa de video.

use std::fmt;

use crate::color::Matrix3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// `ControlLib.dll` no está o `ctlInit` falló.
    DriverNotFound,
    /// No hay pantallas activas en un adaptador Intel.
    NoActiveDisplay,
    /// Hay pantallas activas pero ninguna reporta un bloque de matriz 3×3.
    MatrixUnsupported,
    /// La escritura de la matriz falló; el valor es el código de IGCL.
    ApplyFailed(u32),
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DriverNotFound => {
                write!(f, "No se encontró el driver de gráficos de Intel.")
            }
            Self::NoActiveDisplay => {
                write!(f, "Ninguna pantalla activa está conectada a la placa Intel.")
            }
            Self::MatrixUnsupported => {
                write!(f, "Esta placa o este modo de pantalla no permite ajustar el color.")
            }
            Self::ApplyFailed(code) => {
                write!(f, "No se pudo aplicar el ajuste (IGCL 0x{code:08X}).")
            }
        }
    }
}

impl std::error::Error for BackendError {}

pub trait ColorBackend {
    /// Escribe la matriz en todas las pantallas que maneja el backend.
    fn apply_matrix(&mut self, matrix: &Matrix3) -> Result<(), BackendError>;
    /// Devuelve las pantallas a la imagen sin modificar.
    fn restore_default(&mut self) -> Result<(), BackendError>;
}
```

`src/lib.rs` completo:

```rust
pub mod backend;
pub mod color;
pub mod controller;
```

- [ ] **Step 2: Escribir los tests que fallan**

`src/controller.rs`:

```rust
//! Une el valor del slider con el backend de color.

use crate::backend::{BackendError, ColorBackend};
use crate::color::saturation_matrix;

/// Máximo del slider, en porcentaje. 100 es la imagen sin modificar.
pub const MAX_PERCENT: u32 = 300;

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::color::Matrix3;

    #[derive(Debug, PartialEq)]
    enum Call {
        Apply(Matrix3),
        Restore,
    }

    struct FakeBackend {
        calls: Rc<RefCell<Vec<Call>>>,
        fail_with: Option<BackendError>,
    }

    impl ColorBackend for FakeBackend {
        fn apply_matrix(&mut self, matrix: &Matrix3) -> Result<(), BackendError> {
            self.calls.borrow_mut().push(Call::Apply(*matrix));
            match &self.fail_with {
                Some(e) => Err(e.clone()),
                None => Ok(()),
            }
        }

        fn restore_default(&mut self) -> Result<(), BackendError> {
            self.calls.borrow_mut().push(Call::Restore);
            Ok(())
        }
    }

    fn controller(
        fail_with: Option<BackendError>,
    ) -> (Controller<FakeBackend>, Rc<RefCell<Vec<Call>>>) {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let backend = FakeBackend { calls: Rc::clone(&calls), fail_with };
        (Controller::new(backend), calls)
    }

    #[test]
    fn slider_value_becomes_saturation_matrix() {
        let (mut c, calls) = controller(None);
        c.set_saturation(250).unwrap();
        assert_eq!(*calls.borrow(), vec![Call::Apply(saturation_matrix(2.5))]);
    }

    #[test]
    fn value_above_maximum_is_clamped() {
        let (mut c, calls) = controller(None);
        c.set_saturation(900).unwrap();
        assert_eq!(*calls.borrow(), vec![Call::Apply(saturation_matrix(3.0))]);
    }

    #[test]
    fn write_error_reaches_caller_with_user_message() {
        let (mut c, _) = controller(Some(BackendError::ApplyFailed(0x4000_0001)));
        let err = c.set_saturation(150).unwrap_err();
        assert_eq!(
            err.to_string(),
            "No se pudo aplicar el ajuste (IGCL 0x40000001)."
        );
    }

    #[test]
    fn shutdown_restores_after_a_write() {
        let (mut c, calls) = controller(None);
        c.set_saturation(150).unwrap();
        c.shutdown().unwrap();
        assert_eq!(calls.borrow().last(), Some(&Call::Restore));
    }

    #[test]
    fn shutdown_restores_even_if_the_write_failed() {
        let (mut c, calls) = controller(Some(BackendError::ApplyFailed(1)));
        let _ = c.set_saturation(150);
        c.shutdown().unwrap();
        assert_eq!(calls.borrow().last(), Some(&Call::Restore));
    }

    #[test]
    fn shutdown_without_writes_does_not_touch_the_display() {
        let (mut c, calls) = controller(None);
        c.shutdown().unwrap();
        assert!(calls.borrow().is_empty());
    }
}
```

- [ ] **Step 3: Correr los tests y ver que fallan**

Run: `cargo test --lib controller`
Expected: error de compilación `cannot find type 'Controller' in this scope`.

- [ ] **Step 4: Implementar**

En `src/controller.rs`, entre `MAX_PERCENT` y `#[cfg(test)]`:

```rust
pub struct Controller<B: ColorBackend> {
    backend: B,
    /// Verdadero desde el primer intento de escritura: una escritura fallida
    /// puede haber llegado a alguna pantalla, así que también obliga a restaurar.
    wrote: bool,
}

impl<B: ColorBackend> Controller<B> {
    pub fn new(backend: B) -> Self {
        Self { backend, wrote: false }
    }

    /// Aplica la saturación indicada en porcentaje (100 = sin cambios).
    pub fn set_saturation(&mut self, percent: u32) -> Result<(), BackendError> {
        let s = f64::from(percent.min(MAX_PERCENT)) / 100.0;
        self.wrote = true;
        self.backend.apply_matrix(&saturation_matrix(s))
    }

    /// Devuelve la imagen a la normal si la app llegó a modificarla.
    pub fn shutdown(&mut self) -> Result<(), BackendError> {
        if !self.wrote {
            return Ok(());
        }
        self.wrote = false;
        self.backend.restore_default()
    }
}
```

- [ ] **Step 5: Correr todos los tests y ver que pasan**

Run: `cargo test --lib`
Expected: `test result: ok. 10 passed; 0 failed`.

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs src/backend.rs src/controller.rs
git commit -m "feat: add ColorBackend trait and saturation controller"
```

---

### Task 3: Backend de Intel (IGCL)

Sin tests automáticos: depende de hardware y driver reales. Se verifica compilando y, en la Task 4, con la prueba de riesgo.

**Files:**
- Create: `vendor/igcl/igcl_api.h` (descargado)
- Create: `vendor/igcl/LICENSE` (descargado)
- Create: `src/igcl/bindings.rs` (generado)
- Create: `src/igcl/mod.rs`
- Modify: `Cargo.toml`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `saturacion::backend::{BackendError, ColorBackend}` (Task 2), `saturacion::color::{Matrix3, IDENTITY}` (Task 1).
- Produces (en `saturacion::igcl`):
  - `pub struct IgclBackend` — implementa `ColorBackend`.
  - `pub fn new() -> Result<IgclBackend, BackendError>`
  - `pub fn output_count(&self) -> usize` — pantallas activas con bloque de matriz.

**Referencia de la API (verificada contra `igcl_api.h` y el ejemplo `Samples/Color_Samples/Color_Sample_App.cpp`):**
- `ctlInit(ctl_init_args_t*, ctl_api_handle_t*)`: zerear args, `Size = sizeof`, `AppVersion = (major << 16) | (minor & 0xffff)`, `flags = 0`.
- `ctlEnumerateDevices(api, &count, handles)` y `ctlEnumerateDisplayOutputs(adapter, &count, handles)`: primera llamada con puntero nulo para obtener `count`, segunda con el arreglo.
- `ctlGetDisplayProperties(display, &props)`: `props.Size = sizeof`; la pantalla sirve si `DisplayConfigFlags` tiene `CTL_DISPLAY_CONFIG_FLAG_DISPLAY_ACTIVE` y `CTL_DISPLAY_CONFIG_FLAG_DISPLAY_ATTACHED`.
- `ctlPixelTransformationGetConfig(display, &caps)` con `QueryType = CTL_PIXTX_CONFIG_QUERY_TYPE_CAPABILITY`: primera llamada con `pBlockConfigs` nulo devuelve `NumBlocks`; segunda con un arreglo zereado devuelve los bloques.
- `ctlPixelTransformationSetConfig(display, &args)`: `OpertaionType = CTL_PIXTX_CONFIG_OPERTAION_TYPE_SET_CUSTOM` (el typo es del header), `NumBlocks = 1`, `pBlockConfigs` apunta a una **copia del bloque de capacidades** con `Size = sizeof(ctl_pixtx_block_config_t)` y la matriz en `Config.MatrixConfig.Matrix`.

- [ ] **Step 1: Vendorizar el header y su licencia**

```powershell
New-Item -ItemType Directory -Force vendor/igcl, src/igcl | Out-Null
```

```powershell
curl.exe -fL -o vendor/igcl/igcl_api.h https://raw.githubusercontent.com/intel/drivers.gpu.control-library/1db55c364b15649427a71a05d950ab9c1eaf9a07/include/igcl_api.h
```

```powershell
curl.exe -fL -o vendor/igcl/LICENSE https://raw.githubusercontent.com/intel/drivers.gpu.control-library/1db55c364b15649427a71a05d950ab9c1eaf9a07/LICENSE
```

Verificar: `Select-String -Path vendor/igcl/igcl_api.h -Pattern "ctlPixelTransformationSetConfig" | Measure-Object | Select-Object -ExpandProperty Count`
Expected: un número mayor que 0.

- [ ] **Step 2: Generar los bindings**

```powershell
bindgen vendor/igcl/igcl_api.h -o src/igcl/bindings.rs --dynamic-loading ControlLib --dynamic-link-require-all --no-prepend-enum-name --allowlist-function "ctlInit|ctlClose|ctlEnumerateDevices|ctlEnumerateDisplayOutputs|ctlGetDisplayProperties|ctlPixelTransformationGetConfig|ctlPixelTransformationSetConfig" --allowlist-var "CTL_IMPL_MAJOR_VERSION|CTL_IMPL_MINOR_VERSION" --allowlist-type "_ctl_display_config_flag_t" -- -include stdbool.h
```

Verificar que están los símbolos que usa `mod.rs`:

```powershell
"pub struct ControlLib", "pub const CTL_RESULT_SUCCESS:", "pub const CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX:", "pub const CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS:", "pub const CTL_PIXTX_CONFIG_QUERY_TYPE_CAPABILITY:", "pub const CTL_PIXTX_CONFIG_OPERTAION_TYPE_SET_CUSTOM:", "pub const CTL_DISPLAY_CONFIG_FLAG_DISPLAY_ACTIVE:", "pub const CTL_DISPLAY_CONFIG_FLAG_DISPLAY_ATTACHED:", "pub const CTL_IMPL_MAJOR_VERSION:", "pub const CTL_IMPL_MINOR_VERSION:" | ForEach-Object { "{0} -> {1}" -f $_, [bool](Select-String -Path src/igcl/bindings.rs -SimpleMatch $_ -Quiet) }
```

Expected: diez líneas, todas terminadas en `-> True`. Si alguna da `False`, no seguir: abrir `src/igcl/bindings.rs`, buscar el nombre real que generó bindgen y avisar antes de adaptar `mod.rs`.

- [ ] **Step 3: Agregar la dependencia y el módulo**

En `Cargo.toml`, la sección de dependencias queda:

```toml
[dependencies]
libloading = "0.8"
```

`src/lib.rs` completo:

```rust
pub mod backend;
pub mod color;
pub mod controller;
pub mod igcl;
```

- [ ] **Step 4: Escribir el backend**

`src/igcl/mod.rs`:

```rust
//! Backend de Intel: escribe una matriz de color 3×3 en el pipeline de
//! pantalla de la placa usando IGCL (`ControlLib.dll`, instalada por el driver).

#[allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code,
    clippy::all
)]
mod bindings;

use std::mem::{size_of, zeroed};
use std::ptr;

use libloading::os::windows::{Library, LOAD_LIBRARY_SEARCH_SYSTEM32};

use self::bindings::*;
use crate::backend::{BackendError, ColorBackend};
use crate::color::{Matrix3, IDENTITY};

/// Una pantalla activa y el bloque de matriz que reportó en sus capacidades.
struct Output {
    handle: ctl_display_output_handle_t,
    block: ctl_pixtx_block_config_t,
}

pub struct IgclBackend {
    lib: ControlLib,
    api: ctl_api_handle_t,
    outputs: Vec<Output>,
}

impl IgclBackend {
    pub fn new() -> Result<Self, BackendError> {
        // SAFETY: se carga únicamente la DLL que el driver instaló en System32;
        // nunca se busca en la carpeta de la app ni en el directorio actual.
        let lib = unsafe {
            let raw = Library::load_with_flags("ControlLib.dll", LOAD_LIBRARY_SEARCH_SYSTEM32)
                .map_err(|_| BackendError::DriverNotFound)?;
            ControlLib::from_library(raw).map_err(|_| BackendError::DriverNotFound)?
        };

        // SAFETY: ctl_init_args_t es un struct C plano; todo-ceros es válido.
        let mut args: ctl_init_args_t = unsafe { zeroed() };
        args.Size = size_of::<ctl_init_args_t>() as u32;
        args.AppVersion = (CTL_IMPL_MAJOR_VERSION << 16) | (CTL_IMPL_MINOR_VERSION & 0xffff);
        args.flags = 0;
        let mut api: ctl_api_handle_t = ptr::null_mut();
        // SAFETY: ambos punteros son válidos durante la llamada.
        if unsafe { lib.ctlInit(&mut args, &mut api) } != CTL_RESULT_SUCCESS {
            return Err(BackendError::DriverNotFound);
        }

        // Desde acá, Drop cierra el handle en cualquier camino de salida.
        let mut backend = Self { lib, api, outputs: Vec::new() };

        let mut saw_active_display = false;
        for adapter in backend.adapters()? {
            for display in backend.displays(adapter) {
                if !backend.is_active(display) {
                    continue;
                }
                saw_active_display = true;
                if let Some(block) = backend.matrix_block(display) {
                    backend.outputs.push(Output { handle: display, block });
                }
            }
        }

        if !saw_active_display {
            return Err(BackendError::NoActiveDisplay);
        }
        if backend.outputs.is_empty() {
            return Err(BackendError::MatrixUnsupported);
        }
        Ok(backend)
    }

    /// Cantidad de pantallas activas con bloque de matriz.
    pub fn output_count(&self) -> usize {
        self.outputs.len()
    }

    fn adapters(&self) -> Result<Vec<ctl_device_adapter_handle_t>, BackendError> {
        let mut count = 0u32;
        // SAFETY: con puntero nulo IGCL solo escribe la cantidad en `count`.
        let r = unsafe { self.lib.ctlEnumerateDevices(self.api, &mut count, ptr::null_mut()) };
        if r != CTL_RESULT_SUCCESS {
            return Err(BackendError::DriverNotFound);
        }
        let mut handles: Vec<ctl_device_adapter_handle_t> = vec![ptr::null_mut(); count as usize];
        // SAFETY: `handles` tiene lugar para `count` elementos.
        let r =
            unsafe { self.lib.ctlEnumerateDevices(self.api, &mut count, handles.as_mut_ptr()) };
        if r != CTL_RESULT_SUCCESS {
            return Err(BackendError::DriverNotFound);
        }
        handles.truncate(count as usize);
        Ok(handles)
    }

    /// Salidas de un adaptador. Un adaptador que falla o no tiene salidas da lista vacía.
    fn displays(&self, adapter: ctl_device_adapter_handle_t) -> Vec<ctl_display_output_handle_t> {
        let mut count = 0u32;
        // SAFETY: con puntero nulo IGCL solo escribe la cantidad en `count`.
        let r = unsafe { self.lib.ctlEnumerateDisplayOutputs(adapter, &mut count, ptr::null_mut()) };
        if r != CTL_RESULT_SUCCESS || count == 0 {
            return Vec::new();
        }
        let mut handles: Vec<ctl_display_output_handle_t> = vec![ptr::null_mut(); count as usize];
        // SAFETY: `handles` tiene lugar para `count` elementos.
        let r = unsafe {
            self.lib.ctlEnumerateDisplayOutputs(adapter, &mut count, handles.as_mut_ptr())
        };
        if r != CTL_RESULT_SUCCESS {
            return Vec::new();
        }
        handles.truncate(count as usize);
        handles
    }

    fn is_active(&self, display: ctl_display_output_handle_t) -> bool {
        // SAFETY: struct C plano; todo-ceros es válido.
        let mut props: ctl_display_properties_t = unsafe { zeroed() };
        props.Size = size_of::<ctl_display_properties_t>() as u32;
        // SAFETY: `props` es válido durante la llamada.
        if unsafe { self.lib.ctlGetDisplayProperties(display, &mut props) } != CTL_RESULT_SUCCESS {
            return false;
        }
        let required = (CTL_DISPLAY_CONFIG_FLAG_DISPLAY_ACTIVE
            | CTL_DISPLAY_CONFIG_FLAG_DISPLAY_ATTACHED) as u32;
        props.DisplayConfigFlags & required == required
    }

    /// Bloque de matriz 3×3 de la pantalla, si reporta alguno.
    /// Prefiere el bloque de matriz sola; si no hay, el de matriz con offsets.
    fn matrix_block(&self, display: ctl_display_output_handle_t) -> Option<ctl_pixtx_block_config_t> {
        // SAFETY: struct C plano; todo-ceros es válido (puntero nulo, cero bloques).
        let mut caps: ctl_pixtx_pipe_get_config_t = unsafe { zeroed() };
        caps.Size = size_of::<ctl_pixtx_pipe_get_config_t>() as u32;
        caps.QueryType = CTL_PIXTX_CONFIG_QUERY_TYPE_CAPABILITY;
        // SAFETY: con `pBlockConfigs` nulo IGCL solo completa `NumBlocks`.
        let r = unsafe { self.lib.ctlPixelTransformationGetConfig(display, &mut caps) };
        if r != CTL_RESULT_SUCCESS || caps.NumBlocks == 0 {
            return None;
        }

        // SAFETY: struct C plano; todo-ceros es válido.
        let mut blocks: Vec<ctl_pixtx_block_config_t> =
            vec![unsafe { zeroed() }; caps.NumBlocks as usize];
        caps.pBlockConfigs = blocks.as_mut_ptr();
        // SAFETY: `blocks` tiene lugar para `NumBlocks` elementos.
        let r = unsafe { self.lib.ctlPixelTransformationGetConfig(display, &mut caps) };
        if r != CTL_RESULT_SUCCESS {
            return None;
        }

        blocks
            .iter()
            .find(|b| b.BlockType == CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX)
            .or_else(|| {
                blocks
                    .iter()
                    .find(|b| b.BlockType == CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS)
            })
            .copied()
    }

    /// Escribe `matrix` en el bloque de matriz de cada pantalla.
    /// Intenta todas aunque alguna falle y devuelve el primer error.
    #[allow(unused_unsafe)]
    fn write(&mut self, matrix: &Matrix3) -> Result<(), BackendError> {
        let mut first_error = None;
        for output in &self.outputs {
            // Copia del bloque de capacidades: conserva BlockId y BlockType.
            let mut block = output.block;
            block.Size = size_of::<ctl_pixtx_block_config_t>() as u32;
            // SAFETY: el bloque es de tipo matriz, así que `MatrixConfig` es
            // el miembro activo de la unión; solo se escriben campos `f64`.
            unsafe {
                block.Config.MatrixConfig.Matrix = *matrix;
                block.Config.MatrixConfig.PreOffsets = [0.0; 3];
                block.Config.MatrixConfig.PostOffsets = [0.0; 3];
            }

            // SAFETY: struct C plano; todo-ceros es válido.
            let mut args: ctl_pixtx_pipe_set_config_t = unsafe { zeroed() };
            args.Size = size_of::<ctl_pixtx_pipe_set_config_t>() as u32;
            args.OpertaionType = CTL_PIXTX_CONFIG_OPERTAION_TYPE_SET_CUSTOM;
            args.Flags = 0;
            args.NumBlocks = 1;
            args.pBlockConfigs = &mut block;
            // SAFETY: `args` y `block` viven hasta después de la llamada.
            let r = unsafe { self.lib.ctlPixelTransformationSetConfig(output.handle, &mut args) };
            if r != CTL_RESULT_SUCCESS && first_error.is_none() {
                first_error = Some(BackendError::ApplyFailed(r as u32));
            }
        }
        match first_error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

impl ColorBackend for IgclBackend {
    fn apply_matrix(&mut self, matrix: &Matrix3) -> Result<(), BackendError> {
        self.write(matrix)
    }

    fn restore_default(&mut self) -> Result<(), BackendError> {
        self.write(&IDENTITY)
    }
}

impl Drop for IgclBackend {
    fn drop(&mut self) {
        // SAFETY: `api` vino de un `ctlInit` exitoso y no se usa después.
        unsafe {
            self.lib.ctlClose(self.api);
        }
    }
}
```

- [ ] **Step 5: Compilar y correr los tests existentes**

Run: `cargo build`
Expected: `Finished` sin errores. Advertencias dentro de `bindings.rs` están silenciadas; cualquier advertencia en `mod.rs` se corrige.

Run: `cargo test --lib`
Expected: `test result: ok. 10 passed; 0 failed`.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/lib.rs src/igcl vendor/igcl
git commit -m "feat: add Intel IGCL colour backend"
```

---

### Task 4: Prueba de riesgo

**Necesita a una persona frente a la pantalla.** Es la compuerta del mecanismo: si falla, no se construye la UI.

**Files:**
- Create: `examples/probe.rs`
- Modify: `docs/specs/2026-11-13-slider-saturacion-design.md` (agregar resultados)

**Interfaces:**
- Consumes: `saturacion::igcl::IgclBackend::{new, output_count}` (Task 3), `saturacion::backend::ColorBackend` (Task 2), `saturacion::color::saturation_matrix` (Task 1).
- Produces: decisión de continuar, ajustar el máximo, o detenerse; y si hace falta coalescencia en la Task 5.

- [ ] **Step 1: Escribir el ejemplo**

`examples/probe.rs`:

```rust
//! Prueba de riesgo: aplica una saturación y espera Enter para restaurar.
//! Uso: cargo run --example probe -- 300

use std::io::{self, BufRead};
use std::process::ExitCode;
use std::time::Instant;

use saturacion::backend::ColorBackend;
use saturacion::color::saturation_matrix;
use saturacion::igcl::IgclBackend;

fn main() -> ExitCode {
    let percent: f64 = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(300.0);

    let mut backend = match IgclBackend::new() {
        Ok(backend) => backend,
        Err(e) => {
            eprintln!("No se pudo iniciar: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("Pantallas con bloque de matriz: {}", backend.output_count());

    let start = Instant::now();
    if let Err(e) = backend.apply_matrix(&saturation_matrix(percent / 100.0)) {
        eprintln!("{e}");
        return ExitCode::FAILURE;
    }
    println!("Aplicado {percent}% en {:?}", start.elapsed());

    println!("Enter para restaurar la imagen normal...");
    let _ = io::stdin().lock().lines().next();

    match backend.restore_default() {
        Ok(()) => {
            println!("Restaurado.");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
```

- [ ] **Step 2: Compilar**

Run: `cargo build --example probe`
Expected: `Finished` sin errores.

- [ ] **Step 3: Comprobación 1 — la placa reporta el bloque**

Con una imagen colorida en pantalla (por ejemplo una foto), correr:

Run: `cargo run --example probe -- 125`
Expected: `Pantallas con bloque de matriz: 1` (o más) y `Aplicado 125% en …`. Los colores se ven algo más vivos. Enter restaura.

Si imprime `No se pudo iniciar: …`: **detenerse** y reportar el mensaje exacto. El plan B (perfil MHC2) requiere volver al diseño.

- [ ] **Step 4: Comprobación 2 — el driver no recorta y los grises siguen grises**

Run: `cargo run --example probe -- 300`
Expected:
- La imagen se ve claramente más saturada que con 125.
- Las zonas blancas, grises y negras **no toman color**. Si se tiñen, la convención fila/columna está invertida: detenerse y reportar.

Si 300 se ve igual que 125, el driver recorta. Probar 200, 175 y 150 para encontrar el mayor valor que todavía se distingue del anterior; ese pasa a ser `MAX_PERCENT` y el máximo del slider, siempre que se vea más saturado que el panel de Intel al máximo. Si no lo supera: **detenerse**.

- [ ] **Step 5: Comprobación 3 — se mantiene en pantalla completa exclusiva**

1. Correr `cargo run --example probe -- 300` y **no** apretar Enter.
2. Abrir un juego en pantalla completa exclusiva.
3. Mirar si los colores siguen saturados dentro del juego.
4. Salir del juego y apretar Enter en la terminal.

Expected: el efecto se ve dentro del juego. Si no: **detenerse** y reportar.

- [ ] **Step 6: Comprobación 4 — tiempo de escritura**

Anotar el tiempo de `Aplicado … en …` de tres corridas. Si alguna supera **15 ms**, la Task 5 debe usar la variante con coalescencia (Step 4b). Si todas quedan por debajo, usa la variante directa (Step 4a).

- [ ] **Step 7: Registrar resultados y commit**

Agregar al final de `docs/specs/2026-11-13-slider-saturacion-design.md` una sección con los valores observados:

```markdown
## Resultado de la prueba de riesgo

- Fecha:
- Pantallas con bloque de matriz:
- 300 % se distingue de 125 % (sí/no) y máximo utilizable:
- Grises sin teñir (sí/no):
- Se mantiene en pantalla completa exclusiva (sí/no), juego usado:
- Tiempo de escritura (tres corridas):
```

Completar cada línea con lo observado; ninguna queda vacía.

```bash
git add examples/probe.rs docs/specs/2026-11-13-slider-saturacion-design.md
git commit -m "feat: add IGCL probe and record risk test results"
```

---

### Task 5: Ventana Slint y arranque

**Files:**
- Create: `build.rs`
- Create: `ui/app.slint`
- Create: `src/main.rs`
- Modify: `Cargo.toml`

**Interfaces:**
- Consumes: `saturacion::controller::Controller::{new, set_saturation, shutdown}` (Task 2), `saturacion::igcl::IgclBackend::new` (Task 3), `BackendError: Display` (Task 2).
- Produces: binario `saturacion.exe`. Componente Slint `AppWindow` con:
  - `in-out property <float> saturation` (inicial 100)
  - `in property <bool> controls-enabled` (inicial true)
  - `in property <string> error-text` (inicial vacío)
  - `callback saturation-changed(float)`

- [ ] **Step 1: Dependencias y build script**

`Cargo.toml` completo:

```toml
[package]
name = "saturacion"
version = "0.1.0"
edition = "2021"

[dependencies]
libloading = "0.8"
slint = "1"

[build-dependencies]
slint-build = "1"
```

`build.rs`:

```rust
fn main() {
    slint_build::compile("ui/app.slint").expect("no se pudo compilar ui/app.slint");
}
```

- [ ] **Step 2: Escribir la ventana**

`ui/app.slint`:

```slint
import { Slider } from "std-widgets.slint";

export component AppWindow inherits Window {
    title: "Saturación";
    width: 360px;
    height: 150px;

    in-out property <float> saturation: 100;
    in property <bool> controls-enabled: true;
    in property <string> error-text: "";
    callback saturation-changed(float);

    VerticalLayout {
        padding: 20px;
        spacing: 12px;

        HorizontalLayout {
            Text {
                text: "Saturación";
                vertical-alignment: center;
            }
            Text {
                text: "\{Math.round(root.saturation)}";
                horizontal-alignment: right;
                vertical-alignment: center;
            }
        }

        Slider {
            minimum: 0;
            maximum: 300;
            step: 1;
            enabled: root.controls-enabled;
            value <=> root.saturation;
            changed(value) => {
                root.saturation-changed(value);
            }
        }

        Text {
            text: root.error-text;
            color: #c0392b;
            wrap: word-wrap;
        }
    }
}
```

Si la Task 4 fijó un máximo menor a 300, usar ese valor en `maximum` y en `MAX_PERCENT` (`src/controller.rs`), y actualizar el `900 → 3.0` del test `value_above_maximum_is_clamped` al nuevo máximo.

- [ ] **Step 3: Verificar que la UI compila**

Crear `src/main.rs` mínimo para que el build script corra:

```rust
slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    AppWindow::new()?.run()
}
```

Run: `cargo build`
Expected: `Finished` sin errores (la primera compilación de Slint tarda varios minutos).

- [ ] **Step 4a: Arranque con escritura directa (si la Task 4 midió ≤ 15 ms)**

`src/main.rs` completo:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::rc::Rc;

use saturacion::controller::Controller;
use saturacion::igcl::IgclBackend;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    let controller = match IgclBackend::new() {
        Ok(backend) => Some(Rc::new(RefCell::new(Controller::new(backend)))),
        Err(e) => {
            ui.set_error_text(e.to_string().into());
            ui.set_controls_enabled(false);
            None
        }
    };

    if let Some(controller) = controller.clone() {
        let ui_weak = ui.as_weak();
        ui.on_saturation_changed(move |value| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let percent = value.round().max(0.0) as u32;
            let message = match controller.borrow_mut().set_saturation(percent) {
                Ok(()) => String::new(),
                Err(e) => e.to_string(),
            };
            ui.set_error_text(message.into());
        });
    }

    let run_result = ui.run();

    // Restaurar aunque el bucle de eventos haya terminado con error.
    if let Some(controller) = controller {
        let _ = controller.borrow_mut().shutdown();
    }
    run_result
}
```

- [ ] **Step 4b: Arranque con coalescencia (solo si la Task 4 midió > 15 ms)**

Reemplaza al Step 4a. Guarda el último valor pedido y lo aplica como mucho una vez cada 16 ms. `src/main.rs` completo:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use saturacion::controller::Controller;
use saturacion::igcl::IgclBackend;
use slint::{Timer, TimerMode};

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    let controller = match IgclBackend::new() {
        Ok(backend) => Some(Rc::new(RefCell::new(Controller::new(backend)))),
        Err(e) => {
            ui.set_error_text(e.to_string().into());
            ui.set_controls_enabled(false);
            None
        }
    };

    // Último valor pedido que todavía no se escribió en la placa.
    let pending: Rc<Cell<Option<u32>>> = Rc::new(Cell::new(None));
    let timer = Timer::default();

    if let Some(controller) = controller.clone() {
        let pending_for_slider = Rc::clone(&pending);
        ui.on_saturation_changed(move |value| {
            pending_for_slider.set(Some(value.round().max(0.0) as u32));
        });

        let pending_for_timer = Rc::clone(&pending);
        let ui_weak = ui.as_weak();
        timer.start(TimerMode::Repeated, Duration::from_millis(16), move || {
            let Some(percent) = pending_for_timer.take() else { return };
            let Some(ui) = ui_weak.upgrade() else { return };
            let message = match controller.borrow_mut().set_saturation(percent) {
                Ok(()) => String::new(),
                Err(e) => e.to_string(),
            };
            ui.set_error_text(message.into());
        });
    }

    let run_result = ui.run();
    timer.stop();

    // Aplicar el último valor pendiente no hace falta: se restaura de todos modos.
    if let Some(controller) = controller {
        let _ = controller.borrow_mut().shutdown();
    }
    run_result
}
```

- [ ] **Step 5: Compilar y correr todos los tests**

Run: `cargo build`
Expected: `Finished` sin errores ni advertencias en `src/`.

Run: `cargo test`
Expected: `test result: ok. 10 passed; 0 failed`.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock build.rs ui/app.slint src/main.rs
git commit -m "feat: add Slint window with saturation slider"
```

---

### Task 6: Verificación de aceptación

**Necesita a una persona frente a la pantalla.**

**Files:** ninguno, salvo correcciones que surjan.

**Interfaces:**
- Consumes: el binario de la Task 5.

- [ ] **Step 1: Abrir la app**

Run: `cargo run --release`
Expected: ventana "Saturación" con el slider en 100, el número 100 y sin mensaje de error. La imagen de la pantalla no cambió al abrir.

- [ ] **Step 2: Mover el slider**

Con una imagen colorida visible, arrastrar el slider.

Expected:
- El color cambia **mientras** se arrastra, sin trabas.
- Por encima de 100 se ve más saturado; en 0, en escala de grises; el número acompaña.
- Con el slider al máximo se ve más saturado que con el panel de Intel al máximo.
- No aparece mensaje de error.

- [ ] **Step 3: Cerrar la app**

Cerrar la ventana con el slider en un valor alto.

Expected: la imagen vuelve a la normal al instante.

- [ ] **Step 4: Abrir y cerrar sin tocar**

Abrir la app y cerrarla sin mover el slider.

Expected: la imagen no cambia en ningún momento.

- [ ] **Step 5: Camino de error**

Este criterio no se puede reproducir en la máquina de referencia sin desinstalar el driver. Queda cubierto por el test `write_error_reaches_caller_with_user_message` y por la rama `Err` de `main`, que deshabilita el slider y muestra el mensaje. Dejar constancia de que **no se verificó a mano**.

- [ ] **Step 6: Commit de correcciones, si las hubo**

```bash
git add -A
git commit -m "fix: adjustments from acceptance check"
```

Si no hubo cambios, no se crea commit.
