# Contraste (historia 2) — Plan de implementación

**Goal:** Agregar a la app un slider de contraste de 50 a 200 que separa sombras y luces alrededor del gris medio, aplicado en la curva 1D de salida de la placa Intel, combinable con la saturación y restaurado al cerrar.

**Architecture:** Una función pura nueva calcula la curva de contraste. El trait `ColorBackend` suma `apply_tone_curve` y `supports_tone_curve` con implementaciones por defecto ("no soportado"). `Controller` suma `set_contrast`. `IgclBackend` guarda, por pantalla, la última curva 1D uniforme que reporta, la escribe muestreando la función y la restaura con `RESTORE_DEFAULT`; recuerda qué bloques escribió para restaurar solo esos. La ventana Slint suma un segundo slider.

**Tech Stack:** Rust stable (MSVC), Slint 1.18, libloading 0.8, IGCL (bindings ya generados en `src/igcl/bindings.rs`).

**Spec:** `docs/specs/2026-11-13-contraste-design.md`

## Global Constraints

- Solo Windows x64, toolchain `stable-x86_64-pc-windows-msvc`.
- El slider de contraste va de **50 a 200** en pasos enteros. **100 es la imagen sin modificar.**
- Curva: `y = clamp(0.5 + c · (x − 0.5), 0, 1)` con `c = valor / 100`, igual en los tres canales, sin linealizar.
- El slider de saturación sigue de 0 a 400 y no cambia de comportamiento.
- Mover solo el slider de saturación no escribe la curva; mover solo el de contraste no escribe la matriz.
- Al cerrar se restauran solo los bloques que la app escribió. La curva se restaura con `CTL_PIXTX_CONFIG_OPERTAION_TYPE_RESTORE_DEFAULT`, nunca escribiendo una curva identidad.
- Solo muestreo uniforme (`CTL_PIXTX_LUT_SAMPLING_TYPE_UNIFORM`); sin curva uniforme, el slider de contraste queda deshabilitado y la saturación sigue funcionando.
- `ControlLib.dll` se sigue cargando solo desde System32. No se usa `CTL_PIXTX_PIPE_SET_CONFIG_FLAG_PERSIST_ACROSS_POWER_EVENTS`.
- Todo `unsafe` y todo lo específico de IGCL vive en `src/igcl/`. `src/igcl/bindings.rs` no se edita a mano.
- Textos visibles al usuario en español. Identificadores y mensajes de commit en inglés, sin trailers `Co-Authored-By`.
- Mensaje nuevo, exacto: "Esta placa o este modo de pantalla no permite ajustar el contraste."
- Fuera de alcance: brillo, animaciones, persistencia, reset rápido, NVIDIA/AMD, muestreo no uniforme (HDR).

## Estructura de archivos

| Archivo | Cambio |
|---|---|
| `src/color.rs` | Suma `contrast_curve` |
| `src/backend.rs` | Suma `ToneCurveUnsupported`, `apply_tone_curve` y `supports_tone_curve` con defaults, tests del default |
| `src/controller.rs` | Suma `MIN_CONTRAST_PERCENT`, `MAX_CONTRAST_PERCENT`, `set_contrast`, `supports_contrast`, tests |
| `src/igcl/mod.rs` | Curva 1D por pantalla, escritura y restauración de la curva, seguimiento de bloques escritos, `block_summary`, `sample_curve` con tests |
| `examples/probe.rs` | Lista bloques y acepta contraste |
| `ui/app.slint` | Segundo slider y nota de contraste |
| `src/main.rs` | Conecta el slider de contraste |

---

### Task 1: Curva de contraste

**Files:**
- Modify: `src/color.rs`

**Interfaces:**
- Consumes: nada.
- Produces (en `saturacion::color`): `pub fn contrast_curve(c: f64, x: f64) -> f64` — `c = 1.0` es identidad; resultado siempre en `[0, 1]`.

- [ ] **Step 1: Escribir los tests que fallan**

Agregar al final del módulo `tests` existente en `src/color.rs` (antes de su `}` final):

```rust
    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn contrast_one_is_identity() {
        for x in [0.0, 0.1, 0.25, 0.5, 0.9, 1.0] {
            assert!(close(contrast_curve(1.0, x), x), "x={x}");
        }
    }

    #[test]
    fn contrast_keeps_mid_gray_fixed() {
        for c in [0.5, 1.0, 1.5, 2.0] {
            assert!(close(contrast_curve(c, 0.5), 0.5), "c={c}");
        }
    }

    #[test]
    fn contrast_curve_never_decreases() {
        for c in [0.5, 1.0, 1.5, 2.0] {
            let mut previous = contrast_curve(c, 0.0);
            for i in 1..=100 {
                let y = contrast_curve(c, f64::from(i) / 100.0);
                assert!(y >= previous, "c={c}, i={i}");
                previous = y;
            }
        }
    }

    #[test]
    fn contrast_output_stays_in_unit_range_and_clips() {
        for c in [0.5, 2.0, 4.0] {
            for i in 0..=100 {
                let y = contrast_curve(c, f64::from(i) / 100.0);
                assert!((0.0..=1.0).contains(&y), "c={c}, i={i}, y={y}");
            }
        }
        assert_eq!(contrast_curve(2.0, 0.0), 0.0);
        assert_eq!(contrast_curve(2.0, 1.0), 1.0);
        assert_eq!(contrast_curve(2.0, 0.1), 0.0);
    }

    #[test]
    fn half_contrast_flattens_toward_mid_gray() {
        assert!(close(contrast_curve(0.5, 0.0), 0.25));
        assert!(close(contrast_curve(0.5, 1.0), 0.75));
    }
```

