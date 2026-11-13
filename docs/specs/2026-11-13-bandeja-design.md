# App minimizada en la bandeja del sistema

Fecha: 2026-11-13
Estado: implementada y verificada en hardware (plan: `docs/plans/2026-11-13-bandeja.md`)
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
- Limitación encontrada en la revisión: en Slint 1.18.1 crear y mostrar el componente no tocan todavía la bandeja del sistema; el registro real del ícono ocurre después y, si falla, Slint solo lo anota en su log de depuración. Por eso el mensaje de bandeja no disponible prácticamente no se puede disparar. Si el registro fallara, el ícono no mantiene vivo el bucle: la X oculta la ventana, el bucle termina y la app restaura y sale, sin quedar invisible. Se pierde solo el aviso.

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
- Restaurar al cerrar sesión o apagar Windows. Riesgo conocido: con la app en la bandeja es más probable cerrar sesión o apagar sin usar "Salir"; Windows termina el proceso sin que corra la restauración, y lo mismo pasa si se lo mata desde el Administrador de tareas. Los ajustes quedan aplicados hasta el próximo evento de energía o reinicio del driver (la app no usa el flag de persistencia de IGCL, así que no sobreviven a un reinicio).

## Resultado de la verificación de aceptación

- Fecha: 2026-11-13, con la versión release del commit 2010d94.
- El usuario confirmó en pantalla: el ícono aparece en la ventana y en la bandeja con el tooltip "Ajustes de pantalla"; la X oculta la ventana (también de la barra de tareas) con los ajustes aplicados; el clic izquierdo y "Abrir" vuelven a mostrar la ventana con los valores intactos; con la ventana oculta los ajustes se mantienen en un juego en pantalla completa exclusiva y el consumo de CPU es prácticamente nulo; "Salir" restaura la imagen y quita el ícono; abrir y salir sin tocar no cambia la imagen.
- No verificado a mano: el cierre con la inicialización fallida (sin driver de Intel) y la falla de registro del ícono (no reproducibles en esta máquina).
