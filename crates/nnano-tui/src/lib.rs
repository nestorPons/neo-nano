//! Terminal layer for `nnano`.
//!
//! Owns everything that assumes a terminal: raw mode and its restore path, the
//! panic hook, capability detection, input decoding, drawing and overlays. All
//! editor semantics stay in `nnano-core`.
//!
//! Event loop (phase 4): `poll(16ms)` -> decode one key into an `Action` -> apply
//! -> redraw only when dirty. No fixed tick, no async runtime, no worker thread.
//!
//! Phase 5 adds the overlays. They are not a second key path: an overlay consumes
//! the same `Action` values the editor would, and only one overlay is open at a
//! time.

pub mod term;
pub mod ui;

use std::fs::OpenOptions;
use std::io;
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crossterm::event::{self, Event};
use nnano_core::buffer::{DEFAULT_TAB_WIDTH, EncodeError, Encoding, TextBuffer, encoding};
use nnano_core::config::{Config, Opt};
use nnano_core::editor::{Action, Editor};
use nnano_core::search::{Direction, Search};

use crate::term::Caps;
use crate::ui::overlays::{FindSettings, Overlay, Palette, Prompt, Settings};
use crate::ui::viewport::Viewport;

/// Event-poll granularity. It is not a redraw rate: a frame is drawn only when
/// the app is dirty.
const POLL_INTERVAL: Duration = Duration::from_millis(16);

/// Application state: one buffer, one cursor, one viewport (no split view).
#[derive(Debug)]
pub struct App {
    /// Buffer name as shown in the status bar.
    pub(crate) file: String,
    pub(crate) caps: Caps,
    editor: Editor,
    viewport: Viewport,
    body_height: u16,
    redraw: bool,
    quit: bool,
    /// Where `^S` writes. `None` for an unnamed buffer and for stdin, which is not
    /// seekable and therefore cannot be saved in place.
    path: Option<PathBuf>,
    encoding: Encoding,
    /// One status-bar line, cleared by the next action.
    message: Option<String>,
    /// `^Q` was pressed once on a dirty buffer and is waiting for a second press.
    confirm_quit: bool,
    /// Effective options: the same values the CLI, TOML and nanorc layers
    /// produced, kept live so the settings overlay cannot drift.
    config: Config,
    /// The overlay that owns the keys, if any.
    overlay: Option<Overlay>,
    /// Last search: pattern and toggles survive the prompt, as in nano.
    find: FindSettings,
}

impl App {
    /// Empty buffer named `[No Name]`.
    pub fn new() -> Self {
        Self::from_text("", "")
    }

    /// A buffer with known text and no path, used by tests and by `from_bytes`.
    pub fn from_text(file: &str, text: &str) -> Self {
        let config = Config::defaults();
        let mut editor = Editor::from_text(text);
        editor.set_tab_width(config.int(Opt::TabSize).max(1) as usize);
        Self {
            file: if file.is_empty() {
                "[No Name]".to_string()
            } else {
                file.to_string()
            },
            caps: Caps::detect(),
            editor,
            viewport: Viewport::default(),
            body_height: 0,
            redraw: true,
            quit: false,
            path: None,
            encoding: Encoding::Utf8,
            message: None,
            confirm_quit: false,
            config,
            overlay: None,
            find: FindSettings::default(),
        }
    }

    /// Open `target`: a file path, `-` for stdin, or nothing for an empty buffer.
    /// A missing file opens an empty buffer with that name and is created by `^S`.
    pub fn open(target: Option<String>) -> io::Result<Self> {
        match target.as_deref() {
            None => Ok(Self::new()),
            Some("-") => {
                let mut bytes = Vec::new();
                io::stdin().read_to_end(&mut bytes)?;
                Self::from_bytes("[stdin]", None, &bytes)
            }
            Some(name) => {
                let path = PathBuf::from(name);
                match std::fs::read(&path) {
                    Ok(bytes) => Self::from_bytes(name, Some(path), &bytes),
                    Err(err) if err.kind() == io::ErrorKind::NotFound => {
                        let mut app = Self::from_text(name, "");
                        app.path = Some(path);
                        Ok(app)
                    }
                    Err(err) => Err(err),
                }
            }
        }
    }