- [ ] **Step 2: Correr los tests y ver que fallan**

Run: `cargo test --lib color`
Expected: error de compilación `cannot find function 'contrast_curve' in this scope`.

- [ ] **Step 3: Implementar**

En `src/color.rs`, después de `saturation_matrix` y antes de `#[cfg(test)]`:

```rust
/// Curva de contraste alrededor del gris medio: `y = clamp(0.5 + c·(x − 0.5), 0, 1)`.
/// `c = 1.0` deja la imagen igual; `> 1.0` oscurece sombras y aclara luces; `< 1.0` aplana.
pub fn contrast_curve(c: f64, x: f64) -> f64 {
    (0.5 + c * (x - 0.5)).clamp(0.0, 1.0)
}
```

Cambiar también la primera línea del archivo a:

```rust
//! Ajustes de color: matriz de saturación y curva de contraste. Funciones puras, sin dependencias de placa ni de UI.
```

- [ ] **Step 4: Correr los tests y ver que pasan**

Run: `cargo test --lib color`
Expected: `test result: ok. 9 passed; 0 failed`.

Run: `cargo test`
Expected: `test result: ok. 18 passed; 0 failed`, sin advertencias.

- [ ] **Step 5: Commit**

```bash
git add src/color.rs
git commit -m "feat: add contrast curve"
```

---

### Task 2: Curva de tono en el trait y contraste en el controlador

**Files:**
- Modify: `src/backend.rs`
- Modify: `src/controller.rs`

**Interfaces:**
- Consumes: `saturacion::color::contrast_curve(c: f64, x: f64) -> f64` (Task 1).
- Produces (en `saturacion::backend`):
  - Variante nueva `BackendError::ToneCurveUnsupported`, con `Display` = "Esta placa o este modo de pantalla no permite ajustar el contraste."
  - En `trait ColorBackend`: `fn apply_tone_curve(&mut self, curve: &dyn Fn(f64) -> f64) -> Result<(), BackendError>` (default: `Err(ToneCurveUnsupported)`) y `fn supports_tone_curve(&self) -> bool` (default: `false`).
- Produces (en `saturacion::controller`):
  - `pub const MIN_CONTRAST_PERCENT: u32 = 50;` y `pub const MAX_CONTRAST_PERCENT: u32 = 200;`
  - `pub fn set_contrast(&mut self, percent: u32) -> Result<(), BackendError>` — recorta a 50–200.
  - `pub fn supports_contrast(&self) -> bool`

- [ ] **Step 1: Tests del trait que fallan**

Agregar al final de `src/backend.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Backend que solo implementa los métodos obligatorios.
    struct MatrixOnly;

    impl ColorBackend for MatrixOnly {
        fn apply_matrix(&mut self, _matrix: &Matrix3) -> Result<(), BackendError> {
            Ok(())
        }

        fn restore_default(&mut self) -> Result<(), BackendError> {
            Ok(())
        }
    }

    #[test]
    fn backend_without_tone_curve_reports_it_as_unsupported() {
        let mut backend = MatrixOnly;
        assert!(!backend.supports_tone_curve());
        assert_eq!(
            backend.apply_tone_curve(&|x| x),
            Err(BackendError::ToneCurveUnsupported)
        );
    }

    #[test]
    fn tone_curve_unsupported_has_user_message() {
        assert_eq!(
            BackendError::ToneCurveUnsupported.to_string(),
            "Esta placa o este modo de pantalla no permite ajustar el contraste."
        );
    }
}
```

- [ ] **Step 2: Ver que fallan**

Run: `cargo test --lib backend`
Expected: error de compilación (`no variant ... ToneCurveUnsupported`, `no method named supports_tone_curve`).

- [ ] **Step 3: Implementar el trait**

En `src/backend.rs`, agregar la variante al enum (después de `MatrixUnsupported`):

```rust
    /// Ninguna pantalla reporta una curva 1D con muestreo uniforme.
    ToneCurveUnsupported,
```

Agregar el brazo en `Display` (después del de `MatrixUnsupported`):

```rust
            Self::ToneCurveUnsupported => {
                write!(f, "Esta placa o este modo de pantalla no permite ajustar el contraste.")
            }
```

Reemplazar el trait completo por:

```rust
pub trait ColorBackend {
    /// Escribe la matriz en todas las pantallas que maneja el backend.
    fn apply_matrix(&mut self, matrix: &Matrix3) -> Result<(), BackendError>;

    /// Escribe una curva de tono en las pantallas que la admiten. `curve` recibe y
    /// devuelve valores entre 0 y 1; cada backend elige cuántas muestras tomar.
    fn apply_tone_curve(&mut self, _curve: &dyn Fn(f64) -> f64) -> Result<(), BackendError> {
        Err(BackendError::ToneCurveUnsupported)
    }

    /// Si alguna pantalla admite curva de tono.
    fn supports_tone_curve(&self) -> bool {
        false
    }

    /// Devuelve las pantallas a la imagen sin modificar, restaurando solo los
    /// bloques que el backend escribió.
    fn restore_default(&mut self) -> Result<(), BackendError>;
}
```

- [ ] **Step 4: Ver que pasan**

Run: `cargo test --lib backend`
Expected: `test result: ok. 2 passed`.

