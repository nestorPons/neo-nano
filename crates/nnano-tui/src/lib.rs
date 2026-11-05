//! Terminal layer for `nnano`.
//!
//! Owns everything that assumes a terminal: raw mode and its restore path, the
//! panic hook, capability detection, input decoding, drawing and overlays. All
//! editor semantics stay in `nnano-core`.
//!
//! Event loop (phase 4): `poll(16ms)` -> decode one key into an `Action` -> apply
//! -> redraw only when dirty. No fixed tick, no async runtime, no worker thread.

pub mod term;
pub mod ui;

use std::fs::OpenOptions;
use std::io;
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crossterm::event::{self, Event};
use nnano_core::buffer::{DEFAULT_TAB_WIDTH, EncodeError, Encoding, TextBuffer, encoding};
use nnano_core::editor::{Action, Editor};

use crate::term::Caps;
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
}

impl App {
    /// Empty buffer named `[No Name]`.
    pub fn new() -> Self {
        Self::from_text("", "")
    }

    /// A buffer with known text and no path, used by tests and by `from_bytes`.
    pub fn from_text(file: &str, text: &str) -> Self {
        Self {
            file: if file.is_empty() {
                "[No Name]".to_string()
            } else {
                file.to_string()
            },
            caps: Caps::detect(),
            editor: Editor::from_text(text),
            viewport: Viewport::default(),
            body_height: 0,
            redraw: true,
            quit: false,
            path: None,
            encoding: Encoding::Utf8,
            message: None,
            confirm_quit: false,
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

    /// Single mutation path: keyboard (and later mouse and palette) all land here.
    fn apply(&mut self, action: Action) {
        let confirming = self.confirm_quit;
        self.confirm_quit = false;
        self.message = None;

        match action {
            Action::Quit => {
                if self.editor.is_dirty() && !confirming {
                    self.confirm_quit = true;
                    self.inform("unsaved changes: ^Q again to quit, ^S to save");
                } else {
                    self.quit = true;
                }
            }
            Action::Save => self.save(),
            // Pages are the only actions that need viewport geometry.
            Action::PageUp => self.editor.move_vertical(-page_step(self.body_height)),
            Action::PageDown => self.editor.move_vertical(page_step(self.body_height)),
            other => self.editor.apply(other),
        }
        self.redraw = true;
    }

    /// `^S`. Writes in place when there is a path; nothing is written otherwise.
    fn save(&mut self) {
        let Some(path) = self.path.clone() else {
            self.inform("no filename yet: save-as needs the prompt (phase 5)");
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

    pub(crate) fn tab_width(&self) -> usize {
        // Becomes the `tab_size` registry option; 8 until the registry exists.
        DEFAULT_TAB_WIDTH
    }

    /// Test seam: move the cursor and mark the frame dirty.
    #[cfg(test)]
    pub(crate) fn set_cursor(&mut self, line: usize, col: usize) {
        self.editor.set_cursor(line, col);
        self.redraw = true;
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

/// Entry point used by the `nnano` binary.
pub fn run(file: Option<String>) -> io::Result<()> {
    App::open(file)?.run()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use nnano_core::buffer::Encoding;
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
        assert!(app.message.as_deref().unwrap().contains("^Q again"));

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
        assert!(app.message.as_deref().unwrap().contains("Latin-1"));
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
        assert!(app.message.as_deref().unwrap().starts_with("saved"));
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
        assert!(app.message.as_deref().unwrap().contains("Latin-1"));
        assert!(app.editor().is_dirty(), "a failed save must stay dirty");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unnamed_buffer_cannot_be_saved_yet() {
        let mut app = App::from_text("", "");
        app.apply(Action::Insert('x'));
        app.apply(Action::Save);
        assert!(app.message.as_deref().unwrap().contains("no filename"));
        assert!(app.editor().is_dirty());
    }

    #[test]
    fn no_argument_opens_an_unnamed_buffer() {
        let app = App::open(None).unwrap();
        assert_eq!(app.file, "[No Name]");
    }
}
