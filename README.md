# Ajustes de pantalla

App de Windows para jugadores que sube la saturación de la pantalla por encima del tope de Windows y del panel de Intel, y ajusta contraste y brillo. Los ajustes se aplican en la placa de video, así que se mantienen dentro de los juegos en pantalla completa exclusiva.

## Qué hace

- **Saturación** de 0 a 400 (100 = sin cambios).
- **Contraste** y **brillo** de 50 a 200.
- Recuerda los valores y puede **iniciar con Windows**, minimizada en la bandeja.
- **Restablecer** vuelve todo a la imagen normal.
- Cerrar la ventana la deja en la bandeja; **Salir** desde el ícono restaura la imagen y cierra.

## Requisitos

- Windows 10 u 11 de 64 bits.
- Pantalla conectada a una placa **Intel** con su driver instalado (la app usa la API de Intel; NVIDIA y AMD no están soportadas todavía).

## Instalación

1. Ejecutar `AjustesDePantalla-Setup-<versión>.exe`. No pide permisos de administrador.
2. Si Windows SmartScreen muestra "Windows protegió su PC", elegir **Más información** → **Ejecutar de todas formas** (el instalador no está firmado).
3. El instalador crea el acceso directo en el escritorio, la entrada en el menú Inicio y, si se deja marcado, el inicio con Windows.

Para desinstalar: Configuración → Aplicaciones → "Ajustes de pantalla". Los valores guardados quedan en `%APPDATA%\Ajustes de pantalla`.

## Seguridad con CS2 y FACEIT

Ver [docs/SEGURIDAD.md](docs/SEGURIDAD.md).

## Compilar

Requisitos: Rust (toolchain `stable-x86_64-pc-windows-msvc`), Build Tools de C++ de Visual Studio e Inno Setup 6 para el instalador. Si Windows tiene Smart App Control activado, bloquea los ejecutables sin firma que genera Rust y hay que desactivarlo para compilar.

```bash
cargo test
```

```bash
powershell -ExecutionPolicy Bypass -File scripts/build-installer.ps1
```

El instalador queda en `dist/`.

## Documentación

Las especificaciones y planes de cada historia están en `docs/specs/` y `docs/plans/`.
