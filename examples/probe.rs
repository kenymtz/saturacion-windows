//! Prueba de riesgo: aplica una saturación y espera Enter para restaurar.
//! Uso: cargo run --example probe -- 300

use std::io::{self, BufRead};
use std::process::ExitCode;
use std::time::Instant;

use saturacion::backend::ColorBackend;
use saturacion::color::saturation_matrix;
use saturacion::igcl::IgclBackend;

fn main() -> ExitCode {
    let percent: f64 = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(300.0);

    let mut backend = match IgclBackend::new() {
        Ok(backend) => backend,
        Err(e) => {
            eprintln!("No se pudo iniciar: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("Pantallas con bloque de matriz: {}", backend.output_count());

    let start = Instant::now();
    if let Err(e) = backend.apply_matrix(&saturation_matrix(percent / 100.0)) {
        eprintln!("{e}");
        return ExitCode::FAILURE;
    }
    println!("Aplicado {percent}% en {:?}", start.elapsed());

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
