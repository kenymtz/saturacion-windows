//! Une el valor del slider con el backend de color.

use crate::backend::{BackendError, ColorBackend};
use crate::color::saturation_matrix;

/// Máximo del slider, en porcentaje. 100 es la imagen sin modificar.
pub const MAX_PERCENT: u32 = 300;

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
