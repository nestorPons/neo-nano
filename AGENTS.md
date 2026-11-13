# AGENTS.md — neo-nano (`nnano`)

Rust rewrite of the nano editor: a modern TUI that keeps nano's ethos — **simple, fast, easy, comfortable** — and adds today's TUI expectations. Not a fork of GNU nano's C codebase.

## Status (2026-11-06)
- **Phases 6 and 7 done** (2026-11-06): mouse (click, drag, Alt+drag rectangle, wheel, `mouse` option), bracketed paste, internal clipboard with stream and rectangular selection, grouped undo/redo, replace with per-match confirmation, multiple buffers with a one-row buffer bar, and syntax highlighting from `.nanorc` grammars.
- **Save, open and the file prompts are complete** (2026-11-06): `^S` writes in place and `fsync`s, `M-S` (or `^S` on an unnamed buffer, `nnano -` included) asks for a path, `^O` opens by path with `Tab` completion, `^G`/`M-G` take `line[,column]` or `+N`/`-N`, `^J` justifies to the text width, `M-X` runs `sh -c` and inserts its output, `Ctrl+←/→` move by word.
- **Recovery file done** (2026-11-06): a dirty buffer is snapshotted 2 s after the last edit under `$XDG_STATE_HOME/nnano/recovery/<hash of the absolute path>.swp`, restored on the next start, deleted on save, on close and on a clean exit.
- **OSC 52 copy out done** (2026-11-06): opt-in through the `osc52` registry option, capped at 64 KiB with a visible warning; the internal register is still the storage.
- Registry rows: `tab_size`, `case_sensitive`, `mouse`, `syntax`, `osc52`, `language` — each wired to the CLI, TOML, `.nanorc` (where nano has one) and the settings overlay by the same table.
- **Built-in syntax grammars done** (2026-11-10): `crates/nnano-core/assets/syntax/popular.nanorc` ships inside the binary, loaded after the user and distro dirs by `SyntaxSet::load`, so first-match selection keeps third-party grammars winning and a machine without nano still highlights. Extended patterns cover `htmx`, `phtml`, `jx`, `ts`/`tsx`/`jsx`/`cjs`, `vue`/`svelte`/`astro`, `scss`/`sass`/`less`, `yml`, dotenv, Dockerfile and ignore files; a source-count test fails the build if a grammar or rule in the asset is dropped.
- **Scrollbar and position readout done** (2026-11-10): `ui::areas` reserves the rightmost text column for a proportional one-cell scrollbar (`█` thumb, dim `│` track), so the text width never jumps when a file outgrows the screen; clicking the bar jumps to that page through `ScrollView`, so the cursor does not move. The status right side is `line:column/total` (`display_position` plus `line_count`).
- **`Shift+←`/`Shift+→` character selection done** (2026-11-13): `SelectLeft`/`SelectRight` are the horizontal twin of `SelectUp`/`SelectDown` — either key starts a mark at the cursor when there is none and moves one character with the goal column forgotten, crossing the line break like a plain arrow. The four Shift arrows share one mark and extend it in both axes, so a selection can grow by characters and by lines without reaching for `M-A`. In a panel or a prompt the same keys still move the highlight or the prompt cursor, because an overlay has no mark.
- **`Shift+↑`/`Shift+↓` line selection done** (2026-11-10): VSCode-style mark extension. Either key starts a mark at the cursor when there is none and moves one line with the wanted visual column kept, so the selection grows by whole rows without reaching for `M-A`. A panel is not a text area, so inside the settings/palette/help overlays the same keys still move the highlighted row.
- **Copy on select done (2026-11-12)**: releasing the left button after a drag in the text copies the mark to the internal register, reports `copied N chars`, and sends it out with OSC 52 **unconditionally** — selecting is the request, so `osc52` does not gate this one (`^C` still does). The highlight stays until a click elsewhere or `Esc`; a plain click or a release after pressing the scrollbar/tab bar copies nothing. The help's Mouse section names the gesture.
- **Copy/paste feedback done (2026-11-12)**: `^C` reports `copied N chars` in the status bar and ends the mark after copying, so the next `^V` inserts at the cursor instead of replacing the region an active mark dragged along; `^V` on an empty register says `nothing to paste`. The keys themselves were always live — the mark simply outlived the copy.
- **Ten interface languages** (2026-11-06): English, 简体中文, हिन्दी, Español, العربية, Français, বাংলা, Português, Bahasa Indonesia, 日本語. `language` is the first `Kind::Enum` row (a cycling picker in the overlay); the strings live in one `nnano-tui::i18n` table keyed by the English source text, and a coverage test fails the build when a binding, command, option label or help note has no row.
- **`^R` reload and `M-I` insert-file done** (2026-11-09): `^R` replaces the buffer with the file on disk (dirty buffers ask once; the old undo history is dropped, since its ranges name text that no longer exists; a missing or undecodable file changes nothing) and `M-I` inserts a file's text at the cursor as one undo step behind a path prompt that accepts `Tab`. Both are palette commands too, so the keymap table no longer advertises anything inert.
- **Website done** (2026-11-10): GitHub Pages serves `docs/` from `main` (no Actions, no build step; `docs/.nojekyll`). `docs/index.html` explains the project, shows `docs/assets/neo-nano.png`, the install paths and the author; `docs/shortcuts.html` is the full key reference (`docs/atajos.html` is a redirect); `docs/style.css`, `docs/site.js` and `docs/translator.js` are dependency-free. English is the source language and the selector translates in place with the browser's Google Translator API (the same one nestorpons.com uses) for the app's ten languages — Arabic flips the page to RTL, unsupported target languages are hidden, and the choice is remembered in `localStorage`. The key table mirrors `docs/en/shortcuts.md`: a keymap change means touching that page too.
- **Man page and shell completions done** (2026-11-09, 0.1.1): `man/nnano.1` and `completions/{nnano.bash,_nnano,nnano.fish}`, hand-written because the dependency set has no completion generator. `crates/nnano/tests/docs.rs` is the drift guard: every registry flag, every language code and the two universal flags must be in each file, and the man page header must carry `CARGO_PKG_VERSION`, so a version bump without the man page does not build. Both recipes install them.
- Still open: no LSP, no plugins.
- Undo is an operation log in **char** ranges (the rope indexes by char; storing bytes would need a snap-to-boundary on every line change). Groups close on a 400 ms pause, a word boundary, and hard boundaries (cut, paste, save, buffer switch); "replace all" merges into one group. No undo across runs.
- Selection is half-open `[anchor, head)` in chars: "select all" is exactly the buffer, and a cursor at a line end selects nothing phantom. Block mode is `M-Z` or Alt+drag; copy is `^C`/`M-6`, cut `^X`, paste `^V`/`^U`, and `^K` cuts whole lines and accumulates.
- Help is a floating, scrollable reference of every implemented binding (`^H` or `F1`, `↑↓`/`PgUp`/`PgDn`/wheel, `Esc`), and the status bar advertises `^H: help`.
- `KEYMAP` in `term/input.rs` is the only place a key is named: `decode` is a lookup over it, `help.rs` renders it (so the reference is complete by construction), and the command palette prints each command's chord from `chord_for` and filters by it. Three tests hold that: every row decodes to its own action, no chord is declared twice, every binding belongs to a section.
- Syntax is per line and stateless: earliest rule wins per char, only viewport lines are matched, and a line over 4096 chars is left plain. `start=`/`end=` multi-line rules, `header`, hex colours and unknown names (`pink`, `latte`) degrade instead of failing; the 40 distro grammars in `/usr/share/nano` load without a single warning. `M-Y` toggles the session, `syntax` sets the default.
- Verified in a real pty (80x24, reconstructed screen): copy/paste round-trips and undoes; typing undoes and redoes; `^N`/`^W` add and close buffers with the bar; `M-R` replaces one match and replaces all; `M-Y` and `--no-syntax` remove the colour codes; a click sets the cursor; the wheel scrolls without moving the cursor and the next key re-attaches the view; a bracketed paste is one undo step; the settings overlay turns the mouse off at runtime; no grammar warning reaches stderr.
- Verified in a real pty for the file prompts (2026-11-06): `M-S` writes and adopts the name, `^S` on `nnano -` asks for one, `^O` completes with `Tab` and loads the buffer bar, `^G` lands on `3:1` and `1,3`, `^J` wraps a 20-word paragraph to 80 cells, `M-X` inserts `eco` and reports `exit 3: boom`, `Ctrl+→`/`Ctrl+←` move word by word, OSC 52 stays silent until `--osc52`, and a recovery snapshot appears after 2 s, survives `SIGKILL`, is restored dirty on restart and disappears on save and on a clean exit.
- All v1 decisions are closed: *Open items* is empty, and `DECISIONS.md` was deleted — `LOG.md` is the decision/change history.
- Stack and architecture are already decided (see below) — do not relitigate them.
- Always `ls` before editing; nothing can be assumed to exist.

