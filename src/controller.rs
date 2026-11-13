//! Une el valor del slider con el backend de color.

use crate::backend::{BackendError, ColorBackend};
use crate::color::{saturation_matrix, tone_curve};

/// Máximo del slider de saturación, en porcentaje. 100 es la imagen sin modificar.
pub const MAX_PERCENT: u32 = 400;

/// Rango del slider de contraste, en porcentaje. 100 es la imagen sin modificar.
pub const MIN_CONTRAST_PERCENT: u32 = 50;
pub const MAX_CONTRAST_PERCENT: u32 = 200;

/// Rango del slider de brillo, en porcentaje. 100 es la imagen sin modificar.
pub const MIN_BRIGHTNESS_PERCENT: u32 = 50;
pub const MAX_BRIGHTNESS_PERCENT: u32 = 200;

/// Factor de contraste (1.0 = sin cambios) para un porcentaje, recortado a su rango.
fn contrast_factor(percent: u32) -> f64 {
    f64::from(percent.clamp(MIN_CONTRAST_PERCENT, MAX_CONTRAST_PERCENT)) / 100.0
}

/// Factor de brillo (1.0 = sin cambios) para un porcentaje, recortado a su rango.
fn brightness_factor(percent: u32) -> f64 {
    f64::from(percent.clamp(MIN_BRIGHTNESS_PERCENT, MAX_BRIGHTNESS_PERCENT)) / 100.0
}

pub struct Controller<B: ColorBackend> {
    backend: B,
    /// Verdadero desde el primer intento de escritura: una escritura fallida
    /// puede haber llegado a alguna pantalla, así que también obliga a restaurar.
    wrote: bool,
    /// Factores vigentes de la curva de tono (1.0 = sin cambios).
    contrast: f64,
    brightness: f64,
}

impl<B: ColorBackend> Controller<B> {
    pub fn new(backend: B) -> Self {
        Self { backend, wrote: false, contrast: 1.0, brightness: 1.0 }
    }

    /// Aplica la saturación indicada en porcentaje (100 = sin cambios).
    pub fn set_saturation(&mut self, percent: u32) -> Result<(), BackendError> {
        let s = f64::from(percent.min(MAX_PERCENT)) / 100.0;
        self.wrote = true;
        self.backend.apply_matrix(&saturation_matrix(s))
    }

    /// Aplica el contraste indicado en porcentaje (100 = sin cambios), limitado a 50–200,
    /// conservando el brillo vigente.
    pub fn set_contrast(&mut self, percent: u32) -> Result<(), BackendError> {
        self.contrast = contrast_factor(percent);
        self.write_tone_curve()
    }

    /// Aplica el brillo indicado en porcentaje (100 = sin cambios), limitado a 50–200,
    /// conservando el contraste vigente.
    pub fn set_brightness(&mut self, percent: u32) -> Result<(), BackendError> {
        self.brightness = brightness_factor(percent);
        self.write_tone_curve()
    }

    /// Si la placa permite ajustar contraste y brillo.
    pub fn supports_tone_curve(&self) -> bool {
        self.backend.supports_tone_curve()
    }

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

    /// Escribe la curva que combina el contraste y el brillo vigentes.
    fn write_tone_curve(&mut self) -> Result<(), BackendError> {
        let (c, b) = (self.contrast, self.brightness);
        self.wrote = true;
        self.backend.apply_tone_curve(&|x| tone_curve(c, b, x))
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

/// Restaura la pantalla si la app termina sin llamar a `shutdown` (p. ej. por un pánico).
impl<B: ColorBackend> Drop for Controller<B> {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::color::Matrix3;

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
        assert_eq!(*calls.borrow(), vec![Call::Apply(saturation_matrix(4.0))]);
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

    #[test]
    fn dropping_after_a_write_restores() {
        let (mut c, calls) = controller(None);
        c.set_saturation(150).unwrap();
        drop(c);
        assert_eq!(calls.borrow().last(), Some(&Call::Restore));
    }

    #[test]
    fn dropping_after_shutdown_does_not_restore_twice() {
        let (mut c, calls) = controller(None);
        c.set_saturation(150).unwrap();
        c.shutdown().unwrap();
        drop(c);
        let restores = calls.borrow().iter().filter(|c| **c == Call::Restore).count();
        assert_eq!(restores, 1);
    }

    #[test]
    fn dropping_without_writes_does_not_touch_the_display() {
        let (c, calls) = controller(None);
        drop(c);
        assert!(calls.borrow().is_empty());
    }

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
    fn supports_tone_curve_reflects_backend() {
        let (c, _) = controller(None);
        assert!(c.supports_tone_curve());

        let calls = Rc::new(RefCell::new(Vec::new()));
        let backend = FakeBackend { calls, fail_with: None, curve_supported: false };
        assert!(!Controller::new(backend).supports_tone_curve());
    }

    /// Compara la última curva registrada con muestras esperadas, con tolerancia.
    fn assert_last_curve(calls: &Rc<RefCell<Vec<Call>>>, expected: [f64; 5]) {
        match calls.borrow().last() {
            Some(Call::Curve(samples)) => {
                for (got, want) in samples.iter().zip(expected) {
                    assert!((got - want).abs() < 1e-12, "{samples:?} != {expected:?}");
                }
            }
            other => panic!("se esperaba una curva, llegó {other:?}"),
        }
    }

    #[test]
    fn brightness_value_becomes_tone_curve_without_touching_the_matrix() {
        let (mut c, calls) = controller(None);
        c.set_brightness(50).unwrap();
        assert_eq!(calls.borrow().len(), 1);
        assert_last_curve(&calls, [0.0, 0.0625, 0.25, 0.5625, 1.0]);
    }

    #[test]
    fn brightness_outside_range_is_clamped() {
        let (mut c, calls) = controller(None);
        c.set_brightness(900).unwrap();
        assert_last_curve(&calls, [0.0, 0.5, 0.5_f64.sqrt(), 0.75_f64.sqrt(), 1.0]);
        c.set_brightness(10).unwrap();
        assert_last_curve(&calls, [0.0, 0.0625, 0.25, 0.5625, 1.0]);
    }

    #[test]
    fn brightness_keeps_current_contrast() {
        let (mut c, calls) = controller(None);
        c.set_contrast(200).unwrap();
        c.set_brightness(50).unwrap();
        assert_last_curve(&calls, [0.0, 0.0, 0.25, 1.0, 1.0]);
    }

    #[test]
    fn contrast_keeps_current_brightness() {
        let (mut c, calls) = controller(None);
        c.set_brightness(50).unwrap();
        c.set_contrast(200).unwrap();
        assert_last_curve(&calls, [0.0, 0.0, 0.25, 1.0, 1.0]);
    }

    #[test]
    fn shutdown_restores_after_a_brightness_write() {
        let (mut c, calls) = controller(None);
        c.set_brightness(150).unwrap();
        c.shutdown().unwrap();
        assert_eq!(calls.borrow().last(), Some(&Call::Restore));
    }

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
}
