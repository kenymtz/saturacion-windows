# Entregable final: recordar valores, seguridad, instalador y restablecer

Fecha: 2026-11-13
Estado: implementada y verificada en hardware, con dos puntos sin verificar a mano (plan: `docs/plans/2026-11-13-entregable-final.md`)
Depende de: historias 1 a 3, bandeja y bloque de matriz (todo en `main`).

## Pedido

El entregable final debe incluir:

1. **App compatible**, entendido como segura para usar con CS2 y FACEIT (historia 6).
2. **Un instalador** que instale la app y deje el acceso directo en el escritorio.
3. **Una mejora** elegida en el diseño: el botón "Restablecer" (historia 10).

Junto con la historia 9:

> Como jugador, quiero que la app recuerde mis valores al reiniciar la PC, para no tener que configurarlos cada vez.

El usuario eligió que la app **arranque sola con Windows, minimizada en la bandeja, aplicando los últimos valores**.

## Parte 1: recordar valores y arrancar con Windows (historia 9)

### Archivo de ajustes

- Ubicación: `%APPDATA%\Ajustes de pantalla\ajustes.txt`.
- Formato de texto, una clave por línea:

```
saturacion=150
contraste=120
brillo=110
```

- Lectura tolerante: líneas vacías, claves desconocidas y valores que no son números se ignoran; cada valor se recorta a su rango (saturación 0–400, contraste 50–200, brillo 50–200). Si falta el archivo o una clave, el valor es 100.
- Escritura: se reescribe el archivo completo con los tres valores.
- La opción "Iniciar con Windows" **no** se guarda en el archivo: su fuente de verdad es la entrada del registro (ver abajo), para que el instalador y la app no puedan contradecirse.

### Cuándo se guarda

- Al **soltar** un slider (callback `released` de Slint), no en cada paso del arrastre.
- Al usar "Restablecer" (guarda 100 en los tres).
- Si guardar falla, se muestra en el texto de error de la ventana: "No se pudieron guardar los ajustes." y la app sigue funcionando.

### Al abrir

- Se leen los ajustes, se ponen los sliders en esos valores y se aplican a la placa con el controlador. Solo se escribe lo que difiere de 100: la matriz si la saturación no es 100, la curva si el contraste o el brillo no son 100. Si todo está en 100, la app no toca la placa.
- Si aplicar falla, se muestra el error como cualquier escritura fallida.
- Si la placa no ofrece la curva, contraste y brillo se cargan en sus sliders deshabilitados y no se aplican.

### Arranque con Windows

- Entrada en `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`, valor `Ajustes de pantalla`, dato `"<ruta del exe>" --minimizado`. No requiere permisos de administrador.
- Casilla **"Iniciar con Windows"** en la ventana. Al abrir, refleja si la entrada existe y apunta a este ejecutable. Al marcarla crea o actualiza la entrada; al desmarcarla la borra. Si falla, se muestra "No se pudo cambiar el inicio con Windows." y la casilla vuelve a su estado anterior.
- Con `--minimizado`, la app arranca sin mostrar la ventana: solo el ícono en la bandeja, con los valores aplicados. Si el ícono de la bandeja no está disponible, la ventana se muestra igual (no puede quedar invisible sin forma de cerrarla).

### Una sola instancia

Necesaria porque con el arranque automático es fácil abrir una segunda copia desde el acceso directo; dos copias escribirían los mismos bloques y al cerrar una restauraría la imagen de la otra.

- Al iniciar, la app intenta crear un mutex con nombre (`Local\AjustesDePantalla.Instancia`). Si ya existe, otra copia está corriendo.
- Copia nueva sin argumentos (o con `--minimizado`): avisa a la que corre que muestre su ventana y termina sin tocar la placa.
- Copia nueva con `--salir`: avisa a la que corre que restaure y termine, espera hasta 5 segundos a que el mutex quede libre y termina. Si no había ninguna corriendo, termina enseguida sin hacer nada.
- El aviso usa dos eventos con nombre (`Local\AjustesDePantalla.Mostrar` y `Local\AjustesDePantalla.Salir`). La copia que corre los espera en un hilo y reenvía la orden al hilo de la interfaz con `slint::invoke_from_event_loop`.

## Parte 2: compatible, segura con CS2 y FACEIT (historia 6)

Solo Valve y FACEIT pueden garantizar que la app no cause un ban. El entregable es evidencia y menor apariencia sospechosa:

