# Marca en el 100 de cada slider

Fecha: 2026-11-13
Estado: implementada y verificada en pantalla
Relacionado: historia 4 del backlog ("que los controles reaccionen visualmente"). El usuario eligió mantener la ventana simple y pidió solo esto.

## Pedido

> Dejale una marquita en el 100, que cuando toques la marquita vaya directo a 100. Nada más.

## Comportamiento

- Debajo de cada slider (saturación, contraste, brillo) hay una rayita gris chica alineada con el valor 100.
- Al pasar el mouse por encima, la rayita se pone azul y el cursor cambia a una mano.
- Al tocarla, el slider va a 100. El cambio se aplica a la pantalla y se guarda, igual que arrastrar el slider y soltarlo.
- Si el slider ya está en 100, tocarla no hace nada.
- Si el slider está deshabilitado (por ejemplo, contraste y brillo cuando la placa no tiene curva), la rayita no responde.
- Nada más cambia en la ventana, salvo unos píxeles de alto por la fila de la rayita.

## Diseño

- Componente nuevo `ui/neutral-slider.slint` (`NeutralSlider`): envuelve el `Slider` de Slint con las mismas propiedades (`minimum`, `maximum`, `step`, `enabled`, `value`) y los mismos callbacks (`changed`, `released`), y agrega `neutral` (por defecto 100).
- La rayita va en una fila propia debajo del slider, no sobre la barra: así el clic lo recibe la rayita y el valor queda exactamente en 100 (sobre la barra, el slider saltaría al píxel tocado y podría quedar en 99 o 101).
- Posición horizontal: el estilo Fluent de Slint 1.18.1 dibuja el recorrido del slider desde la mitad del círculo (10 px) hasta el ancho menos 10 px, así que el centro de la rayita va en `10px + (ancho − 20px) × (neutral − mínimo) / (máximo − mínimo)`.
- Al tocarla, el componente pone `value` en `neutral` y emite `changed(neutral)` y `released(neutral)`. Los manejadores existentes de `main.rs` aplican y guardan; el código Rust no cambia.
- `ui/app.slint` reemplaza los tres `Slider` por `NeutralSlider`.

## Verificación

- Sin tests nuevos (solo interfaz); los 60 existentes siguen pasando.
- Manual: tocar la rayita de cada slider lleva el valor a 100 y la imagen cambia; al cerrar y reabrir, el valor sigue en 100.

## Limitación

La posición depende de la geometría del estilo Fluent de Slint 1.18.1. Si cambia el estilo o la versión de Slint, hay que revisar la fórmula.

## Resultado de la verificación

- Fecha: 2026-11-13, con la versión release del commit 33a1a11.
- El usuario confirmó que la rayita queda justo debajo del 100 y que al tocarla el slider va a 100 y la imagen cambia.
- El archivo de ajustes confirmó el guardado: tras tocar la marca de saturación, `ajustes.txt` pasó a `saturacion=100` con los demás valores intactos.
