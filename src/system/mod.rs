//! Integración con Windows: instancia única y arranque con el sistema.
//! Junto con `src/igcl/`, es el único lugar con código `unsafe`.

pub mod autostart;
pub mod single_instance;

/// Texto en UTF-16 terminado en cero, como esperan las funciones `W` de Windows.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
