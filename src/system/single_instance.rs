//! Instancia única: un mutex con nombre detecta otra copia y dos eventos con nombre
//! le pasan órdenes (mostrar la ventana o salir).

use std::ptr;
use std::thread;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, WAIT_OBJECT_0,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateMutexW, OpenEventW, OpenMutexW, SetEvent, WaitForMultipleObjects,
    EVENT_MODIFY_STATE, INFINITE, SYNCHRONIZATION_SYNCHRONIZE,
};

use super::wide;

const MUTEX_NAME: &str = r"Local\AjustesDePantalla.Instancia";
const SHOW_EVENT: &str = r"Local\AjustesDePantalla.Mostrar";
const QUIT_EVENT: &str = r"Local\AjustesDePantalla.Salir";

/// Orden que una copia nueva le manda a la que corre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Show,
    Quit,
}

/// Resultado de intentar ser la copia principal.
pub enum Startup {
    Primary(Instance),
    Secondary,
}

/// Mutex de la copia principal: mientras vive, las demás copias la detectan.
pub struct Instance {
    mutex: HANDLE,
}

/// Intenta ser la copia principal. Si el mutex no se puede crear, sigue como si fuera
/// la única copia.
pub fn acquire() -> Startup {
    let name = wide(MUTEX_NAME);
    // SAFETY: atributos nulos y nombre terminado en cero.
    let mutex = unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) };
    // SAFETY: se lee inmediatamente después de CreateMutexW.
    let already_running = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    if !mutex.is_null() && already_running {
        // SAFETY: `mutex` es un handle válido que no se usa después.
        unsafe { CloseHandle(mutex) };
        return Startup::Secondary;
    }
    Startup::Primary(Instance { mutex })
}

impl Instance {
    /// Atiende en un hilo propio las órdenes de otras copias. `on_command` corre en ese
    /// hilo: para tocar la interfaz debe reenviar la orden al hilo de Slint.
    pub fn listen(&self, on_command: impl Fn(Command) + Send + 'static) {
        let show = create_event(SHOW_EVENT);
        let quit = create_event(QUIT_EVENT);
        if show.is_null() || quit.is_null() {
            return;
        }
        // Los handles son punteros crudos (no `Send`); se pasan al hilo como números.
        let raw = [show as usize, quit as usize];
        thread::spawn(move || {
            let handles: [HANDLE; 2] = [raw[0] as HANDLE, raw[1] as HANDLE];
            loop {
                // SAFETY: dos eventos válidos que este hilo nunca cierra.
                let r = unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, INFINITE) };
                if r == WAIT_OBJECT_0 {
                    on_command(Command::Show);
                } else if r == WAIT_OBJECT_0 + 1 {
                    on_command(Command::Quit);
                } else {
                    return;
                }
            }
        });
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        if !self.mutex.is_null() {
            // SAFETY: handle válido de CreateMutexW que no se usa después.
            unsafe { CloseHandle(self.mutex) };
        }
    }
}

/// Evento de reinicio automático con nombre; nulo si falla.
fn create_event(name: &str) -> HANDLE {
    let name = wide(name);
    // SAFETY: atributos nulos y nombre terminado en cero.
    unsafe { CreateEventW(ptr::null(), 0, 0, name.as_ptr()) }
}

/// Le manda `command` a la copia que corre. `false` si no hay ninguna escuchando.
pub fn send(command: Command) -> bool {
    let name = wide(match command {
        Command::Show => SHOW_EVENT,
        Command::Quit => QUIT_EVENT,
    });
    // SAFETY: nombre terminado en cero.
    let event = unsafe { OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr()) };
    if event.is_null() {
        return false;
    }
    // SAFETY: `event` es válido hasta CloseHandle.
    let signaled = unsafe { SetEvent(event) } != 0;
    // SAFETY: handle válido que no se usa después.
    unsafe { CloseHandle(event) };
    signaled
}

/// Si hay una copia principal corriendo.
fn is_running() -> bool {
    let name = wide(MUTEX_NAME);
    // SAFETY: nombre terminado en cero.
    let mutex = unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr()) };
    if mutex.is_null() {
        return false;
    }
    // SAFETY: handle válido que no se usa después.
    unsafe { CloseHandle(mutex) };
    true
}

/// Espera hasta `timeout` a que no quede ninguna copia corriendo. `true` si terminó.
pub fn wait_until_gone(timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if !is_running() {
            return true;
        }
        thread::sleep(Duration::from_millis(100));
    }
    !is_running()
}