## Product requirements (do not silently drop)
- **Zero-config default**: usable immediately, like nano; no mandatory setup file.
- **Dual configuration**: every option settable from CLI flags *and* from an in-app floating/overlay settings UI. Neither path may drift — new option ⇒ both.
- **Mouse support** throughout (menus, text selection, scroll).
- **Desktop-style keymap is the default**; nano bindings survive only where they do not collide (table below). This is decided — do not relitigate it per feature.
- **Invocation**: primary binary `nnano`. **No `nn` alias, ever** (decided 2026-11-04). Never install a `nano` shim that shadows GNU nano.
- **Distribution**: distro packages, AUR/yay first (`yay` is the AUR helper, not a registry). Packaging is a later phase — explain it when reached; do not add packaging files now.

## Product scope: v1 (decided 2026-11-04)

Reading rule: where this section is silent, the recommendation is adopted as the default; where it says **nano**, the feature copies GNU nano's semantics *and* its configuration surface — not a modern reinterpretation of it.

| Feature | v1 | Note |
|---|---|---|
| Save | in-place truncate of the real file | no temp file, no `rename`, **no `file~` backup** |
| Syntax highlighting | **in** — nano's model | `.nanorc` `syntax`/`color`, per line, stateless |
| Search | **in** — nano's model | literal default, runs on `Enter`, not incremental |
| Split view | **no** | one viewport for the whole app |
| File tree / browser | **no** | command palette; it lists open buffers once tabs exist, and never the filesystem |
| Multiple cursors | **no** | one cursor, one selection |
| Rectangular selection | yes | range + mode flag; cheap on a rope |
| Undo granularity | grouped | ~400 ms pause + hard boundaries |
| Clipboard | internal + OSC 52 opt-in | works over SSH, no X needed |
| Autosave | **off** by default | `^S` stays the contract |
| Recovery file | **in** | restore after crash/panic |
| Encoding | detect and warn, never transcode | bytes preserved; UTF-16 unsupported |
| `nnano -` / `$EDITOR` | **in** | reads stdin; needs a name to save |
| Tabs | **in** (already decided) | `^T` |
| Plugins | never | out of scope by design, not a v1 gap |

Each row that becomes an option costs one registry row, one overlay widget, one translation row per language and one doc row in ten languages — count that before adding more.

### Save: in place (decided 2026-11-04)

