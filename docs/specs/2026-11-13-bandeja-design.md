# App minimizada en la bandeja del sistema

Fecha: 2026-11-13
Estado: diseño aprobado, pendiente de plan de implementación
Depende de: historias 1 a 3. La rama sale de la historia 3 (`story-3-brightness`) mientras su PR siga abierto.

## Pedido

> Que la app pueda quedar corriendo minimizada en la bandeja, con los ajustes aplicados mientras juego.

Hoy cerrar la ventana restaura la imagen y termina la app, así que para mantener los ajustes hay que dejar la ventana abierta.

## Comportamiento

- **X de la ventana:** oculta la ventana. La app sigue corriendo, con los ajustes aplicados, y queda solo el ícono en la bandeja (no aparece en la barra de tareas).
- **Clic izquierdo en el ícono:** vuelve a mostrar la ventana, con los sliders en los valores que tenían.
- **Clic derecho en el ícono:** menú con **Abrir** (muestra la ventana) y **Salir** (restaura la imagen y termina la app).
- **Tooltip del ícono:** "Ajustes de pantalla".
- **Si la inicialización falló** (sin driver de Intel, sin pantallas, sin matriz): la X termina la app directamente, porque no hay ajustes que mantener. El ícono igual aparece mientras la ventana está abierta, y su "Salir" también termina.
- Al terminar por cualquier camino ("Salir", X con la inicialización fallida, error del bucle de eventos o pánico), se restaura la imagen como hasta ahora: solo los bloques que la app escribió.
- La ventana usa el mismo ícono que la bandeja.

## Decisión técnica

Se usa el componente `SystemTrayIcon` que trae Slint (presente en la versión 1.18.1 que usa el proyecto; en Windows usa el área de notificación del shell). No hace falta ninguna dependencia nueva.

Alternativa descartada:

| Alternativa | Motivo |
|---|---|
| Crate `tray-icon` | Necesita coordinar a mano su bucle de mensajes con el de Slint; el componente nativo de Slint ya resuelve eso. |

Puntos del componente que el diseño usa (documentación de Slint):

- Un `SystemTrayIcon` visible mantiene vivo el bucle de eventos igual que una ventana visible. Por eso, con el ícono presente, ocultar la ventana no termina la app.
- Tiene un único `Menu` hijo con `MenuItem` (callback `activated`), y el callback `clicked` para el clic izquierdo en Windows.
- Se corre con `slint::run_event_loop()` y se termina con `slint::quit_event_loop()`.
- Un `SystemTrayIcon` exportado junto a una ventana no comparte globales con ella; el diseño no usa globales.

## Arquitectura

| Pieza | Cambio |
|---|---|
| `ui/tray.slint` (nuevo) | `export component AppTray inherits SystemTrayIcon` con `icon` (de `ui/icon.png`), `tooltip: "Ajustes de pantalla"`, `Menu` con `MenuItem` "Abrir", separador y `MenuItem` "Salir"; callbacks `open()` y `quit()`; `clicked => { root.open(); }`. |
| `ui/app.slint` | Reexporta `AppTray` desde `tray.slint` (el build compila un solo archivo) y asigna `icon` a la ventana con la misma imagen. |
| `ui/icon.png` (nuevo) | Ícono simple de 64×64 generado por un script reproducible commiteado junto al ícono. Reemplazable por uno del usuario. |
| `src/main.rs` | Crea la ventana y el ícono. Registra: `on_close_requested` de la ventana → si hay controlador, ocultar la ventana (la app sigue); si no, terminar el bucle. `open` → mostrar la ventana. `quit` → terminar el bucle. Muestra la ventana y corre `slint::run_event_loop()`. Al volver del bucle, restaura con `shutdown()` aunque el bucle haya terminado con error. |

`controller`, `backend`, `color` e `igcl` no cambian. La restauración ante un pánico sigue cubierta por el `Drop` del controlador.

## Manejo de errores

- Si crear el ícono falla (Slint devuelve error al crear el componente), la app sigue funcionando como hasta ahora, sin bandeja: la X termina la app y restaura. Se registra el motivo en la nota de error de la ventana: "No se pudo crear el ícono de la bandeja; cerrar la ventana termina la app."
- Los errores de escritura y de inicialización se muestran como en la historia 3.

## Verificación

Esta historia es casi toda integración con la interfaz; no agrega lógica que se pueda probar con tests unitarios. Los 38 tests existentes deben seguir pasando.

Comprobación temprana: después de agregar el ícono, el usuario confirma que aparece en la bandeja, que el clic izquierdo y el menú responden.

## Criterios de aceptación

1. La X oculta la ventana; el ícono queda en la bandeja y los ajustes siguen aplicados.
2. El clic izquierdo reabre la ventana con los sliders en sus valores.
3. "Abrir" muestra la ventana; "Salir" restaura la imagen y termina la app.
4. Con la app en la bandeja, los ajustes se mantienen en un juego en pantalla completa exclusiva.
5. Con la ventana oculta, el uso de CPU de la app es prácticamente nulo en el Administrador de tareas.
6. Abrir y cerrar con "Salir" sin tocar nada no cambia la imagen.

## Fuera de alcance

- Arrancar con Windows ya minimizada: va con la historia 9 (recordar valores).
- Evitar dos instancias a la vez. Riesgo conocido: con la app oculta es más fácil abrirla otra vez sin notarlo; las dos instancias escribirían los mismos bloques y al salir una restauraría la imagen de la otra. Se resuelve aparte.
- Notificaciones o globos del ícono.
