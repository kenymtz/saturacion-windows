# Historia 2: ajustar el contraste desde la misma app

Fecha: 2026-11-13
Estado: implementada y verificada en hardware (plan: `docs/plans/2026-11-13-contraste.md`)
Depende de: historia 1 (`docs/specs/2026-11-13-slider-saturacion-design.md`), ya mergeada en `main`.

## Historia

> Como jugador, quiero ajustar el contraste desde la misma app, para definir mejor la imagen.

## Contexto y restricciones

- Stack: Rust + Slint. Se construye sobre el código de la historia 1: `color`, `backend`, `controller`, `igcl`, `ui`.
- El usuario juega en **pantalla completa exclusiva**; el ajuste tiene que actuar en el pipeline de pantalla de la placa.
- Máquina de referencia: Intel UHD Graphics (driver 32.0.101.7077), pantalla interna 1920×1080 a 165 Hz conectada a la Intel.
- La app no inyecta código ni lee memoria de ningún juego.

## Decisión de mecanismo

El pipeline de pantalla de Intel expuesto por IGCL reporta cinco bloques, en este orden (confirmado en la prueba de riesgo): LUT 3D, curva 1D de 2049×1, matriz 3×3 con desplazamientos, curva 1D de 2049×3 y matriz 3×3.

- La saturación sigue en la **matriz 3×3 simple** (el último bloque), como en la historia 1.
- El contraste va en la **última curva 1D** (2049×3). Es la etapa que usa el ejemplo oficial de Intel (`TestBrightnessContrastGamma` en `Samples/Color_Samples/Color_Sample_App.cpp`) para brillo, contraste y gamma, y la misma curva servirá para el brillo de la historia 3.

Cada ajuste usa su propio bloque, así que uno no pisa al otro.

Alternativas descartadas:

| Alternativa | Motivo |
|---|---|
| Matriz con desplazamientos (`CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS`), combinando saturación y contraste en una escritura | Al diseñar no se sabía si la UHD exponía ese bloque; la prueba de riesgo mostró que sí (bloque 327427). Quedó como plan B, que no hizo falta porque la curva pasó todas las comprobaciones. |
| Matriz 3×3 sola (multiplicar por `c`) | Sin desplazamiento no puede pivotar en el gris medio: aclara u oscurece toda la imagen en vez de separar sombras y luces. |

## Alcance

Incluye:

- Segundo slider "Contraste" en la misma ventana, con su valor numérico.
- Aplicación en vivo del contraste, combinable con la saturación.
- Restauración de la imagen normal al cerrar la app.
- Mensaje de error visible y slider de contraste deshabilitado cuando la placa no ofrece la curva.
- Ampliación del programa de prueba para listar los bloques de la placa y aplicar contraste.

