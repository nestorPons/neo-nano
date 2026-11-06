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
| Ayuda | `F1` | — |
| Menú de configuración | `Alt+,` | — |
| Paleta de comandos | `Ctrl+P` | — |
| Alternar resaltado de sintaxis | `Alt+Y` | `Alt+Y` |
| Refrescar pantalla | `Ctrl+L` | `Ctrl+L` |

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
