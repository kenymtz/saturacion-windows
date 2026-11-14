#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use saturacion::backend::BackendError;
use saturacion::controller::Controller;
use saturacion::igcl::IgclBackend;
use saturacion::language::{Language, Message};
use saturacion::settings::{self, Settings};
use saturacion::system::autostart;
use saturacion::system::single_instance::{self, Command, Startup};
use slint::CloseRequestResponse;

slint::include_modules!();

type SharedController = Rc<RefCell<Controller<IgclBackend>>>;

/// Argumento con el que el instalador pide cerrar la copia que corre.
const QUIT_ARG: &str = "--salir";
/// Cuánto espera `--salir` a que la otra copia termine.
const QUIT_TIMEOUT: Duration = Duration::from_secs(5);

/// Idioma activo y mensajes visibles, para volver a escribirlos al cambiar de idioma.
#[derive(Default)]
struct Texts {
    language: Cell<Language>,
    error: RefCell<Option<Message>>,
    tone_note: RefCell<Option<Message>>,
}

type SharedTexts = Rc<Texts>;

impl Texts {
    fn show_error(&self, ui: &AppWindow, message: Option<Message>) {
        let text = message
            .as_ref()
            .map(|m| m.text(self.language.get()))
            .unwrap_or_default();
        ui.set_error_text(text.into());
        *self.error.borrow_mut() = message;
    }

    fn show_tone_note(&self, ui: &AppWindow, message: Option<Message>) {
        let text = message
            .as_ref()
            .map(|m| m.text(self.language.get()))
            .unwrap_or_default();
        ui.set_tone_note(text.into());
        *self.tone_note.borrow_mut() = message;
    }

    fn has_error(&self) -> bool {
        self.error.borrow().is_some()
    }

    /// Cambia el idioma de la ventana y vuelve a escribir los mensajes visibles.
    fn set_language(&self, ui: &AppWindow, language: Language) {
        self.language.set(language);
        ui.set_english(language.is_english());
        let error = self.error.borrow().clone();
        self.show_error(ui, error);
        let note = self.tone_note.borrow().clone();
        self.show_tone_note(ui, note);
    }
}

/// Muestra el resultado de una escritura: borra el error si salió bien.
fn show_result(ui: &AppWindow, texts: &Texts, result: Result<(), BackendError>) {
    texts.show_error(ui, result.err().map(Message::Backend));
}

/// Arma el manejador de un slider: pasa el valor redondeado a `apply` y muestra el resultado.
fn slider_handler(
    ui: &AppWindow,
    texts: &SharedTexts,
    controller: &SharedController,
    apply: fn(&mut Controller<IgclBackend>, u32) -> Result<(), BackendError>,
) -> impl FnMut(f32) + 'static {
    let ui_weak = ui.as_weak();
    let texts = Rc::clone(texts);
    let controller = Rc::clone(controller);
    move |value| {
        let Some(ui) = ui_weak.upgrade() else { return };
        let percent = value.round().max(0.0) as u32;
        let result = apply(&mut *controller.borrow_mut(), percent);
        show_result(&ui, &texts, result);
    }
}

/// Valores que muestran los sliders y el idioma activo, tal como se guardan.
fn current_settings(ui: &AppWindow, texts: &Texts) -> Settings {
    Settings {
        saturation: ui.get_saturation().round().max(0.0) as u32,
        contrast: ui.get_contrast().round().max(0.0) as u32,
        brightness: ui.get_brightness().round().max(0.0) as u32,
        language: texts.language.get(),
    }
}

/// Pone los sliders en `values` (no dispara `changed`: solo lo hace el usuario).
fn show_settings(ui: &AppWindow, values: Settings) {
    ui.set_saturation(values.saturation as f32);
    ui.set_contrast(values.contrast as f32);
    ui.set_brightness(values.brightness as f32);
}

/// Guarda los ajustes; si no se puede, lo muestra en la ventana.
fn save_settings(ui: &AppWindow, texts: &Texts, path: Option<&Path>, values: Settings) {
    let saved = path.is_some_and(|path| values.save(path).is_ok());
    if !saved {
        texts.show_error(ui, Some(Message::SaveFailed));
    }
}

