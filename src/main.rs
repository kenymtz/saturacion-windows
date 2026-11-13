#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::rc::Rc;

use saturacion::backend::BackendError;
use saturacion::controller::Controller;
use saturacion::igcl::IgclBackend;
use slint::CloseRequestResponse;

slint::include_modules!();

type SharedController = Rc<RefCell<Controller<IgclBackend>>>;

/// Texto que se muestra si no se pudo crear el ícono de la bandeja.
const TRAY_UNAVAILABLE: &str =
    "No se pudo crear el ícono de la bandeja; cerrar la ventana termina la app.";

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

/// Crea y muestra el ícono de la bandeja. `None` si la plataforma no lo permite.
fn create_tray(ui: &AppWindow) -> Option<AppTray> {
    let tray = AppTray::new().ok()?;
    tray.show().ok()?;

    let ui_weak = ui.as_weak();
    let show_window = move || {
        if let Some(ui) = ui_weak.upgrade() {
            let _ = ui.show();
            ui.window().set_minimized(false);
        }
    };
    tray.on_open(show_window); // el clic izquierdo también llama a `open` (ver tray.slint)
    tray.on_quit(|| {
        let _ = slint::quit_event_loop();
    });
    Some(tray)
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

    // El ícono debe vivir hasta el final de `main`: al soltarlo desaparece de la bandeja.
    let tray = create_tray(&ui);
    if tray.is_none() && controller.is_some() {
        ui.set_error_text(TRAY_UNAVAILABLE.into());
    }

    // Con controlador e ícono, la X solo oculta la ventana (el ícono mantiene vivo el
    // bucle). Sin alguno de los dos, la X termina la app.
    let keep_running_in_tray = controller.is_some() && tray.is_some();
    ui.window().on_close_requested(move || {
        if !keep_running_in_tray {
            let _ = slint::quit_event_loop();
        }
        CloseRequestResponse::HideWindow
    });

    ui.show()?;
    let run_result = slint::run_event_loop();

    // Restaurar aunque el bucle de eventos haya terminado con error.
    if let Some(controller) = controller {
        let _ = controller.borrow_mut().shutdown();
    }
    drop(tray);
    run_result
}
