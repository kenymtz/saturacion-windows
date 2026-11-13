# Historia 3: ajustar el brillo desde la misma app

Fecha: 2026-11-13
Estado: implementada y verificada en hardware (plan: `docs/plans/2026-11-13-brillo.md`)
Depende de: historia 1 (`docs/specs/2026-11-13-slider-saturacion-design.md`) e historia 2 (`docs/specs/2026-11-13-contraste-design.md`). La rama sale de la historia 2 mientras su PR siga abierto.

## Historia

> Como jugador, quiero ajustar el brillo desde la misma app, para aclarar u oscurecer la imagen sin ir a otro panel.

## Contexto y restricciones

- Stack: Rust + Slint, sobre el código de las historias 1 y 2.
- El usuario juega en **pantalla completa exclusiva**.
- Máquina de referencia: Intel UHD Graphics (driver 32.0.101.7077), pantalla interna 1920×1080 a 165 Hz. En la historia 2 se comprobó en esta placa que la curva 1D de salida (2049 muestras × 3 canales) se escribe en unos 6,5 ms, se mantiene en pantalla completa exclusiva y se restaura con `RESTORE_DEFAULT`.
- La app no inyecta código ni lee memoria de ningún juego.

## Decisión de mecanismo

El brillo se aplica en la **misma curva 1D de salida que el contraste**. La app guarda el valor de contraste y el de brillo, y cada vez que cambia cualquiera de los dos reescribe la curva combinada. El backend y el trait `ColorBackend` no cambian: `apply_tone_curve` ya acepta cualquier función.

Alternativas descartadas:

| Alternativa | Motivo |
|---|---|
| Curva 1D de entrada (la otra que reporta la UHD, 1 canal) como bloque propio del brillo | No se sabe qué tiene cargado por defecto ni cómo interactúa con la matriz de saturación; requeriría otra prueba de riesgo completa. |
| Brillo como suma (`y = x + b`) | El usuario eligió que el negro siga negro y la imagen no se lave; una suma eleva el negro a gris. |

## Alcance

Incluye:

- Tercer slider "Brillo" en la misma ventana, con su valor numérico.
- Aplicación en vivo del brillo, combinable con saturación y contraste.
- Restauración de la imagen normal al cerrar la app (ya cubierta por la restauración de la curva de la historia 2).
- Si la placa no ofrece la curva, contraste y brillo quedan deshabilitados juntos, con una nota; la saturación sigue funcionando.
- Título de la ventana "Ajustes de pantalla" y alto de ventana ajustado al contenido.