/// Vuelve todo a 100 (el idioma no cambia): restaura la placa, mueve los sliders y guarda.
fn reset_all(ui: &AppWindow, texts: &Texts, controller: &SharedController, path: Option<&Path>) {
    let result = controller.borrow_mut().reset();
    let reset_ok = result.is_ok();
    let neutral = Settings::neutral(texts.language.get());
    show_settings(ui, neutral);
    show_result(ui, texts, result);
    // El error de la pantalla gana: solo se avisa del guardado si el reinicio salió bien.
    if reset_ok {
        save_settings(ui, texts, path, neutral);
    } else if let Some(path) = path {
        let _ = neutral.save(path);
    }
}

/// Crea y muestra el ícono de la bandeja. `None` si la plataforma no lo permite.
fn create_tray(ui: &AppWindow, on_reset: impl Fn() + 'static) -> Option<AppTray> {
    let tray = AppTray::new().ok()?;
    tray.show().ok()?;

    let ui_weak = ui.as_weak();
    tray.on_open(move || {
        if let Some(ui) = ui_weak.upgrade() {
            let _ = ui.show();
            ui.window().set_minimized(false);
        }
    }); // el clic izquierdo también llama a `open` (ver tray.slint)
    tray.on_reset(on_reset);
    tray.on_quit(|| {
        let _ = slint::quit_event_loop();
    });
    Some(tray)
}

