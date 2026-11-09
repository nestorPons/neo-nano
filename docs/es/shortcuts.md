# Atajos de teclado

nnano usa un **mapa de teclas de escritorio** por defecto. Las teclas de nano que no chocan se conservan.

**Notación:** `Ctrl` = `^` · `Alt` = `M-`. En macOS, `Alt` es `Option`; `Cmd+*` no llega a la aplicación.
La columna *Alternativa* es el binding heredado de nano, cuando existe.

## Archivo

| Acción | Tecla | Alternativa |
|---|---|---|
| Guardar | `Ctrl+S` | — |
| Guardar como | `Alt+S` | — |
| Abrir archivo | `Ctrl+O` | — |
| Recargar desde disco | `Ctrl+R` | — |
| Insertar archivo en el cursor | `Alt+I` | `Ctrl+R` (nano) |
| Nuevo búfer | `Ctrl+N` | — |
| Nueva pestaña | `Ctrl+T` | — |
| Cerrar búfer | `Ctrl+W` | — |
| Salir | `Ctrl+Q` | — |

## Edición

| Acción | Tecla | Alternativa |
|---|---|---|
| Deshacer | `Ctrl+Z` | `Alt+U` |
| Rehacer | `Ctrl+Y` | `Alt+E` |
| Cortar | `Ctrl+X` | — |
| Copiar | `Ctrl+C` | — |
| Pegar | `Ctrl+V` | `Ctrl+U` |
| Cortar línea | `Ctrl+K` | — |
| Seleccionar todo | `Ctrl+A` | — |
| Marcar para seleccionar | `Alt+A` | `Alt+A` |
| Copiar texto marcado | `Alt+6` | `Alt+6` |
| Marca rectangular | `Alt+Z` | — |
| Borrar carácter | `Ctrl+D` | — |
| Reemplazar | `Ctrl+\` | `Alt+R` |
| Justificar párrafo | `Ctrl+J` | — |
| Ejecutar comando externo | `Alt+X` | — |

## Navegación

| Acción | Tecla | Alternativa |
|---|---|---|
| Ir a línea | `Ctrl+G` | `Alt+G` |
| Inicio de línea | `Home` | — |
| Fin de línea | `End` | `Ctrl+E` |
| Primera línea | `Alt+\` | `Alt+\` |
| Última línea | `Alt+/` | `Alt+/` |
| Palabra a la izquierda / derecha | `Ctrl+←` / `Ctrl+→` | `Alt+←` / `Alt+→` |
| Página arriba | `PgUp` | `Alt+↑` |
| Página abajo | `PgDn` | `Alt+↓` |
| Retroceder un carácter | `Ctrl+B` | `Ctrl+B` |

## Búsqueda

| Acción | Tecla | Alternativa |
|---|---|---|
| Buscar | `Ctrl+F` | — |
| Buscar siguiente | `F3` | `Alt+W` |
| Buscar anterior | `Shift+F3` | `Alt+B` |
| Alternar mayúsculas (en el prompt) | `Alt+C` | `Alt+C` |
| Alternar regexp (en el prompt) | `Alt+R` | `Alt+R` |

La búsqueda imita a nano: **no es incremental** (se ejecuta al aceptar el prompt), es literal por defecto, no distingue mayúsculas por defecto y da la vuelta al búfer con el aviso *Búsqueda reiniciada*.
`Alt+R` es modal: dentro de un prompt alterna regexp; fuera de él abre Reemplazar.

## Interfaz

| Acción | Tecla | Alternativa |
|---|---|---|
| Ayuda | `Ctrl+H` | `F1` |
| Menú de configuración | `Alt+,` | — |
| Paleta de comandos | `Ctrl+P` | — |
| Alternar resaltado de sintaxis | `Alt+Y` | `Alt+Y` |
| Refrescar pantalla | `Ctrl+L` | `Ctrl+L` |
| Idioma de la interfaz | `Alt+,` → Idioma | `--language <código>` |

La paleta (`Ctrl+P`) muestra a la derecha la tecla de cada comando, y escribir una tecla (`ctrl+q`) filtra por atajo. La ayuda (`Ctrl+H`) y la paleta se generan desde la misma tabla de atajos que interpreta el teclado, así que nunca anuncian una tecla que no funcione.

La interfaz habla diez idiomas: English, 简体中文, हिन्दी, Español, العربية, Français, বাংলা, Português, Bahasa Indonesia y 日本語. Elige uno en los ajustes (`Alt+,` → Idioma) o con `--language es`; `language = "auto"` en `config.toml` sigue a `$LC_ALL`/`$LANG`.

## Qué cambia respecto a nano

| Antes (nano) | Ahora |
|---|---|
| `Ctrl+O` guardar | `Ctrl+S` guardar |
| `Ctrl+X` salir | `Ctrl+Q` salir · `Ctrl+X` corta |
| `Ctrl+W` buscar | `Ctrl+F` buscar · `Ctrl+W` cierra búfer |
| `Ctrl+C` posición del cursor | Barra de estado · `Ctrl+C` copia |
| `Ctrl+V` página abajo | `PgDn` / `Alt+↓` · `Ctrl+V` pega |
| `Ctrl+Y` página arriba | `PgUp` / `Alt+↑` · `Ctrl+Y` rehace |
| `Ctrl+A` inicio de línea | `Home` · `Ctrl+A` selecciona todo |
| `Ctrl+Z` suspender | Eliminado · `Ctrl+Z` deshace |
| `Ctrl+R` insertar archivo | `Alt+I` · `Ctrl+R` recarga |
| `Ctrl+N` / `Ctrl+P` línea siguiente / anterior | Flechas `↓` / `↑` · nuevo búfer / paleta |
| `Ctrl+T` ejecutar | `Alt+X` · `Ctrl+T` nueva pestaña |
| `Ctrl+G` ayuda | `F1` · `Ctrl+G` va a línea |

Se conservan sin cambios: `Ctrl+K`, `Ctrl+U`, `Ctrl+E`, `Ctrl+D`, `Ctrl+J`, `Ctrl+L`, `Ctrl+B`, `Ctrl+\`, `Alt+U`, `Alt+E`, `Alt+A`, `Alt+6`, `Alt+G`, `Alt+\`, `Alt+/`, `Alt+W`, `Alt+B`, `Alt+C`, `Alt+R`, `Alt+Y`.

## Límites del terminal

- `Ctrl+S` y `Ctrl+Q` requieren desactivar el control de flujo XON/XOFF; si no, el terminal se los come.
- `Enter`, `Tab`, `Backspace` y `Esc` no son reasignables.
- `Ctrl+B` es el prefijo por defecto de tmux; `Ctrl+A`, el de screen.
- `Ctrl+Shift+letra` no es fiable en terminales antiguos: no hay atajo de rehacer con `Ctrl+Shift+Z`.
- Para seleccionar texto nativo del terminal, mantén `Shift` al arrastrar.
- nnano usa el ratón si la opción `mouse` está activa (por defecto): clic coloca el cursor, arrastrar marca, `Alt`+arrastrar marca un rectángulo y la rueda desplaza sin mover el cursor.
- Grupos de deshacer: se agrupa por palabra y por una pausa de 400 ms; cortar, pegar, guardar y cambiar de búfer cierran el grupo. «Reemplazar todo» es un solo paso.
- `Ctrl+H` abre la referencia de atajos. Retroceso es la tecla Backspace del terminal (`0x7F`): si tu terminal envía `0x08` al pulsar Backspace, esa tecla abre la ayuda; configúralo para enviar `0x7F`.

## Prompts y archivo de recuperación

- `Alt+S` pide una ruta, y también `Ctrl+S` en un búfer sin nombre (`nnano -`, un búfer nuevo o un guardar-como cancelado). `Tab` completa la ruta en *Guardar como*, *Abrir* e *Insertar archivo*: una sola coincidencia la rellena, varias la extienden al prefijo común.
- `Alt+S` escribe un archivo nuevo y adopta su nombre: la barra de búferes, la barra de estado y la gramática de sintaxis lo siguen. Si la escritura falla, se conserva el nombre anterior.
- `Alt+X` ejecuta la línea con `sh -c` e inserta su salida estándar en el cursor como un solo paso de deshacer; la barra de estado informa el código de salida y la primera línea de stderr.
- `Ctrl+R` relee del disco el archivo de este búfer y reemplaza el texto; el historial de deshacer del texto anterior se descarta. Si hay cambios sin guardar, pregunta una vez (`Ctrl+R` otra vez recarga, `Ctrl+S` guarda).
- `Alt+I` pide un archivo e inserta su texto en el cursor como un solo paso de deshacer; `Tab` completa la ruta. El búfer conserva su nombre y su codificación.
- Mientras un búfer tiene cambios sin guardar, nnano escribe una copia de recuperación 2 s después de la última edición en `$XDG_STATE_HOME/nnano/recovery/`. Un cuelgue o una sesión SSH cortada se restauran automáticamente al arrancar (`recovered unsaved changes for …`); guardar o salir limpiamente la borran.
- `Ctrl+C` y `Alt+6` siempre copian al registro interno. Activa la opción `osc52` (`--osc52`) para enviar además la copia al portapapeles del sistema, con tope de 64 KiB; si se trunca, se avisa en la barra de estado.
- `Ctrl+J` reajusta el párrafo del cursor (o las líneas marcadas) al ancho del área de texto, conservando la sangría de la primera línea. Es un solo paso de deshacer.
