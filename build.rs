fn main() {
    slint_build::compile("ui/app.slint").expect("no se pudo compilar ui/app.slint");

    // Ícono y datos de producto en el .exe (versión tomada de Cargo.toml).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=ui/icon.ico");
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("ui/icon.ico")
            .set("ProductName", "Ajustes de pantalla")
            .set("FileDescription", "Saturación, contraste y brillo de la pantalla")
            .set("OriginalFilename", "Ajustes de pantalla.exe");
        resource.compile().expect("no se pudo compilar el recurso de Windows");
    }
}
