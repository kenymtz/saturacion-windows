# Bloque de matriz de la saturación y su restauración

Fecha: 2026-11-13
Estado: implementada y verificada en hardware (plan: `docs/plans/2026-11-13-bloque-matriz-saturacion.md`)
Origen: riesgo conocido (a) de `docs/specs/2026-11-13-contraste-design.md`.

## Problema

La saturación (historia 1) se escribe en el bloque `CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX`. En la Intel UHD de referencia el pipeline reporta, en este orden: LUT 3D (327425), curva 1D 2049×1 (327426), matriz 3×3 con desplazamientos (327427), curva 1D 2049×3 (327428) y matriz 3×3 (327429). La matriz 3×3 simple es el **último** bloque, después de la curva de salida, y probablemente es el CSC de salida de Intel: la etapa que convierte RGB→YCbCr cuando el enlace con la pantalla es YCbCr (algunos televisores por HDMI).

Hoy pasan dos cosas en esa matriz:

1. Mientras la app está abierta, la saturación reemplaza lo que el driver tuviera ahí.
2. Al cerrar, `restore_default` escribe la matriz identidad. En un enlace YCbCr la identidad no es el valor correcto, así que los colores quedarían mal después de cerrar la app.

La pantalla de referencia es RGB, así que esto no se puede reproducir en la máquina de referencia.

## Decisión

Se aplican los dos arreglos:

- **(b) Preferir la matriz con desplazamientos.** `matrix_block` elige primero `CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS` (el CSC de la tubería, antes de la curva de salida) y, solo si la pantalla no la reporta, la `CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX`. Los desplazamientos se escriben en 0.
- **(a) Restaurar la matriz con `RESTORE_DEFAULT`.** Al cerrar, la matriz se restaura con `CTL_PIXTX_CONFIG_OPERTAION_TYPE_RESTORE_DEFAULT`, igual que la curva desde la historia 2, en lugar de escribir la identidad. Así el driver vuelve a poner su propio valor, sea cual sea el bloque.

Alternativas descartadas:

| Alternativa | Motivo |
|---|---|
| Solo (a): seguir en la matriz 3×3 simple y restaurar con `RESTORE_DEFAULT` | Cambio mínimo, pero mientras la app está abierta sigue pisando el CSC de salida en enlaces YCbCr. |
| Solo (b): pasar a la matriz con desplazamientos y restaurar con la identidad | Si el driver usa el CSC de la tubería para algo propio (por ejemplo, ajuste de gama en paneles de gama amplia), la identidad lo perdería al cerrar. |
| Leer la matriz actual y componer la saturación sobre ella | `CTL_PIXTX_CONFIG_QUERY_TYPE_CURRENT` devuelve lo último que se escribió con una llamada set, no el valor por defecto del driver, así que no sirve para conocer el estado inicial. |

## Consecuencias

- En la UHD la saturación pasa a aplicarse **antes** de la curva de salida, es decir, antes del contraste y del brillo. Es el orden natural (primero el color, después el tono). Con la curva de contraste el resultado es el mismo salvo en el recorte; con la gamma de brillo de la historia 3, que no conmuta con la matriz, este orden es el esperado.
- La curva 1D de 2049×1 (327426) va antes de la matriz con desplazamientos. Si el driver la usa como degamma, la saturación actúa sobre valores linealizados. Como las filas de la matriz de saturación suman 1, los grises siguen neutros en cualquier espacio; la intensidad exacta del efecto puede cambiar un poco y se comprueba en hardware.
- En placas que solo reportan la matriz 3×3 simple el comportamiento al usar la app no cambia, pero la restauración ya no escribe la identidad.

## Arquitectura

Solo cambia `src/igcl/mod.rs` y, para el diagnóstico, `examples/probe.rs`.

| Pieza | Cambio |
|---|---|
| `matrix_block` | Función pura. Orden de preferencia: matriz con desplazamientos, luego matriz sola, si no `None`. |
| `restore_matrices` (nueva) | Por cada pantalla, `ctlPixelTransformationSetConfig` con `RESTORE_DEFAULT` y una copia del bloque de matriz, con `Config.MatrixConfig.Size` correcto. Intenta todas y devuelve el primer error, como `restore_curves`. |
| `restore_default` | Si escribió la matriz, llama a `restore_matrices` en lugar de `write_matrix(&IDENTITY)`. Se mantiene el seguimiento de bloques escritos. |
| `matrix_summary` (nueva) | Una línea por pantalla con el bloque que usa la saturación, para el probe. |
| `examples/probe.rs` | Muestra qué bloque usa la saturación. |

No cambia el trait `ColorBackend`, ni el controlador, ni la ventana.

## Pruebas

- **Unitarias de `matrix_block`** con bloques armados a mano:
  - con los cinco bloques de la UHD elige el 327427 (matriz con desplazamientos);
  - con solo la matriz 3×3 simple elige esa;
  - sin bloques de matriz devuelve `None`.
- `restore_matrices` no tiene test automático: depende del driver.
- **Manual en hardware** con `examples/probe.rs`, con el usuario mirando la pantalla:
  1. El probe informa que la saturación usa el bloque 327427.
  2. Con saturación 300 los colores se ven más vivos; con 0 la imagen queda en grises.
  3. Los grises quedan neutros (sin tinte) con saturación 300.
  4. El efecto se mantiene al entrar a un juego en pantalla completa exclusiva.
  5. Con saturación 300 y contraste 150 se ven los dos efectos.
  6. Al restaurar la imagen vuelve a la normal.

Si el bloque 327427 falla (la escritura da error, no se ve el efecto o no se mantiene en pantalla completa exclusiva), se vuelve a preferir la matriz 3×3 simple y se mantiene solo el arreglo (a).

## Riesgos que quedan

- Si el driver tiene un valor propio en el CSC de la tubería, la saturación lo reemplaza mientras la app está abierta; `RESTORE_DEFAULT` lo recupera al cerrar.
- El comportamiento en un enlace YCbCr real sigue sin probarse: la pantalla de referencia es RGB.

## Resultado de la verificación en hardware

- Fecha: 2026-11-13. Máquina de referencia: pantalla interna (RGB), Intel UHD Graphics, driver 32.0.101.7077.
- Bloque usado: el probe informó "saturación en bloque 327427: matriz 3x3 con desplazamientos". Las escrituras devolvieron éxito.
- Saturación 300: colores más vivos y grises neutros, sin tinte (sí).
- Saturación 0: imagen en grises, sin tinte (sí).
- Pantalla completa exclusiva: con saturación 300 el efecto se mantuvo dentro de un juego (sí); el usuario no especificó el juego.
- Saturación 300 con contraste 150: se ven los dos efectos (sí).
- Restauración con `RESTORE_DEFAULT`: en las tres corridas el probe imprimió "Restaurado." y la imagen volvió a la normal (sí).
- Tiempo de escritura de la matriz en el bloque 327427: 5,65 ms, 5,48 ms y 4,47 ms (en la matriz 3×3 simple de la historia 1 era de 1 a 1,5 ms). Sigue por debajo de los 15 ms que la historia 2 fijó como límite para escribir sin coalescencia.
- No hizo falta el plan alternativo: se mantiene la preferencia por la matriz con desplazamientos.
