# Bloque de matriz de la saturación — Plan de implementación

**Goal:** Sacar la saturación del CSC de salida de Intel: escribirla en la matriz con desplazamientos (CSC de la tubería) cuando la pantalla la reporta, y restaurar la matriz con `RESTORE_DEFAULT` en lugar de escribir la identidad.

**Architecture:** `matrix_block` (función pura en `src/igcl/mod.rs`) invierte su orden de preferencia. `IgclBackend` suma `restore_matrices`, análoga a `restore_curves`, y `restore_default` la usa. El probe muestra qué bloque usa la saturación. No cambian el trait `ColorBackend`, el controlador ni la ventana.

**Tech Stack:** Rust stable (MSVC), libloading 0.8, IGCL (bindings ya generados en `src/igcl/bindings.rs`).

**Spec:** `docs/specs/2026-11-13-bloque-matriz-saturacion-design.md`

## Global Constraints

- Solo Windows x64, toolchain `stable-x86_64-pc-windows-msvc`.
- Preferencia de bloque: `CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS`, luego `CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX`, si no ninguno.
- Desplazamientos (`PreOffsets`, `PostOffsets`) siempre en 0.
- La matriz se restaura con `CTL_PIXTX_CONFIG_OPERTAION_TYPE_RESTORE_DEFAULT`, nunca escribiendo la identidad.
- Se restauran solo los bloques que la app escribió (`wrote_matrix`, `wrote_curve`).
- Todo `unsafe` y todo lo específico de IGCL vive en `src/igcl/`. `src/igcl/bindings.rs` no se edita a mano.
- Textos visibles al usuario en español. Identificadores y mensajes de commit en inglés, sin trailers.
- No ejecutar la app ni el probe sin que el usuario esté mirando la pantalla.

## Estructura de archivos

| Archivo | Cambio |
|---|---|
| `src/igcl/mod.rs` | `matrix_block` con nueva preferencia y tests; `restore_matrices`; `restore_default` la usa; `matrix_summary` |
| `examples/probe.rs` | Imprime `matrix_summary` |
| `docs/specs/2026-11-13-bloque-matriz-saturacion-design.md` | Resultado de la verificación en hardware |

---

### Task 1: Preferir la matriz con desplazamientos

**Files:**
- Modify: `src/igcl/mod.rs` (función `matrix_block` y módulo `tests`)

**Interfaces:**
- Consumes: nada nuevo.
- Produces: `fn matrix_block(blocks: &[ctl_pixtx_block_config_t]) -> Option<ctl_pixtx_block_config_t>` con la nueva preferencia (firma sin cambios).

- [ ] **Step 1: Escribir los tests que fallan**

Agregar al módulo `tests` de `src/igcl/mod.rs`:

```rust
    fn block(id: u32, block_type: ctl_pixtx_block_type_t) -> ctl_pixtx_block_config_t {
        // SAFETY: struct C plano; todo-ceros es válido.
        let mut b: ctl_pixtx_block_config_t = unsafe { zeroed() };
        b.BlockId = id;
        b.BlockType = block_type;
        b
    }

    /// Bloques en el orden que los reporta la Intel UHD de referencia.
    fn uhd_blocks() -> Vec<ctl_pixtx_block_config_t> {
        vec![
            block(327425, CTL_PIXTX_BLOCK_TYPE_3D_LUT),
            block(327426, CTL_PIXTX_BLOCK_TYPE_1D_LUT),
            block(327427, CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS),
            block(327428, CTL_PIXTX_BLOCK_TYPE_1D_LUT),
            block(327429, CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX),
        ]
    }

    #[test]
    fn matrix_block_prefers_the_pipe_csc_over_the_output_csc() {
        assert_eq!(matrix_block(&uhd_blocks()).map(|b| b.BlockId), Some(327427));
    }

    #[test]
    fn matrix_block_falls_back_to_the_plain_matrix() {
        let blocks = [
            block(1, CTL_PIXTX_BLOCK_TYPE_1D_LUT),
            block(2, CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX),
        ];
        assert_eq!(matrix_block(&blocks).map(|b| b.BlockId), Some(2));
    }

    #[test]
    fn matrix_block_is_none_without_a_matrix() {
        let blocks = [
            block(1, CTL_PIXTX_BLOCK_TYPE_3D_LUT),
            block(2, CTL_PIXTX_BLOCK_TYPE_1D_LUT),
        ];
        assert!(matrix_block(&blocks).is_none());
    }
```

- [ ] **Step 2: Correr los tests y ver que falla el de preferencia**

Run: `cargo test --lib igcl::tests::matrix_block`
Expected: `matrix_block_prefers_the_pipe_csc_over_the_output_csc` FALLA (`Some(327429)` en lugar de `Some(327427)`); los otros dos pasan.

- [ ] **Step 3: Invertir la preferencia**

Reemplazar `matrix_block` por:

