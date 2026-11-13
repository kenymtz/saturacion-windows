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
    /// Ninguna pantalla reporta una curva 1D con muestreo uniforme.
    ToneCurveUnsupported,
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
            Self::ToneCurveUnsupported => {
                write!(f, "Esta placa o este modo de pantalla no permite ajustar el contraste ni el brillo.")
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

    /// Escribe una curva de tono en las pantallas que la admiten. `curve` recibe y
    /// devuelve valores entre 0 y 1; cada backend elige cuántas muestras tomar.
    fn apply_tone_curve(&mut self, _curve: &dyn Fn(f64) -> f64) -> Result<(), BackendError> {
        Err(BackendError::ToneCurveUnsupported)
    }

    /// Si alguna pantalla admite curva de tono. Un backend que admite curvas debe
    /// sobrescribir a la vez este método y `apply_tone_curve`.
    fn supports_tone_curve(&self) -> bool {
        false
    }

    /// Devuelve las pantallas a la imagen sin modificar, restaurando solo los
    /// bloques que el backend escribió.
    fn restore_default(&mut self) -> Result<(), BackendError>;
}

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
            "Esta placa o este modo de pantalla no permite ajustar el contraste ni el brillo."
        );
    }
}