- [ ] **Step 5: Tests del controlador que fallan**

En `src/controller.rs`, dentro de `mod tests`:

Reemplazar el enum `Call`, el struct `FakeBackend`, su `impl` y la función `controller` por:

```rust
    /// Puntos donde el backend falso muestrea la curva que recibe.
    const SAMPLE_POINTS: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

    #[derive(Debug, PartialEq)]
    enum Call {
        Apply(Matrix3),
        Curve(Vec<f64>),
        Restore,
    }

    struct FakeBackend {
        calls: Rc<RefCell<Vec<Call>>>,
        fail_with: Option<BackendError>,
        curve_supported: bool,
    }

    impl FakeBackend {
        fn result(&self) -> Result<(), BackendError> {
            match &self.fail_with {
                Some(e) => Err(e.clone()),
                None => Ok(()),
            }
        }
    }

    impl ColorBackend for FakeBackend {
        fn apply_matrix(&mut self, matrix: &Matrix3) -> Result<(), BackendError> {
            self.calls.borrow_mut().push(Call::Apply(*matrix));
            self.result()
        }

        fn apply_tone_curve(&mut self, curve: &dyn Fn(f64) -> f64) -> Result<(), BackendError> {
            let samples = SAMPLE_POINTS.iter().map(|&x| curve(x)).collect();
            self.calls.borrow_mut().push(Call::Curve(samples));
            self.result()
        }

        fn supports_tone_curve(&self) -> bool {
            self.curve_supported
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
        let backend = FakeBackend { calls: Rc::clone(&calls), fail_with, curve_supported: true };
        (Controller::new(backend), calls)
    }
```

Agregar al final de `mod tests` (antes de su `}`):

```rust
    #[test]
    fn contrast_value_becomes_tone_curve_without_touching_the_matrix() {
        let (mut c, calls) = controller(None);
        c.set_contrast(150).unwrap();
        assert_eq!(*calls.borrow(), vec![Call::Curve(vec![0.0, 0.125, 0.5, 0.875, 1.0])]);
    }

    #[test]
    fn contrast_outside_range_is_clamped() {
        let (mut c, calls) = controller(None);
        c.set_contrast(900).unwrap();
        c.set_contrast(10).unwrap();
        assert_eq!(
            *calls.borrow(),
            vec![
                Call::Curve(vec![0.0, 0.0, 0.5, 1.0, 1.0]),
                Call::Curve(vec![0.25, 0.375, 0.5, 0.625, 0.75]),
            ]
        );
    }

    #[test]
    fn contrast_write_error_reaches_caller() {
        let (mut c, _) = controller(Some(BackendError::ApplyFailed(0x4000_0001)));
        let err = c.set_contrast(150).unwrap_err();
        assert_eq!(err.to_string(), "No se pudo aplicar el ajuste (IGCL 0x40000001).");
    }

    #[test]
    fn shutdown_restores_after_a_contrast_write() {
        let (mut c, calls) = controller(None);
        c.set_contrast(150).unwrap();
        c.shutdown().unwrap();
        assert_eq!(calls.borrow().last(), Some(&Call::Restore));
    }

    #[test]
    fn supports_contrast_reflects_backend() {
        let (c, _) = controller(None);
        assert!(c.supports_contrast());

        let calls = Rc::new(RefCell::new(Vec::new()));
        let backend = FakeBackend { calls, fail_with: None, curve_supported: false };
        assert!(!Controller::new(backend).supports_contrast());
    }
```

- [ ] **Step 6: Ver que fallan**

Run: `cargo test --lib controller`
Expected: error de compilación `no method named 'set_contrast'` / `supports_contrast`.

- [ ] **Step 7: Implementar en el controlador**

En `src/controller.rs`:

Cambiar el import de color a:

```rust
use crate::color::{contrast_curve, saturation_matrix};
```

Reemplazar la constante y su comentario por:

```rust
/// Máximo del slider de saturación, en porcentaje. 100 es la imagen sin modificar.
pub const MAX_PERCENT: u32 = 400;

/// Rango del slider de contraste, en porcentaje. 100 es la imagen sin modificar.
pub const MIN_CONTRAST_PERCENT: u32 = 50;
pub const MAX_CONTRAST_PERCENT: u32 = 200;
```

Agregar dentro de `impl<B: ColorBackend> Controller<B>`, después de `set_saturation`:

```rust
    /// Aplica el contraste indicado en porcentaje (100 = sin cambios), limitado a 50–200.
    pub fn set_contrast(&mut self, percent: u32) -> Result<(), BackendError> {
        let c = f64::from(percent.clamp(MIN_CONTRAST_PERCENT, MAX_CONTRAST_PERCENT)) / 100.0;
        self.wrote = true;
        self.backend.apply_tone_curve(&|x| contrast_curve(c, x))
    }

    /// Si la placa permite ajustar el contraste.
    pub fn supports_contrast(&self) -> bool {
        self.backend.supports_tone_curve()
    }
```

- [ ] **Step 8: Ver que pasan**

Run: `cargo test`
Expected: `test result: ok. 25 passed; 0 failed`, sin advertencias. `cargo build` también sin advertencias (`IgclBackend` compila usando los defaults del trait).

- [ ] **Step 9: Commit**

```bash
git add src/backend.rs src/controller.rs
git commit -m "feat: add tone curve to backend trait and contrast to controller"
```

---

### Task 3: Curva 1D en el backend de Intel

