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
