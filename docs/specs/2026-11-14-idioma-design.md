# Idioma de la interfaz: español o inglés

Fecha: 2026-11-14
Estado: implementada y verificada en pantalla

## Pedido

> Agregale a la app la capacidad de cambiar de idioma a inglés, y que si cambian a inglés el título también se cambie a inglés.

## Comportamiento

- En la fila de "Iniciar con Windows" hay un selector con dos opciones: "Español" e "English".
- Al elegir un idioma cambian al instante todos los textos: título de la ventana ("Ajustes de pantalla" / "Display Settings"), etiquetas de los sliders, casilla, botón, tooltip y menú del ícono de la bandeja, y los mensajes de error y avisos que estén a la vista.
- La elección se guarda en `ajustes.txt` (`idioma=es` o `idioma=en`) y se respeta al volver a abrir la app.
- Restablecer vuelve los valores a 100 pero no cambia el idioma.
- Un archivo de una versión anterior, sin la clave `idioma`, abre en español. Un valor desconocido también.

## Diseño

- `src/language.rs`: `Language` (español por defecto, código `es`/`en`) y `Message`, con los textos de los mensajes que arma el código Rust en los dos idiomas.
- `BackendError::message(language)` da el mensaje en cada idioma; `Display` sigue en español.
- `Settings` suma el campo `language`; `Settings::neutral(language)` da los valores en 100 conservando el idioma (lo usa Restablecer).
- `ui/app.slint`: propiedad `english` y callback `language-changed`; cada texto elige su versión con `root.english ? … : …`. `ui/tray.slint` recibe la misma propiedad.
- `main.rs` guarda el idioma activo y el último mensaje mostrado (`Texts`) para volver a escribirlo en el idioma nuevo.

## Verificación

- 71 tests (11 nuevos: códigos de idioma, lectura y guardado de `idioma`, archivos viejos, valores neutros con idioma, mensajes en los dos idiomas).
- Manual, con la versión release: abre en inglés con `idioma=en` y en español con `idioma=es`; al elegir "English" en el selector el título pasa a "Display Settings", cambian todos los textos y el archivo queda con `idioma=en`.