Sin tests de hardware. Sí hay tests unitarios de la función pura `sample_curve`.

**Files:**
- Modify: `src/igcl/mod.rs`

**Interfaces:**
- Consumes: `ColorBackend` con `apply_tone_curve`/`supports_tone_curve` y `BackendError::ToneCurveUnsupported` (Task 2); `Matrix3`, `IDENTITY` (`saturacion::color`).
- Produces (en `saturacion::igcl`):
  - `IgclBackend` implementa `apply_tone_curve` y `supports_tone_curve`; `restore_default` restaura solo lo escrito.
  - `pub fn block_summary(&self) -> Vec<String>` — una línea por bloque de cada pantalla, para diagnóstico.
  - `pub fn output_count(&self) -> usize` sin cambios.

**Referencia de la API (verificada contra `vendor/igcl/igcl_api.h` y `src/igcl/bindings.rs`):**
- `ctl_pixtx_block_config_t.Config` es una unión; para bloques `CTL_PIXTX_BLOCK_TYPE_1D_LUT` el miembro activo es `OneDLutConfig: ctl_pixtx_1dlut_config_t` con campos `Size: u32`, `Version: u8`, `SamplingType` (`CTL_PIXTX_LUT_SAMPLING_TYPE_UNIFORM = 0`), `NumSamplesPerChannel: u32`, `NumChannels: u32`, `pSampleValues: *mut f64` (canal por canal: R, luego G, luego B), `pSamplePositions: *mut f64` (puede ser nulo con muestreo uniforme).
- La consulta de capacidades informa `SamplingType`, `NumSamplesPerChannel` y `NumChannels` de cada curva.
- El ejemplo oficial de Intel usa la **última** curva 1D reportada como curva de salida (GLUT).
- Restaurar: `ctlPixelTransformationSetConfig` con `OpertaionType = CTL_PIXTX_CONFIG_OPERTAION_TYPE_RESTORE_DEFAULT` y el bloque, sin muestras.
- Leer un campo de una unión es `unsafe`; asignar campos `Copy` de una unión es seguro.

- [ ] **Step 1: Tests de `sample_curve` que fallan**

Agregar al final de `src/igcl/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_curve_takes_uniform_samples_including_both_ends() {
        assert_eq!(sample_curve(&|x| x, 5, 1), vec![0.0, 0.25, 0.5, 0.75, 1.0]);
    }

    #[test]
    fn sample_curve_repeats_the_curve_for_each_channel() {
        assert_eq!(
            sample_curve(&|x| 1.0 - x, 3, 3),
            vec![1.0, 0.5, 0.0, 1.0, 0.5, 0.0, 1.0, 0.5, 0.0]
        );
    }
}
```

- [ ] **Step 2: Ver que fallan**

Run: `cargo test --lib igcl`
Expected: error de compilación `cannot find function 'sample_curve'`.

- [ ] **Step 3: Reemplazar `src/igcl/mod.rs` completo**

El contenido completo nuevo (la carga de la DLL, `ctlInit`, `adapters`, `displays`, `is_active` y `Drop` no cambian; se separa la consulta de capacidades de la selección de bloques, se agrega la curva y el seguimiento de bloques escritos):