```rust
/// Bloque de matriz 3×3 para la saturación. Prefiere la matriz con desplazamientos
/// (el CSC de la tubería, antes de la curva de salida); la matriz sola suele ser el
/// CSC de salida, que el driver usa para RGB→YCbCr en enlaces YCbCr.
fn matrix_block(blocks: &[ctl_pixtx_block_config_t]) -> Option<ctl_pixtx_block_config_t> {
    blocks
        .iter()
        .find(|b| b.BlockType == CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS)
        .or_else(|| {
            blocks
                .iter()
                .find(|b| b.BlockType == CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX)
        })
        .copied()
}
```

- [ ] **Step 4: Correr todos los tests**

Run: `cargo test`
Expected: todos PASAN.

- [ ] **Step 5: Commit**

```bash
git add src/igcl/mod.rs
git commit -m "fix: prefer the pipe CSC for the saturation matrix"
```

---

### Task 2: Restaurar la matriz con RESTORE_DEFAULT y mostrar el bloque en el probe

**Files:**
- Modify: `src/igcl/mod.rs` (`restore_matrices` nueva, `restore_default`, `matrix_summary` nueva, imports)
- Modify: `examples/probe.rs`

**Interfaces:**
- Consumes: `set_block`, `describe_block`, `Output::matrix` (ya existentes).
- Produces: `pub fn matrix_summary(&self) -> Vec<String>` en `IgclBackend`.

Sin test automático: depende del driver. Se verifica con `cargo build --example probe`, `cargo clippy` y en hardware (Task 3).

- [ ] **Step 1: Agregar `restore_matrices`**

En `impl IgclBackend`, junto a `restore_curves`:

```rust
    /// Pide al driver que vuelva el bloque de matriz de cada pantalla a su valor por defecto.
    fn restore_matrices(&self) -> Result<(), BackendError> {
        let mut first_error = None;
        for output in &self.outputs {
            let mut block = output.matrix;
            // `Size` anidado es un campo `[in]`; asignar un campo `Copy` de una unión es seguro.
            block.Config.MatrixConfig.Size = size_of::<ctl_pixtx_matrix_config_t>() as u32;
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
```

- [ ] **Step 2: Usarla en `restore_default`**

En `restore_default`, reemplazar `self.write_matrix(&IDENTITY)` por `self.restore_matrices()`, y el import `use crate::color::{Matrix3, IDENTITY};` por `use crate::color::Matrix3;`.

- [ ] **Step 3: Agregar `matrix_summary`**

En `impl IgclBackend`, después de `block_summary`:

```rust
    /// Una línea por pantalla con el bloque donde se escribe la saturación.
    pub fn matrix_summary(&self) -> Vec<String> {
        self.outputs
            .iter()
            .enumerate()
            .map(|(i, o)| format!("Pantalla {i}: saturación en {}", describe_block(&o.matrix)))
            .collect()
    }
```

- [ ] **Step 4: Mostrarla en el probe**

En `examples/probe.rs`, después del bucle que imprime `block_summary`:

```rust
    for line in backend.matrix_summary() {
        println!("  {line}");
    }
```

- [ ] **Step 5: Compilar y correr los tests**

Run: `cargo test`, `cargo build --example probe` y `cargo clippy --all-targets`
Expected: tests PASAN, compila sin warnings.

- [ ] **Step 6: Commit**

```bash
git add src/igcl/mod.rs examples/probe.rs
git commit -m "fix: restore the saturation matrix with the driver default"
```

---

### Task 3: Verificación en hardware y registro

**Files:**
- Modify: `docs/specs/2026-11-13-bloque-matriz-saturacion-design.md` (sección "Resultado de la verificación en hardware")

Con el usuario mirando la pantalla, una corrida por paso:

- [ ] **Step 1:** `cargo run --example probe -- 300`: el probe informa "saturación en bloque 327427"; colores más vivos; grises neutros (sin tinte). Enter: imagen normal.
- [ ] **Step 2:** `cargo run --example probe -- 0`: imagen en grises. Enter: imagen normal.
- [ ] **Step 3:** `cargo run --example probe -- 300` y entrar a un juego en pantalla completa exclusiva: el efecto se mantiene. Salir y Enter: imagen normal.
- [ ] **Step 4:** `cargo run --example probe -- 300 150`: se ven saturación y contraste juntos. Enter: imagen normal.
- [ ] **Step 5:** Si algún paso falla por el bloque 327427, volver a preferir la matriz 3×3 simple en `matrix_block` (ajustando el test de la Task 1), conservar `restore_matrices` y repetir los pasos.
- [ ] **Step 6:** Registrar resultados en la spec, cambiar su estado a "implementada y verificada en hardware" y commitear:

```bash
git add docs/specs/2026-11-13-bloque-matriz-saturacion-design.md
git commit -m "docs: record hardware results for the saturation matrix block"
```
