# AGENTS.md — neo-nano (`nnano`)

Rust rewrite of the nano editor: a modern TUI that keeps nano's ethos — **simple, fast, easy, comfortable** — and adds today's TUI expectations. Not a fork of GNU nano's C codebase.

## Status (2026-11-05)
- **Scaffolded** (2026-11-05): workspace `Cargo.toml`, `Cargo.lock`, `.gitignore`, three crates under `crates/` with the decided module skeleton. Builds, `fmt`, `clippy -D warnings` and `test` pass.
- Phase 1 (workspace + `term/` + blank screen + status bar) is **not** done: `term/` and `ui/` are empty module stubs, `nnano` exits with *not implemented*.
- No `.git` yet: repo init still waits for the user.
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
| File tree / browser | **no** | palette over open buffers instead |
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

Each row that becomes an option costs one registry row, one overlay widget and four doc languages — count that before adding more.

### Save: in place (decided 2026-11-04)

- `^S` truncates and rewrites the file the user opened. Explicitly rejected: temp file + `rename` — it breaks hardlinks, replaces a symlink with a regular file and changes the inode other processes watch.
- Order: open the real path with truncation → write → flush. One step, no copy.
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
- Grammar sources, in order: `~/.config/nnano/syntax/*.nanorc`, `/usr/share/nnano/syntax/*.nanorc`, `/usr/share/nano/*.nanorc` (distro grammars, read-only — inherits nano's set instead of shipping one).
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
- File access: `^O` open-by-path with completion, and the palette over **open buffers** only — it does not walk the filesystem (permissions, symlinks, `.gitignore`, cost on huge trees).
- Rationale: nano's ethos is "the shell launched me on a file". A tree re-imports the IDE model and a persistent UI region the keymap and layout never budget for.

### Other adopted defaults (2026-11-04)

- **No multiple cursors**; **rectangular selection yes** (block range, `M-6` copy still applies).
- **Undo grouped**: break on a 400 ms pause (fixed in v1), word boundary, and hard boundaries (`^K`, paste, save, buffer switch). Not per keystroke.
- **Clipboard**: internal register always; OSC 52 opt-in, capped at 64 KiB per copy — above that, truncate with a visible warning rather than block on a slow SSH tty.
- **Autosave off**; **recovery file on**: `$XDG_STATE_HOME/nnano/recovery/<hash of absolute path>.swp` (fallback `~/.local/state/nnano/recovery/`), written on a 2 s idle debounce once the buffer is dirty, never from the redraw cycle; deleted after a successful save and on a clean exit.
- **Colours**: no truecolor and no theme files in v1; the default style is the terminal's own foreground/background plus the semantic colour enum.
- **Encoding**: detect and warn only. Saving never transcodes; a Latin-1 file round-trips byte-identical.
- **`nnano -`**: reads stdin into an ephemeral buffer; saving requires a filename (stdin is not seekable, so there is no in-place save).

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
| `^G` | Go To Line | Help | moved to `F1` |
| `^H` | Backspace alias (not an action of its own) | Replace | never bound to Replace; `^\` + `M-R` cover it |

Kept unchanged (no collision): `^K` cut line, `^U` paste alias, `^E` end of line, `^D` delete char, `^J` justify, `^L` refresh, `^B` back char, `^\` replace, `M-U`/`M-E` undo/redo, `M-A` mark, `M-6` copy, `M-G` goto, `M-\`/`M-/` first/last line, `M-W` find next, `M-B` find backwards, `M-C` case toggle, `M-R` regexp toggle, `M-Y` syntax highlighting toggle.
`^K` + `^U` are the pair worth defending: they are nano's identity and collide with nothing.

Added with the keymap: `M-S` save as, `M-,` settings menu, `F3`/`Shift+F3` find next/previous.

Prompt-scoped keys are modal, as in nano: inside the find/replace prompt `M-C` / `M-R` / `M-B` toggle case / regexp / backwards, while outside a prompt `M-R` opens Replace. Same key, different mode — not a collision.

User-facing keymap reference: `docs/<lang>/shortcuts.md` (en, es, zh-Hans, ja). A keymap change means editing all four.

Terminal constraints that gate the table (verify at startup, never assume):
- Disable flow control and `ISIG` so `^S`, `^Q`, `^C`, `^Z` reach the app as keys, not XOFF/SIGINT/SIGTSTP.
- `Enter`=`^M`, `Tab`=`^I`, `Backspace`=`^H`/`^?`, `Esc`=`^[` are unassignable.
- `^H` is **Backspace, never Replace** (decided 2026-11-05): Linux console and some `TERM`s send `^H` for backspace, so leaving it unbound would break backspace there. `input.rs` maps both `^H` and `^?` to `Action::Backspace`; nano's `^H`-adjacent binding does not survive.
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
| `nnano-core` | rope buffer, cursor/selection, undo/redo, nano operations, search, config model + option registry | no TUI crate; `ropey`, `regex-lite`, `unicode-width`, `unicode-segmentation`, `toml` |
| `nnano-tui` | `term/`, `ui/`, overlays, keymap, mouse regions | `nnano-core`, `ratatui`, `crossterm` |
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
         features = ["std", "help", "usage", "error-context", "suggestions"] }

# workspace dev-dependencies
insta = "=1.49.0"
```

- **Exact pins only** (decided 2026-11-05): every entry is `=x.y.z`, no caret ranges, in the manifests as well as `Cargo.lock`. `ropey 2.0.0-beta.1` exists and stays out; `toml 1.x` exists and stays out. Upgrading a pin is a normal `LOG.md`-worthy change, not an implicit drift.

- Each crate declares only what it uses; nothing is inherited implicitly.
- No `serde` derive for config: the TOML file is parsed into `toml::Table` and validated against the option registry at runtime (see *Option registry*). `toml` stays for parsing and span-aware errors; add `serde` only if a non-config format (e.g. the recovery file) needs typed (de)serialization.
- `clap` is used as a **builder** only: the `derive` feature is not enabled and must not be re-added. Flags come from the option registry, not from a struct.
- Ratatui is pre-1.0 and modularized in 0.30: pin exact versions, keep `Cargo.lock`.
- No `widget-calendar` feature (unused weight). No async runtime, LSP or plugins until a real requirement appears.

### Layout by crate (approved 2026-11-04; not created yet)

```text
crates/
  nnano-core/src/
    lib.rs          public API + re-exports
    buffer/         rope, cursor, selection, undo/redo, grapheme map
    editor/         nano operations (^K/^U, justify, replace, goto)
    search.rs       nano search semantics (literal + ERE, wrap, case)
    syntax.rs       nanorc syntax parser + per-line span matcher
    config/         option registry, defaults, TOML table reader, nanorc reader
  nnano-tui/src/
    lib.rs          App::run
    term/           raw mode, restore, panic hook, caps, input
    ui/             status, overlays/, regions, style map
  nnano/src/
    main.rs         args -> config -> terminal init -> App::run
```

### Event loop

```text
poll(16ms) -> drain events -> apply Action -> if dirty: draw -> diff
```

- `Action` enum is the only mutation path: keyboard, mouse and command palette all produce it, so `^S` and the settings overlay cannot drift.
- Draw only when `dirty`; no fixed tick. Cursor blink optional, off by default.
- `Viewport::Fullscreen` only; `Viewport::Inline` rejected (nano always owns the screen).
- One thread; a worker thread is allowed later for slow I/O or search, via channel.

### Text model

"The Text model: rope" section above is normative; this only adds the rendering side.

- Cursor as `(line, byte offset)` resolved through the rope's cached newline index; visual column derived via `unicode-width`.
- Undo as an operation log of byte ranges (not text snapshots) to bound memory.
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
- Overlay: widgets built from `kind` + `overlay`; they read and write the same `Value` the CLI parsed.
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
5. Overlays: settings (config mirror), command palette (open buffers), find (nano prompt semantics), help.
6. Mouse: selection, scroll, menu clicks; bracketed paste; rectangular selection.
7. Undo/redo (grouped), replace, tabs, syntax highlighting (nano `.nanorc` model); keep `docs/{en,es,zh-Hans,ja}/shortcuts.md` in sync.

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
| Option surfaces (2026-11-05) | runtime registry: clap **builder** + `toml::Table`, no `derive`, no codegen (see *Option registry*) |
| `^H` (2026-11-05) | Backspace alias in `input.rs`, never Replace — documented in *Terminal constraints* |
| Version pinning (2026-11-05) | exact `=x.y.z` for every dependency, manifests included (see *Dependencies*) |
| MSRV bump (2026-11-05) | 1.85 → **1.88**, forced by `ratatui 0.30.2`'s `rust-version`; ratatui 0.30 kept |

## Verification (once code exists)
- Order: `cargo fmt --check` → `cargo clippy --workspace -- -D warnings` → `cargo test --workspace`.
- Single test: `cargo test <name>`. Per crate: `cargo test -p nnano-core <name>`.
- Editor logic tests belong in `nnano-core` and must not need a TTY; UI snapshots use `insta` + `TestBackend` in `nnano-tui`.
- `target/` never committed. `Cargo.lock` is committed (binary project).

## Language policy
- **AI-facing docs in English** (this file, `LOG.md`, design notes, ADRs).
- **User-facing docs in en, es, zh-Hans, ja** under `docs/<lang>/`; keep all four in sync when editing.
- Code, identifiers, commit messages, CLI help: English.

## Inherited rules
Global rules in `~/.config/opencode/AGENTS.md` still apply: UI radii ≤ 6px, no GitHub Actions unless requested, `docker-compose.yml` stays repo-only.