```rust
//! Backend de Intel: escribe la matriz de color 3×3 y la curva 1D de salida del
//! pipeline de pantalla de la placa usando IGCL (`ControlLib.dll`, instalada por el driver).

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

/// Una pantalla activa con su bloque de matriz y, si la tiene, su curva 1D de salida.
struct Output {
    handle: ctl_display_output_handle_t,
    matrix: ctl_pixtx_block_config_t,
    curve: Option<ctl_pixtx_block_config_t>,
}

pub struct IgclBackend {
    lib: ControlLib,
    api: ctl_api_handle_t,
    outputs: Vec<Output>,
    /// Bloques escritos desde la última restauración; solo esos se restauran.
    wrote_matrix: bool,
    wrote_curve: bool,
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
        let mut backend = Self {
            lib,
            api,
            outputs: Vec::new(),
            wrote_matrix: false,
            wrote_curve: false,
        };

        let mut saw_active_display = false;
        for adapter in backend.adapters()? {
            for display in backend.displays(adapter) {
                if !backend.is_active(display) {
                    continue;
                }
                saw_active_display = true;
                let Some(blocks) = backend.capability_blocks(display) else { continue };
                if let Some(matrix) = matrix_block(&blocks) {
                    let curve = curve_block(&blocks);
                    backend.outputs.push(Output { handle: display, matrix, curve });
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

    /// Una línea por bloque de color de cada pantalla, para diagnóstico.
    pub fn block_summary(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for (i, output) in self.outputs.iter().enumerate() {
            match self.capability_blocks(output.handle) {
                Some(blocks) => {
                    for block in &blocks {
                        lines.push(format!("Pantalla {i}: {}", describe_block(block)));
                    }
                }
                None => lines.push(format!("Pantalla {i}: no se pudieron leer los bloques")),
            }
        }
        lines
    }

    fn adapters(&self) -> Result<Vec<ctl_device_adapter_handle_t>, BackendError> {
        let mut count = 0u32;
        // SAFETY: con puntero nulo IGCL solo escribe la cantidad en `count`.
        let r = unsafe { self.lib.ctlEnumerateDevices(self.api, &mut count, ptr::null_mut()) };
        if r != CTL_RESULT_SUCCESS {
            return Err(BackendError::DriverNotFound);
        }
        if count == 0 {
            return Ok(Vec::new());
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

    /// Bloques de color que reporta la pantalla en sus capacidades.
    fn capability_blocks(
        &self,
        display: ctl_display_output_handle_t,
    ) -> Option<Vec<ctl_pixtx_block_config_t>> {
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
        for b in &mut blocks {
            b.Size = size_of::<ctl_pixtx_block_config_t>() as u32;
        }
        caps.pBlockConfigs = blocks.as_mut_ptr();
        // SAFETY: `blocks` tiene lugar para `NumBlocks` elementos.
        let r = unsafe { self.lib.ctlPixelTransformationGetConfig(display, &mut caps) };
        if r != CTL_RESULT_SUCCESS {
            return None;
        }
        blocks.truncate(caps.NumBlocks as usize);
        Some(blocks)
    }

    /// Envía un bloque a la placa con la operación indicada.
    fn set_block(
        &self,
        display: ctl_display_output_handle_t,
        block: &mut ctl_pixtx_block_config_t,
        operation: ctl_pixtx_config_opertaion_type_t,
    ) -> Result<(), BackendError> {
        block.Size = size_of::<ctl_pixtx_block_config_t>() as u32;
        // SAFETY: struct C plano; todo-ceros es válido.
        let mut args: ctl_pixtx_pipe_set_config_t = unsafe { zeroed() };
        args.Size = size_of::<ctl_pixtx_pipe_set_config_t>() as u32;
        args.OpertaionType = operation;
        args.Flags = 0;
        args.NumBlocks = 1;
        args.pBlockConfigs = block;
        // SAFETY: `args` y `block` (y lo que este apunte) viven hasta después de la llamada.
        let r = unsafe { self.lib.ctlPixelTransformationSetConfig(display, &mut args) };
        if r == CTL_RESULT_SUCCESS {
            Ok(())
        } else {
            Err(BackendError::ApplyFailed(r as u32))
        }
    }

    /// Escribe `matrix` en el bloque de matriz de cada pantalla.
    /// Intenta todas aunque alguna falle y devuelve el primer error.
    fn write_matrix(&mut self, matrix: &Matrix3) -> Result<(), BackendError> {
        self.wrote_matrix = true;
        let mut first_error = None;
        for output in &self.outputs {
            // Copia del bloque de capacidades: conserva BlockId y BlockType.
            let mut block = output.matrix;
            // El bloque es de tipo matriz, así que `MatrixConfig` es el miembro
            // activo de la unión (asignar campos `Copy` de una unión es seguro).
            block.Config.MatrixConfig.Size = size_of::<ctl_pixtx_matrix_config_t>() as u32;
            block.Config.MatrixConfig.Matrix = *matrix;
            block.Config.MatrixConfig.PreOffsets = [0.0; 3];
            block.Config.MatrixConfig.PostOffsets = [0.0; 3];
            if let Err(e) =
                self.set_block(output.handle, &mut block, CTL_PIXTX_CONFIG_OPERTAION_TYPE_SET_CUSTOM)
            {
                first_error.get_or_insert(e);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Escribe `curve` en la curva 1D de salida de cada pantalla que la tiene.
    /// Intenta todas aunque alguna falle y devuelve el primer error.
    fn write_curve(&mut self, curve: &dyn Fn(f64) -> f64) -> Result<(), BackendError> {
        if !self.supports_tone_curve() {
            return Err(BackendError::ToneCurveUnsupported);
        }
        self.wrote_curve = true;
        let mut first_error = None;
        for output in &self.outputs {
            let Some(mut block) = output.curve else { continue };
            // SAFETY: el bloque es una curva 1D, así que `OneDLutConfig` es el miembro
            // activo de la unión; se leen los tamaños que informó el driver.
            let lut = unsafe { block.Config.OneDLutConfig };
            let mut values = sample_curve(
                curve,
                lut.NumSamplesPerChannel as usize,
                lut.NumChannels as usize,
            );
            block.Config.OneDLutConfig.Size = size_of::<ctl_pixtx_1dlut_config_t>() as u32;
            block.Config.OneDLutConfig.pSampleValues = values.as_mut_ptr();
            block.Config.OneDLutConfig.pSamplePositions = ptr::null_mut();
            // `values` vive hasta el final de esta iteración, después de la llamada.
            if let Err(e) =
                self.set_block(output.handle, &mut block, CTL_PIXTX_CONFIG_OPERTAION_TYPE_SET_CUSTOM)
            {
                first_error.get_or_insert(e);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Pide al driver que vuelva la curva 1D de salida de cada pantalla a su valor por defecto.
    fn restore_curves(&self) -> Result<(), BackendError> {
        let mut first_error = None;
        for output in &self.outputs {
            let Some(mut block) = output.curve else { continue };
            if let Err(e) = self.set_block(
                output.handle,
                &mut block,
                CTL_PIXTX_CONFIG_OPERTAION_TYPE_RESTORE_DEFAULT,
            ) {
                first_error.get_or_insert(e);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

impl ColorBackend for IgclBackend {
    fn apply_matrix(&mut self, matrix: &Matrix3) -> Result<(), BackendError> {
        self.write_matrix(matrix)
    }

    fn apply_tone_curve(&mut self, curve: &dyn Fn(f64) -> f64) -> Result<(), BackendError> {
        self.write_curve(curve)
    }

    fn supports_tone_curve(&self) -> bool {
        self.outputs.iter().any(|o| o.curve.is_some())
    }

    fn restore_default(&mut self) -> Result<(), BackendError> {
        let mut first_error = None;
        if self.wrote_matrix {
            if let Err(e) = self.write_matrix(&IDENTITY) {
                first_error.get_or_insert(e);
            }
            self.wrote_matrix = false;
        }
        if self.wrote_curve {
            if let Err(e) = self.restore_curves() {
                first_error.get_or_insert(e);
            }
            self.wrote_curve = false;
        }
        first_error.map_or(Ok(()), Err)
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

/// Bloque de matriz 3×3. Prefiere el de matriz sola; si no hay, el de matriz con offsets.
fn matrix_block(blocks: &[ctl_pixtx_block_config_t]) -> Option<ctl_pixtx_block_config_t> {
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

/// Última curva 1D reportada (la de salida, como en el ejemplo de Intel), si tiene
/// muestreo uniforme y tamaños utilizables.
fn curve_block(blocks: &[ctl_pixtx_block_config_t]) -> Option<ctl_pixtx_block_config_t> {
    let block = blocks
        .iter()
        .rev()
        .find(|b| b.BlockType == CTL_PIXTX_BLOCK_TYPE_1D_LUT)?;
    // SAFETY: el bloque es una curva 1D, así que `OneDLutConfig` es el miembro activo.
    let lut = unsafe { block.Config.OneDLutConfig };
    let usable = lut.SamplingType == CTL_PIXTX_LUT_SAMPLING_TYPE_UNIFORM
        && lut.NumSamplesPerChannel >= 2
        && lut.NumChannels >= 1;
    usable.then_some(*block)
}

/// Texto legible de un bloque de capacidades.
fn describe_block(block: &ctl_pixtx_block_config_t) -> String {
    let id = block.BlockId;
    match block.BlockType {
        CTL_PIXTX_BLOCK_TYPE_1D_LUT => {
            // SAFETY: el bloque es una curva 1D, así que `OneDLutConfig` es el miembro activo.
            let lut = unsafe { block.Config.OneDLutConfig };
            let sampling = if lut.SamplingType == CTL_PIXTX_LUT_SAMPLING_TYPE_UNIFORM {
                "uniforme"
            } else {
                "no uniforme"
            };
            format!(
                "bloque {id}: curva 1D, muestreo {sampling}, {} muestras x {} canales",
                lut.NumSamplesPerChannel, lut.NumChannels
            )
        }
        CTL_PIXTX_BLOCK_TYPE_3D_LUT => format!("bloque {id}: LUT 3D"),
        CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX => format!("bloque {id}: matriz 3x3"),
        CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS => {
            format!("bloque {id}: matriz 3x3 con desplazamientos")
        }
        other => format!("bloque {id}: tipo {other}"),
    }
}

/// Muestrea `curve` en `samples` puntos uniformes de 0 a 1 y repite el resultado
/// para cada canal, en el orden que espera IGCL (R, luego G, luego B).
fn sample_curve(curve: &dyn Fn(f64) -> f64, samples: usize, channels: usize) -> Vec<f64> {
    let last = (samples - 1) as f64;
    let one_channel: Vec<f64> = (0..samples).map(|i| curve(i as f64 / last)).collect();
    one_channel.repeat(channels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_curve_takes_uniform_samples_including_both_ends() {
        assert_eq!(sample_curve(&|x| x, 5, 1), vec![0.0, 0.25, 0.5, 0.75, 1.0]);
    }

    #[test]
    fn sample_curve_repeats_the_curve_for_each_channel() {
        assert_eq!(
            sample_curve(&|x| 1.0 - x, 3, 3),
            vec![1.0, 0.5, 0.0, 1.0, 0.5, 0.0, 1.0, 0.5, 0.0]
        );
    }
}
```

