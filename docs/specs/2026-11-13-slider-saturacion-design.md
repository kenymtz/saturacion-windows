# Historia 1: slider de saturación por encima de 100

Fecha: 2026-11-13
Estado: diseño aprobado, pendiente de plan de implementación

## Historia

> Como jugador, quiero subir la saturación de mi pantalla por encima de 100 con un control deslizante, para ver colores más vivos de lo que Windows o el panel de Intel permiten.

## Contexto y restricciones

- Stack: Rust + Slint.
- El usuario juega en **pantalla completa exclusiva**. El mecanismo tiene que actuar en el pipeline de pantalla de la placa, no en la composición del escritorio.
- Máquina de referencia: notebook con Intel UHD Graphics (driver 32.0.101.7077) y NVIDIA RTX 5050. La pantalla interna (1920×1080, 165 Hz) está conectada a la Intel, por eso el tope lo pone el panel de Intel y el control de NVIDIA no aplica.
- La app no inyecta código ni lee memoria de ningún juego (relevante para la historia 6).

## Decisión de mecanismo

Se usa **IGCL** (Intel Graphics Control Library), la API pública de Intel que se distribuye con el driver como `ControlLib.dll`. IGCL expone el pipeline de transformación de píxeles de la placa, que incluye un bloque de matriz de color 3×3. La app escribe en ese bloque una matriz de saturación.

Alternativas descartadas:

| Alternativa | Motivo |
|---|---|
| API de Lupa de Windows (`MagSetFullscreenColorEffect`) | Actúa sobre el escritorio compuesto; la pantalla completa exclusiva lo saltea. |
| Perfil ICC con tag MHC2 | Agnóstico de marca y a nivel de hardware, pero cada cambio exige regenerar y reasociar un perfil: lento para un slider y pisa el perfil del usuario. Queda como plan B. |
| `igfxDHLib.dll` (lo que usa Color-Goggles) | Interfaz no documentada, la DLL cambia entre versiones de driver y no se puede distribuir. |

## Alcance

Incluye:

- Ventana con un slider de saturación y su valor numérico.
- Aplicación en vivo del valor a las pantallas activas conectadas a la placa Intel.
- Restauración de la imagen normal al cerrar la app.
- Mensaje de error visible cuando el mecanismo no está disponible.