- **Ejecutable identificable:** ícono embebido en el `.exe` y recurso de versión con nombre de producto "Ajustes de pantalla", descripción "Saturación, contraste y brillo de la pantalla" y versión del paquete. No se incluyen datos personales.
- **`docs/SEGURIDAD.md`** (en español) con:
  - qué hace la app (escribe la matriz de color y la curva de salida de la placa por la API pública de Intel, `ControlLib.dll`, cargada solo desde System32) y qué no hace (no abre ni lee el proceso del juego, no inyecta código, no instala drivers, no usa red);
  - antecedentes: vibranceGUI, que hace lo mismo con las APIs de NVIDIA y AMD, figura en la lista de FACEIT de programas que no causan ban; el caso AMD Anti-Lag+ de 2023, baneado por inyectar en las DLL de CS2, como ejemplo de lo que la app no hace;
  - que el código es público para auditarlo;
  - riesgo residual: FACEIT podría mostrar un aviso con software que no conoce (según su política, un aviso, no un ban); la app no está firmada;
  - un mensaje redactado para enviar a soporte de FACEIT;
  - el resultado de la prueba real (abajo).
- **Prueba real con el usuario:** con la app en la bandeja y valores aplicados, abrir el cliente de FACEIT y registrar si aparece algún aviso. Si el usuario no tiene FACEIT instalado, se registra que no se probó.

## Parte 3: instalador

- Herramienta: **Inno Setup** (gratuita), instalada en la máquina de desarrollo con winget. Script en `installer/setup.iss`.
- Instalación por usuario, sin administrador (`PrivilegesRequired=lowest`), en `%LOCALAPPDATA%\Programs\Ajustes de pantalla`, con el ejecutable `Ajustes de pantalla.exe`.
- Crea acceso directo en el **escritorio** (tarea marcada por defecto) y entrada en el **menú Inicio**.
- Tarea **"Iniciar con Windows"** (marcada por defecto) que crea la entrada del registro descrita en la Parte 1. La app y el instalador escriben exactamente el mismo valor.
- Antes de instalar, actualizar o desinstalar, ejecuta la app instalada con `--salir` para que restaure la imagen y cierre.
- Al desinstalar borra la entrada de arranque; conserva `ajustes.txt` en `%APPDATA%`.
- Aparece en Configuración > Aplicaciones con el ícono de la app.
- Al terminar ofrece abrir la app.
- Script `scripts/build-installer.ps1`: compila en release y ejecuta el compilador de Inno Setup; deja `dist/AjustesDePantalla-Setup-<versión>.exe`. `dist/` no se versiona.
- `scripts/install.ps1` se borra: el instalador lo reemplaza.
- Limitación: el instalador no está firmado, así que Windows SmartScreen puede mostrar "Windows protegió su PC"; se continúa con "Más información" → "Ejecutar de todas formas".

## Parte 4: mejora, botón Restablecer (historia 10)

- Botón **"Restablecer"** en la ventana y ítem **"Restablecer"** en el menú de la bandeja.
- Pone los tres sliders en 100, devuelve la placa a la imagen normal (restaura los bloques que la app escribió, igual que al salir) y guarda 100 en los ajustes.
- La app sigue corriendo; mover un slider después vuelve a escribir normalmente.

## Arquitectura

| Pieza | Cambio |
|---|---|
| `src/settings.rs` (nuevo) | `Settings { saturation, contrast, brightness }` con `Default` (100, 100, 100), `parse(&str) -> Settings`, `to_text(&self) -> String`, `load(path)`, `save(path)` y `default_path()`. Lógica pura con tests; solo `load`/`save` tocan disco. |
| `src/system/` (nuevo) | `single_instance` (mutex y eventos con nombre) y `autostart` (registro `Run`). Usa `windows-sys` (ya presente en el árbol de dependencias por Slint). Es el único lugar fuera de `src/igcl/` con código `unsafe`. La parte pura (armar el dato del registro y decidir si apunta a este exe) tiene tests. |
| `src/controller.rs` | `reset()`: pone los factores en 1.0 y llama a `restore_default` del backend si hubo escrituras (mismo criterio que `shutdown`), dejando el controlador listo para seguir escribiendo. Getters de los valores no hacen falta: la UI es la fuente de los porcentajes. |
| `ui/app.slint` | Casilla "Iniciar con Windows", botón "Restablecer", callbacks `released` de los sliders y de la casilla. |
| `ui/tray.slint` | Ítem "Restablecer". |
| `src/main.rs` | Argumentos `--minimizado` y `--salir`; instancia única; carga y aplicación inicial; guardado al soltar; restablecer; casilla de arranque. |
| `build.rs` | Embebe ícono y recurso de versión en el `.exe` (crate de compilación para recursos de Windows). |
| `ui/icon.ico` (nuevo) | Generado por `ui/make-icon.ps1` junto al PNG. |
| `installer/setup.iss`, `scripts/build-installer.ps1` (nuevos) | Instalador. |
| `docs/SEGURIDAD.md` (nuevo) | Evidencia de seguridad. |
| `README.md` (nuevo) | Qué es la app, cómo instalarla, cómo compilar, enlace a seguridad. |