    /// Decode already-read bytes. Detection only: Latin-1 loads with a warning,
    /// UTF-16 is refused.
    fn from_bytes(name: &str, path: Option<PathBuf>, bytes: &[u8]) -> io::Result<Self> {
        let loaded = encoding::decode(bytes)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        let mut app = Self::from_text(name, &loaded.text);
        app.path = path;
        app.encoding = loaded.encoding;
        if loaded.encoding == Encoding::Latin1 {
            app.inform("read as Latin-1 (not UTF-8); saving keeps the same bytes");
        }
        Ok(app)
    }

    /// Adopt the merged configuration: the `tab_size` the renderer uses and the
    /// default search case come from here, never from a hardcoded constant.
    pub fn apply_config(&mut self, config: Config) {
        self.find.case_sensitive = config.bool(Opt::CaseSensitive);
        self.config = config;
        self.sync_editor();
    }

    /// Run until the user quits. The `Session` guard restores the terminal on
    /// every exit path: normal quit, `?`, or unwind.
    pub fn run(mut self) -> io::Result<()> {
        // The editor owns the whole screen, so there is nothing to render into a
        // pipe. Refuse early with a readable message instead of failing inside
        // raw-mode setup with `ENXIO`.
        if !io::stdout().is_terminal() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "standard output is not a terminal",
            ));
        }
        let mut session = term::Session::start()?;
        while !self.quit {
            if self.redraw {
                let size = session.terminal().size()?;
                let body_height = size.height.saturating_sub(1);
                self.body_height = body_height;
                ui::ensure_cursor_visible(&mut self, size.width, body_height);
                session.terminal().draw(|frame| ui::draw(frame, &self))?;
                self.redraw = false;
            }
            if !event::poll(POLL_INTERVAL)? {
                continue;
            }
            match event::read()? {
                Event::Key(key) => {
                    if let Some(action) = term::input::decode(&key) {
                        self.apply(action);
                    }
                }
                Event::Resize(_, _) => self.redraw = true,
                _ => {}
            }
        }
        Ok(())
    }

    /// Single mutation path: keyboard, palette and (later) mouse all land here.
    fn apply(&mut self, action: Action) {
        let confirming = std::mem::take(&mut self.confirm_quit);
        self.message = None;

        if self.overlay.is_some() {
            self.apply_overlay(action, confirming);
            self.redraw = true;
            return;
        }

        match action {
            Action::Quit => self.request_quit(confirming),
            Action::Save => self.save(),
            Action::OpenFind => self.overlay = Some(Overlay::Find(self.find.open())),
            Action::OpenPalette => self.overlay = Some(Overlay::Palette(Palette::new())),
            Action::OpenSettings => self.overlay = Some(Overlay::Settings(Settings::new())),
            Action::OpenHelp => self.overlay = Some(Overlay::Help),
            Action::FindNext => self.repeat_find(Direction::Forward),
            Action::FindPrevious => self.repeat_find(Direction::Backward),
            // `M-B` outside a prompt repeats the search backwards; inside one it
            // toggles the direction (handled by the overlay).
            Action::ToggleBackwards => self.repeat_find(Direction::Backward),
            // `M-C`/`M-R` only mean something in a prompt; `M-R` will open Replace
            // in phase 7.
            Action::Cancel | Action::ToggleCase | Action::ToggleRegexp => {}
            // Pages are the only actions that need viewport geometry.
            Action::PageUp => self.editor.move_vertical(-page_step(self.body_height)),
            Action::PageDown => self.editor.move_vertical(page_step(self.body_height)),
            other => self.editor.apply(other),
        }
        self.redraw = true;
    }

    /// Keys while an overlay owns them. Text-editing actions go to the prompt; the
    /// rest are per-overlay.
    fn apply_overlay(&mut self, action: Action, confirming: bool) {
        if let Some(Overlay::Find(prompt)) = &mut self.overlay
            && prompt.apply(action)
        {
            return;
        }
        if let Some(Overlay::Palette(palette)) = &mut self.overlay
            && palette.edit(action)
        {
            return;
        }

        let Some(overlay) = self.overlay.take() else {
            return;
        };
        match overlay {
            Overlay::Find(prompt) => match action {
                Action::Cancel => {}
                Action::Newline => self.accept_find(&prompt),
                Action::ToggleCase => {
                    self.find.case_sensitive = !self.find.case_sensitive;
                    self.overlay = Some(Overlay::Find(prompt));
                }
                Action::ToggleRegexp => {
                    self.find.regexp = !self.find.regexp;
                    self.overlay = Some(Overlay::Find(prompt));
                }
                Action::ToggleBackwards => {
                    self.find.backwards = !self.find.backwards;
                    self.overlay = Some(Overlay::Find(prompt));
                }
                Action::Quit => {
                    self.overlay = Some(Overlay::Find(prompt));
                    self.request_quit(confirming);
                }
                _ => self.overlay = Some(Overlay::Find(prompt)),
            },
            Overlay::Palette(mut palette) => match action {
                Action::Cancel => {}
                Action::Newline => {
                    if let Some(action) = palette.selected_action() {
                        self.apply(action);
                    }
                }
                Action::MoveUp => {
                    palette.select_previous();
                    self.overlay = Some(Overlay::Palette(palette));
                }
                Action::MoveDown => {
                    palette.select_next();
                    self.overlay = Some(Overlay::Palette(palette));
                }
                Action::Quit => {
                    self.overlay = Some(Overlay::Palette(palette));
                    self.request_quit(confirming);
                }
                _ => self.overlay = Some(Overlay::Palette(palette)),
            },
            Overlay::Settings(mut settings) => match action {
                Action::Cancel => {}
                Action::Quit => {
                    self.overlay = Some(Overlay::Settings(settings));
                    self.request_quit(confirming);
                }
                other => {
                    if settings.handle(other, &mut self.config).is_some() {
                        self.sync_editor();
                    }
                    self.overlay = Some(Overlay::Settings(settings));
                }
            },
            Overlay::Help => match action {
                Action::Cancel | Action::Newline => {}
                Action::Quit => self.request_quit(confirming),
                _ => self.overlay = Some(Overlay::Help),
            },
        }
    }

    /// `^Q`: quit, or ask once when the buffer has unsaved changes.
    fn request_quit(&mut self, confirming: bool) {
        if self.editor.is_dirty() && !confirming {
            self.confirm_quit = true;
            self.inform("unsaved changes: ^Q again to quit, ^S to save");
        } else {
            self.quit = true;
        }
    }

    /// `^S`. Writes in place when there is a path; nothing is written otherwise.
    fn save(&mut self) {
        let Some(path) = self.path.clone() else {
            self.inform("no filename yet: save-as is not implemented");
            return;
        };
        match save_in_place(&path, self.editor.buffer(), self.encoding) {
            Ok(()) => {
                self.editor.mark_saved();
                self.inform(format!("saved {}", path.display()));
            }
            Err(err) => self.inform(format!("save failed: {err}")),
        }
    }

    /// The prompt was accepted: compile once and jump to the first hit.
    fn accept_find(&mut self, prompt: &Prompt) {
        self.find.query = prompt.text().to_string();
        if prompt.is_empty() {
            self.inform("empty search pattern");
            return;
        }
        let direction = self.find.direction();
        let search =
            match Search::compile(prompt.text(), self.find.regexp, self.find.case_sensitive) {
                Ok(search) => search,
                Err(err) => {
                    self.inform(format!("invalid pattern: {err}"));
                    return;
                }
            };
        let from = self.cursor_index();
        let hit = search.find(self.editor.buffer(), from, direction);
        self.jump_to(hit);
    }

    /// `M-W`/`M-B`: reuse the last pattern and step past the current match.
    fn repeat_find(&mut self, direction: Direction) {
        if self.find.query.is_empty() {
            self.inform("no search yet: ^F");
            return;
        }
        let search =
            match Search::compile(&self.find.query, self.find.regexp, self.find.case_sensitive) {
                Ok(search) => search,
                Err(err) => {
                    self.inform(format!("invalid pattern: {err}"));
                    return;
                }
            };
        let cursor = self.cursor_index();
        let from = match direction {
            Direction::Forward => cursor.saturating_add(1),
            Direction::Backward => cursor,
        };
        let hit = search.find(self.editor.buffer(), from, direction);
        self.jump_to(hit);
    }

    fn jump_to(&mut self, hit: Option<nnano_core::search::Hit>) {
        match hit {
            Some(hit) => {
                let (line, col) = self.editor.buffer().char_to_position(hit.start);
                self.editor.set_cursor(line, col);
                if hit.wrapped {
                    self.inform("search wrapped");
                }
            }
            None => {
                let query = self.find.query.clone();
                self.inform(format!("not found: {query}"));
            }
        }
    }

    fn cursor_index(&self) -> usize {
        self.editor.cursor().char_index(self.editor.buffer())
    }

    /// Push `tab_size` into the editor after the settings overlay changed it.
    fn sync_editor(&mut self) {
        let width = self.tab_width();
        self.editor.set_tab_width(width);
        self.redraw = true;
    }

    fn inform(&mut self, message: impl Into<String>) {
        self.message = Some(message.into());
    }

    pub(crate) fn editor(&self) -> &Editor {
        &self.editor
    }

    pub(crate) fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    pub(crate) fn viewport_mut(&mut self) -> &mut Viewport {
        &mut self.viewport
    }

    pub(crate) fn config(&self) -> &Config {
        &self.config
    }

    pub(crate) fn overlay(&self) -> Option<&Overlay> {
        self.overlay.as_ref()
    }

    pub(crate) fn find_settings(&self) -> &FindSettings {
        &self.find
    }

    /// `tab_size` from the registry; the renderer and the editor share it.
    pub(crate) fn tab_width(&self) -> usize {
        let width = self.config.int(Opt::TabSize);
        usize::try_from(width.max(1)).unwrap_or(DEFAULT_TAB_WIDTH)
    }

    /// Test seam: move the cursor and mark the frame dirty.
    #[cfg(test)]
    pub(crate) fn set_cursor(&mut self, line: usize, col: usize) {
        self.editor.set_cursor(line, col);
        self.redraw = true;
    }

    /// Test seam: the one status-line message.
    #[cfg(test)]
    pub(crate) fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Left of the status bar: a message if there is one, otherwise identity,
    /// dirty marker and the multiplexer notice. `^B`/`^A` are documented, never
    /// rebound: the multiplexer keeps its prefix.
    fn status_left(&self) -> String {
        if let Some(message) = &self.message {
            return format!(" {message}");
        }
        let mut text = format!(" nnano {}", env!("CARGO_PKG_VERSION"));
        if self.editor.is_dirty() {
            text.push_str("  [Modified]");
        }
        if self.caps.tmux {
            text.push_str("   ^B: tmux prefix");
        } else if self.caps.screen {
            text.push_str("   ^A: screen prefix");
        }
        text
    }

    /// Right of the status bar: `line:column  file`, one-based and visual, as
    /// nano prints it.
    fn status_right(&self) -> String {
        let (line, col) = self
            .editor
            .cursor()
            .display_position(self.editor.buffer(), self.tab_width());
        format!("{line}:{col}  {} ", self.file)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// Write the buffer over the file the user opened.
///
/// In place on purpose: truncate the real path, write, flush. No temp file, no
/// `rename`, **no `file~`**. Hardlinks and symlinks survive, and a crash or a full
/// disk mid-write leaves a truncated file — the recovery file is the only net.
fn save_in_place(path: &Path, buffer: &TextBuffer, encoding: Encoding) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    match encoding::write_encoded(&mut file, buffer.chunks(), encoding) {
        Ok(()) => {}
        Err(EncodeError::Io(err)) => return Err(err),
        Err(err @ EncodeError::NotRepresentable(_)) => {
            return Err(io::Error::new(io::ErrorKind::InvalidData, err));
        }
    }
    file.flush()
}

