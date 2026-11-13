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