## Manejo de errores

| Caso | Comportamiento |
|---|---|
| No se puede leer `ajustes.txt` | Valores 100, sin mensaje. |
| No se puede guardar | "No se pudieron guardar los ajustes." en el texto de error. |
| No se puede cambiar el registro | "No se pudo cambiar el inicio con Windows." y la casilla vuelve atrás. |
| No se puede crear el mutex | La app sigue como si fuera la única copia. |
| `--salir` y la otra copia no termina en 5 s | La nueva termina igual; el instalador sigue. |
| `--minimizado` sin bandeja disponible | Se muestra la ventana. |

## Pruebas

- **Unitarias:** `settings` (parse, valores fuera de rango, claves desconocidas, líneas basura, ida y vuelta texto → valores), `autostart` (dato del registro con comillas y `--minimizado`; detección de si apunta a este exe), `controller::reset` con el backend falso (restaura si hubo escrituras, no toca si no; las escrituras posteriores funcionan).
- **Manuales en hardware:** instalar con el Setup, acceso directo, reinicio con arranque en bandeja y valores aplicados, segunda copia trae la ventana, Restablecer, casilla de arranque, actualizar reinstalando con la app abierta, desinstalar, prueba con el cliente de FACEIT.

## Criterios de aceptación

1. Los valores se recuerdan al cerrar y volver a abrir la app.
2. Tras reiniciar Windows, la app arranca sola en la bandeja con los valores aplicados (si "Iniciar con Windows" está activo).
3. Abrir el acceso directo con la app corriendo solo trae su ventana al frente.
4. "Restablecer" (ventana y bandeja) vuelve todo a 100 y la imagen a la normal.
5. El `Setup.exe` instala sin administrador, crea el acceso directo en el escritorio y la entrada en el menú Inicio, y la app figura en Configuración > Aplicaciones.
6. Reinstalar con la app abierta la cierra restaurando la imagen; desinstalar quita la app, el acceso directo y el arranque automático.
7. El `.exe` muestra ícono y datos de producto en sus propiedades.
8. `docs/SEGURIDAD.md` existe con la evidencia y el resultado de la prueba con FACEIT (o la constancia de que no se probó).

## Fuera de alcance

- Firma de código (requiere un certificado pago).
- Soporte NVIDIA/AMD (historia 7), reacción visual de controles (historia 4).
- Restaurar al cerrar sesión o apagar (riesgo documentado en la spec de la bandeja).
- Publicar el instalador como release en GitHub (se decide al final).

## Resultado de la verificación de aceptación

Fecha: 2026-11-13, con el instalador generado desde el commit a25782d.

Verificado:

- **Instalación** (comprobado en el sistema): sin administrador; `Ajustes de pantalla.exe` 1.0.0 en `%LOCALAPPDATA%\Programs\Ajustes de pantalla`; acceso directo en el escritorio y en el menú Inicio; entrada en Configuración > Aplicaciones con el ícono; entrada `Run` con el formato exacto; el archivo sobrante del script anterior (`icon.ico`) se eliminó.
- **Recordar valores** (usuario y archivo): 250/120/110 se guardaron al soltar los sliders y la app volvió a abrir con esos valores aplicados.
- **Instancia única** (usuario): abrir el acceso directo con la app corriendo trajo la misma ventana, sin segunda copia.
- **Casilla "Iniciar con Windows"** (comprobado en el registro): aparecía marcada; al desmarcarla se borró la entrada `Run` y al marcarla volvió con el formato exacto.
- **Arranque como al iniciar sesión** (usuario): ejecutando el comando de la entrada `Run` la app arrancó solo en la bandeja, sin ventana, con los valores aplicados. No se hizo un reinicio real de Windows.
- **Reinstalar con la app abierta** (usuario): el instalador cerró la app restaurando la imagen, reinstaló y al abrirla aplicó los valores guardados.
- **FACEIT** (usuario): sin aviso con la app en la bandeja (detalle en `docs/SEGURIDAD.md`).

No verificado a mano:

- **Restablecer** (ventana y bandeja): cubierto por los tests del controlador (`reset_*`) y por las revisiones de código, pero no se probó en pantalla.
- **Desinstalar**: no se probó la desinstalación (cierre de la app, borrado de accesos directos y de la entrada `Run`, conservación de `ajustes.txt`).
- Reinicio real de Windows.