/// One page is the visible area minus the row the cursor already sits on.
fn page_step(body_height: u16) -> isize {
    isize::try_from(body_height.saturating_sub(1).max(1)).unwrap_or(isize::MAX)
}

/// Entry point used by the `nnano` binary: read args, then run with the merged
/// configuration the binary resolved from the environment.
pub fn run(file: Option<String>, config: Config) -> io::Result<()> {
    let mut app = App::open(file)?;
    app.apply_config(config);
    app.run()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use nnano_core::buffer::Encoding;
    use nnano_core::config::{Config, Layer, Opt, Value};
    use nnano_core::editor::Action;

    use super::{App, page_step};

    /// Unique path under the system temp dir, removed by the test.
    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("nnano-{name}-{}", std::process::id()))
    }

    fn open_file(path: &Path) -> App {
        App::open(Some(path.display().to_string())).unwrap()
    }

    fn text(app: &App) -> String {
        (0..app.editor().buffer().line_count())
            .map(|line| app.editor().buffer().line_text(line))
            .collect::<Vec<_>>()
            .join("\\n")
    }

    /// A config built the way the binary builds it, without touching the disk.
    fn config(values: &[(Opt, Value)]) -> Config {
        Config::resolve(vec![Layer {
            origin: "test".to_string(),
            values: values.to_vec(),
            warnings: Vec::new(),
        }])
    }

    #[test]
    fn page_step_leaves_the_cursor_row_visible() {
        assert_eq!(page_step(24), 23);
        assert_eq!(page_step(1), 1);
        assert_eq!(page_step(0), 1);
    }

    #[test]
    fn typing_through_actions_marks_the_buffer_dirty() {
        let mut app = App::from_text("x.txt", "");
        app.apply(Action::Insert('a'));
        app.apply(Action::Insert('b'));
        assert!(app.editor().is_dirty());
        assert_eq!(app.editor().buffer().line_text(0), "ab");
    }

    #[test]
    fn quit_action_ends_the_loop() {
        let mut app = App::from_text("x.txt", "");
        app.apply(Action::Quit);
        assert!(app.quit);
    }

    #[test]
    fn quit_with_unsaved_changes_asks_once_and_then_quits() {
        let mut app = App::from_text("x.txt", "");
        app.apply(Action::Insert('x'));
        app.apply(Action::Quit);
        assert!(!app.quit);
        assert!(app.confirm_quit);
        assert!(app.message().unwrap().contains("^Q again"));

        app.apply(Action::Quit);
        assert!(app.quit);
    }

    #[test]
    fn any_other_key_cancels_the_pending_quit() {
        let mut app = App::from_text("x.txt", "");
        app.apply(Action::Insert('x'));
        app.apply(Action::Quit);
        app.apply(Action::MoveLeft);
        assert!(!app.confirm_quit);
        app.apply(Action::Quit);
        assert!(!app.quit);
    }

    #[test]
    fn pages_move_by_the_body_height() {
        let text = (0..100).map(|n| format!("line {n}\n")).collect::<String>();
        let mut app = App::from_text("long.txt", &text);
        app.body_height = 10;
        app.apply(Action::PageDown);
        assert_eq!(app.editor().cursor().line(), 9);
        app.apply(Action::PageUp);
        assert_eq!(app.editor().cursor().line(), 0);
    }

    #[test]
    fn missing_file_opens_an_empty_buffer_with_that_name() {
        let path = temp("missing.txt");
        let _ = std::fs::remove_file(&path);
        let app = open_file(&path);
        assert!(app.editor().buffer().is_empty());
        assert!(app.file.contains("missing"));
    }

    #[test]
    fn a_file_is_read_in_one_pass() {
        let path = temp("open.txt");
        std::fs::write(&path, "uno\ndos\n").unwrap();
        let app = open_file(&path);
        assert_eq!(app.editor().buffer().line_count(), 3);
        assert_eq!(app.editor().buffer().line_text(1), "dos");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn latin1_is_detected_with_a_warning_instead_of_failing() {
        let path = temp("latin1.txt");
        std::fs::write(&path, b"caf\xE9\r\n").unwrap();
        let app = open_file(&path);
        assert_eq!(app.encoding, Encoding::Latin1);
        assert_eq!(text(&app), "café\\n");
        assert!(app.message().unwrap().contains("Latin-1"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn utf16_is_refused() {
        let path = temp("utf16.txt");
        std::fs::write(&path, [0xFF, 0xFE, b'h', 0]).unwrap();
        let err = App::open(Some(path.display().to_string())).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("UTF-16"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn saving_writes_in_place_and_keeps_crlf_and_bytes() {
        let path = temp("save.txt");
        let original = b"uno\r\ndos\r\n";
        std::fs::write(&path, original).unwrap();
        let mut app = open_file(&path);

        app.apply(Action::Save);
        assert!(app.message().unwrap().starts_with("saved"));
        assert_eq!(
            std::fs::read(&path).unwrap(),
            original,
            "untouched save must not rewrite bytes"
        );

        // Edit the first line and save: CRLF of the other line must survive.
        app.set_cursor(0, 3);
        app.apply(Action::Insert('!'));
        assert!(app.editor().is_dirty());
        app.apply(Action::Save);
        assert!(!app.editor().is_dirty());
        assert_eq!(std::fs::read(&path).unwrap(), b"uno!\r\ndos\r\n");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn saving_a_missing_file_creates_it() {
        let path = temp("created.txt");
        let _ = std::fs::remove_file(&path);
        let mut app = App::open(Some(path.display().to_string())).unwrap();
        app.apply(Action::Insert('h'));
        app.apply(Action::Insert('i'));
        app.apply(Action::Save);
        assert_eq!(std::fs::read(&path).unwrap(), b"hi");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn latin1_save_refuses_a_char_it_cannot_hold() {
        let path = temp("latin1-save.txt");
        std::fs::write(&path, b"caf\xE9\n").unwrap();
        let mut app = open_file(&path);
        app.apply(Action::Insert('漢'));
        app.apply(Action::Save);
        assert!(app.message().unwrap().contains("Latin-1"));
        assert!(app.editor().is_dirty(), "a failed save must stay dirty");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unnamed_buffer_cannot_be_saved_yet() {
        let mut app = App::from_text("", "");
        app.apply(Action::Insert('x'));
        app.apply(Action::Save);
        assert!(app.message().unwrap().contains("no filename"));
        assert!(app.editor().is_dirty());
    }

    #[test]
    fn no_argument_opens_an_unnamed_buffer() {
        let app = App::open(None).unwrap();
        assert_eq!(app.file, "[No Name]");
    }

    #[test]
    fn the_tab_width_comes_from_the_registry() {
        let mut app = App::from_text("t.txt", "\tx");
        app.apply_config(config(&[(Opt::TabSize, Value::Int(4))]));
        assert_eq!(app.tab_width(), 4);
        assert_eq!(app.editor().tab_width(), 4);
    }

    #[test]
    fn the_find_prompt_jumps_to_the_first_match() {
        let mut app = App::from_text("s.txt", "uno\ndos tres\n");
        app.apply(Action::OpenFind);
        for ch in "tres".chars() {
            app.apply(Action::Insert(ch));
        }
        app.apply(Action::Newline);
        assert!(app.overlay().is_none(), "accept closes the prompt");
        let (line, col) = (app.editor().cursor().line(), app.editor().cursor().col());
        assert_eq!((line, col), (1, 4));
    }

    #[test]
    fn a_wrapping_search_says_so() {
        let mut app = App::from_text("s.txt", "uno\ndos\n");
        app.set_cursor(1, 0);
        app.apply(Action::OpenFind);
        for ch in "uno".chars() {
            app.apply(Action::Insert(ch));
        }
        app.apply(Action::Newline);
        assert_eq!(app.editor().cursor().line(), 0);
        assert!(app.message().unwrap().contains("wrapped"));
    }

    #[test]
    fn a_missing_pattern_reports_not_found() {
        let mut app = App::from_text("s.txt", "uno\n");
        app.apply(Action::OpenFind);
        for ch in "zzz".chars() {
            app.apply(Action::Insert(ch));
        }
        app.apply(Action::Newline);
        assert!(app.message().unwrap().contains("not found"));
        assert_eq!(app.editor().cursor().line(), 0);
    }

    #[test]
    fn an_invalid_regexp_is_reported_and_keeps_the_buffer() {
        let mut app = App::from_text("s.txt", "a(b\n");
        app.apply(Action::OpenFind);
        app.apply(Action::ToggleRegexp);
        for ch in "a(".chars() {
            app.apply(Action::Insert(ch));
        }
        app.apply(Action::Newline);
        assert!(app.message().unwrap().contains("invalid pattern"));
        assert!(!app.editor().is_dirty());
    }

    #[test]
    fn find_next_steps_past_the_current_match() {
        let mut app = App::from_text("s.txt", "uno dos uno\n");
        app.apply(Action::OpenFind);
        for ch in "uno".chars() {
            app.apply(Action::Insert(ch));
        }
        app.apply(Action::Newline);
        assert_eq!(app.editor().cursor().col(), 0);
        app.apply(Action::FindNext);
        assert_eq!(app.editor().cursor().col(), 8);
        app.apply(Action::FindPrevious);
        assert_eq!(app.editor().cursor().col(), 0);
    }

    #[test]
    fn m_b_toggles_the_direction_in_the_prompt_and_repeats_outside_it() {
        let mut app = App::from_text("s.txt", "uno dos uno\n");
        app.apply(Action::OpenFind);
        for ch in "uno".chars() {
            app.apply(Action::Insert(ch));
        }
        app.apply(Action::ToggleBackwards);
        assert!(app.find_settings().backwards);
        // Backwards from the start wraps to the last match.
        app.apply(Action::Newline);
        assert_eq!(app.editor().cursor().col(), 8);

        // Outside a prompt the same key repeats the last search backwards.
        app.apply(Action::ToggleBackwards);
        assert_eq!(app.editor().cursor().col(), 0);
    }

    #[test]
    fn find_next_without_a_pattern_asks_for_one() {
        let mut app = App::from_text("s.txt", "uno\n");
        app.apply(Action::FindNext);
        assert!(app.message().unwrap().contains("^F"));
    }

    #[test]
    fn the_case_option_sets_the_default_search_case() {
        let mut app = App::from_text("s.txt", "Hola Mundo\n");
        app.apply_config(config(&[(Opt::CaseSensitive, Value::Bool(true))]));
        app.apply(Action::OpenFind);
        for ch in "mundo".chars() {
            app.apply(Action::Insert(ch));
        }
        app.apply(Action::Newline);
        assert!(app.message().unwrap().contains("not found"));
    }

    #[test]
    fn escape_closes_every_overlay() {
        for open in [
            Action::OpenFind,
            Action::OpenPalette,
            Action::OpenSettings,
            Action::OpenHelp,
        ] {
            let mut app = App::from_text("s.txt", "");
            app.apply(open);
            assert!(app.overlay().is_some(), "{open:?}");
            app.apply(Action::Cancel);
            assert!(app.overlay().is_none(), "{open:?}");
        }
    }

    #[test]
    fn the_settings_overlay_changes_the_tab_width_live() {
        let mut app = App::from_text("s.txt", "\tx");
        app.apply(Action::OpenSettings);
        app.apply(Action::MoveRight);
        assert_eq!(app.tab_width(), 9);
        assert_eq!(app.editor().tab_width(), 9);
        app.apply(Action::MoveLeft);
        assert_eq!(app.tab_width(), 8);
    }

    #[test]
    fn the_palette_runs_a_command_and_closes() {
        let mut app = App::from_text("s.txt", "");
        app.apply(Action::OpenPalette);
        for ch in "settings".chars() {
            app.apply(Action::Insert(ch));
        }
        app.apply(Action::Newline);
        assert!(matches!(app.overlay(), Some(super::Overlay::Settings(_))));
    }

    #[test]
    fn the_palette_can_quit() {
        let mut app = App::from_text("s.txt", "");
        app.apply(Action::OpenPalette);
        for ch in "quit".chars() {
            app.apply(Action::Insert(ch));
        }
        app.apply(Action::Newline);
        assert!(app.quit);
    }

    #[test]
    fn the_palette_navigates_with_the_arrows() {
        let mut app = App::from_text("s.txt", "");
        app.apply(Action::OpenPalette);
        app.apply(Action::MoveDown);
        app.apply(Action::Newline);
        // The second row is "Find": it opens the prompt.
        assert!(matches!(app.overlay(), Some(super::Overlay::Find(_))));
    }

    #[test]
    fn typing_while_a_panel_is_open_never_reaches_the_buffer() {
        let mut app = App::from_text("s.txt", "");
        app.apply(Action::OpenSettings);
        app.apply(Action::Insert('x'));
        assert_eq!(app.editor().buffer().line_text(0), "");
        assert!(!app.editor().is_dirty());
    }

    #[test]
    fn quit_inside_a_prompt_still_asks_and_then_quits() {
        let mut app = App::from_text("s.txt", "");
        app.apply(Action::Insert('x'));
        app.apply(Action::OpenFind);
        app.apply(Action::Quit);
        assert!(!app.quit);
        assert!(app.confirm_quit);
        app.apply(Action::Quit);
        assert!(app.quit);
    }
}