No incluye: reacción visual de los controles (#4), validación completa dentro de juegos (#5), NVIDIA/AMD (#7), persistencia (#9), reset rápido (#10), ni el riesgo de la matriz de saturación en enlaces YCbCr anotado en la historia 2 (se trata aparte).

## Comportamiento

- El slider de brillo va de **50 a 200** en pasos enteros. **100 es la imagen sin modificar.**
- Por encima de 100, los tonos medios y las sombras se aclaran; el negro puro y el blanco puro no cambian. Por debajo de 100, los medios se oscurecen.
- Al abrir, el slider está en 100 y la app no escribe la curva hasta que se mueve el contraste o el brillo.
- Mover el brillo conserva el contraste vigente y viceversa: la curva escrita siempre combina los dos valores actuales.
- Mover solo la saturación no escribe la curva; mover el contraste o el brillo no escribe la matriz.
- Al cerrar, se restauran solo los bloques que la app escribió (comportamiento de la historia 2).

## Curva de brillo

Con `b = valor / 100` y `x` entre 0 y 1:

```
brillo(b, x) = x^(1 / b)
```

La curva completa que se escribe es, con `c` el contraste de la historia 2:

```
tono(c, b, x) = brillo(b, contraste(c, x))
```

Es decir, primero el contraste y después el brillo. Como `contraste` devuelve valores entre 0 y 1, `brillo` siempre recibe valores en ese rango.

Propiedades que los tests verifican:

- `b = 1` da `y = x`.
- `x = 0` da 0 y `x = 1` da 1 para cualquier `b`.
- Con `b > 1`, `y ≥ x`; con `b < 1`, `y ≤ x`.
- La curva es monótona no decreciente.
- Con `b = 2`, `x = 0.25` da 0.5; con `b = 0.5`, `x = 0.5` da 0.25.
- `tono(1, 1, x) = x`, y `tono(c, b, x)` es igual a `brillo(b, contraste(c, x))`.

### Orden respecto de la saturación

En la UHD la matriz de saturación es el último bloque, después de la curva de salida. En la historia 2 eso no cambiaba el resultado salvo en los recortes, porque el contraste es afín. El brillo es una potencia, no es afín, así que aplicar la saturación después de la curva no da exactamente lo mismo que aplicarla antes. La diferencia es leve (la matriz conserva los grises y el brillo no cambia el negro ni el blanco) y se acepta.

## Arquitectura

| Módulo | Cambio |
|---|---|
| `color` | Funciones puras nuevas `brightness_curve(b: f64, x: f64) -> f64` y `tone_curve(c: f64, b: f64, x: f64) -> f64`. |
| `backend` | Solo cambia el texto de `ToneCurveUnsupported` a "Esta placa o este modo de pantalla no permite ajustar el contraste ni el brillo." |
| `controller` | Guarda `contrast` y `brightness` como factores (inician en 1.0). `set_contrast(percent)` actualiza el contraste y escribe `tone_curve(contraste, brillo, x)`. Nuevo `set_brightness(percent)`, limitado a 50–200 con `MIN_BRIGHTNESS_PERCENT = 50` y `MAX_BRIGHTNESS_PERCENT = 200`, que actualiza el brillo y escribe la misma curva combinada. `supports_contrast` se renombra `supports_tone_curve`. El valor se guarda aunque la escritura falle, para que el próximo movimiento use el valor que muestra la UI. |
| `igcl` | Sin cambios. |
| `ui` | Tercer slider "Brillo". La propiedad que habilitaba solo el contraste pasa a habilitar contraste y brillo (`tone-enabled`), y la nota pasa a `tone-note`. Título "Ajustes de pantalla". Alto de ventana según el contenido, sin valor fijo. |
| `main` | Conecta el slider de brillo con `set_brightness`. El cableado de los tres sliders usa una función común en lugar de tres bloques casi iguales. Si `supports_tone_curve()` es falso, deshabilita contraste y brillo y muestra la nota. |
| `examples/probe.rs` | Acepta un tercer argumento opcional de brillo y escribe la curva combinada, para diagnóstico. |

## Flujo de datos

```
slider brillo (50–200) → main → Controller::set_brightness
    → guarda b = valor / 100
    → ColorBackend::apply_tone_curve(&|x| color::tone_curve(c, b, x))
    → IGCL → curva 1D de salida
```

`set_contrast` sigue el mismo camino guardando `c`.

## Manejo de errores

Sin casos nuevos. Los de la historia 2 se mantienen, con el mensaje de `ToneCurveUnsupported` actualizado:

| Caso | Mensaje al usuario |
|---|---|
| Ninguna pantalla activa reporta una curva 1D con muestreo uniforme | Esta placa o este modo de pantalla no permite ajustar el contraste ni el brillo. (nota bajo los sliders, ambos deshabilitados) |
| La escritura de la curva falla | No se pudo aplicar el ajuste (con el código de IGCL); el slider sigue habilitado. |

## Verificación

No hay prueba de riesgo aparte: el bloque, la restauración, la pantalla completa exclusiva y los tiempos de escritura se comprobaron en la historia 2 y el brillo solo cambia la forma de la curva. Las comprobaciones visuales van en la verificación de aceptación.

## Pruebas

- **Unitarias en `color`**: las propiedades de `brightness_curve` y de `tone_curve` listadas arriba.
- **Unitarias del controlador** con el backend falso: un valor de brillo produce la curva esperada (muestras literales); el brillo se recorta a 50–200; mover el brillo conserva el contraste vigente y mover el contraste conserva el brillo vigente; mover el brillo no escribe la matriz; al cerrar se restaura después de una escritura de brillo.

## Criterios de aceptación

- Con el brillo por encima de 100, sombras y medios más claros; con 50, más oscuros; el negro y el blanco puros no cambian.
- El cambio se ve mientras se arrastra el slider.
- Saturación, contraste y brillo se combinan; mover uno no deshace los otros.
- El efecto se mantiene en pantalla completa exclusiva.
- Al cerrar, la imagen vuelve a como estaba.
- En una placa sin curva 1D uniforme, contraste y brillo quedan deshabilitados con su nota y la saturación sigue funcionando.

## Resultado de la verificación de aceptación

- Fecha: 2026-11-13, con la versión release del commit 9e10af6.
- El usuario confirmó en pantalla: ventana "Ajustes de pantalla" con los tres sliders en 100, sin nota ni error y sin cambio de imagen al abrir; el brillo cambia en vivo (200 aclara sombras y medios, 50 los oscurece) sin mover el negro ni el blanco puros; saturación, contraste y brillo se combinan y mover uno no deshace los otros; el brillo aumentado se mantiene en un juego en pantalla completa exclusiva; al cerrar con valores altos la imagen vuelve a la normal; abrir y cerrar sin tocar no cambia la imagen.
- No verificado a mano: el camino sin curva 1D uniforme (no reproducible en esta placa; cubierto por `backend_without_tone_curve_reports_it_as_unsupported`, `supports_tone_curve_reflects_backend` y la rama de `main` que deshabilita contraste y brillo) y el acomodo de la ventana cuando aparecen la nota o un mensaje de error (cosmético).
