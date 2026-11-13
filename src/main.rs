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
            if !controller.supports_tone_curve() {
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