Notas para adaptar si el compilador lo exige, sin cambiar comportamiento:
- Si `ctl_pixtx_config_opertaion_type_t` no existe con ese nombre en `bindings.rs`, usar el tipo que tenga `OpertaionType` en `ctl_pixtx_pipe_set_config_t` (buscarlo con `Select-String`).
- Si los `const` del tipo de bloque no se pueden usar como patrones en el `match`, reemplazar el `match` por una cadena de `if block.BlockType == ...`.
- `sample_curve` solo se llama con `samples >= 2` (lo garantiza `curve_block`).

- [ ] **Step 4: Compilar y correr los tests**

Run: `cargo build`
Expected: `Finished` sin advertencias en `src/`.

Run: `cargo test`
Expected: `test result: ok. 27 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add src/igcl/mod.rs
git commit -m "feat: write and restore the output tone curve in the Intel backend"
```

---

### Task 4: Prueba de riesgo del contraste

**Necesita a una persona frente a la pantalla para los Steps 3 a 8.** Es la compuerta del mecanismo: si falla, no se toca la ventana.

**Files:**
- Modify: `examples/probe.rs`
- Modify: `docs/specs/2026-11-13-contraste-design.md` (agregar resultados)

**Interfaces:**
- Consumes: `IgclBackend::{new, output_count, block_summary}` (Task 3), `ColorBackend::{apply_matrix, apply_tone_curve, supports_tone_curve, restore_default}`, `saturacion::color::{saturation_matrix, contrast_curve}`.
- Produces: decisión de continuar o volver al plan B; si hace falta coalescencia en la Task 5.

- [ ] **Step 1: Reemplazar `examples/probe.rs` completo**