No incluye (otras historias): brillo (#3), reacción visual de los controles (#4), validación completa dentro de juegos (#5), NVIDIA/AMD (#7), persistencia (#9), reset rápido (#10). Tampoco incluye muestreo no uniforme de la curva (modo HDR).

## Comportamiento

- El slider de contraste va de **50 a 200** en pasos enteros. **100 es la imagen sin modificar.** Por encima de 100, las sombras se oscurecen y las luces se aclaran; el gris medio no cambia. Por debajo de 100, la imagen se aplana hacia el gris medio.
- Al abrir, el slider está en 100 y la app no escribe la curva hasta el primer movimiento de ese slider. Mover solo el slider de saturación no escribe la curva, y mover solo el de contraste no escribe la matriz.
- Cada cambio se aplica de inmediato, sin botón de confirmar.
- Contraste y saturación se combinan. En la UHD la matriz de saturación es el último bloque, así que la placa aplica la saturación **después** del contraste. El resultado es el mismo que en el orden inverso: las filas de la matriz de saturación suman 1 y la curva de contraste sin recorte es afín con un desplazamiento constante e igual en los tres canales, así que ambos órdenes coinciden salvo donde los valores se recortan en 0 o 1. La prueba en hardware confirmó que los dos efectos se combinan. Una curva futura no afín (por ejemplo gamma) no conmutaría con la matriz.
- La curva se aplica a todas las pantallas activas de la placa Intel que la reporten con muestreo uniforme.
- Al cerrar la ventana, si la app llegó a escribir algo en la placa, restaura los bloques que tocó y termina. Si no se movió ningún slider, no toca nada.
- Mientras la app está abierta se considera dueña de la curva de salida. Windows (luz nocturna, calibración) y algunos juegos también escriben esa curva y pueden pisarse con la app. Es una limitación aceptada; la prueba de riesgo comprueba cuánto afecta.

## Curva de contraste

Con `c = valor / 100` y `x` el valor de entrada normalizado entre 0 y 1:

```
y = clamp(0.5 + c · (x − 0.5), 0, 1)
```

Se aplica igual en los tres canales, sobre los valores tal como llegan a la curva (sin linealizar), igual que el ejemplo de Intel.

Propiedades que los tests verifican:

- `c = 1` da `y = x` para todo `x`.
- `x = 0.5` da `y = 0.5` para cualquier `c`.
- La curva es monótona no decreciente.
- La salida queda siempre entre 0 y 1; con `c = 2`, `x = 0` da 0 y `x = 1` da 1 (recorte).
- Con `c = 0.5`, `x = 0` da 0.25 y `x = 1` da 0.75.

## Arquitectura

| Módulo | Cambio | Depende de |
|---|---|---|
| `color` | Función pura nueva `contrast_curve(c: f64, x: f64) -> f64`. | nada |
| `backend` | El trait `ColorBackend` suma `apply_tone_curve(&mut self, curve: &dyn Fn(f64) -> f64) -> Result<(), BackendError>` y `supports_tone_curve(&self) -> bool`. `BackendError` suma la variante `ToneCurveUnsupported`. `restore_default` pasa a restaurar solo los bloques que el backend escribió: el backend recuerda si escribió la matriz, la curva o ambas, para no resetear una curva de Windows que la app nunca tocó. | `color` |
| `controller` | Guarda el valor de saturación y el de contraste. Suma `set_contrast(percent)` y las constantes `MIN_CONTRAST_PERCENT = 50` y `MAX_CONTRAST_PERCENT = 200`. El seguimiento de escrituras cubre ambos ajustes. | `backend`, `color` |
| `igcl` | Por cada pantalla activa, además del bloque de matriz, guarda la **última** curva 1D que reporta, si tiene muestreo uniforme. Escribe la curva muestreando la función en `NumSamplesPerChannel` puntos uniformes y repitiéndola en cada canal. Restaura la curva con la operación `CTL_PIXTX_CONFIG_OPERTAION_TYPE_RESTORE_DEFAULT` del driver, no escribiendo una curva identidad, porque la curva por defecto podría no serlo. | `backend`, bindings FFI |
| `ui` | Segundo slider "Contraste" con su valor. Propiedad propia para habilitarlo/deshabilitarlo. | Slint |
| `main` | Conecta el slider de contraste con `Controller::set_contrast`. Si `supports_tone_curve()` es falso, deshabilita solo el slider de contraste y muestra el mensaje. | todos |

El trait recibe la curva como función, no como arreglo, para que cada backend elija cuántas muestras y en qué posiciones (Intel informa la cantidad de muestras por pantalla; otras placas en la historia 7 pueden ser distintas).

### Uso de IGCL

- Capacidades: en la misma consulta `ctlPixelTransformationGetConfig` con `CTL_PIXTX_CONFIG_QUERY_TYPE_CAPABILITY` que ya se hace, buscar el último bloque `CTL_PIXTX_BLOCK_TYPE_1D_LUT`. Se acepta si su `SamplingType` es `CTL_PIXTX_LUT_SAMPLING_TYPE_UNIFORM`.
- Escribir: copia del bloque de capacidades con `Size` correcto, `Config.OneDLutConfig.Size` correcto, `NumSamplesPerChannel` y `NumChannels` tal como los informó el driver, y `pSampleValues` apuntando a un buffer de `NumSamplesPerChannel × NumChannels` valores `f64`, canal por canal (R, luego G, luego B), con la muestra `i` igual a `curve(i / (N − 1))`. Operación `SET_CUSTOM`, un bloque, `Flags = 0`.
- Restaurar: `ctlPixelTransformationSetConfig` con `CTL_PIXTX_CONFIG_OPERTAION_TYPE_RESTORE_DEFAULT` y el bloque de la curva. La matriz se sigue restaurando como en la historia 1.

## Flujo de datos

```
slider contraste (50–200) → main: c = valor / 100
    → ColorBackend::apply_tone_curve(&|x| color::contrast_curve(c, x))
    → IGCL → curva 1D de salida de la placa
```

La saturación sigue su camino de la historia 1. Las escrituras se hacen en el hilo de la UI; en la historia 1 una escritura de matriz tardó entre 1 y 1,5 ms. Si la prueba de riesgo mide más de 15 ms por escritura de curva, el plan agrega coalescencia al último valor.

## Manejo de errores

| Caso | Mensaje al usuario |
|---|---|
| Ninguna pantalla activa reporta una curva 1D con muestreo uniforme | Esta placa o este modo de pantalla no permite ajustar el contraste. |
| La escritura de la curva falla | No se pudo aplicar el ajuste (con el código de error de IGCL), igual que en la historia 1. |

- Si la placa no ofrece la curva, la ventana muestra el mensaje y deshabilita **solo** el slider de contraste; la saturación sigue funcionando.
- Si la inicialización general falla (sin driver, sin pantallas, sin matriz), se mantiene el comportamiento de la historia 1 y ambos sliders quedan deshabilitados.
- Si falla una escritura de curva, se muestra el mensaje y el slider sigue habilitado para reintentar.
- Si falla la restauración al cerrar, la app termina igual.

## Prueba de riesgo previa

Antes de tocar la ventana, ampliar `examples/probe.rs` para que liste los bloques que reporta cada pantalla (tipo, y para curvas 1D, muestreo, muestras por canal y canales) y acepte un valor de contraste además del de saturación. Debe confirmar en la máquina de referencia:

1. La UHD reporta una curva 1D con muestreo uniforme.
2. Con contraste 200 la imagen se ve con más contraste y el gris medio no cambia; con 50 se ve más plana.
3. Con contraste aplicado, el efecto se mantiene al entrar a un juego en pantalla completa exclusiva.
4. Con saturación y contraste aplicados juntos se ven los dos efectos.
5. La restauración deja la imagen como estaba antes de abrir la prueba. Si el usuario usa la luz nocturna de Windows, comprobar también cómo queda con ella activada.
6. Tiempo de una escritura de curva, para decidir si hace falta coalescencia.

Si 1 o 3 fallan, se detiene la implementación y se vuelve a elegir mecanismo (plan B: matriz con desplazamientos). Si 5 falla, se cambia la restauración a guardar la curva actual al abrir (`CTL_PIXTX_CONFIG_QUERY_TYPE_CURRENT`) y reescribirla al cerrar.

## Pruebas

- **Unitarias en `color`**: las cinco propiedades de la curva.
- **Unitarias del controlador** con el backend falso: un valor de contraste produce una escritura de curva con la función esperada (comparando muestras), valores fuera de rango se recortan a 50 y 200, mover solo un slider no escribe el otro bloque, un error de escritura llega como mensaje, y al cerrar se restaura si hubo escrituras y no se toca nada si no las hubo.
- **Manual en hardware**: la prueba de riesgo y, al final, ambos sliders con una imagen de referencia en pantalla.

`igcl` no tiene tests automáticos: depende de hardware y driver reales.

## Criterios de aceptación

- Con el contraste por encima de 100, sombras más oscuras y luces más claras; con 50, imagen más plana; el gris medio se mantiene.
- El cambio se ve mientras se arrastra el slider.
- Saturación y contraste se combinan sin pisarse.
- La prueba de riesgo pasó, incluida la comprobación en pantalla completa exclusiva.
- Al cerrar la app, la imagen vuelve a como estaba.
- En una placa sin curva 1D uniforme, el slider de contraste queda deshabilitado con su mensaje y la saturación sigue funcionando.

## Riesgos conocidos

- La matriz 3×3 simple que va después de la curva de salida probablemente es el CSC de salida de Intel (usado para RGB→YCbCr en enlaces YCbCr, como algunos televisores por HDMI). Escribir la saturación ahí y restaurarla escribiendo la matriz identidad (comportamiento de la historia 1) podría ser incorrecto en esos enlaces; sin probar porque la pantalla de referencia es RGB. Posible arreglo futuro: restaurar la matriz con `RESTORE_DEFAULT` tras una comprobación en hardware, o usar el bloque de matriz con desplazamientos (CSC de la tubería, antes de la curva de salida). Resuelto con ambos arreglos en `docs/specs/2026-11-13-bloque-matriz-saturacion-design.md`: la saturación pasó a la matriz con desplazamientos y la matriz se restaura con `RESTORE_DEFAULT`.
- Restaurar la curva con `RESTORE_DEFAULT` puede descartar una curva que Windows aplicó (luz nocturna, calibración ICC) hasta que Windows la vuelva a aplicar; la luz nocturna no se probó porque el usuario no la usa.

## Resultado de la prueba de riesgo

- Fecha: 2026-11-13
- Bloques reportados por pantalla: 1 pantalla (interna, Intel UHD Graphics, driver 32.0.101.7077) con, en este orden: LUT 3D (327425), curva 1D uniforme de 2049 muestras x 1 canal (327426), matriz 3x3 con desplazamientos (327427), curva 1D uniforme de 2049 muestras x 3 canales (327428) y matriz 3x3 (327429).
- Curva 1D uniforme disponible (sí/no), muestras y canales: sí. La app usa la última curva 1D (327428): 2049 muestras x 3 canales.
- 200 con más contraste y gris medio fijo (sí/no); 50 más plano (sí/no): sí y sí. Con 50 el usuario la describió como "muy blanca y los oscuros se ven casi grises": imagen lavada, con los negros elevados hacia el gris, lo esperado para contraste bajo.
- Se mantiene en pantalla completa exclusiva (sí/no), juego usado: sí, con contraste 200 aplicado; el usuario no especificó el juego.
- Combinado con saturación (sí/no): sí, con saturación 300 y contraste 150 se ven los dos efectos.
- Restauración deja la imagen como estaba (sí/no); luz nocturna (probada/no usada, resultado): sí, restaurando la curva con RESTORE_DEFAULT la imagen quedó normal. Luz nocturna no usada por el usuario; no probada.
- Tiempo de escritura de la curva (tres corridas): 6,56 ms (200), 6,48 ms (50), 6,53 ms (150); en las demás corridas entre 6,29 y 7,58 ms. Todas por debajo de 15 ms: la Task 5 usa escritura directa, sin coalescencia.

## Resultado de la verificación de aceptación

- Fecha: 2026-11-13, con la versión release que incluye los arreglos de la revisión final (commit 028204f).
- El usuario confirmó en pantalla: al abrir, ambos sliders en 100, sin nota ni error y sin cambio de imagen; el contraste cambia en vivo al arrastrar (200 con más contraste, 50 lavada) con el gris medio fijo; saturación y contraste altos se combinan sin pisarse; al cerrar con valores altos la imagen vuelve a la normal; abrir y cerrar sin tocar no cambia la imagen.
- No verificado a mano: el camino sin curva 1D uniforme (no reproducible en esta placa). Lo cubren los tests `backend_without_tone_curve_reports_it_as_unsupported` y `supports_contrast_reflects_backend`, y la rama de `main` que deshabilita solo el slider de contraste y muestra la nota.