- `^S` truncates and rewrites the file the user opened. Explicitly rejected: temp file + `rename` — it breaks hardlinks, replaces a symlink with a regular file and changes the inode other processes watch.
- Order: open the real path with truncation → write → flush → `fsync` (`2026-11-06`: `sync_all` is the second half of the promise; without it the bytes are only in the page cache when `^S` returns).
- A Latin-1 buffer is **validated before opening** (`2026-11-06`): the whole buffer is scanned for a char above `U+00FF` first, because opening truncates and a refusal after that would destroy the file. One extra O(n) pass, only for Latin-1, is the price of keeping the in-place rule without a temp copy.
- **No backup file** (decided 2026-11-04): nnano never writes `file~`. `backup` and `backupdir` are **not** registry options; a `.nanorc` `set backup` / `set backupdir` is accepted and ignored with the usual one startup warning, never an error.
- `save as` to a different path creates that file normally; the in-place rule applies to overwriting a file that exists.
- Consequence to accept: an interrupted write (crash, `ENOSPC`, killed SSH) leaves a truncated file with nothing beside it. The recovery file is the only net; never advertise atomic saves or a backup copy.

### Syntax highlighting: nano's model (decided 2026-11-04)

- Definitions are nano's: `syntax <name> [extensions]`, `color <fg,bg> <regex>`, `icolor`. No tree-sitter, no syntect, no TextMate grammars, no plugin system.
- Regex flavour is POSIX ERE, as nano (`[[:alpha:]]` and friends must work). Engine: **`regex-lite`** (decided 2026-11-04) — no `regex`/`regex-automata`/`fancy-regex`, no POSIX C binding. First syntax test must assert `[[:alpha:]]` parsing; re-measure binary size and startup when the module lands.
- Evaluation is **per line and stateless**: no multi-line constructs, exactly like nano (an unterminated `/*` does not colour the following lines). This is what keeps it compatible with the render rule — no per-line cache, no `Vec<Line>` mirror, no invalidation after an edit.
- Only viewport lines are matched, on redraw: cost is O(visible lines × rules) per frame, never O(file).
- Overlaps resolve as in nano: earliest rule wins per byte; `icolor` is case-insensitive.
- Colours are a semantic enum in `nnano-core` mapped to `ratatui::Style` in `nnano-tui`; they degrade on the terminal palette (8/16/256) and never fail. Truecolor themes are not v1.
- Toggle at runtime with `M-Y` (nano's key, no collision); option `syntax` and extension-based auto-selection.
- Grammar sources, in order: `~/.config/nnano/syntax/*.nanorc`, `/usr/share/nnano/syntax/*.nanorc`, `/usr/share/nano/*.nanorc` (distro grammars, read-only), then the **built-in** set embedded from `crates/nnano-core/assets/syntax/popular.nanorc`. The built-ins are the lowest layer — a user or distro grammar with the same name is found first — so they only fill the gaps, most visibly on a system where nano is not installed. They cover html/htmx/vue/svelte/astro, js/ts/tsx/jsx/jx, php/phtml, go, css/scss/sass/less, json, xml/svg, yaml, toml, ini, dotenv, Dockerfile, ignore files, markdown, shell, python, rust, c/c++, java and sql.
- A `syntax` block is **not** an option-registry row: the nanorc reader parses it into its own table.

### Search: nano's model (decided 2026-11-04)

- `^F` opens the prompt (`^W` is close buffer).
- **Not incremental**: the search runs when the prompt is accepted. No per-keystroke work, no debounce, no worker thread in v1.
- Literal by default; POSIX ERE only while the *regexp* toggle is on — same `regex-lite` engine as syntax, compiled once per accepted prompt.
- Case-insensitive by default; `M-C` toggles case sensitivity. Toggle is session state shown in the prompt; the default comes from the `case_sensitive` option.
- Wraps around and reports *Search wrapped*, like nano. Starts at the cursor.
- `M-W` find next, `M-B` find backwards; `F3` / `Shift+F3` stay as aliases.
- Prompt-scoped keys are modal, as in nano: inside the find/replace prompt `M-C` / `M-R` / `M-B` toggle case / regexp / backwards; outside a prompt `M-R` opens Replace. Not a collision — document it, do not "fix" it.
- Dropped from nano's prompt: the `^T` file browser (no tree, below).

### No split view, no file tree (decided 2026-11-04)

- One editor pane. `App` holds a single viewport; do **not** pre-design `Vec<Pane>` "for later". Tabs already go through the per-buffer rope, which is the whole point.
- No tree, no browser, no sidebar in v1.
- File access: `^O` open-by-path with completion, and the palette over **open buffers** only — it does not walk the filesystem (permissions, symlinks, `.gitignore`, cost on huge trees). Until tabs land the palette lists commands plus nothing else; the buffer rows are a phase-7 addition, not a new surface.
- Rationale: nano's ethos is "the shell launched me on a file". A tree re-imports the IDE model and a persistent UI region the keymap and layout never budget for.

### Other adopted defaults (2026-11-04)

- **No multiple cursors**; **rectangular selection yes** (block range, `M-6` copy still applies).
- **Undo grouped**: break on a 400 ms pause (fixed in v1), word boundary, and hard boundaries (`^K`, paste, save, buffer switch). Not per keystroke.
- **Clipboard**: internal register always; OSC 52 opt-in, capped at 64 KiB per copy — above that, truncate with a visible warning rather than block on a slow SSH tty. Implementation (2026-11-06): the `osc52` registry option (off by default, no nanorc equivalent), base64 written in `nnano-core::clipboard` (no new dependency), sequence written to stdout on `^C`/`M-6` only.
- **Autosave off**; **recovery file on**: `$XDG_STATE_HOME/nnano/recovery/<hash of absolute path>.swp` (fallback `~/.local/state/nnano/recovery/`), written on a 2 s idle debounce once the buffer is dirty, never from the redraw cycle; deleted after a successful save and on a clean exit. Implementation (2026-11-06): `nnano-core::recovery` owns the path, the header (`nnano-recovery v1`, absolute path, body encoding) and the write-through-temp-then-rename, so a torn snapshot cannot shadow a good one; the body keeps the buffer's encoding, so Latin-1 round-trips, and falls back to UTF-8 when the buffer holds a char Latin-1 cannot encode. A snapshot is restored **automatically** on the next start when it differs from the file on disk, and the buffer comes back dirty (`recovered unsaved changes for …`); a snapshot identical to the file is deleted instead. Recovery is disabled when there is no state directory, and tests never touch it.
- **Colours**: no truecolor and no theme files in v1; the default style is the terminal's own foreground/background plus the semantic colour enum.
- **Encoding**: detect and warn only. Saving never transcodes; a Latin-1 file round-trips byte-identical. Implementation (2026-11-05): UTF-8 when the bytes parse as UTF-8, otherwise Latin-1 read as the byte→`U+00NN` bijection so every byte survives; a UTF-16 BOM is refused, never guessed; a UTF-8 BOM stays in the text as `U+FEFF`. Saving a Latin-1 buffer stops at the first char outside `U+0000..=U+00FF` with an error instead of replacing it.
- **`nnano -`**: reads stdin into an ephemeral buffer; saving requires a filename (stdin is not seekable, so there is no in-place save). Implemented (2026-11-06): `^S` with no path opens the same *Save as* prompt `M-S` does, so the buffer only needs a name once.
- **`^O` completion** (2026-11-06): one path component deep, as a shell: the text up to the last `/` is the directory, the rest is the prefix; directories get a trailing `/`, dotfiles only complete after a dot, one candidate fills the line, several extend it to the common prefix. No listing widget, no filesystem walk beyond one `read_dir`.

### Justify, goto, execute, word movement (decided 2026-11-06)

- **`^J` justify**: the paragraph around the cursor, or the marked lines when there is a mark. The width is the **text area's**, resolved by the app loop (`Action::Justify` → `Editor::justify(width)`), so the editor never knows the screen. Words are split on whitespace and rejoined with single spaces, the first line's indent is kept, wrapping is measured in cells (CJK-safe), a word wider than the limit is not hyphenated, and the whole rewrite is one undo group. No `fill` option in v1.
- **`^G` / `M-G` goto**: `line`, `line,column` or `line:column`, one-based; `+N`/`-N` are relative to the current line. Out-of-range values are clamped, a non-numeric one is reported and changes nothing.
- **`M-X` execute**: run the line with `sh -c`, insert its stdout at the cursor as one undo step, report the exit code and the first stderr line in the status bar. No terminal handoff, no pager, no `$SHELL` lookup, no shell configuration in v1 — one command, one insertion.
- **Word movement**: `Ctrl+←`/`Ctrl+→` stop at word starts, where a word char is alphanumeric or `_` (Unicode letters count); a line break is a gap, so movement crosses lines.

## Keymap: desktop-first (decided 2026-11-04)

Desktop wins every collision; non-colliding nano bindings are kept. No legacy preset by default.

| Key | nnano (new) | nano was | Fate of nano binding |
|---|---|---|---|
| `^S` | Save | unbound (blocked by XOFF) | — |
| `^Q` | Quit (confirm if dirty) | unbound (blocked by XON) | — |
| `^O` | Open file | Write Out / save | moved to `^S` |
| `^X` | Cut | Exit | moved to `^Q` |
| `^C` | Copy | Location (cursor pos) | dropped → always shown in status bar |
| `^V` | Paste | Next Page | moved to `PgDn` / `M-↓` |
| `^Y` | Redo | Prev Page | moved to `PgUp` / `M-↑` |
| `^Z` | Undo | Suspend (SIGTSTP) | dropped |
| `^A` | Select All | Home (line start) | moved to `Home` |
| `^F` | Find | Forward 1 char | dropped (arrows do it) |
| `^W` | Close buffer | Where Is / search | moved to `^F` |
| `^R` | Reload from disk | Read File (insert) | moved to `M-I` |
| `^N` | New buffer | Next Line | dropped (arrows) |
| `^P` | Command palette | Prev Line | dropped (arrows) |
| `^T` | New tab | Execute command | moved to `M-X` |
| `^G` | Go To Line | Help | moved to `F1` / `^H` |
| `^H` | Help — the whole shortcut reference | Replace | Replace stays on `^\` + `M-R`; Backspace is the terminal's Backspace key (`0x7F`), decided 2026-11-06 |

Kept unchanged (no collision): `^K` cut line, `^U` paste alias, `^E` end of line, `^D` delete char, `^J` justify, `^L` refresh, `^B` back char, `^\` replace, `M-U`/`M-E` undo/redo, `M-A` mark, `M-6` copy, `M-G` goto, `M-\`/`M-/` first/last line, `M-W` find next, `M-B` find backwards, `M-C` case toggle, `M-R` regexp toggle, `M-Y` syntax highlighting toggle.
`^K` + `^U` are the pair worth defending: they are nano's identity and collide with nothing.

Added with the keymap: `M-S` save as, `M-,` settings menu, `F3`/`Shift+F3` find next/previous, `Ctrl+←`/`Ctrl+→` word movement, `Shift+←`/`Shift+→` extend the mark by characters and `Shift+↑`/`Shift+↓` by lines.

Prompt-scoped keys are modal, as in nano: inside the find/replace prompt `M-C` / `M-R` / `M-B` toggle case / regexp / backwards, while outside a prompt `M-R` opens Replace. Same key, different mode — not a collision.

User-facing keymap reference: `docs/<lang>/shortcuts.md` (en, es, zh-Hans, ja, hi, ar, fr, bn, pt, id). A keymap change means editing all ten.

Implemented rows (2026-11-09): `term/input.rs` decodes movement (arrows, `Ctrl+←`/`Ctrl+→`, `Shift+←`/`Shift+→`, `Shift+↑`/`Shift+↓`, `Home`/`End`, `PgUp`/`PgDn`, `M-↑`/`M-↓`), typing (`Enter`, `Tab`, printable chars), deletion (`Backspace`/`^?`, `Delete`), `^B`, `^D`, `^E`, `^L`, `^Q`, `^S`, `M-S`, `^O`, `^R`, `M-I`, `^G`/`M-G`, `^J`, `^F`, `^P`, `M-,`, `M-X`, `^H`/`F1`, `Esc`, `M-W`/`F3`, `M-B`/`Shift+F3`, `M-C`, `M-R`, `^\`, `M-Y`, and the editing block: `^A`, `^C`, `^V`/`^U`, `^X`, `^K`, `^Z`, `^Y`, `M-A`, `M-6`, `M-U`, `M-E`, `M-Z`, `^N`, `^T`, `^W`. Every other row decodes to `None` — not to a no-op action — and `input.rs` asserts that deferred list, so a row only starts working when its feature lands. The deferred list is down to `M-Q`; every binding the docs name is live.

`Shift+←`/`Shift+→` sit after the `Ctrl+←`/`Ctrl+→` rows and before the plain arrows: a row matches when its modifiers are a subset of the event's, so `Ctrl+Shift+←` stays a word move and plain `←` stays `MoveLeft`, while `Shift+←` starts or extends a mark.

`M-C`, `M-R` and `M-B` are modal by interpretation: they decode everywhere, the find/replace prompt acts on them, and outside a prompt `M-R` opens Replace and `M-B` repeats the search backwards. `Enter` (`Newline`) confirms inside a prompt or a panel; `Esc` (`Cancel`) closes an overlay and clears the mark.

`^Q` on a dirty buffer asks once: the status bar shows *unsaved changes: `^Q` again to quit, `^S` to save* and any other key cancels the pending quit. `^W` on a dirty buffer asks the same way before closing it.

Terminal constraints that gate the table (verify at startup, never assume):
- Disable flow control and `ISIG` so `^S`, `^Q`, `^C`, `^Z` reach the app as keys, not XOFF/SIGINT/SIGTSTP.
- `Enter`=`^M`, `Tab`=`^I`, `Backspace`=`^H`/`^?`, `Esc`=`^[` are unassignable.
- `^H` opens the shortcut reference (decided 2026-11-06, replaces the 2026-11-05 "Backspace alias"), and `F1` is the same overlay. Backspace is the terminal's Backspace key (`0x7F`); `input.rs` maps `KeyCode::Backspace` to `Action::Backspace`. A terminal that sends `0x08` for Backspace (some consoles) will open help on that key; `AGENTS.md`, `docs/*/shortcuts.md` and `LOG.md` record the trade-off, and no other binding uses `^H`.
- `^B` is tmux's default prefix, `^A` is screen's; document, don't fight.
- `Ctrl+Shift+<letter>` is unreliable on legacy terminals — do not promise `^Shift+Z`.
- macOS: `Cmd+*` never arrives; `M-` is Option and may emit accented characters.

## Text model: rope (decided 2026-11-04)

Each buffer is a **rope**. Not nano's linked list of `malloc`'d lines, not a piece table. Decided; do not relitigate per feature.

Why:
- Edits are O(log n) anywhere, including the middle of a 5 MB single line. Nano does `realloc` + `memmove` per keystroke → O(line length).
- No per-line allocation: avoids nano's ~48–56 B per line and the heap fragmentation of 13M-line files.
- Line/column lookups come from cached newline counts in the tree (O(log n)), which is what the viewport, status bar, `^G` and search need.
- Undo and selection act on byte ranges, not node surgery.

Consequences that must hold:
- **Load is one O(n) pass**: read the file, build the rope bottom-up. Never insert line by line.
- **Line-oriented API on top only**: `line_to_char` / `char_to_line`; no per-frame rope scans.
- `^K` / `^U` are range operations: cut a range into an internal rope, insert it back. No relinking.
- The rope stores bytes **as loaded**; `\r\n`, grapheme and width mapping stay at the edges (`render`, `save`).
- Startup latency and binary size are product requirements: measure both when the rope lands.

Decided (2026-11-04): the **`ropey`** crate, not an in-house rope, with its **byte-sized chunks** (~1 KB, `ChunkSize`) rather than line-aligned ones. Pin the exact version at scaffolding and keep `Cargo.lock`; wrap it behind `nnano-core::buffer` so the crate never leaks into `nnano-tui`.

## Stack and architecture (decided 2026-11-04)

| Area | Decision |
|---|---|
| TUI substrate | `ratatui 0.30` + `crossterm 0.29` backend |
| Text area | custom buffer (no `ratatui-textarea`) |
| Runtime | synchronous, single thread, `poll(16ms)` + redraw on `dirty` — **no async runtime** |
| Crate layout | workspace: `nnano-core` + `nnano-tui` + `nnano` (binary) |
| Config format | TOML at the XDG path, legacy `.nanorc` readable |
| Rust baseline | MSRV **1.88**, `edition = "2024"`, `rust-version = "1.88"` in every crate. Raised from 1.85 on 2026-11-05: `ratatui 0.30.2` (and `0.30.1`) declare `rust-version = 1.88.0`, and ratatui 0.30 is itself decided — no 0.30.x works on 1.85. Alternative rejected: `ratatui 0.30.0` (MSRV 1.86) misses the 0.30.1/0.30.2 fixes |

Rejected — do not revive without new evidence:
- `cursive`: retained mode, last release 2024-08, fights overlay control.
- `tui-realm` / `iocraft`: extra component model, no reduction in editor work.
- `ratatui-textarea`: cannot express multi-buffer, nano operation semantics and the decided keymap.
- `tokio`: weight and startup latency, unneeded for local file editing.
- Any other async runtime (`async-std`, `smol`): same reason. No `async fn` in the event loop, no `#[tokio::main]`; a slow task gets an `std::thread` + channel, not an executor.
- `termion` / `termwiz`: crossterm already covers Linux, macOS and Windows.
- `regex` / `regex-automata` / `fancy-regex` / a POSIX C binding for ERE: heavier than `regex-lite`, which already parses the `[[:alpha:]]` classes nano needs.
- In-house rope: `ropey` already gives byte chunks plus a cached line index; a home-grown one is not the product.

Speed and startup latency are product requirements, not nice-to-haves: weigh every dependency against binary size and start time.

### Crate layout

| Crate | Contents | Depends on |
|---|---|---|
| `nnano-core` | rope buffer, cursor/selection, undo/redo, nano operations, search, config model + option registry, file I/O, recovery snapshot, OSC 52 | no TUI crate; `ropey`, `regex-lite`, `unicode-width`, `unicode-segmentation`, `toml` |
| `nnano-tui` | `term/`, `ui/`, overlays, keymap, mouse regions, interface translations | `nnano-core`, `ratatui`, `crossterm` |
| `nnano` | `main.rs` only: clap args → config → terminal init → `App::run` | `nnano-tui`, `nnano-core`, `clap` |

Rules:
- Dependency direction is one-way: `nnano` → `nnano-tui` → `nnano-core`. Never the reverse; `nnano-core` must not know that a terminal exists.
- All editor semantics are testable in `nnano-core` without a TTY. UI snapshots (`TestBackend`) live in `nnano-tui`.
- The `Action` enum lives in `nnano-core`, so keyboard, mouse and command palette all produce the same type and cannot drift.
- No `[patch]`, no circular dev-dependencies: if something needs the reverse edge, it belongs in the lower crate.

### Dependencies (initial, per crate)

```toml
# nnano-core/Cargo.toml   (rust-version = "1.88")
ropey = "=1.6.1"
regex-lite = "=0.1.9"
toml = "=0.9.12"
unicode-width = "=0.2.3"
unicode-segmentation = "=1.13.3"

# nnano-tui/Cargo.toml
ratatui = { version = "=0.30.2", default-features = false,
            features = ["crossterm", "layout-cache", "macros",
                        "scrolling-regions", "underline-color"] }
crossterm = "=0.29.0"

# nnano/Cargo.toml (binary)
clap = { version = "=4.6.7", default-features = false,
         features = ["std", "help", "usage", "error-context", "suggestions",
                     "string"] }

# workspace dev-dependencies
insta = "=1.49.0"
```

- **Exact pins only** (decided 2026-11-05): every entry is `=x.y.z`, no caret ranges, in the manifests as well as `Cargo.lock`. `ropey 2.0.0-beta.1` exists and stays out; `toml 1.x` exists and stays out. Upgrading a pin is a normal `LOG.md`-worthy change, not an implicit drift.

- Each crate declares only what it uses; nothing is inherited implicitly.
- No `serde` derive for config: the TOML file is parsed into `toml::Table` and validated against the option registry at runtime (see *Option registry*). `toml` stays for parsing and span-aware errors; add `serde` only if a non-config format (e.g. the recovery file) needs typed (de)serialization.
- `clap` is used as a **builder** only: the `derive` feature is not enabled and must not be re-added. Flags come from the option registry, not from a struct. The `string` feature is on (2026-11-06) because the registry builds flag ids and long names at runtime; without it clap only accepts `&'static str`.
- Ratatui is pre-1.0 and modularized in 0.30: pin exact versions, keep `Cargo.lock`.
- No `widget-calendar` feature (unused weight). No async runtime, LSP or plugins until a real requirement appears.

### Layout by crate (approved 2026-11-04; created, phase 5)

```text
crates/
  nnano-core/src/
    lib.rs          public API + re-exports
    buffer/         rope, cursor, selection, undo/redo, grapheme map
    editor/         nano operations (^K/^U, justify, replace, goto, word movement)
    search.rs       nano search semantics (literal + ERE, wrap, case)
    syntax.rs       nanorc syntax parser + per-line span matcher
    config/         registry, Config + layers, toml_source, nanorc, paths
    file.rs         in-place save (truncate → write → flush → fsync), completion
    recovery.rs     snapshot path, header, encode/decode, write/load/remove
    clipboard.rs    base64 + OSC 52 sequence, 64 KiB cap
    i18n.rs         Lang enum, language codes, $LC_ALL/$LANG detection
  nnano-tui/src/
    lib.rs          App::run + App state (config, overlay, find, recovery clock)
    term/           raw mode, restore, panic hook, caps, input
    i18n/           translations: Lang columns, chrome.rs, keys.rs, t()/tf()
    ui/             status, compose_line, viewport, text
    ui/overlays/    Overlay enum, find/line prompts, settings, palette, help, prompt
  nnano/src/
    main.rs         args -> config -> terminal init -> App::run
```

`file`/`recovery`/`clipboard` live in `nnano-core` on purpose (`2026-11-06`): the
truncate-write-flush-sync order, the Latin-1 refusal, the snapshot format and the
OSC 52 cap are all testable without a TTY, and the TUI keeps only the decisions it
owns (which buffer, which message). Moving the old `save_in_place` out of
`nnano-tui/src/lib.rs` is what made the Latin-1 truncation bug visible.

### Event loop

```text
poll(16ms) -> drain events -> apply Action -> if dirty: draw -> diff
                              \-> snapshot when the 2 s recovery clock is due
```

- `Action` enum is the only mutation path: keyboard, mouse and command palette all produce it, so `^S` and the settings overlay cannot drift.
- Draw only when `dirty`; no fixed tick. Cursor blink optional, off by default.
- The recovery snapshot is written from the **idle path** of the loop (`flush_recovery(now)`), never from `draw`, and only when the buffer is dirty; every edit re-arms a 2 s debounce (`2026-11-06`).
- `Viewport::Fullscreen` only; `Viewport::Inline` rejected (nano always owns the screen).
- One thread; a worker thread is allowed later for slow I/O or search, via channel.

### Text model

"The Text model: rope" section above is normative; this only adds the rendering side.

- Cursor as `(line, char offset)` resolved through the rope's cached newline index; visual column derived via `unicode-width`. *Clarified 2026-11-05*: the offset is stored in chars, not bytes — ropey indexes lines by char, and a stored byte offset would need snapping back to a char boundary on every line change (O(line) on a 5 MB single line). `TextBuffer::char_to_byte` converts at the save/render edges. The invariant that matters is unchanged: **no visual column is ever stored**.
- Line breaks are ropey's Unicode set (`\n`, `\r`, `\r\n`, VT, FF, NEL, LS, PS), so a lone `\r` splits lines like `\n` does. `line_len`/`line_text` strip the terminator; **save must re-emit each line's own terminator**, never assume `\n` (phase 4).
- Display width is accumulated by one `buffer::Width` used by both the cursor and the renderer, so a cursor cannot drift from the glyphs under it: tab stops (8 until the registry exists), `unicode-width` per char, zero cells for ZWJ/VS16/skin-tone continuations and combining marks.
- Undo as an operation log of char ranges (not text snapshots) to bound memory. The written decision said "byte ranges"; chars are what ropey indexes, so the log stores chars and never stores a visual column.
- The viewport reads ranges from the rope; never keep a parallel `Vec<Line>` mirror.
- Required tests: CJK/emoji widths, tabs, `\r\n`, CRLF save preservation, 5 MB single line, 1M-line file.

### Configuration (decided 2026-11-04)

Standard location and format, with nano compatibility **on read**.

| Layer | Location | Writable | Notes |
|---|---|---|---|
| CLI flags | `argv` | — | highest precedence; long names reuse nano's spelling |
| User config | `$XDG_CONFIG_HOME/nnano/config.toml`, fallback `~/.config/nnano/config.toml` | yes | TOML, commented template written only on demand |
| System config | `/etc/nnano/config.toml` | no | optional |
| nano legacy | `~/.nanorc`, `$XDG_CONFIG_HOME/nano/nanorc` | **read-only** | never migrated, never rewritten |
| Built-in defaults | compiled in | — | zero-config works with none of the above |

Precedence: `CLI > user TOML > system TOML > nanorc > defaults`. Higher layer wins per key, not per file.

Compatibility rules:
- Map nanorc directives that correspond 1:1 to an option (`set linenumbers`, `set tabsize 4`, `set autoindent`, `set mouse`, …). `syntax` / `color` / `icolor` blocks are **not** options: the nanorc reader parses them into its own table (see *Product scope → Syntax highlighting*).
- Never write, reformat or migrate `.nanorc`; `nano` and `nnano` must be safe to use side by side.
- Unknown directive ⇒ one stderr warning at startup, then continue. Never abort.
- `include` is never followed (arbitrary file reads, including from a system file). The whitelisted grammar directories are glob-expanded by us instead; anything else warns.
- CLI flags keep nano's long spelling where semantics are unchanged, so existing aliases and scripts keep working. TOML keys are `snake_case` (`tabsize` flag ⇄ `tab_size` key).

#### Option registry (single source of truth)

One `const OPTIONS: &[Opt]` table in `nnano-core::config`; each option is declared exactly once:

| Field | Purpose |
|---|---|
| `id` | `Opt` enum variant: the compile-time handle code uses (`cfg.int(Opt::TabSize)`) |
| `key` | TOML key, e.g. `tab_size` |
| `long` / `short` | clap flag, e.g. `--tabsize` / `-T` |
| `kind` + `default` | type (`Bool`, `Int { min, max }`, `Text`, `Enum(&[&str])`) and the value used when nothing sets it |
| `nanorc` | legacy directive name, or `None` |
| `overlay` | group, label and widget in the settings overlay |
| `help` | CLI help text, reused verbatim by the overlay |

Generation rules (runtime, **no codegen**):
- CLI: `Command::args(OPTIONS.iter().map(row_to_arg))` — clap **builder**, never `derive`. `kind` produces both the `Arg` and its `ValueParser` (range checks, `PossibleValuesParser`), so clap's errors and the TOML validation come from the same row. `overlay.group` is reused as the `help_heading`.
- TOML: parsed into `toml::Table` (untyped), then each key is resolved against the table and coerced by `kind`. Unknown key ⇒ one warning; wrong type ⇒ warning for that key only; never abort.
- nanorc: the same string-keyed lookup (`lookup(directive) -> Option<&Opt>`). This layer already needs the runtime table, so CLI and TOML reuse it instead of adding two more parsers.
- Overlay: widgets built from `kind` + `overlay`; they read and write the same `Value` the CLI parsed. Integers step inside `Kind::Int`'s bounds, booleans toggle; the change goes into the live `Config`, never to disk.
- Layers are `Vec<Layer>` with an `origin`, merged highest first against the registry defaults; warnings are deduplicated per message so a repeated unknown directive warns once.
- Search is separate from config: the prompt's flags belong to the session, seeded from `case_sensitive`, so toggling them does not rewrite the file.
- A macro cannot do this: `#[derive(Parser)]` / `#[derive(Deserialize)]` expand before the table exists and only see the struct the author wrote. Hand-writing any of the four surfaces is a bug.
- Keep the surfaces coupled by ID, not by text: the overlay and `Action` reference `Opt` variants and `overlay` metadata, never label strings, so a renamed label cannot desync them.
- Bool rows render as a flag pair (`--mouse` / `--no-mouse`, `overrides_with` each other) so the CLI can express "off" and still win over a lower layer.
- Detect "set on the CLI" with `ArgMatches::value_source(id) == Some(ValueSource::CommandLine)`, not `contains_id`. Registry defaults are applied last, after merging layers; `--help` shows them from the table.
- Precedence is one merge over `Vec<(Layer, Map<Opt, Value>)>`; higher layer wins per key. No `Option<T>` field per option.
- The commented TOML template (written on demand) is emitted by iterating the table.
- Adding an option = one row + one `Opt` variant. The drift test is generic, not per option.

The drift test iterates the table and asserts:
- every `Opt` variant has exactly one row and vice versa;
- every `key`, `long` and `short` is unique and reachable from CLI, TOML, the overlay and (when `nanorc` is `Some`) the nanorc reader;
- every `default` validates under its own `kind`.

That test is what enforces "dual configuration".

### Terminal risks and mitigations

| Risk | Mitigation |
|---|---|
| `^S`/`^Q`/`^C` eaten by the tty | `cfmakeraw` + `tcsetattr` without `IXON`/`ISIG` before first draw; pty smoke test |
| `^B` = tmux prefix, `^A` = screen prefix | detect `$TMUX`/`$STY` and warn, never hijack |
| `Ctrl+Shift+<letter>` unreliable | try `PushKeyboardEnhancementFlags(DISAMBIGUATE_ESCAPE_CODES)`, silent fallback, never advertise the binding |
| ratatui pre-1.0 churn | backend isolated in `term/`, version pinned, migration cost stays local |
| Mouse differs across terminals | capability detection + graceful degradation; all mouse paths optional |
| ratatui has no editor primitives | custom buffer; `ratatui-textarea` only as design reference |

### Phases

1. Scaffolding: workspace + three crates, `term/` (init/restore/panic hook/caps), blank screen with status bar.
2. `nnano-core::buffer` + text render + cursor + scroll; `TestBackend` snapshots in `nnano-tui`.
3. `input.rs` complete against the keymap table; table-driven tests.
4. Open/save in place (no `file~` backup) / dirty / quit-confirmation; `nnano -` stdin buffer.
5. **Done (2026-11-06).** Overlays: settings (config mirror), command palette, find (nano prompt semantics), help; option registry and layered config behind them.
6. **Done (2026-11-06).** Mouse: selection, scroll, panel row clicks; bracketed paste; rectangular selection.
7. **Done (2026-11-06).** Undo/redo (grouped), replace, tabs (buffers), syntax highlighting (nano `.nanorc` model); `docs/{en,es,zh-Hans,ja}/shortcuts.md` updated with `M-Z` and the mouse/undo notes.
8. **Done (2026-11-06).** Save-as, `^O` open-by-path with completion, `^G` goto, `^J` justify, `M-X` execute, `Ctrl+←/→` word movement, `fsync` after `^S`, the recovery file and OSC 52 copy out; the Latin-1 truncation bug fixed; settings panel height fixed.

### Open items

**None.** Every v1 decision is closed (as of 2026-11-05); anything new gets added here *before* it reaches code.

Closed decisions:

| Item | Decision |
|---|---|
| Scaffolding | approved |
| `nn` alias | never ships |
| MSRV / edition | 1.88 / `edition 2024` (raised from 1.85 on 2026-11-05 by ratatui 0.30.2's `rust-version`) |
| ERE engine (search + syntax) | `regex-lite` |
| Rope crate + chunking | `ropey`, byte chunks |
| `file~` backup | removed from scope |
| Recovery, OSC 52, undo, colours | defaults recorded in *Other adopted defaults* |
| Save-as, `^O`, `^G`, `^J`, `M-X`, word movement (2026-11-06) | implemented as described in the keymap table, *Save: in place* and *Justify, goto, execute, word movement* |
| `^R` reload, `M-I` insert-file (2026-11-09) | implemented: reload replaces the buffer and drops the undo history, dirty buffers ask once; insert-file is one undo step behind a completing prompt |
| Option surfaces (2026-11-05) | runtime registry: clap **builder** + `toml::Table`, no `derive`, no codegen (see *Option registry*) |
| `^H` (2026-11-05) | Backspace alias in `input.rs`, never Replace — documented in *Terminal constraints* |
| Version pinning (2026-11-05) | exact `=x.y.z` for every dependency, manifests included (see *Dependencies*) |
| MSRV bump (2026-11-05) | 1.85 → **1.88**, forced by `ratatui 0.30.2`'s `rust-version`; ratatui 0.30 kept |

## Verification (once code exists)
- Order: `cargo fmt --check` → `cargo clippy --workspace -- -D warnings` → `cargo test --workspace`.
- Single test: `cargo test <name>`. Per crate: `cargo test -p nnano-core <name>`.
- Editor logic tests belong in `nnano-core` and must not need a TTY; UI snapshots use `insta` + `TestBackend` in `nnano-tui`.
- `TestBackend` is not enough for `term/`: any change there needs a pty run that asserts alternate screen in/out, cursor restore, `^Q` exit 0, and `^S`/`^C`/`^Z` reaching the app as keys. Do not commit the harness; it is a one-off script.
- `target/` never committed. `Cargo.lock` is committed (binary project).

## Language policy
- **AI-facing docs in English** (this file, `LOG.md`, design notes, ADRs).
- **User-facing docs in en, es, zh-Hans, ja, hi, ar, fr, bn, pt, id** under `docs/<lang>/`; keep all ten in sync when editing.
- **Interface strings are translated, key names are not**: `Ctrl+S`, `Alt+,` and file paths stay as typed in every language. A new user-visible string goes into `nnano-tui::i18n` with one column per language; `t()` falls back to English so a missing row degrades instead of breaking the UI.
- **`man/nnano.1` and `completions/` stay English**: a man page is not translated in this project, and a completion is option text. Both are drift-guarded against the registry by `crates/nnano/tests/docs.rs`; translating them is a separate decision, not an omission.
- Code, identifiers, commit messages, CLI help: English.

## Inherited rules
Global rules in `~/.config/opencode/AGENTS.md` still apply: UI radii ≤ 6px, no GitHub Actions unless requested, `docker-compose.yml` stays repo-only.
