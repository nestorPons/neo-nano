# Resumen — documentación de atajos

Referencia rápida de lo creado y de las decisiones tomadas. Documento de consulta, no normativo: la fuente de verdad es `AGENTS.md`.

## 1. Archivos

| Archivo | Estado | Idioma |
|---|---|---|
| `docs/en/shortcuts.md` | creado | inglés |
| `docs/es/shortcuts.md` | creado | español |
| `docs/zh-Hans/shortcuts.md` | creado | chino simplificado |
| `docs/ja/shortcuts.md` | creado | japonés |
| `AGENTS.md` | modificado | — (nota de sincronización) |
| `LOG.md` | modificado | — (entrada del cambio) |

## 2. Estructura (idéntica en los 4 archivos, mismo orden)

| # | Sección | Contenido |
|---|---|---|
| 1 | Archivo | guardar, guardar como, abrir, recargar, insertar, búfer, pestaña, salir |
| 2 | Edición | deshacer, cortar/copiar/pegar, marcar (normal y rectangular), borrar, reemplazar |
| 3 | Navegación | ir a línea, inicio/fin, primera/última, palabras, páginas |
| 4 | Búsqueda | buscar, siguiente, anterior, mayúsculas, regexp (prompt) |
| 5 | Interfaz | ayuda, ajustes, paleta, resaltado, refrescar |
| 6 | Cambios vs nano | migración tecla antigua → tecla nueva |
| 7 | Límites del terminal | XON/XOFF, teclas no reasignables, prefijos, `Ctrl+Shift`, ratón, grupos de deshacer |

Extensión: 105 líneas por archivo, 420 en total.

## 3. Convenciones

- **Notación larga**: `Ctrl+S`, `Alt+X`. No se usa `^S` / `M-X` en docs de usuario.
- **Columna "Alternativa"**: binding heredado de nano, cuando existe. Hace visible la retrocompatibilidad.
- **macOS**: `Alt` = `Option`; `Cmd+*` nunca llega. Anotado en la cabecera.
- Cada archivo abre con la regla: escritorio por defecto, nano solo donde no choca.

## 4. Decisiones

| Decisión | Motivo |
|---|---|
| `Alt+S` guardar como, `Alt+,` ajustes, `F3`/`Shift+F3` buscar | faltaban en el spec; añadidos a `AGENTS.md` para no crear deriva |
| `Alt+W`/`Alt+B`/`Alt+C`/`Alt+R` (nano) recuperados en búsqueda | búsqueda "como en nano": sin ellos la semántica no sería replicable desde el teclado |
| `Alt+R` documentado como modal | dentro del prompt = regexp; fuera = reemplazar; mismo key, distinto modo, como nano |
| `Alt+Y` resaltado de sintaxis | nano lo usa; sin colisión, se conserva |
| Aviso de que `Ctrl+Shift+Z` no existe | muchos terminales no lo envían; evita documentar un rehacer irreal |
| Sección de límites del terminal | el fallo más probable al probar es `Ctrl+S`/`Ctrl+Q` comidos por el flujo XON/XOFF |
| Nota de selección con `Shift` | el ratón capturado rompe la selección nativa del terminal |
| `Alt+Z` marca rectangular | no había tecla decidida para el modo bloque; `Alt+6` copia la marca tal cual |
| Ratón y grupos de deshacer documentados | la captura de ratón y la granularidad de undo son decisiones de producto, no detalles internos |

## 5. Sincronización

- Cambiar una tecla ⇒ editar **los 4** `docs/*/shortcuts.md` + tabla de `AGENTS.md`.
- La regla está escrita dentro de `AGENTS.md` (sección Keymap).

## 6. Pendientes

- No hay índices `README.md` en `docs/en`, `docs/zh-Hans` ni `docs/ja`.
- Las capturas/ejemplos visuales de los overlays no están documentadas todavía; el estado real vive en `AGENTS.md` y `LOG.md`.
