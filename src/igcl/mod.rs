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