```rust
//! Prueba de riesgo: lista los bloques de color, aplica saturación y, opcionalmente,
//! contraste, y espera Enter para restaurar.
//! Uso: cargo run --example probe -- <saturación> [contraste]
//! Ejemplos: `-- 300` (saturación), `-- 100 200` (contraste), `-- 300 150` (ambos).

use std::io::{self, BufRead};
use std::process::ExitCode;
use std::time::Instant;

use saturacion::backend::ColorBackend;
use saturacion::color::{contrast_curve, saturation_matrix};
use saturacion::igcl::IgclBackend;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let saturation: f64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(300.0);
    let contrast: Option<f64> = args.next().and_then(|a| a.parse().ok());

    let mut backend = match IgclBackend::new() {
        Ok(backend) => backend,
        Err(e) => {
            eprintln!("No se pudo iniciar: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("Pantallas con bloque de matriz: {}", backend.output_count());
    for line in backend.block_summary() {
        println!("  {line}");
    }
    let curve_available = if backend.supports_tone_curve() { "sí" } else { "no" };
    println!("Curva 1D uniforme disponible: {curve_available}");

    let start = Instant::now();
    if let Err(e) = backend.apply_matrix(&saturation_matrix(saturation / 100.0)) {
        eprintln!("{e}");
        let _ = backend.restore_default();
        return ExitCode::FAILURE;
    }
    println!("Saturación {saturation}% aplicada en {:?}", start.elapsed());

    if let Some(contrast) = contrast {
        let c = contrast / 100.0;
        let start = Instant::now();
        if let Err(e) = backend.apply_tone_curve(&|x| contrast_curve(c, x)) {
            eprintln!("{e}");
            let _ = backend.restore_default();
            return ExitCode::FAILURE;
        }
        println!("Contraste {contrast}% aplicado en {:?}", start.elapsed());
    }

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

- [ ] **Step 2: Compilar y commit del ejemplo**

Run: `cargo build --example probe`
Expected: `Finished` sin advertencias.

```bash
git add examples/probe.rs
git commit -m "feat: list colour blocks and apply contrast in the probe"
```

- [ ] **Step 3: Comprobación 1 — la placa reporta la curva**

Run: `cargo run --example probe -- 100 100`
Expected: la lista de bloques incluye al menos una línea `curva 1D, muestreo uniforme, N muestras x M canales` y la línea `Curva 1D uniforme disponible: sí`. Enter restaura.

Si dice `no`: **detenerse** y reportar la lista de bloques completa (plan B: matriz con desplazamientos, requiere volver al diseño).

- [ ] **Step 4: Comprobación 2 — contraste visible con gris medio fijo**

Con una imagen que tenga sombras, luces y grises medios en pantalla:

Run: `cargo run --example probe -- 100 200`
Expected: sombras más oscuras y luces más claras; un gris medio (por ejemplo, `#808080`) se ve igual que antes. Enter restaura.

Run: `cargo run --example probe -- 100 50`
Expected: imagen más plana, negros grisáceos y blancos apagados. Enter restaura.

Si 200 se ve igual que 100: **detenerse** y reportar.

- [ ] **Step 5: Comprobación 3 — se mantiene en pantalla completa exclusiva**

1. Correr `cargo run --example probe -- 100 200` y **no** apretar Enter.
2. Abrir un juego en pantalla completa exclusiva (anotar cuál).
3. Mirar si el contraste sigue aumentado dentro del juego.
4. Salir del juego y apretar Enter.

Expected: el efecto se ve dentro del juego. Si no: **detenerse** y reportar.

- [ ] **Step 6: Comprobación 4 — combinado con saturación**

Run: `cargo run --example probe -- 300 150`
Expected: colores más saturados **y** más contraste a la vez. Enter restaura.

- [ ] **Step 7: Comprobación 5 — la restauración deja la imagen como estaba**

Después de un Enter en cualquiera de las corridas anteriores, la imagen se ve igual que antes de abrir la prueba.

Si el usuario usa la luz nocturna de Windows: activarla, correr `cargo run --example probe -- 100 200`, apretar Enter y comprobar que la luz nocturna sigue activa (tono cálido). Si se pierde y no vuelve en un minuto, **detenerse**: hay que cambiar la restauración a guardar la curva actual al abrir (ver la spec).

- [ ] **Step 8: Comprobación 6 — tiempo de escritura**

Anotar el tiempo de `Contraste … aplicado en …` de tres corridas. Si alguna supera **15 ms**, la Task 5 debe agregar coalescencia (reportarlo para ajustar la Task 5 antes de empezarla).

- [ ] **Step 9: Registrar resultados y commit**

Agregar al final de `docs/specs/2026-11-13-contraste-design.md`:

```markdown
## Resultado de la prueba de riesgo

- Fecha:
- Bloques reportados por pantalla:
- Curva 1D uniforme disponible (sí/no), muestras y canales:
- 200 con más contraste y gris medio fijo (sí/no); 50 más plano (sí/no):
- Se mantiene en pantalla completa exclusiva (sí/no), juego usado:
- Combinado con saturación (sí/no):
- Restauración deja la imagen como estaba (sí/no); luz nocturna (probada/no usada, resultado):
- Tiempo de escritura de la curva (tres corridas):
```

Completar cada línea con lo observado; ninguna queda vacía.

```bash
git add docs/specs/2026-11-13-contraste-design.md
git commit -m "docs: record contrast risk test results"
```

---

### Task 5: Slider de contraste en la ventana

