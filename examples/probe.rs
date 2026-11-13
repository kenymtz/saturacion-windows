//! Prueba de riesgo: lista los bloques de color, aplica saturación y, opcionalmente,
//! contraste y brillo, y espera Enter para restaurar.
//! Uso: cargo run --example probe -- <saturación> [contraste] [brillo]
//! Ejemplos: `-- 300` (saturación), `-- 100 200` (contraste), `-- 100 100 150` (brillo).

use std::io::{self, BufRead};
use std::process::ExitCode;
use std::time::Instant;

use saturacion::backend::ColorBackend;
use saturacion::color::{saturation_matrix, tone_curve};
use saturacion::igcl::IgclBackend;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let saturation: f64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(300.0);
    let contrast: Option<f64> = args.next().and_then(|a| a.parse().ok());
    let brightness: Option<f64> = args.next().and_then(|a| a.parse().ok());

    let mut backend = match IgclBackend::new() {
        Ok(backend) => backend,
        Err(e) => {
            eprintln!("No se pudo iniciar: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("Pantallas con bloque de matriz: {}", backend.output_count());
    for line in backend.block_summary() {
        println!("  {line}");
    }
    let curve_available = if backend.supports_tone_curve() { "sí" } else { "no" };
    println!("Curva 1D uniforme disponible: {curve_available}");

    let start = Instant::now();
    if let Err(e) = backend.apply_matrix(&saturation_matrix(saturation / 100.0)) {
        eprintln!("{e}");
        let _ = backend.restore_default();
        return ExitCode::FAILURE;
    }
    println!("Saturación {saturation}% aplicada en {:?}", start.elapsed());

    if contrast.is_some() || brightness.is_some() {
        let contrast = contrast.unwrap_or(100.0);
        let brightness = brightness.unwrap_or(100.0);
        let (c, b) = (contrast / 100.0, brightness / 100.0);
        let start = Instant::now();
        if let Err(e) = backend.apply_tone_curve(&|x| tone_curve(c, b, x)) {
            eprintln!("{e}");
            let _ = backend.restore_default();
            return ExitCode::FAILURE;
        }
        println!(
            "Contraste {contrast}% y brillo {brightness}% aplicados en {:?}",
            start.elapsed()
        );
    }

    println!("Enter para restaurar la imagen normal...");
    let _ = io::stdin().lock().lines().next();

    match backend.restore_default() {
        Ok(()) => {
            println!("Restaurado.");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