fn main() -> Result<(), slint::PlatformError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let quit_requested = args.iter().any(|arg| arg == QUIT_ARG);
    let start_minimized = args.iter().any(|arg| arg == autostart::MINIMIZED_ARG);

    // Instancia única: si ya hay una copia, se le pasa la orden y esta termina.
    let instance = match single_instance::acquire() {
        Startup::Secondary => {
            if quit_requested {
                single_instance::send(Command::Quit);
                single_instance::wait_until_gone(QUIT_TIMEOUT);
            } else {
                single_instance::send(Command::Show);
            }
            return Ok(());
        }
        Startup::Primary(instance) => instance,
    };
    if quit_requested {
        return Ok(()); // no había otra copia corriendo
    }

    let ui = AppWindow::new()?;

    // Órdenes de otras copias: llegan en otro hilo y se reenvían al de la interfaz.
    // Se atienden desde ya para no perder un Mostrar/Salir mientras arranca IGCL.
    let ui_weak = ui.as_weak();
    instance.listen(move |command| match command {
        Command::Show => {
            let _ = ui_weak.upgrade_in_event_loop(|ui| {
                let _ = ui.show();
                ui.window().set_minimized(false);
            });
        }
        Command::Quit => {
            // En cola: si el bucle aún no arrancó, corre apenas arranca.
            let _ = slint::invoke_from_event_loop(|| {
                let _ = slint::quit_event_loop();
            });
        }
    });

    let settings_path = settings::default_path();
    let saved = settings_path
        .as_deref()
        .map(Settings::load)
        .unwrap_or_default();
    show_settings(&ui, saved);
    let texts: SharedTexts = Rc::new(Texts::default());
    texts.set_language(&ui, saved.language);

    let (controller, startup_failed) = match IgclBackend::new() {
        Ok(backend) => {
            let mut controller = Controller::new(backend);
            if !controller.supports_tone_curve() {
                ui.set_tone_enabled(false);
                texts.show_tone_note(
                    &ui,
                    Some(Message::Backend(BackendError::ToneCurveUnsupported)),
                );
            }
            let applied =
                controller.apply_saved(saved.saturation, saved.contrast, saved.brightness);
            let failed = applied.is_err();
            show_result(&ui, &texts, applied);
            (Some(Rc::new(RefCell::new(controller))), failed)
        }
        Err(e) => {
            texts.show_error(&ui, Some(Message::Backend(e)));
            ui.set_controls_enabled(false);
            (None, true)
        }
    };

    if let Some(controller) = &controller {
        ui.on_saturation_changed(slider_handler(
            &ui,
            &texts,
            controller,
            Controller::set_saturation,
        ));
        ui.on_contrast_changed(slider_handler(
            &ui,
            &texts,
            controller,
            Controller::set_contrast,
        ));
        ui.on_brightness_changed(slider_handler(
            &ui,
            &texts,
            controller,
            Controller::set_brightness,
        ));

        let ui_weak = ui.as_weak();
        let shared_texts = Rc::clone(&texts);
        let path = settings_path.clone();
        ui.on_settings_committed(move || {
            if let Some(ui) = ui_weak.upgrade() {
                let values = current_settings(&ui, &shared_texts);
                save_settings(&ui, &shared_texts, path.as_deref(), values);
            }
        });

        let ui_weak = ui.as_weak();
        let shared_texts = Rc::clone(&texts);
        let shared = Rc::clone(controller);
        let path = settings_path.clone();
        ui.on_reset_requested(move || {
            if let Some(ui) = ui_weak.upgrade() {
                reset_all(&ui, &shared_texts, &shared, path.as_deref());
            }
        });
    }

    // Casilla "Iniciar con Windows": su estado sale del registro, no del archivo.
    match std::env::current_exe() {
        Ok(exe) => {
            ui.set_autostart(autostart::is_enabled(&exe));
            let ui_weak = ui.as_weak();
            let shared_texts = Rc::clone(&texts);
            ui.on_autostart_toggled(move |enabled| {
                let Some(ui) = ui_weak.upgrade() else { return };
                if autostart::set_enabled(&exe, enabled).is_err() {
                    ui.set_autostart(!enabled);
                    shared_texts.show_error(&ui, Some(Message::AutostartFailed));
                }
            });
        }
        Err(_) => ui.set_autostart_available(false),
    }

    // El ícono debe vivir hasta el final de `main`: al soltarlo desaparece de la bandeja.
    let tray_reset = {
        let ui_weak = ui.as_weak();
        let texts = Rc::clone(&texts);
        let controller = controller.clone();
        let path = settings_path.clone();
        move || {
            if let (Some(ui), Some(controller)) = (ui_weak.upgrade(), &controller) {
                reset_all(&ui, &texts, controller, path.as_deref());
            }
        }
    };
    let tray = create_tray(&ui, tray_reset);
    if let Some(tray) = &tray {
        tray.set_english(saved.language.is_english());
    }
    if tray.is_none() && controller.is_some() && !texts.has_error() {
        texts.show_error(&ui, Some(Message::TrayUnavailable));
    }

    // Selector de idioma: cambia la ventana y la bandeja, y se guarda con los valores.
    let tray_weak = tray.as_ref().map(|tray| tray.as_weak());
    let ui_weak = ui.as_weak();
    let shared_texts = Rc::clone(&texts);
    let path = settings_path.clone();
    ui.on_language_changed(move |english| {
        let Some(ui) = ui_weak.upgrade() else { return };
        shared_texts.set_language(&ui, Language::from_english(english));
        if let Some(tray) = tray_weak.as_ref().and_then(|weak| weak.upgrade()) {
            tray.set_english(english);
        }
        let values = current_settings(&ui, &shared_texts);
        save_settings(&ui, &shared_texts, path.as_deref(), values);
    });

    // Con controlador e ícono, la X solo oculta la ventana (el ícono mantiene vivo el
    // bucle). Sin alguno de los dos, la X termina la app.
    let keep_running_in_tray = controller.is_some() && tray.is_some();
    ui.window().on_close_requested(move || {
        if !keep_running_in_tray {
            let _ = slint::quit_event_loop();
        }
        CloseRequestResponse::HideWindow
    });

    // Con `--minimizado` y bandeja disponible, arranca solo con el ícono, salvo que
    // el arranque haya fallado: el error tiene que verse.
    if !(start_minimized && tray.is_some() && !startup_failed) {
        ui.show()?;
    }
    let run_result = slint::run_event_loop();

    // Restaurar aunque el bucle de eventos haya terminado con error.
    if let Some(controller) = controller {
        let _ = controller.borrow_mut().shutdown();
    }
    drop(tray);
    drop(instance);
    run_result
}
