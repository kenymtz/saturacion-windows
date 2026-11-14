//! Idioma de la interfaz (español o inglés) y los mensajes que arma el código Rust.

use crate::backend::BackendError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    Spanish,
    English,
}

impl Language {
    /// Código que se guarda en el archivo de ajustes.
    pub fn code(self) -> &'static str {
        match self {
            Self::Spanish => "es",
            Self::English => "en",
        }
    }

    /// Lee un código guardado; `None` si no es uno conocido.
    pub fn from_code(code: &str) -> Option<Self> {
        match code.trim() {
            "es" => Some(Self::Spanish),
            "en" => Some(Self::English),
            _ => None,
        }
    }

    pub fn is_english(self) -> bool {
        self == Self::English
    }

    pub fn from_english(english: bool) -> Self {
        if english {
            Self::English
        } else {
            Self::Spanish
        }
    }
}

/// Mensaje que la ventana muestra debajo de los controles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Backend(BackendError),
    SaveFailed,
    AutostartFailed,
    TrayUnavailable,
}

impl Message {
    pub fn text(&self, language: Language) -> String {
        match (self, language) {
            (Self::Backend(error), language) => error.message(language),
            (Self::SaveFailed, Language::Spanish) => "No se pudieron guardar los ajustes.".into(),
            (Self::SaveFailed, Language::English) => "Couldn't save the settings.".into(),
            (Self::AutostartFailed, Language::Spanish) => {
                "No se pudo cambiar el inicio con Windows.".into()
            }
            (Self::AutostartFailed, Language::English) => {
                "Couldn't change the Start with Windows setting.".into()
            }
            (Self::TrayUnavailable, Language::Spanish) => {
                "No se pudo crear el ícono de la bandeja; cerrar la ventana termina la app.".into()
            }
            (Self::TrayUnavailable, Language::English) => {
                "Couldn't create the tray icon; closing the window quits the app.".into()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_language_is_spanish() {
        assert_eq!(Language::default(), Language::Spanish);
    }

    #[test]
    fn codes_round_trip() {
        for language in [Language::Spanish, Language::English] {
            assert_eq!(Language::from_code(language.code()), Some(language));
        }
    }

    #[test]
    fn unknown_code_is_rejected() {
        assert_eq!(Language::from_code("fr"), None);
        assert_eq!(Language::from_code(""), None);
    }

    #[test]
    fn english_flag_round_trips() {
        assert!(Language::from_english(true).is_english());
        assert!(!Language::from_english(false).is_english());
    }

    #[test]
    fn every_message_has_both_languages() {
        let messages = [
            Message::Backend(BackendError::DriverNotFound),
            Message::Backend(BackendError::NoActiveDisplay),
            Message::Backend(BackendError::MatrixUnsupported),
            Message::Backend(BackendError::ToneCurveUnsupported),
            Message::Backend(BackendError::ApplyFailed(7)),
            Message::SaveFailed,
            Message::AutostartFailed,
            Message::TrayUnavailable,
        ];
        for message in messages {
            let es = message.text(Language::Spanish);
            let en = message.text(Language::English);
            assert!(!es.is_empty() && !en.is_empty());
            assert_ne!(es, en, "{message:?}");
        }
    }

    #[test]
    fn backend_message_in_english() {
        assert_eq!(
            Message::Backend(BackendError::ApplyFailed(0x40000001)).text(Language::English),
            "Couldn't apply the setting (IGCL 0x40000001)."
        );
    }
}
