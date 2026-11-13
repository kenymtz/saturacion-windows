# Marca en el 100 — Plan de implementación

**Goal:** Una rayita bajo el 100 de cada slider que, al tocarla, lleva el valor a 100, lo aplica y lo guarda.

**Architecture:** Componente Slint `NeutralSlider` que envuelve el `Slider` estándar y agrega una fila con la rayita clicable. Al tocarla emite los mismos callbacks que soltar el slider, así que `main.rs` no cambia.

**Tech Stack:** Slint 1.18.1 (estilo Fluent).

**Spec:** `docs/specs/2026-11-13-marca-neutra-design.md`

## Global Constraints

- Solo cambian `ui/neutral-slider.slint` (nuevo) y `ui/app.slint`. El código Rust no cambia.
- Centro de la rayita: `10px + (ancho − 20px) × (neutral − mínimo) / (máximo − mínimo)`.
- Rayita gris `#8a8a8a`, azul `#2d7ff9` al pasar el mouse; cursor de mano; no responde si el slider está deshabilitado; si el valor ya es el neutro no hace nada.
- Textos visibles en español. Mensajes de commit en inglés, sin trailers `Co-Authored-By` ni menciones a herramientas de IA.

---

### Task 1: Componente y uso en la ventana

**Files:**
- Create: `ui/neutral-slider.slint`
- Modify: `ui/app.slint`

- [ ] **Step 1: Crear `ui/neutral-slider.slint`**

```slint
import { Slider } from "std-widgets.slint";

// Slider con una marca en el valor neutro. Tocar la marca lleva el valor a ese punto y
// emite `changed` y `released`, igual que arrastrar y soltar.
export component NeutralSlider inherits VerticalLayout {
    in property <float> minimum: 0;
    in property <float> maximum: 100;
    in property <float> step: 1;
    in property <float> neutral: 100;
    in property <bool> enabled: true;
    in-out property <float> value;
    callback changed(float);
    callback released(float);

    spacing: 2px;

    Slider {
        minimum: root.minimum;
        maximum: root.maximum;
        step: root.step;
        enabled: root.enabled;
        value <=> root.value;
        changed(value) => {
            root.changed(value);
        }
        released(value) => {
            root.released(value);
        }
    }

    Rectangle {
        height: 10px;

        // El estilo Fluent dibuja el recorrido desde la mitad del círculo (10 px) hasta
        // el ancho menos 10 px.
        mark := TouchArea {
            width: 16px;
            height: parent.height;
            x: 10px + (parent.width - 20px) * (root.neutral - root.minimum) / (root.maximum - root.minimum) - self.width / 2;
            enabled: root.enabled;
            mouse-cursor: pointer;
            clicked => {
                if (root.value != root.neutral) {
                    root.value = root.neutral;
                    root.changed(root.neutral);
                    root.released(root.neutral);
                }
            }

            Rectangle {
                x: (parent.width - self.width) / 2;
                y: 0;
                width: 2px;
                height: 6px;
                border-radius: 1px;
                background: mark.has-hover && root.enabled ? #2d7ff9 : #8a8a8a;
            }
        }
    }
}
```

- [ ] **Step 2: Usarlo en la ventana**

En `ui/app.slint`:
- Después de la línea `export { AppTray } from "tray.slint";` agregar `import { NeutralSlider } from "neutral-slider.slint";`.
- Reemplazar la palabra `Slider {` por `NeutralSlider {` en los tres sliders (saturación, contraste, brillo). Sus propiedades y callbacks quedan iguales.
- Quitar `Slider` del import de `std-widgets.slint` si ya no se usa (queda `import { Button, CheckBox } from "std-widgets.slint";`).

Si el compilador de Slint no permite `inherits VerticalLayout`, envolver el contenido en un `VerticalLayout` dentro de un componente que herede de nada (`export component NeutralSlider { VerticalLayout { ... } }`) sin cambiar el comportamiento, y reportarlo.

- [ ] **Step 3: Compilar y correr los tests**

Run: `cargo build`
Expected: `Finished` sin advertencias en `src/` ni `ui/`.

Run: `cargo test`
Expected: `test result: ok. 60 passed`.

No correr la app.

- [ ] **Step 4: Commit**

```bash
git add ui/neutral-slider.slint ui/app.slint
git commit -m "feat: add a clickable mark at 100 under each slider"
```

---

### Task 2: Verificación con el usuario

- [ ] Abrir la app compilada (`cargo run --release`, cerrando antes la instalada con "Salir"). Expected: rayita bajo el 100 de cada slider; al tocarla el slider va a 100 y la imagen cambia; al cerrar y reabrir, el valor sigue en 100.
- [ ] Registrar el resultado en la spec (`Estado:` y sección `## Resultado de la verificación`) y commitear con `docs: record neutral mark verification`.