**Files:**
- Modify: `ui/app.slint`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `Controller::{new, set_saturation, set_contrast, supports_contrast, shutdown}` (Task 2), `IgclBackend::new` (Task 3), `BackendError::ToneCurveUnsupported` (Task 2).
- Produces: componente `AppWindow` con, además de lo existente:
  - `in-out property <float> contrast` (inicial 100)
  - `in property <bool> contrast-enabled` (inicial true)
  - `in property <string> contrast-note` (inicial vacío)
  - `callback contrast-changed(float)`

- [ ] **Step 1: Reemplazar `ui/app.slint` completo**

```slint
import { Slider } from "std-widgets.slint";

export component AppWindow inherits Window {
    title: "Saturación";
    width: 360px;
    height: 240px;

    in-out property <float> saturation: 100;
    in-out property <float> contrast: 100;
    in property <bool> controls-enabled: true;
    in property <bool> contrast-enabled: true;
    in property <string> contrast-note: "";
    in property <string> error-text: "";
    callback saturation-changed(float);
    callback contrast-changed(float);

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
            maximum: 400;
            step: 1;
            enabled: root.controls-enabled;
            value <=> root.saturation;
            changed(value) => {
                root.saturation-changed(value);
            }
        }

        HorizontalLayout {
            Text {
                text: "Contraste";
                vertical-alignment: center;
            }
            Text {
                text: "\{Math.round(root.contrast)}";
                horizontal-alignment: right;
                vertical-alignment: center;
            }
        }

        Slider {
            minimum: 50;
            maximum: 200;
            step: 1;
            enabled: root.controls-enabled && root.contrast-enabled;
            value <=> root.contrast;
            changed(value) => {
                root.contrast-changed(value);
            }
        }

        Text {
            text: root.contrast-note;
            color: #7f8c8d;
            wrap: word-wrap;
        }

        Text {
            text: root.error-text;
            color: #c0392b;
            wrap: word-wrap;
        }
    }
}
```

- [ ] **Step 2: Reemplazar `src/main.rs` completo**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::rc::Rc;

use saturacion::backend::BackendError;
use saturacion::controller::Controller;
use saturacion::igcl::IgclBackend;

slint::include_modules!();

/// Muestra el resultado de una escritura: borra el error si salió bien.
fn show_result(ui: &AppWindow, result: Result<(), BackendError>) {
    let message = match result {
        Ok(()) => String::new(),
        Err(e) => e.to_string(),
    };
    ui.set_error_text(message.into());
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    let controller = match IgclBackend::new() {
        Ok(backend) => {
            let controller = Controller::new(backend);
            if !controller.supports_contrast() {
                ui.set_contrast_enabled(false);
                ui.set_contrast_note(BackendError::ToneCurveUnsupported.to_string().into());
            }
            Some(Rc::new(RefCell::new(controller)))
        }
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
            show_result(&ui, controller.borrow_mut().set_saturation(percent));
        });
    }

    if let Some(controller) = controller.clone() {
        let ui_weak = ui.as_weak();
        ui.on_contrast_changed(move |value| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let percent = value.round().max(0.0) as u32;
            show_result(&ui, controller.borrow_mut().set_contrast(percent));
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

Si la Task 4 midió más de 15 ms por escritura de curva, no implementar este Step tal cual: reportarlo para ajustar el plan con coalescencia.

- [ ] **Step 3: Compilar y correr los tests**

Run: `cargo build`
Expected: `Finished` sin advertencias en `src/` ni `ui/`.

Run: `cargo test`
Expected: `test result: ok. 27 passed; 0 failed`.

- [ ] **Step 4: Commit**

```bash
git add ui/app.slint src/main.rs
git commit -m "feat: add contrast slider to the window"
```

---

### Task 6: Verificación de aceptación

**Necesita a una persona frente a la pantalla.**

**Files:** ninguno, salvo el registro de resultados.

- [ ] **Step 1: Abrir la app**

Run: `cargo run --release`
Expected: ventana con dos sliders, Saturación en 100 y Contraste en 100, sin nota ni mensaje de error. La imagen no cambió al abrir.

- [ ] **Step 2: Mover el contraste**

Con una imagen con sombras, luces y grises medios, arrastrar el slider de contraste.

Expected: el contraste cambia mientras se arrastra; en 200 sombras más oscuras y luces más claras; en 50 imagen más plana; el gris medio no cambia; el número acompaña.

- [ ] **Step 3: Combinar**

Subir la saturación y el contraste.

Expected: se ven los dos efectos; mover uno no deshace el otro.

- [ ] **Step 4: Cerrar con ajustes altos**

Expected: la imagen vuelve a como estaba al instante.

- [ ] **Step 5: Abrir y cerrar sin tocar**

Expected: la imagen no cambia en ningún momento.

- [ ] **Step 6: Camino sin curva**

No se puede reproducir en la máquina de referencia. Queda cubierto por `backend_without_tone_curve_reports_it_as_unsupported`, `supports_contrast_reflects_backend` y la rama de `main` que deshabilita el slider de contraste y muestra la nota. Dejar constancia de que **no se verificó a mano**.

- [ ] **Step 7: Registrar y commit**

Agregar al final de `docs/specs/2026-11-13-contraste-design.md` una sección `## Resultado de la verificación de aceptación` con la fecha, el commit probado y lo observado en los Steps 1 a 5, más la constancia del Step 6.

```bash
git add docs/specs/2026-11-13-contraste-design.md
git commit -m "docs: record contrast acceptance results"
```
