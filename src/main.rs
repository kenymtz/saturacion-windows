#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::rc::Rc;

use saturacion::backend::BackendError;
use saturacion::controller::Controller;
use saturacion::igcl::IgclBackend;

slint::include_modules!();

type SharedController = Rc<RefCell<Controller<IgclBackend>>>;

/// Muestra el resultado de una escritura: borra el error si salió bien.
fn show_result(ui: &AppWindow, result: Result<(), BackendError>) {
    let message = match result {
        Ok(()) => String::new(),
        Err(e) => e.to_string(),
    };
    ui.set_error_text(message.into());
}

/// Arma el manejador de un slider: pasa el valor redondeado a `apply` y muestra el resultado.
fn slider_handler(
    ui: &AppWindow,
    controller: &SharedController,
    apply: fn(&mut Controller<IgclBackend>, u32) -> Result<(), BackendError>,
) -> impl FnMut(f32) + 'static {
    let ui_weak = ui.as_weak();
    let controller = Rc::clone(controller);
    move |value| {
        let Some(ui) = ui_weak.upgrade() else { return };
        let percent = value.round().max(0.0) as u32;
        let result = apply(&mut *controller.borrow_mut(), percent);
        show_result(&ui, result);
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    let controller = match IgclBackend::new() {
        Ok(backend) => {
            let controller = Controller::new(backend);
            if !controller.supports_tone_curve() {
                ui.set_tone_enabled(false);
                ui.set_tone_note(BackendError::ToneCurveUnsupported.to_string().into());
            }
            Some(Rc::new(RefCell::new(controller)))
        }
        Err(e) => {
            ui.set_error_text(e.to_string().into());
            ui.set_controls_enabled(false);
            None
        }
    };

    if let Some(controller) = &controller {
        ui.on_saturation_changed(slider_handler(&ui, controller, Controller::set_saturation));
        ui.on_contrast_changed(slider_handler(&ui, controller, Controller::set_contrast));
        ui.on_brightness_changed(slider_handler(&ui, controller, Controller::set_brightness));
    }

    let run_result = ui.run();

    // Restaurar aunque el bucle de eventos haya terminado con error.
    if let Some(controller) = controller {
        let _ = controller.borrow_mut().shutdown();
    }
    run_result
}
