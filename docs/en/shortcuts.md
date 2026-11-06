# Keyboard shortcuts

nnano uses a **desktop-style keymap** by default. nano bindings that do not collide are kept.

**Notation:** `Ctrl` = `^` · `Alt` = `M-`. On macOS, `Alt` is `Option`; `Cmd+*` never reaches the application.
The *Alternative* column is the inherited nano binding, where one exists.

## File

| Action | Key | Alternative |
|---|---|---|
| Save | `Ctrl+S` | — |
| Save as | `Alt+S` | — |
| Open file | `Ctrl+O` | — |
| Reload from disk | `Ctrl+R` | — |
| Insert file at cursor | `Alt+I` | `Ctrl+R` (nano) |
| New buffer | `Ctrl+N` | — |
| New tab | `Ctrl+T` | — |
| Close buffer | `Ctrl+W` | — |
| Quit | `Ctrl+Q` | — |

## Editing

| Action | Key | Alternative |
|---|---|---|
| Undo | `Ctrl+Z` | `Alt+U` |
| Redo | `Ctrl+Y` | `Alt+E` |
| Cut | `Ctrl+X` | — |
| Copy | `Ctrl+C` | — |
| Paste | `Ctrl+V` | `Ctrl+U` |
| Cut line | `Ctrl+K` | — |
| Select all | `Ctrl+A` | — |
| Start marking | `Alt+A` | `Alt+A` |
| Copy marked text | `Alt+6` | `Alt+6` |
| Rectangular mark | `Alt+Z` | — |
| Delete character | `Ctrl+D` | — |
| Replace | `Ctrl+\` | `Alt+R` |
| Justify paragraph | `Ctrl+J` | — |
| Execute external command | `Alt+X` | — |

## Navigation

| Action | Key | Alternative |
|---|---|---|
| Go to line | `Ctrl+G` | `Alt+G` |
| Start of line | `Home` | — |
| End of line | `End` | `Ctrl+E` |
| First line | `Alt+\` | `Alt+\` |
| Last line | `Alt+/` | `Alt+/` |
| Word left / right | `Ctrl+←` / `Ctrl+→` | `Alt+←` / `Alt+→` |
| Page up | `PgUp` | `Alt+↑` |
| Page down | `PgDn` | `Alt+↓` |
| Back one character | `Ctrl+B` | `Ctrl+B` |

## Search

| Action | Key | Alternative |
|---|---|---|
| Find | `Ctrl+F` | — |
| Find next | `F3` | `Alt+W` |
| Find previous | `Shift+F3` | `Alt+B` |
| Case-sensitive toggle (in prompt) | `Alt+C` | `Alt+C` |
| Regexp toggle (in prompt) | `Alt+R` | `Alt+R` |

Search mirrors nano: it is **not incremental** (it runs when you accept the prompt), it is literal by default, case-insensitive by default, and it wraps around the buffer with a *Search wrapped* notice.
`Alt+R` is modal: inside a prompt it toggles regexp, outside it opens Replace.

## Interface

| Action | Key | Alternative |
|---|---|---|
| Help | `F1` | — |
| Settings menu | `Alt+,` | — |
| Command palette | `Ctrl+P` | — |
| Toggle syntax highlighting | `Alt+Y` | `Alt+Y` |
| Refresh screen | `Ctrl+L` | `Ctrl+L` |

## What changed from nano

| Before (nano) | Now |
|---|---|
| `Ctrl+O` save | `Ctrl+S` save |
| `Ctrl+X` exit | `Ctrl+Q` quit · `Ctrl+X` cuts |
| `Ctrl+W` search | `Ctrl+F` search · `Ctrl+W` closes buffer |
| `Ctrl+C` cursor position | Status bar · `Ctrl+C` copies |
| `Ctrl+V` page down | `PgDn` / `Alt+↓` · `Ctrl+V` pastes |
| `Ctrl+Y` page up | `PgUp` / `Alt+↑` · `Ctrl+Y` redoes |
| `Ctrl+A` start of line | `Home` · `Ctrl+A` selects all |
| `Ctrl+Z` suspend | Removed · `Ctrl+Z` undoes |
| `Ctrl+R` insert file | `Alt+I` · `Ctrl+R` reloads |
| `Ctrl+N` / `Ctrl+P` next / previous line | `↓` / `↑` arrows · new buffer / palette |
| `Ctrl+T` execute | `Alt+X` · `Ctrl+T` opens a tab |
| `Ctrl+G` help | `F1` · `Ctrl+G` goes to line |

Unchanged: `Ctrl+K`, `Ctrl+U`, `Ctrl+E`, `Ctrl+D`, `Ctrl+J`, `Ctrl+L`, `Ctrl+B`, `Ctrl+\`, `Alt+U`, `Alt+E`, `Alt+A`, `Alt+6`, `Alt+G`, `Alt+\`, `Alt+/`, `Alt+W`, `Alt+B`, `Alt+C`, `Alt+R`, `Alt+Y`.

## Terminal limits

- `Ctrl+S` and `Ctrl+Q` require XON/XOFF flow control to be disabled; otherwise the terminal swallows them.
- `Enter`, `Tab`, `Backspace` and `Esc` cannot be reassigned.
- `Ctrl+B` is tmux's default prefix; `Ctrl+A` is screen's.
- `Ctrl+Shift+<letter>` is unreliable on legacy terminals: there is no `Ctrl+Shift+Z` redo.
- To select text with the terminal natively, hold `Shift` while dragging.
- nnano uses the mouse when the `mouse` option is on (the default): click places the cursor, drag marks, `Alt`+drag marks a rectangle, and the wheel scrolls without moving the cursor.
- Undo groups: typing is grouped by word and by a 400 ms pause; cut, paste, save and a buffer switch close the group. "Replace all" is one undo step.
