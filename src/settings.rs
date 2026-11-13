//! Ajustes guardados entre sesiones: saturación, contraste y brillo en un archivo de texto.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::controller::{
    MAX_BRIGHTNESS_PERCENT, MAX_CONTRAST_PERCENT, MAX_PERCENT, MIN_BRIGHTNESS_PERCENT,
    MIN_CONTRAST_PERCENT,
};

/// Carpeta dentro de `%APPDATA%` y nombre del archivo de ajustes.
pub const APP_DIR: &str = "Ajustes de pantalla";
pub const FILE_NAME: &str = "ajustes.txt";

/// Valores de los tres sliders, en porcentaje (100 = sin cambios).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub saturation: u32,
    pub contrast: u32,
    pub brightness: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self { saturation: 100, contrast: 100, brightness: 100 }
    }
}

impl Settings {
    /// Lee el texto de forma tolerante: ignora líneas vacías, claves desconocidas y valores
    /// que no son números, y recorta cada valor a su rango. Lo que falta queda en 100.
    pub fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let Ok(value) = value.trim().parse::<u32>() else { continue };
            match key.trim() {
                "saturacion" => settings.saturation = value.min(MAX_PERCENT),
                "contraste" => {
                    settings.contrast = value.clamp(MIN_CONTRAST_PERCENT, MAX_CONTRAST_PERCENT)
                }
                "brillo" => {
                    settings.brightness =
                        value.clamp(MIN_BRIGHTNESS_PERCENT, MAX_BRIGHTNESS_PERCENT)
                }
                _ => {}
            }
        }
        settings
    }

    /// Texto que se guarda en el archivo.
    pub fn to_text(&self) -> String {
        format!(
            "saturacion={}\ncontraste={}\nbrillo={}\n",
            self.saturation, self.contrast, self.brightness
        )
    }

    /// Lee el archivo; si no existe o no se puede leer, devuelve los valores por defecto.
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path).map(|text| Self::parse(&text)).unwrap_or_default()
    }

    /// Guarda en el archivo, creando la carpeta si hace falta.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, self.to_text())
    }
}

/// `%APPDATA%\Ajustes de pantalla\ajustes.txt`, o `None` si `APPDATA` no está definida.
pub fn default_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join(APP_DIR).join(FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("ajustes-test-{}-{name}", std::process::id()))
            .join(FILE_NAME)
    }

    #[test]
    fn default_is_neutral() {
        assert_eq!(
            Settings::default(),
            Settings { saturation: 100, contrast: 100, brightness: 100 }
        );
    }

    #[test]
    fn parse_reads_all_keys() {
        let s = Settings::parse("saturacion=150\ncontraste=120\nbrillo=80\n");
        assert_eq!(s, Settings { saturation: 150, contrast: 120, brightness: 80 });
    }

    #[test]
    fn parse_ignores_blank_unknown_and_garbage_lines() {
        let text = "\n  \notra=5\nsaturacion=abc\nsin igual\nbrillo = 130 \ncontraste=-4\n";
        assert_eq!(
            Settings::parse(text),
            Settings { saturation: 100, contrast: 100, brightness: 130 }
        );
    }

    #[test]
    fn parse_clamps_values_to_their_ranges() {
        let s = Settings::parse("saturacion=900\ncontraste=10\nbrillo=999\n");
        assert_eq!(s, Settings { saturation: 400, contrast: 50, brightness: 200 });
    }

    #[test]
    fn missing_keys_stay_neutral() {
        assert_eq!(
            Settings::parse("contraste=150"),
            Settings { saturation: 100, contrast: 150, brightness: 100 }
        );
    }

    #[test]
    fn text_round_trip_keeps_values() {
        let s = Settings { saturation: 0, contrast: 200, brightness: 50 };
        assert_eq!(Settings::parse(&s.to_text()), s);
    }

    #[test]
    fn load_of_missing_file_is_neutral() {
        assert_eq!(Settings::load(&temp_path("missing")), Settings::default());
    }

    #[test]
    fn save_creates_folder_and_load_reads_it_back() {
        let path = temp_path("save");
        let s = Settings { saturation: 250, contrast: 110, brightness: 90 };
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn default_path_is_under_appdata() {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            assert_eq!(
                default_path(),
                Some(PathBuf::from(appdata).join(APP_DIR).join(FILE_NAME))
            );
        }
    }
}
