# Seguridad con CS2 y FACEIT

Solo Valve y FACEIT pueden garantizar que un programa no cause un ban. Este documento reúne qué hace la app, por qué el riesgo es bajo y qué queda abierto.

## Qué hace la app

- Escribe dos ajustes en el pipeline de pantalla de la placa Intel: una matriz de color 3×3 (saturación) y la curva 1D de salida (contraste y brillo).
- Lo hace con la API pública de Intel, Intel Graphics Control Library (`ControlLib.dll`), la misma que usa el panel de Intel. La DLL se carga solo desde `C:\Windows\System32`, donde la instala el driver.
- El cambio ocurre en la placa, después de que el juego entrega la imagen.

## Qué no hace

- No abre, lee ni escribe el proceso del juego.
- No inyecta código ni carga DLL en el juego.
- No instala drivers propios.
- No usa la red.

## Antecedentes

- **vibranceGUI** hace lo mismo con las APIs de NVIDIA y AMD (sube la saturación desde el driver). FACEIT lo incluye en su lista de programas que no causan ban: [What is deemed to be a cheat?](https://support.faceit.com/hc/en-us/articles/360015788779-What-is-deemed-to-be-a-cheat). Su autor indica que es seguro con VAC: [guía en Steam](https://steamcommunity.com/sharedfiles/filedetails/?id=752422230).
- **AMD Anti-Lag+ (2023)**: Valve baneó a quienes lo activaron en CS2 porque el driver desviaba funciones de las DLL del juego, y después revirtió esos bans: [PC Gamer](https://pcgamer.com/valve-warns-counter-strike-2-players-use-amds-anti-lag-feature-get-banned). Es el ejemplo de lo que esta app no hace.

## Transparencia

El código es público: <https://github.com/kenymtz/saturacion-windows>. Cualquiera puede comprobar lo anterior. El ejecutable lleva nombre de producto, descripción y versión en sus propiedades.

## Riesgo residual

- FACEIT puede avisar que un programa que no conoce no se puede usar con su anticheat activo. Según su política, es un aviso que pide cerrarlo, no un ban.
- El ejecutable y el instalador no están firmados digitalmente (requiere un certificado pago). Windows SmartScreen puede mostrar un aviso al instalar.

## Mensaje para soporte de FACEIT

> Hello, I use a small open-source Windows app, "Ajustes de pantalla" (https://github.com/kenymtz/saturacion-windows), to raise my screen saturation above the Intel control panel's limit, similar to what vibranceGUI does for NVIDIA and AMD. It only calls Intel's official Graphics Control Library (ControlLib.dll, installed with the driver) to set the display pipeline's colour matrix and output curve. It never opens, reads or injects into the game process, installs no drivers and uses no network. Could you confirm it is allowed while the FACEIT Anti-Cheat is running? Thank you.

## Prueba con el cliente de FACEIT

2026-11-13: con la app 1.0.0 instalada, corriendo en la bandeja y con ajustes aplicados, el usuario abrió el cliente de FACEIT y no apareció ningún aviso sobre la app. Es una prueba puntual en una sola PC, no una certificación de FACEIT.
