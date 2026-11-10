# Changelog

All notable changes to this project. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning: [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Project website at <https://nestorpons.github.io/neo-nano/>: a dependency-free terminal-retro page in English (what it is, screenshot, install paths, full key reference, author) published from `docs/` on `main` with GitHub Pages — no build step and no GitHub Actions. A selector translates it in place to the app's ten languages with the browser's Google Translator API, right-to-left for Arabic.
- `Shift+↑` / `Shift+↓` extend the mark by whole lines, editor-style: the first press starts the mark at the cursor, later presses grow the selection and keep the wanted visual column. Inside a panel the same keys move the highlighted row.
- A one-column scrollbar on the right edge of the text area, with a proportional thumb; clicking it jumps to that part of the file without moving the cursor. The column is reserved, so the text width does not change when a file grows past the screen.
- A built-in grammar set covers the most common languages, so highlighting works without nano installed: html/htmx/vue/svelte/astro, js/ts/tsx/jsx/jx, php/phtml, go, css/scss/sass/less, json, xml/svg, yaml, toml, ini, dotenv, Dockerfile, ignore files, markdown, shell, python, rust, c/c++, java and sql. A grammar with the same name in a user or distro directory wins.
- The status bar now shows `line:column/total`, adding the document's line count to the cursor position.

## [0.1.1] — 2026-11-09

### Added

- `nnano(1)` man page: options, the key reference, files, environment and the save/encoding guarantees.
- Shell completions for bash, zsh and fish, covering every option and the language codes.
- Both are installed by the `nnano` and `nnano-git` packages, and a test fails the build when a registry flag is missing from the man page or a completion.

## [0.1.0] — 2026-11-09

First release. A nano-compatible terminal editor with a desktop-first keymap.

### Added

- **Editing**: rope-backed buffers, grouped undo/redo (`^Z`, `^Y`), cut/copy/paste with an internal register (`^K`, `^X`, `^C`, `^V`), stream and rectangular selection (`M-Z`), bracketed paste.
- **Files**: in-place save with `fsync` (`^S`), save-as (`M-S`), open by path with `Tab` completion (`^O`), reload from disk (`^R`), insert a file at the cursor (`M-I`), multiple buffers with a one-row buffer bar (`^N`, `^T`, `^W`), `nnano -` reading stdin.
- **Search**: literal or POSIX-ERE search (`^F`), find next/previous (`F3`/`Shift+F3`), replace with per-match confirmation (`^\`), nano's wrap and case toggles.
- **Text tools**: goto line/column and relative offsets (`^G`), justify to the text width (`^J`), execute a command and insert its output (`M-X`), word movement (`Ctrl+←/→`).
- **Interface**: command palette (`^P`), settings overlay (`M-,`), scrollable shortcut reference (`^H`/`F1`), mouse (click, drag, Alt+drag rectangle, wheel), ten interface languages (English, 简体中文, हिन्दी, Español, العربية, Français, বাংলা, Português, Bahasa Indonesia, 日本語).
- **Syntax highlighting**: nano's `.nanorc` grammar model, per line, loaded from `~/.config/nnano/syntax`, `/usr/share/nnano/syntax` and `/usr/share/nano`.
- **Configuration**: one option registry behind CLI flags, `config.toml`, `/etc/nnano/config.toml` and legacy `.nanorc` (read-only), with a settings overlay that cannot drift from the flags.
- **Safety**: recovery snapshots for dirty buffers (`$XDG_STATE_HOME/nnano/recovery/`), encoding detection without transcoding (UTF-8 and Latin-1; UTF-16 refused), opt-in OSC 52 copy-out capped at 64 KiB.

### Notes

- No backup file and no atomic rename: `^S` truncates and rewrites the file in place.
- No plugins, LSP or split view, by design.
- `Ctrl+S`, `Ctrl+Q`, `Ctrl+C` and `Ctrl+Z` need a terminal without flow control and with `ISIG` disabled; nnano sets both up itself.

[0.1.1]: https://github.com/nestorPons/neo-nano/releases/tag/v0.1.1
[0.1.0]: https://github.com/nestorPons/neo-nano/releases/tag/v0.1.0