No incluye (otras historias): contraste (#2), brillo (#3), reacción visual de los controles (#4), validación dentro de juegos más allá de la prueba de riesgo (#5), anti-cheat (#6), NVIDIA/AMD (#7), optimización de consumo (#8), persistencia de valores (#9), reset rápido (#10).

## Comportamiento

- El slider va de **0 a 300** en pasos enteros. **100 es la imagen sin modificar**; 0 es escala de grises; 300 triplica la saturación.
- Al abrir, el slider está en 100 y la app no escribe nada en la placa hasta el primer movimiento.
- Cada cambio de valor se aplica de inmediato, sin botón de confirmar.
- El valor se aplica a todas las pantallas activas de la placa Intel que reporten el bloque de matriz 3×3.
- Al cerrar la ventana, la app escribe la matriz identidad en el bloque y termina. No se usa el flag de persistencia entre eventos de energía de IGCL.
- Mientras la app está abierta se considera dueña del bloque de matriz: un ajuste de tono o saturación hecho en el panel de Intel al mismo tiempo puede pisarse mutuamente con el de la app. Es una limitación aceptada.

## Matriz de saturación

Con `s = valor / 100` y los pesos de luma BT.709 `L = (0.2126, 0.7152, 0.0722)`:

```
M = s · I + (1 − s) · [L; L; L]
```

Es decir, `M[i][j] = (1 − s) · L[j] + (s si i == j, 0 si no)`.

Propiedades que los tests verifican:

- `s = 1` da la identidad.
- `s = 0` da tres filas iguales a `L` (escala de grises).
- Cada fila suma 1 para cualquier `s`, así que los grises no cambian.

La matriz se aplica sobre los valores tal como llegan al bloque (sin configurar los bloques de de-gamma y gamma), igual que el ejemplo oficial de tono/saturación de Intel.

## Arquitectura

Un único binario. Módulos:

| Módulo | Responsabilidad | Depende de |
|---|---|---|
| `color` | Función pura `saturation_matrix(s: f64) -> [[f64; 3]; 3]`. | nada |
| `backend` | Trait `ColorBackend` con `apply_matrix(&mut self, m)` y `restore_default(&mut self)`, y el tipo de error `BackendError`. | `color` (tipo de matriz) |
| `igcl` | Implementación de `ColorBackend` para Intel. Carga `ControlLib.dll` en tiempo de ejecución, enumera adaptadores y salidas, consulta capacidades y escribe la matriz. | `backend`, bindings FFI |
| `ui` | Ventana Slint: slider, valor numérico, área de mensaje de error, estado habilitado/deshabilitado. | Slint |
| `main` | Inicializa el backend, conecta el callback del slider con `saturation_matrix` + `apply_matrix`, y llama a `restore_default` al salir. | todos |

Límites:

- `color` no sabe nada de Intel ni de la UI.
- `ui` no conoce el backend; solo emite el valor y muestra estado.
- Todo lo `unsafe` y lo específico de IGCL queda dentro de `igcl`.
- El trait `ColorBackend` es el punto de extensión para la historia 7.

### Uso de IGCL

Secuencia de inicialización: `ctlInit` → `ctlEnumerateDevices` → `ctlEnumerateDisplayOutputs` → `ctlGetDisplayProperties` (filtrar salidas activas) → `ctlPixelTransformationGetConfig` para leer las capacidades y ubicar el bloque de tipo `CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX`.

Aplicar: `ctlPixelTransformationSetConfig` con un solo bloque, operación `CTL_PIXTX_CONFIG_OPERTAION_TYPE_SET_CUSTOM`, la matriz en `Config.MatrixConfig.Matrix` y offsets en cero.

Restaurar: la misma llamada con la matriz identidad.

Cierre: `ctlClose`.

Los bindings FFI cubren solo esas funciones y sus structs. Se generan una vez con bindgen a partir del header `igcl_api.h` del repositorio oficial y se commitean, para que la compilación normal no requiera libclang. `ControlLib.dll` no se distribuye con la app: se carga la que instaló el driver.

## Flujo de datos

```
slider (0–300) → main: s = valor / 100 → color::saturation_matrix(s)
              → ColorBackend::apply_matrix → IGCL → bloque 3×3 de la placa
```

Las llamadas se hacen en el hilo de la UI. Si la prueba de riesgo muestra que una escritura tarda lo suficiente como para trabar el slider, el plan agrega coalescencia al último valor; de entrada no se implementa.

## Manejo de errores

`BackendError` distingue:

| Caso | Mensaje al usuario |
|---|---|
| `ControlLib.dll` no se encuentra o `ctlInit` falla | No se encontró el driver de gráficos de Intel. |
| No hay adaptador Intel con pantallas activas | Ninguna pantalla activa está conectada a la placa Intel. |
| Ninguna pantalla reporta el bloque de matriz 3×3 | Esta placa o este modo de pantalla no permite ajustar el color. |
| La escritura de la matriz falla | No se pudo aplicar el ajuste (con el código de error de IGCL). |

Si la inicialización falla, la ventana se abre igual, muestra el mensaje y deja el slider deshabilitado. Si falla una escritura, se muestra el mensaje y el slider sigue habilitado para reintentar.

Si falla la restauración al cerrar, la app termina igual; no hay a quién mostrarle el error.

## Prueba de riesgo previa

Antes de construir la UI, un binario de consola mínimo (parte del mismo crate, como ejemplo o subcomando) que aplica un factor dado y espera una tecla para restaurar. Debe confirmar en la máquina de referencia:

1. La UHD reporta un bloque `CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX`.
2. El driver acepta la matriz para `s = 3` y el resultado visible es más saturado que con `s = 1.25` (el máximo del ejemplo de Intel), es decir, no se recorta.
3. Con un factor aplicado, el efecto se mantiene al entrar a un juego en pantalla completa exclusiva.
4. Tiempo de una escritura, para decidir si hace falta coalescencia.

Si 1 o 3 fallan, se detiene la implementación y se vuelve a elegir mecanismo (plan B: MHC2). Si 2 falla, se ajusta el máximo del slider al mayor factor que el driver respete, siempre que supere el tope del panel de Intel; si no lo supera, también se vuelve a elegir mecanismo.

## Pruebas

- **Unitarias en `color`**: las tres propiedades de la matriz.
- **Unitarias de la lógica de conexión**: con un `ColorBackend` falso que registra llamadas, verificar que un valor del slider produce la matriz esperada, que un error de escritura llega a la UI como mensaje, y que al salir se llama a `restore_default`.
- **Manual en hardware**: la prueba de riesgo y, al final, mover el slider con una imagen de referencia en pantalla.

`igcl` no tiene tests automáticos: depende de hardware y driver reales.

## Criterios de aceptación

- Con el slider por encima de 100, los colores de la pantalla se ven más saturados que con el panel de Intel al máximo.
- El cambio se ve mientras se arrastra el slider.
- La prueba de riesgo pasó, incluida la comprobación en pantalla completa exclusiva (la validación completa dentro de juegos es la historia 5).
- Al cerrar la app, la imagen vuelve a la normal.
- En una máquina sin el mecanismo disponible, la app abre y explica por qué no puede actuar.

## Prerrequisitos de desarrollo

- Rust (rustup, toolchain `stable-x86_64-pc-windows-msvc`).
- Build Tools de C++ de Visual Studio (enlazador MSVC y Windows SDK).
- LLVM/libclang, solo para generar los bindings una vez.
