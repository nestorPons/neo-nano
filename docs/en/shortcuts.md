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
| Extend selection by character | `Shift+←` / `Shift+→` | — |
| Extend selection by line | `Shift+↑` / `Shift+↓` | — |
| Page up | `PgUp` | `Alt+↑` |
| Page down | `PgDn` | `Alt+↓` |
| Back one character | `Ctrl+B` | `Ctrl+B` |

`Shift+←`/`Shift+→` extend the mark by characters, and `Shift+↑`/`Shift+↓` by lines keeping the visual column, as in a desktop editor; `Esc` clears it. Any movement extends an existing mark, as in nano. `Ctrl+C` copies and then ends the mark, so the next `Ctrl+V` inserts at the cursor.

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
| Help | `Ctrl+H` | `F1` |
| Settings menu | `Alt+,` | — |
| Command palette | `Ctrl+P` | — |
| Toggle syntax highlighting | `Alt+Y` | `Alt+Y` |
| Refresh screen | `Ctrl+L` | `Ctrl+L` |
| Interface language | `Alt+,` → Language | `--language <code>` |

The palette (`Ctrl+P`) prints the key that runs each command on the right, and typing a key (`ctrl+q`) filters by shortcut. The help (`Ctrl+H`) and the palette are generated from the same keymap table the decoder reads, so they never advertise a key that does not work.

The interface speaks ten languages: English, 简体中文, हिन्दी, Español, العربية, Français, বাংলা, Português, Bahasa Indonesia and 日本語. Pick one in the settings overlay (`Alt+,` → Language) or with `--language es`; `language = "auto"` in `config.toml` follows `$LC_ALL`/`$LANG`.

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
- nnano uses the mouse when the `mouse` option is on (the default): click places the cursor, drag marks, `Alt`+drag marks a rectangle, the wheel scrolls without moving the cursor, and clicking the scrollbar on the right jumps to that part of the file. Releasing the button copies the marked text to the internal register and to the system clipboard (OSC 52); the highlight stays until you click elsewhere or press `Esc`.
- The status bar shows `line:column/total`: the cursor line, its visual column and the document's total line count.
- Undo groups: typing is grouped by word and by a 400 ms pause; cut, paste, save and a buffer switch close the group. "Replace all" is one undo step.
- `Ctrl+H` opens the shortcut reference. Backspace is the terminal's Backspace key (`0x7F`): if your terminal sends `0x08` for Backspace, that key opens help — set it to send `0x7F`.

## Prompts and the recovery file

- `Alt+S` asks for a path, and so does `Ctrl+S` on a buffer with no name (`nnano -`, a fresh buffer, or a save-as that was cancelled). `Tab` completes the path in *Save as*, *Open* and *Insert file*; a single match fills it, several extend it to the common prefix.
- `Alt+S` writes a new file and adopts its name: the buffer bar, the status bar and the syntax grammar follow it. A failed write leaves the old name in place.
- `Alt+X` runs the line with `sh -c` and inserts its stdout at the cursor as one undo step; the status bar reports the exit code and the first line of stderr.
- `Ctrl+R` re-reads this buffer's file from disk and replaces the text; the undo history of the old text is dropped. With unsaved changes it asks once (`Ctrl+R` again reloads, `Ctrl+S` saves).
- `Alt+I` asks for a file and inserts its text at the cursor as one undo step; `Tab` completes the path. The buffer keeps its own name and encoding.
- While a buffer has unsaved changes, nnano writes a recovery snapshot 2 s after the last edit under `$XDG_STATE_HOME/nnano/recovery/`. A crash or a killed SSH session is restored automatically on the next start (`recovered unsaved changes for …`); saving or a clean exit deletes it.
- `Ctrl+C` and `Alt+6` always copy into the internal register. Enable the `osc52` option (`--osc52`) to also send the copy to the system clipboard, capped at 64 KiB; truncation is reported in the status bar. The status bar reports the copied size, and `Ctrl+V` on an empty register says so.
- `Ctrl+J` re-wraps the paragraph around the cursor (or the marked lines) to the width of the text area, keeping the first line's indent. It is one undo step.
