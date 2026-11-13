# Ajustes de pantalla

[Español](#español) · [English](#english)

---

## Español

### Qué es

**Ajustes de pantalla** es una app de Windows para jugadores que sube la **saturación** de la pantalla por encima del límite que imponen Windows y el panel del fabricante, y además ajusta **contraste** y **brillo**. Los cambios se aplican en la placa de video, así que se ven también dentro de los juegos en pantalla completa exclusiva.

### Qué problema soluciona

En muchas PCs no hay forma de subir la saturación lo suficiente:

- **Intel limita la saturación.** El panel de gráficos de Intel tiene un tope bajo, muy por debajo de lo que ofrecen herramientas como el *Digital Vibrance* de NVIDIA.
- **Windows no trae un control de saturación.**
- **Si tu monitor tampoco lo permite, te quedás sin nada.** Muchos monitores, sobre todo los de notebooks, no tienen un ajuste de saturación en su menú.

Esta app usa la API oficial del driver de Intel para escribir directamente en el pipeline de color de la placa, sin ese tope: la saturación llega hasta 400 (100 = sin cambios).

### Probado en juego

- **CS2 en pantalla completa exclusiva:** se jugó con la app abierta, con los ajustes aplicados todo el tiempo y sin problemas.
- **FACEIT:** se jugó una partida de FACEIT con la app corriendo, sin avisos ni problemas.

La app no abre, lee ni inyecta nada en el juego. Aun así, solo Valve y FACEIT pueden garantizar que un programa no cause un ban. Detalles y antecedentes en [docs/SEGURIDAD.md](docs/SEGURIDAD.md).

### Por qué Rust

La app está escrita en **Rust** para que gaste lo mínimo mientras jugás: se compila a código nativo, sin máquina virtual ni recolector de basura, y la interfaz (Slint) se dibuja solo cuando la ventana está visible. Minimizada en la bandeja prácticamente no usa CPU.

### Qué hace

- **Saturación** de 0 a 400, **contraste** y **brillo** de 50 a 200 (100 = sin cambios).
- Debajo de cada control hay una **marca en el 100**: al tocarla, el valor vuelve a 100.
- Recuerda los valores y puede **iniciar con Windows**, minimizada en la bandeja.
- **Restablecer** vuelve todo a la imagen normal.
- Cerrar la ventana la deja en la bandeja; **Salir** desde el ícono restaura la imagen y cierra.

### Requisitos

- Windows 10 u 11 de 64 bits.
- Pantalla conectada a una placa **Intel** con su driver instalado. NVIDIA y AMD todavía no están soportadas.

### Instalación

1. Descargar `AjustesDePantalla-Setup-<versión>.exe` desde [Releases](https://github.com/kenymtz/saturacion-windows/releases) y ejecutarlo. No pide permisos de administrador.
2. Si Windows SmartScreen muestra "Windows protegió su PC", elegir **Más información** → **Ejecutar de todas formas** (el instalador no está firmado).
3. El instalador crea el acceso directo en el escritorio, la entrada en el menú Inicio y, si se deja marcado, el inicio con Windows.

Para desinstalar: Configuración → Aplicaciones → "Ajustes de pantalla". Los valores guardados quedan en `%APPDATA%\Ajustes de pantalla`.

### Compilar

Requisitos: Rust (toolchain `stable-x86_64-pc-windows-msvc`), Build Tools de C++ de Visual Studio e Inno Setup 6 para el instalador. Si Windows tiene Smart App Control activado, bloquea los ejecutables sin firma que genera Rust y hay que desactivarlo para compilar.

```bash
cargo test
```

```bash
powershell -ExecutionPolicy Bypass -File scripts/build-installer.ps1
```

El instalador queda en `dist/`. Las especificaciones y planes de cada historia están en `docs/specs/` y `docs/plans/`.

---

## English

### What it is

**Ajustes de pantalla** ("Display settings") is a Windows app for gamers that raises screen **saturation** beyond the limit set by Windows and the manufacturer's control panel, and also adjusts **contrast** and **brightness**. Changes are applied on the graphics card, so they also show inside games running in exclusive fullscreen.

### The problem it solves

On many PCs there is no way to raise saturation enough:

- **Intel caps saturation.** Intel's graphics control panel has a low ceiling, far below what tools such as NVIDIA's *Digital Vibrance* offer.
- **Windows has no saturation control.**
- **If your monitor doesn't offer it either, you're left with nothing.** Many monitors, especially laptop panels, have no saturation setting in their menu.

This app uses Intel's official driver API to write directly to the card's colour pipeline, without that cap: saturation goes up to 400 (100 = unchanged).

### Tested in game

- **CS2 in exclusive fullscreen:** played with the app open, adjustments applied the whole time, with no problems.
- **FACEIT:** a FACEIT match was played with the app running, with no warnings or problems.

The app never opens, reads or injects into the game. Even so, only Valve and FACEIT can guarantee a program won't cause a ban. Details and precedents (in Spanish) in [docs/SEGURIDAD.md](docs/SEGURIDAD.md).

### Why Rust

The app is written in **Rust** so it uses as little as possible while you play: it compiles to native code with no virtual machine or garbage collector, and the UI (Slint) only renders while the window is visible. Minimized to the tray it uses practically no CPU.

### Features

- **Saturation** from 0 to 400, **contrast** and **brightness** from 50 to 200 (100 = unchanged).
- A **mark at 100** under each slider: click it to snap the value back to 100.
- Remembers your values and can **start with Windows**, minimized to the tray.
- **Restablecer** (Reset) brings everything back to the normal image.
- Closing the window keeps the app in the tray; **Salir** (Quit) from the tray icon restores the image and exits.

The interface is in Spanish.

### Requirements

- Windows 10 or 11, 64-bit.
- A display connected to an **Intel** graphics card with its driver installed. NVIDIA and AMD are not supported yet.

### Installation

1. Download `AjustesDePantalla-Setup-<version>.exe` from [Releases](https://github.com/kenymtz/saturacion-windows/releases) and run it. No administrator rights needed.
2. If Windows SmartScreen shows "Windows protected your PC", choose **More info** → **Run anyway** (the installer is not code-signed).
3. The installer creates a desktop shortcut, a Start menu entry and, if left checked, starts the app with Windows.

To uninstall: Settings → Apps → "Ajustes de pantalla". Saved values stay in `%APPDATA%\Ajustes de pantalla`.

### Building

Requirements: Rust (toolchain `stable-x86_64-pc-windows-msvc`), Visual Studio C++ Build Tools, and Inno Setup 6 for the installer. If Windows Smart App Control is on, it blocks the unsigned executables Rust produces and must be turned off to build.

```bash
cargo test
```

```bash
powershell -ExecutionPolicy Bypass -File scripts/build-installer.ps1
```

The installer is written to `dist/`. Specs and plans for each story are in `docs/specs/` and `docs/plans/` (in Spanish).
