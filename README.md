 _   _ _____ ___       _   _   _    _   _  ___
| \ | | ____/ _ \     | \ | | / \  | \ | |/ _ \
|  \| |  _|| | | |    |  \| |/ _ \ |  \| | | | |
| |\  | |__| |_| |    | |\  / ___ \| |\  | |_| |
|_| \_|_____\___/     |_| \_/_/   \_\_| \_|\___/


A modern terminal text editor that keeps nano's ethos — **simple, fast, easy, comfortable** — with the interface people expect from a TUI today. Written in Rust.

> The binary is `nnano`. This repository is `neo-nano`.
>
> Website: <https://nestorpons.github.io/neo-nano/> · full reference in ten languages under [`docs/`](docs/).

## What it is

- **Zero config**: `nnano file.txt` opens and edits. Nothing mandatory to set up.
- **Desktop-first keymap**: `Ctrl+S` saves, `Ctrl+Q` quits, `Ctrl+O` opens. Non-colliding nano bindings stay (`Ctrl+K` cut line, `Ctrl+U` paste).
- **Nano's soul**: in-place saves, `.nanorc` syntax grammars, search that wraps, literal by default.
- **Syntax out of the box**: nano's `.nanorc` grammars are read when present, and built-in grammars cover html/htmx, js/ts/tsx/jsx/jx, php/phtml, go, css/scss, json, xml, yaml, toml, sql, markdown, shell, python, rust, c/c++ and java — no nano install required.
- **Modern parts**: rope-backed buffers, grouped undo, mouse, rectangular selection, command palette, in-app settings, ten interface languages, recovery of unsaved work.

One viewport, one cursor, no plugins, no LSP. That is the design, not a gap.

## Install

```sh
# Arch / AUR
yay -S nnano-git

# From source (Rust 1.88 or newer)
git clone https://github.com/nestorPons/neo-nano
cd neo-nano
cargo build --release
install -Dm755 target/release/nnano ~/.local/bin/nnano
```

The package installs `nnano(1)` (`man nnano`) and shell completions for bash, zsh and fish. From source they live in `man/` and `completions/`.

## Quick start

```sh
nnano              # empty buffer
nnano file.txt     # open a file
nnano -            # read stdin; ^S asks for a name
```

| Key | Does |
|---|---|
| `Ctrl+S` / `Alt+S` | Save in place / save as |
| `Ctrl+O` / `Ctrl+R` / `Alt+I` | Open by path / reload from disk / insert a file |
| `Ctrl+Q` / `Ctrl+W` | Quit / close buffer (asks once if modified) |
| `Ctrl+Z` / `Ctrl+Y` | Undo / redo |
| `Ctrl+F` / `F3` | Find / find next |
| `Ctrl+P` | Command palette |
| `Alt+,` | Settings (including the interface language) |
| `Ctrl+H` | Every shortcut, scrollable |

Full reference in your language: [en](docs/en/shortcuts.md) · [es](docs/es/shortcuts.md) · [zh-Hans](docs/zh-Hans/shortcuts.md) · [ja](docs/ja/shortcuts.md) · [hi](docs/hi/shortcuts.md) · [ar](docs/ar/shortcuts.md) · [fr](docs/fr/shortcuts.md) · [bn](docs/bn/shortcuts.md) · [pt](docs/pt/shortcuts.md) · [id](docs/id/shortcuts.md).

## Configuration

Every option is settable from the command line **and** from the settings overlay; neither path can drift, because both are generated from one table.

```sh
nnano --tabsize 4 --mouse --language es --osc52
```

```toml
# ~/.config/nnano/config.toml
tab_size = 4
case_sensitive = false
mouse = true
syntax = true
osc52 = false
language = "auto"
```

Precedence: CLI > user `config.toml` > `/etc/nnano/config.toml` > `~/.nanorc` > defaults. A `.nanorc` is read, never rewritten; unknown directives warn once and are ignored.

## Safety

- `Ctrl+S` truncates and rewrites the file in place, then `fsync`s. There is no `file~` backup and no temp-file rename: hardlinks, symlinks and watched inodes stay intact.
- A dirty buffer is snapshotted 2 s after the last edit under `$XDG_STATE_HOME/nnano/recovery/` and restored on the next start after a crash.
- Encoding is detected, never transcoded; UTF-16 is refused instead of guessed.

## Build and test

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Workspace: `nnano-core` (editor semantics, no terminal), `nnano-tui` (ratatui/crossterm layer), `nnano` (binary). Dependency edges point one way only.

## License

MIT. See [LICENSE](LICENSE).
