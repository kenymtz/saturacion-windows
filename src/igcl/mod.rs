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
            // `Size` anidado es un campo `[in]`; asignar un campo `Copy` de una unión es seguro.
            block.Config.OneDLutConfig.Size = size_of::<ctl_pixtx_1dlut_config_t>() as u32;
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
