//! Terminal layer for `nnano`.
//!
//! Owns everything that assumes a terminal: raw mode and its restore path, the
//! panic hook, capability detection, input decoding, drawing and overlays. All
//! editor semantics stay in `nnano-core`.
//!
//! Event loop: `poll(16ms)` -> decode one event into an `Action` -> apply ->
//! redraw only when dirty. No fixed tick, no async runtime, no worker thread.
//!
//! Tabs are buffers: `App` owns a `Vec<Document>`, each with its own rope, cursor,
//! viewport and undo history. There is still one viewport on screen; a one-row
//! buffer bar appears when more than one buffer is open.

pub mod i18n;
pub mod term;
pub mod ui;

use std::io;
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use nnano_core::buffer::{DEFAULT_TAB_WIDTH, Encoding, Selection, encoding};
use nnano_core::config::{self, Config, Opt};
use nnano_core::editor::{Action, Editor};
use nnano_core::search::{Direction, Search};
use nnano_core::syntax::{self, Span as SyntaxSpan, SyntaxSet};
use nnano_core::{clipboard, file, recovery};

use crate::i18n::{Lang, t, tf};
use crate::term::Caps;
use crate::ui::overlays::line::Kind;
use crate::ui::overlays::replace::{Replace, Stage};
use crate::ui::overlays::{FindSettings, Help, Line, Overlay, Palette, Prompt, Settings};
use crate::ui::viewport::Viewport;

/// Event-poll granularity. It is not a redraw rate: a frame is drawn only when
/// the app is dirty.
const POLL_INTERVAL: Duration = Duration::from_millis(16);

/// One open buffer: its rope, cursor, undo history and scroll offset.
#[derive(Debug)]
pub struct Document {
    /// Name as shown in the status bar and the buffer bar.
    pub(crate) file: String,
    /// Where `^S` writes. `None` for an unnamed buffer and for stdin, which is not
    /// seekable and therefore cannot be saved in place.
    path: Option<PathBuf>,
    encoding: Encoding,
    /// Grammar selected by the file name, from the loaded set.
    grammar: Option<String>,
    editor: Editor,
    viewport: Viewport,
}

impl Document {
    fn empty(name: &str) -> Self {
        Self {
            // The raw name on purpose: the *display* name of an unnamed buffer is
            // translated at render time, so switching the interface language
            // does not have to rewrite every `Document`.
            file: name.to_string(),
            path: None,
            encoding: Encoding::Utf8,
            grammar: None,
            editor: Editor::new(),
            viewport: Viewport::default(),
        }
    }

    fn from_text(name: &str, text: &str) -> Self {
        Self {
            editor: Editor::from_text(text),
            ..Self::empty(name)
        }
    }

    /// Decode already-read bytes. Detection only: Latin-1 loads with a warning,
    /// UTF-16 is refused.
    fn from_bytes(name: &str, path: Option<PathBuf>, bytes: &[u8]) -> io::Result<Self> {
        let loaded = encoding::decode(bytes)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        Ok(Self {
            path,
            encoding: loaded.encoding,
            ..Self::from_text(name, &loaded.text)
        })
    }
}

/// Display name of a buffer: the file name, or the translated "no name" label.
fn display_name(name: &str, lang: Lang) -> String {
    if name.is_empty() {
        t(lang, "[No Name]").to_string()
    } else {
        name.to_string()
    }
}

/// Application state: a list of buffers, one active, one viewport on screen.
#[derive(Debug)]
pub struct App {
    docs: Vec<Document>,
    active: usize,
    pub(crate) caps: Caps,
    body_height: u16,
    redraw: bool,
    quit: bool,
    /// One status-bar line, cleared by the next action.
    message: Option<String>,
    /// `^Q` was pressed once on a dirty buffer and is waiting for a second press.
    confirm_quit: bool,
    /// `^W` was pressed once on a dirty buffer and is waiting for a second press.
    confirm_close: bool,
    /// `^R` was pressed once on a dirty buffer and is waiting for a second press.
    confirm_reload: bool,
    /// Effective options: the same values the CLI, TOML and nanorc layers
    /// produced, kept live so the settings overlay cannot drift.
    config: Config,
    /// The overlay that owns the keys, if any.
    overlay: Option<Overlay>,
    /// Last search: pattern and toggles survive the prompt, as in nano.
    find: FindSettings,
    /// Last known terminal size, so a mouse event can be mapped without asking
    /// the terminal again.
    screen: (u16, u16),
    /// Grammars loaded from the syntax directories.
    syntax: SyntaxSet,
    /// Session toggle behind `M-Y` and the `syntax` option.
    syntax_enabled: bool,
    /// Whether the next frame must scroll to keep the cursor visible. The wheel
    /// turns it off so scrolling the view does not get undone by the cursor.
    follow_cursor: bool,
    /// Where snapshots go (`<state>/nnano/recovery`). `None` disables the
    /// recovery file entirely: tests and a `HOME`-less environment must not
    /// write anything.
    recovery_dir: Option<PathBuf>,
    /// When the next snapshot is due. Set by every edit, cleared when it is
    /// written or when the buffer becomes clean: a 2 s idle debounce, never a
    /// timer in the redraw path.
    recovery_due: Option<Instant>,
}

impl App {
    /// Empty buffer named `[No Name]`.
    pub fn new() -> Self {
        Self::from_docs(vec![Document::empty("")])
    }

    /// A buffer with known text and no path, used by tests and by `from_bytes`.
    pub fn from_text(file: &str, text: &str) -> Self {
        Self::from_docs(vec![Document::from_text(file, text)])
    }

    fn from_docs(docs: Vec<Document>) -> Self {
        let config = Config::defaults();
        let mut app = Self {
            docs,
            active: 0,
            caps: Caps::detect(),
            body_height: 0,
            redraw: true,
            quit: false,
            message: None,
            confirm_quit: false,
            confirm_close: false,
            confirm_reload: false,
            config,
            overlay: None,
            find: FindSettings::default(),
            screen: (0, 0),
            syntax: SyntaxSet::new(),
            syntax_enabled: true,
            follow_cursor: true,
            recovery_dir: None,
            recovery_due: None,
        };
        app.sync_editors();
        app
    }

    /// Open `target`: a file path, `-` for stdin, or nothing for an empty buffer.
    /// A missing file opens an empty buffer with that name and is created by `^S`.
    pub fn open(target: Option<String>) -> io::Result<Self> {
        let doc = match target.as_deref() {
            None => Document::empty(""),
            Some("-") => {
                let mut bytes = Vec::new();
                io::stdin().read_to_end(&mut bytes)?;
                Document::from_bytes("[stdin]", None, &bytes)?
            }
            Some(name) => {
                // Absolute from the start: the recovery file is keyed by the
                // absolute path, and `^S` must write where the user meant even if
                // the process changes directory (it never does, but the hash must
                // be stable across runs).
                let path = file::absolute(name).unwrap_or_else(|_| PathBuf::from(name));
                match std::fs::read(&path) {
                    Ok(bytes) => Document::from_bytes(name, Some(path), &bytes)?,
                    Err(err) if err.kind() == io::ErrorKind::NotFound => Document {
                        path: Some(path),
                        ..Document::empty(name)
                    },
                    Err(err) => return Err(err),
                }
            }
        };
        let mut app = Self::from_docs(vec![doc]);
        if app.document().encoding == Encoding::Latin1 {
            app.inform(t(
                app.lang(),
                "read as Latin-1 (not UTF-8); saving keeps the same bytes",
            ));
        }
        Ok(app)
    }

    /// Adopt the merged configuration: the `tab_size` every buffer uses and the
    /// default for syntax highlighting.
    pub fn apply_config(&mut self, config: Config) {
        self.find.case_sensitive = config.bool(Opt::CaseSensitive);
        self.syntax_enabled = config.bool(Opt::Syntax);
        self.config = config;
        self.sync_editors();
    }

    /// Adopt the loaded grammars and resolve each open buffer's grammar by name.
    pub fn apply_syntax(&mut self, syntax: SyntaxSet) {
        self.syntax = syntax;
        self.resolve_grammar();
    }

    /// Re-select every buffer's grammar from the loaded set. The name is what
    /// decides, so a save-as to another extension changes the colours.
    fn resolve_grammar(&mut self) {
        for document in &mut self.docs {
            document.grammar = self
                .syntax
                .select(&document.file)
                .map(|grammar| grammar.name.clone());
        }
    }

    /// Spans for one line of the active buffer, or nothing when highlighting is
    /// off, the buffer has no grammar, or the line is beyond the cost cap.
    pub(crate) fn syntax_spans(&self, line: usize) -> Vec<SyntaxSpan> {
        if !self.syntax_enabled {
            return Vec::new();
        }
        let Some(grammar) = self
            .document()
            .grammar
            .as_deref()
            .and_then(|name| self.syntax.get(name))
        else {
            return Vec::new();
        };
        if self.editor().buffer().line_len(line) > syntax::MAX_LINE {
            return Vec::new();
        }
        let text = self.editor().buffer().line_text(line);
        grammar.highlight(&text, syntax::MAX_LINE)
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
            // The `mouse` option can be flipped in the settings overlay, so the
            // capture state is synced every tick; `set_mouse` is a no-op when it
            // already matches.
            session.set_mouse(self.config.bool(Opt::Mouse))?;
            // A snapshot is due 2 s after the last edit. It is checked here, on
            // the idle path, and never from the redraw cycle.
            self.flush_recovery(Instant::now());
            if self.redraw {
                let size = session.terminal().size()?;
                self.screen = (size.width, size.height);
                let (_, body, _, _) = ui::areas(self.screen_rect(), self.buffer_count());
                self.body_height = body.height;
                if self.follow_cursor {
                    ui::ensure_cursor_visible(&mut self, body.width, body.height);
                }
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
                Event::Mouse(mouse) => self.on_mouse(mouse),
                // Bracketed paste arrives as one event, not as keystrokes, so it
                // is one undo step and cannot trip over Ctrl shortcuts.
                Event::Paste(text) => self.apply(Action::PasteText(text)),
                Event::Resize(width, height) => {
                    self.screen = (width, height);
                    self.redraw = true;
                }
                _ => {}
            }
        }
        // A clean exit owns no snapshots: the buffers were saved or the user
        // accepted losing them. Anything left (a panic, a kill) stays on disk.
        self.forget_all_recovery();
        Ok(())
    }

    /// Single mutation path: keyboard, palette and mouse all land here.
    fn apply(&mut self, action: Action) {
        let confirming_quit = std::mem::take(&mut self.confirm_quit);
        let confirming_close = std::mem::take(&mut self.confirm_close);
        let confirming_reload = std::mem::take(&mut self.confirm_reload);
        self.message = None;
        // Any key or click re-attaches the view to the cursor; the wheel is the
        // one action that scrolls without it.
        self.follow_cursor = !matches!(action, Action::ScrollView(_));

        if self.overlay.is_some() {
            // Shift+↑/↓ extend a mark in the text, but a panel has no mark: there
            // they move the highlighted row like the plain arrows, so the
            // existing navigation does not regress.
            let action = match action {
                Action::SelectUp => Action::MoveUp,
                Action::SelectDown => Action::MoveDown,
                other => other,
            };
            self.apply_overlay(action, confirming_quit);
            // An overlay can still edit the buffer (replace), so the snapshot
            // clock is re-armed here too, not only on the editor path.
            self.schedule_recovery(Instant::now());
            self.redraw = true;
            return;
        }

        match action {
            Action::Quit => self.request_quit(confirming_quit),
            Action::Save => self.save(),
            Action::SaveAs => self.overlay = Some(Overlay::Line(Line::new(Kind::SaveAs))),
            Action::OpenPath => self.overlay = Some(Overlay::Line(Line::new(Kind::Open))),
            Action::InsertFile => {
                self.overlay = Some(Overlay::Line(Line::new(Kind::InsertFile)));
            }
            Action::Reload => self.reload(confirming_reload),
            Action::Goto => self.overlay = Some(Overlay::Line(Line::new(Kind::Goto))),
            Action::Execute => self.overlay = Some(Overlay::Line(Line::new(Kind::Execute))),
            Action::Justify => {
                let width = self.body_width();
                self.editor_mut().justify(width);
            }
            // Copy is the one edit that may leave the process: OSC 52 is opt-in.
            Action::Copy => {
                self.editor_mut().apply(Action::Copy);
                self.copy_out();
            }
            Action::NewBuffer | Action::NewTab => self.new_buffer(),
            Action::CloseBuffer => self.close_buffer(confirming_close),
            Action::SwitchBuffer(index) => self.switch_to(index),
            Action::OpenFind => self.overlay = Some(Overlay::Find(self.find.open())),
            Action::OpenReplace => self.overlay = Some(Overlay::Replace(Replace::new(&self.find))),
            Action::OpenPalette => {
                self.overlay = Some(Overlay::Palette(Palette::new(self.lang())));
            }
            Action::OpenSettings => self.overlay = Some(Overlay::Settings(Settings::new())),
            Action::OpenHelp => self.overlay = Some(Overlay::Help(Help::new())),
            Action::FindNext => self.repeat_find(Direction::Forward),
            Action::FindPrevious => self.repeat_find(Direction::Backward),
            // `M-B` outside a prompt repeats the search backwards; inside one it
            // toggles the direction (handled by the overlay).
            Action::ToggleBackwards => self.repeat_find(Direction::Backward),
            // `M-C` only means something in a prompt; `M-R` outside one is nano's
            // Replace binding, inside one it toggles regexp.
            Action::Cancel => self.editor_mut().clear_selection(),
            Action::ToggleCase => {}
            Action::ToggleRegexp => self.overlay = Some(Overlay::Replace(Replace::new(&self.find))),
            Action::ToggleSyntax => {
                self.syntax_enabled = !self.syntax_enabled;
                let state = if self.syntax_enabled {
                    "syntax highlighting on"
                } else {
                    "syntax highlighting off"
                };
                self.inform(t(self.lang(), state));
            }
            Action::ScrollView(delta) => self.scroll(delta),
            // Pages are the only actions that need viewport geometry.
            Action::PageUp => {
                let step = page_step(self.body_height);
                self.editor_mut().move_vertical(-step);
            }
            Action::PageDown => {
                let step = page_step(self.body_height);
                self.editor_mut().move_vertical(step);
            }
            other => self.editor_mut().apply(other),
        }
        self.schedule_recovery(Instant::now());
        self.redraw = true;
    }

    /// Keys while an overlay owns them. Text-editing actions go to the prompt; the
    /// rest are per-overlay.
    fn apply_overlay(&mut self, action: Action, confirming: bool) {
        if let Some(Overlay::Find(prompt)) = &mut self.overlay
            && prompt.apply(action.clone())
        {
            return;
        }
        if let Some(Overlay::Palette(palette)) = &mut self.overlay
            && palette.edit(action.clone())
        {
            return;
        }
        if let Some(Overlay::Replace(replace)) = &mut self.overlay
            && replace.edits(&action)
        {
            return;
        }
        if let Some(Overlay::Line(line)) = &mut self.overlay
            && line.edit(action.clone())
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
                    let buffers = self.buffer_names();
                    if let Some(row) = palette.selected_row(&buffers) {
                        match row {
                            crate::ui::overlays::palette::Row::Command(command) => {
                                self.apply(command.action.clone());
                            }
                            crate::ui::overlays::palette::Row::Buffer(index) => {
                                self.switch_to(index);
                            }
                        }
                    }
                }
                Action::MoveUp => {
                    palette.select_previous();
                    self.overlay = Some(Overlay::Palette(palette));
                }
                Action::MoveDown => {
                    let buffers = self.buffer_names();
                    palette.select_next(&buffers);
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
                        self.sync_editors();
                    }
                    self.overlay = Some(Overlay::Settings(settings));
                }
            },
            Overlay::Help(mut help) => {
                let page = self.help_viewport();
                let viewport = page;
                let keep = match action {
                    Action::Cancel | Action::Newline => false,
                    Action::MoveUp => {
                        help.scroll_by(-1, viewport);
                        true
                    }
                    Action::MoveDown => {
                        help.scroll_by(1, viewport);
                        true
                    }
                    Action::PageUp => {
                        help.scroll_by(-(page as isize), viewport);
                        true
                    }
                    Action::PageDown => {
                        help.scroll_by(page as isize, viewport);
                        true
                    }
                    Action::MoveFileStart => {
                        help.scroll_to(false, viewport);
                        true
                    }
                    Action::MoveFileEnd => {
                        help.scroll_to(true, viewport);
                        true
                    }
                    Action::Quit => {
                        self.request_quit(confirming);
                        false
                    }
                    _ => true,
                };
                if keep {
                    self.overlay = Some(Overlay::Help(help));
                }
            }
            Overlay::Replace(mut replace) => {
                let keep = match action {
                    Action::Cancel => false,
                    Action::Newline => self.replace_accept(&mut replace),
                    Action::ToggleCase => {
                        replace.case_sensitive = !replace.case_sensitive;
                        true
                    }
                    Action::ToggleRegexp => {
                        replace.regexp = !replace.regexp;
                        true
                    }
                    Action::Insert(ch) if replace.stage() == Stage::Confirm => match ch {
                        'y' => self.replace_current(&mut replace),
                        'n' => self.replace_skip(&mut replace),
                        'a' => self.replace_all(&mut replace),
                        'q' => false,
                        _ => true,
                    },
                    Action::Quit => {
                        self.request_quit(confirming);
                        false
                    }
                    _ => true,
                };
                if keep {
                    self.overlay = Some(Overlay::Replace(replace));
                }
            }
            Overlay::Line(mut line) => {
                let keep = match action {
                    Action::Cancel => false,
                    Action::Newline => self.accept_line(&line),
                    Action::InsertTab if line.kind.completes() => {
                        self.complete_path(&mut line);
                        true
                    }
                    Action::Quit => {
                        self.request_quit(confirming);
                        false
                    }
                    _ => true,
                };
                if keep {
                    self.overlay = Some(Overlay::Line(line));
                }
            }
        }
    }

    /// Replace flow, stage by stage. Each helper returns whether the overlay stays
    /// open; `self.overlay` is `None` while they run, so they can mutate the editor.
    fn replace_accept(&mut self, replace: &mut Replace) -> bool {
        match replace.stage() {
            Stage::Search => {
                let query = replace.search_text().to_string();
                if query.is_empty() {
                    self.inform(t(self.lang(), "empty search pattern"));
                    return true;
                }
                if let Err(err) = Search::compile(&query, replace.regexp, replace.case_sensitive) {
                    self.inform(tf(self.lang(), "invalid pattern: {}", &[&err.to_string()]));
                    return true;
                }
                self.find.query = query;
                self.find.case_sensitive = replace.case_sensitive;
                self.find.regexp = replace.regexp;
                replace.set_stage(Stage::Replacement);
                true
            }
            Stage::Replacement => {
                self.find.replace_with = replace.replacement_text().to_string();
                replace.set_stage(Stage::Confirm);
                let from = self.cursor_index();
                self.replace_seek(replace, from, false)
            }
            // Enter in the confirm stage means "yes, replace this one".
            Stage::Confirm => self.replace_current(replace),
        }
    }

    /// Find the next match at or after `from` and jump to it. `stop_on_wrap` ends
    /// the run when the search has come round the end of the buffer, so skipping or
    /// replacing the last match does not find it again forever.
    fn replace_seek(&mut self, replace: &mut Replace, from: usize, stop_on_wrap: bool) -> bool {
        let search = match Search::compile(&self.find.query, replace.regexp, replace.case_sensitive)
        {
            Ok(search) => search,
            Err(err) => {
                self.inform(tf(self.lang(), "invalid pattern: {}", &[&err.to_string()]));
                return false;
            }
        };
        match search.find(self.editor().buffer(), from, Direction::Forward) {
            Some(hit) if hit.wrapped && stop_on_wrap => {
                self.finish_replace(replace);
                false
            }
            Some(hit) => {
                replace.set_active(Some((hit.start, hit.end)));
                let (line, col) = self.editor().buffer().char_to_position(hit.start);
                self.editor_mut().set_cursor(line, col);
                true
            }
            None => {
                self.finish_replace(replace);
                false
            }
        }
    }

    /// End of a replace run: report the count, or that nothing matched.
    fn finish_replace(&mut self, replace: &Replace) {
        match replace.replaced() {
            0 => {
                let query = self.find.query.clone();
                self.inform(tf(self.lang(), "not found: {}", &[&query]));
            }
            count => self.inform(tf(self.lang(), "replaced {}", &[&count.to_string()])),
        }
    }

    /// Replace the active match, then look for the next one.
    fn replace_current(&mut self, replace: &mut Replace) -> bool {
        let Some((start, end)) = replace.active() else {
            let from = self.cursor_index();
            return self.replace_seek(replace, from, false);
        };
        let with = self.find.replace_with.clone();
        self.editor_mut().replace_range(start, end, &with);
        replace.note_replaced();
        let next = end + with.chars().count();
        self.replace_seek(replace, next, true)
    }

    /// Skip the active match.
    fn replace_skip(&mut self, replace: &mut Replace) -> bool {
        let from = replace
            .active()
            .map_or_else(|| self.cursor_index(), |(_, end)| end);
        replace.set_active(None);
        self.replace_seek(replace, from, true)
    }

    /// Replace every match in one pass, then close. The whole run is one undo step.
    fn replace_all(&mut self, replace: &mut Replace) -> bool {
        let with = self.find.replace_with.clone();
        let search = match Search::compile(&self.find.query, replace.regexp, replace.case_sensitive)
        {
            Ok(search) => search,
            Err(err) => {
                self.inform(tf(self.lang(), "invalid pattern: {}", &[&err.to_string()]));
                return false;
            }
        };
        let mut from = 0;
        let mut count = 0;
        for _ in 0..100_000 {
            let Some(hit) = search.find(self.editor().buffer(), from, Direction::Forward) else {
                break;
            };
            if hit.wrapped {
                break;
            }
            if self.editor_mut().replace_range(hit.start, hit.end, &with) {
                count += 1;
            }
            let width = with.chars().count();
            from = hit.start + width.max(usize::from(width == 0 && hit.end > hit.start));
            if from <= hit.start {
                from = hit.start + 1;
            }
        }
        self.editor_mut().merge_undo_groups(count);
        replace.set_active(None);
        let total = replace.replaced() + count;
        self.inform(tf(self.lang(), "replaced {}", &[&total.to_string()]));
        false
    }

    /// `^Q`: quit, or ask once when any buffer has unsaved changes.
    fn request_quit(&mut self, confirming: bool) {
        if self.any_dirty() && !confirming {
            self.confirm_quit = true;
            self.inform(t(
                self.lang(),
                "unsaved changes: ^Q again to quit, ^S to save",
            ));
        } else {
            self.quit = true;
        }
    }

    /// `^S`. Writes the active buffer in place when it has a path; with no path
    /// (`nnano -`, a fresh buffer, a save that was cancelled) it asks for one
    /// instead of refusing: save-as is the only way out, so it opens it.
    fn save(&mut self) {
        if self.document().path.is_none() {
            self.overlay = Some(Overlay::Line(Line::new(Kind::SaveAs)));
            return;
        }
        match self.write_active() {
            Ok(path) => {
                let path = path.display().to_string();
                self.inform(tf(self.lang(), "saved {}", &[&path]));
            }
            Err(err) => self.inform(tf(self.lang(), "save failed: {}", &[&err.to_string()])),
        }
    }

    /// `M-S`: write to a new path and adopt it. The file is written before the
    /// buffer is renamed: a failed save must not leave the editor pointing at a
    /// path it never wrote.
    fn save_as(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.inform(t(self.lang(), "no file name given"));
            return;
        }
        let path = match file::absolute(name) {
            Ok(path) => path,
            Err(err) => {
                self.inform(tf(
                    self.lang(),
                    "cannot resolve {}: {}",
                    &[name, &err.to_string()],
                ));
                return;
            }
        };
        if path.is_dir() {
            self.inform(tf(
                self.lang(),
                "{} is a directory",
                &[&path.display().to_string()],
            ));
            return;
        }
        match self.write_active_as(&path) {
            Ok(()) => {
                let path = path.display().to_string();
                self.inform(tf(self.lang(), "saved {}", &[&path]));
            }
            Err(err) => self.inform(tf(self.lang(), "save failed: {}", &[&err.to_string()])),
        }
    }

    /// Write the active buffer to the path it already has. The snapshot is dropped
    /// only after the bytes landed.
    fn write_active(&mut self) -> io::Result<PathBuf> {
        let Some(path) = self.document().path.clone() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "this buffer has no file name",
            ));
        };
        let encoding = self.document().encoding;
        file::write_in_place(&path, self.editor().buffer(), encoding)?;
        self.editor_mut().mark_saved();
        self.forget_recovery(&path);
        Ok(path)
    }

    /// Write to `path` and make it the buffer's file: name, grammar and undo
    /// boundary all follow, and the snapshot of the old name is dropped because
    /// the text is on disk now.
    fn write_active_as(&mut self, path: &Path) -> io::Result<()> {
        let encoding = self.document().encoding;
        file::write_in_place(path, self.editor().buffer(), encoding)?;
        let old = self.document().path.clone();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let grammar = self
            .syntax
            .select(&name)
            .map(|grammar| grammar.name.clone());
        let document = self.document_mut();
        document.path = Some(path.to_path_buf());
        document.file = name;
        document.grammar = grammar;
        document.editor.mark_saved();
        if let Some(old) = old.filter(|old| old != path) {
            self.forget_recovery(&old);
        }
        self.forget_recovery(path);
        Ok(())
    }

    /// `^O`: open `name` in a new buffer and make it active. A missing file opens
    /// an empty buffer with that name, exactly like the command line.
    fn open_path(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.inform(t(self.lang(), "no file name given"));
            return;
        }
        let path = match file::absolute(name) {
            Ok(path) => path,
            Err(err) => {
                self.inform(tf(
                    self.lang(),
                    "cannot resolve {}: {}",
                    &[name, &err.to_string()],
                ));
                return;
            }
        };
        let document = match std::fs::read(&path) {
            Ok(bytes) => match Document::from_bytes(name, Some(path.clone()), &bytes) {
                Ok(document) => document,
                Err(err) => {
                    self.inform(tf(
                        self.lang(),
                        "cannot open {}: {}",
                        &[&path.display().to_string(), &err.to_string()],
                    ));
                    return;
                }
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => Document {
                path: Some(path),
                ..Document::empty(name)
            },
            Err(err) => {
                self.inform(tf(
                    self.lang(),
                    "cannot open {}: {}",
                    &[&path.display().to_string(), &err.to_string()],
                ));
                return;
            }
        };
        self.docs.push(document);
        self.active = self.docs.len() - 1;
        self.sync_editors();
        self.resolve_grammar();
        self.restore_recovery_for(self.active);
    }

    /// `^R`: replace this buffer with the file as it is on disk. The whole text
    /// is replaced, so the undo history is dropped (its ranges name text that no
    /// longer exists) and a dirty buffer asks once. A missing file, or one that
    /// cannot be decoded, changes nothing.
    fn reload(&mut self, confirming: bool) {
        let Some(path) = self.document().path.clone() else {
            self.inform(t(self.lang(), "this buffer has no file name"));
            return;
        };
        if self.document().editor.is_dirty() && !confirming {
            self.confirm_reload = true;
            self.inform(t(
                self.lang(),
                "unsaved changes: ^R again to reload, ^S to save",
            ));
            return;
        }
        let name = path.display().to_string();
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) => {
                self.inform(tf(
                    self.lang(),
                    "cannot open {}: {}",
                    &[&name, &err.to_string()],
                ));
                return;
            }
        };
        let loaded = match encoding::decode(&bytes) {
            Ok(loaded) => loaded,
            Err(err) => {
                self.inform(tf(
                    self.lang(),
                    "cannot open {}: {}",
                    &[&name, &err.to_string()],
                ));
                return;
            }
        };
        // The bytes may have been rewritten by another program in another
        // encoding; the buffer follows the file, exactly like an open does.
        let document = self.document_mut();
        document.encoding = loaded.encoding;
        document.editor.load(&loaded.text);
        self.forget_recovery(&path);
        self.inform(tf(self.lang(), "reloaded {}", &[&name]));
    }

    /// `M-I`: read a file and insert its text at the cursor as one undo step. The
    /// buffer keeps its own name and encoding: this is an edit, not an open.
    fn insert_file(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.inform(t(self.lang(), "no file name given"));
            return;
        }
        let path = match file::absolute(name) {
            Ok(path) => path,
            Err(err) => {
                self.inform(tf(
                    self.lang(),
                    "cannot resolve {}: {}",
                    &[name, &err.to_string()],
                ));
                return;
            }
        };
        let shown = path.display().to_string();
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) => {
                self.inform(tf(
                    self.lang(),
                    "cannot open {}: {}",
                    &[&shown, &err.to_string()],
                ));
                return;
            }
        };
        let loaded = match encoding::decode(&bytes) {
            Ok(loaded) => loaded,
            Err(err) => {
                self.inform(tf(
                    self.lang(),
                    "cannot open {}: {}",
                    &[&shown, &err.to_string()],
                ));
                return;
            }
        };
        self.editor_mut().apply(Action::PasteText(loaded.text));
        self.inform(tf(self.lang(), "inserted {}", &[&shown]));
    }

    /// `^G` / `M-G`: `line[,column]`, one-based, or `+N`/`-N` relative to the
    /// current line. Anything else is reported and changes nothing.
    fn goto(&mut self, text: &str) {
        let raw = text.trim();
        if raw.is_empty() {
            self.inform(t(self.lang(), "no line given"));
            return;
        }
        let (line_text, column_text) = match raw.split_once([',', ':']) {
            Some((line, column)) => (line.trim(), Some(column.trim())),
            None => (raw, None),
        };
        let (line, column) = (self.editor().cursor().line(), self.editor().cursor().col());
        let Some(target) = parse_target(line_text, line + 1) else {
            self.inform(tf(self.lang(), "not a line: {}", &[line_text]));
            return;
        };
        let target = target
            .saturating_sub(1)
            .min(self.editor().buffer().line_count() - 1);
        let column = match column_text {
            None => 0,
            Some(text) => match parse_target(text, column + 1) {
                Some(column) => column.saturating_sub(1),
                None => {
                    self.inform(tf(self.lang(), "not a column: {}", &[text]));
                    return;
                }
            },
        };
        let column = column.min(self.editor().buffer().line_len(target));
        self.editor_mut().set_cursor(target, column);
    }

    /// `M-X`: run `sh -c <command>` and insert its stdout at the cursor as one
    /// undo step. The status bar reports the exit code and the first stderr line,
    /// so a failing command is visible without a second pane.
    fn execute(&mut self, text: &str) {
        let command = text.trim();
        if command.is_empty() {
            self.inform(t(self.lang(), "no command given"));
            return;
        }
        let output = match std::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
        {
            Ok(output) => output,
            Err(err) => {
                self.inform(tf(self.lang(), "cannot run: {}", &[&err.to_string()]));
                return;
            }
        };
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let inserted = !stdout.is_empty();
        if inserted {
            self.editor_mut().apply(Action::PasteText(stdout));
        }
        let status = output
            .status
            .code()
            .map_or_else(|| "signal".to_string(), |code| code.to_string());
        let stderr = String::from_utf8_lossy(&output.stderr);
        let first = stderr.lines().next().unwrap_or_default().to_string();
        let lang = self.lang();
        let report = match (inserted, first.is_empty()) {
            (true, true) => tf(lang, "exit {}: inserted output", &[&status]),
            (true, false) => tf(lang, "exit {}: inserted output; {}", &[&status, &first]),
            (false, true) => tf(lang, "exit {}: no output", &[&status]),
            (false, false) => tf(lang, "exit {}: {}", &[&status, &first]),
        };
        self.inform(report);
    }

    /// Accept a one-line prompt. Returns whether the overlay stays open; every
    /// outcome is reported on the status row, so a failure closes it.
    fn accept_line(&mut self, line: &Line) -> bool {
        match line.kind {
            Kind::SaveAs => self.save_as(line.text()),
            Kind::Open => self.open_path(line.text()),
            Kind::InsertFile => self.insert_file(line.text()),
            Kind::Goto => self.goto(line.text()),
            Kind::Execute => self.execute(line.text()),
        }
        false
    }

    /// Tab in Save as / Open: one candidate fills the line, several extend it to
    /// the longest common prefix. No candidate leaves it alone — a prompt never
    /// complains mid-typing.
    fn complete_path(&mut self, line: &mut Line) {
        let candidates = file::complete(line.text());
        match candidates.len() {
            0 => {}
            1 => line.set_text(&candidates[0]),
            _ => line.set_text(&file::common_prefix(&candidates)),
        }
    }

    /// The OSC 52 sequence for the current register, when the `osc52` option is on
    /// and there is something to send. Split out from the write so the decision is
    /// testable without a terminal.
    fn clipboard_sequence(&self) -> Option<(String, bool)> {
        if !self.config.bool(Opt::Osc52) {
            return None;
        }
        let text = self.editor().register();
        if text.is_empty() {
            return None;
        }
        Some(clipboard::sequence(text))
    }

    /// Send the freshly copied register to the terminal's clipboard when the
    /// `osc52` option is on. Truncation is reported, never silent.
    fn copy_out(&mut self) {
        let Some((sequence, truncated)) = self.clipboard_sequence() else {
            return;
        };
        if let Err(err) = clipboard::emit(&sequence) {
            self.inform(tf(self.lang(), "clipboard: {}", &[&err.to_string()]));
        } else if truncated {
            let limit = (clipboard::OSC52_LIMIT / 1024).to_string();
            self.inform(tf(self.lang(), "clipboard truncated to {} KiB", &[&limit]));
        }
    }

    /// Schedule (or cancel) the next snapshot after an action. Only a dirty buffer
    /// needs one, and every edit re-arms the 2 s debounce.
    fn schedule_recovery(&mut self, now: Instant) {
        if self.recovery_dir.is_none() {
            return;
        }
        self.recovery_due = if self.any_dirty() {
            Some(now + recovery::DEBOUNCE)
        } else {
            None
        };
    }

    /// Write the snapshot when it is due. Called from the idle path of the event
    /// loop, never from `draw`.
    fn flush_recovery(&mut self, now: Instant) {
        let Some(due) = self.recovery_due else {
            return;
        };
        if now < due {
            return;
        }
        self.recovery_due = None;
        self.write_recovery();
    }

    /// Snapshot every dirty buffer that has a path. One failure is reported once
    /// and never aborts the session.
    fn write_recovery(&mut self) {
        let Some(dir) = self.recovery_dir.clone() else {
            return;
        };
        let mut failure = None;
        for document in &self.docs {
            if !document.editor.is_dirty() {
                continue;
            }
            let Some(path) = &document.path else {
                continue;
            };
            let snapshot = recovery::Snapshot {
                path: path.clone(),
                encoding: document.encoding,
                text: document.editor.buffer().chunks().collect(),
            };
            if let Err(err) = recovery::write(&dir, &snapshot) {
                failure = Some(err);
            }
        }
        if let Some(err) = failure {
            self.inform(tf(self.lang(), "recovery: {}", &[&err.to_string()]));
        }
    }

    /// Adopt a snapshot for every open buffer at startup.
    fn restore_recovery(&mut self) {
        for index in 0..self.docs.len() {
            self.restore_recovery_for(index);
        }
    }

    /// Adopt the snapshot of one buffer when the file on disk does not already
    /// contain it. The buffer becomes dirty by definition: it is unsaved work.
    fn restore_recovery_for(&mut self, index: usize) {
        let Some(dir) = self.recovery_dir.clone() else {
            return;
        };
        let Some(path) = self.docs[index].path.clone() else {
            return;
        };
        let Some(snapshot) = recovery::load(&dir, &path) else {
            return;
        };
        if snapshot.text
            == self.docs[index]
                .editor
                .buffer()
                .chunks()
                .collect::<String>()
        {
            // The file already holds it: the snapshot is stale, not useful.
            recovery::remove(&dir, &path);
            return;
        }
        let tab_width = self.tab_width();
        let document = &mut self.docs[index];
        document.editor = Editor::from_text(&snapshot.text);
        document.editor.set_tab_width(tab_width);
        document.editor.set_dirty(true);
        document.encoding = snapshot.encoding;
        document.viewport = Viewport::default();
        let path = path.display().to_string();
        self.inform(tf(
            self.lang(),
            "recovered unsaved changes for {}; ^S saves them",
            &[&path],
        ));
    }

    /// Drop the snapshot for `path`: it is obsolete after a save or a close.
    fn forget_recovery(&self, path: &Path) {
        if let Some(dir) = &self.recovery_dir {
            recovery::remove(dir, path);
        }
    }

    /// A clean exit leaves no snapshots behind.
    fn forget_all_recovery(&self) {
        let Some(dir) = &self.recovery_dir else {
            return;
        };
        for document in &self.docs {
            if let Some(path) = &document.path {
                recovery::remove(dir, path);
            }
        }
    }

    /// `^N`/`^T`: a fresh empty buffer becomes the active one.
    fn new_buffer(&mut self) {
        self.docs.push(Document::empty(""));
        self.active = self.docs.len() - 1;
        self.sync_editors();
        self.document_mut().grammar = None;
    }

    /// `^W`: close the active buffer, asking once when it is dirty. Closing the
    /// last buffer leaves the editor.
    fn close_buffer(&mut self, confirming: bool) {
        if self.document().editor.is_dirty() && !confirming {
            self.confirm_close = true;
            self.inform(t(
                self.lang(),
                "unsaved changes: ^W again to close this buffer, ^S to save",
            ));
            return;
        }
        if let Some(path) = self.document().path.clone() {
            // The buffer is gone, so its snapshot has nothing left to rescue; the
            // close was confirmed, which is the same answer as "discard".
            self.forget_recovery(&path);
        }
        if self.docs.len() == 1 {
            self.quit = true;
            return;
        }
        self.docs.remove(self.active);
        self.active = self.active.min(self.docs.len() - 1);
        self.redraw = true;
    }

    fn switch_to(&mut self, index: usize) {
        if index < self.docs.len() {
            self.active = index;
            self.redraw = true;
        }
    }

    /// Wheel scrolling moves the window, not the cursor, so a mouse wheel never
    /// destroys a mark or edits anything.
    fn scroll(&mut self, delta: isize) {
        let last = self.editor().buffer().line_count().saturating_sub(1);
        let top = self.viewport().top_line.saturating_add_signed(delta);
        self.viewport_mut().top_line = top.min(last);
    }

    /// Click on the scrollbar: the clicked cell becomes the top of the view,
    /// clamped to the last page. It goes through `ScrollView`, so the mouse keeps
    /// the single-mutation-path rule and the cursor does not move.
    fn scroll_to_bar(&mut self, row: u16, height: u16) {
        let total = self.editor().buffer().line_count();
        let visible = usize::from(height);
        if visible == 0 || total <= visible {
            return;
        }
        let max_top = total - visible;
        let target = if visible <= 1 {
            0
        } else {
            usize::from(row) * max_top / (visible - 1)
        };
        let current = self.viewport().top_line;
        self.apply(Action::ScrollView(target as isize - current as isize));
    }

    /// The prompt was accepted: compile once and jump to the first hit.
    fn accept_find(&mut self, prompt: &Prompt) {
        self.find.query = prompt.text().to_string();
        if prompt.is_empty() {
            self.inform(t(self.lang(), "empty search pattern"));
            return;
        }
        let direction = self.find.direction();
        let search =
            match Search::compile(prompt.text(), self.find.regexp, self.find.case_sensitive) {
                Ok(search) => search,
                Err(err) => {
                    self.inform(tf(self.lang(), "invalid pattern: {}", &[&err.to_string()]));
                    return;
                }
            };
        let from = self.cursor_index();
        let hit = search.find(self.editor().buffer(), from, direction);
        self.jump_to(hit);
    }

    /// `M-W`/`M-B`: reuse the last pattern and step past the current match.
    fn repeat_find(&mut self, direction: Direction) {
        if self.find.query.is_empty() {
            self.inform(t(self.lang(), "no search yet: ^F"));
            return;
        }
        let search =
            match Search::compile(&self.find.query, self.find.regexp, self.find.case_sensitive) {
                Ok(search) => search,
                Err(err) => {
                    self.inform(tf(self.lang(), "invalid pattern: {}", &[&err.to_string()]));
                    return;
                }
            };
        let cursor = self.cursor_index();
        let from = match direction {
            Direction::Forward => cursor.saturating_add(1),
            Direction::Backward => cursor,
        };
        let hit = search.find(self.editor().buffer(), from, direction);
        self.jump_to(hit);
    }

    fn jump_to(&mut self, hit: Option<nnano_core::search::Hit>) {
        match hit {
            Some(hit) => {
                let (line, col) = self.editor().buffer().char_to_position(hit.start);
                self.editor_mut().set_cursor(line, col);
                if hit.wrapped {
                    self.inform(t(self.lang(), "search wrapped"));
                }
            }
            None => {
                let query = self.find.query.clone();
                self.inform(tf(self.lang(), "not found: {}", &[&query]));
            }
        }
    }

    fn cursor_index(&self) -> usize {
        self.editor().cursor().char_index(self.editor().buffer())
    }

    /// Push `tab_size` into every buffer after the settings overlay changed it.
    fn sync_editors(&mut self) {
        let width = self.tab_width();
        for document in &mut self.docs {
            document.editor.set_tab_width(width);
        }
        self.redraw = true;
    }

    /// Rows the help panel can show: its height is min(content + 3, body), minus
    /// the two border rows and the footer.
    fn help_viewport(&self) -> usize {
        usize::from(self.body_height.saturating_sub(3))
            .max(1)
            .min(crate::ui::overlays::help::content_len())
    }

    fn any_dirty(&self) -> bool {
        self.docs.iter().any(|document| document.editor.is_dirty())
    }

    fn inform(&mut self, message: impl Into<String>) {
        self.message = Some(message.into());
    }

    pub(crate) fn document(&self) -> &Document {
        &self.docs[self.active]
    }

    pub(crate) fn document_mut(&mut self) -> &mut Document {
        &mut self.docs[self.active]
    }

    pub(crate) fn editor(&self) -> &Editor {
        &self.document().editor
    }

    pub(crate) fn editor_mut(&mut self) -> &mut Editor {
        &mut self.document_mut().editor
    }

    pub(crate) fn viewport(&self) -> &Viewport {
        &self.document().viewport
    }

    pub(crate) fn viewport_mut(&mut self) -> &mut Viewport {
        &mut self.document_mut().viewport
    }

    pub(crate) fn config(&self) -> &Config {
        &self.config
    }

    pub(crate) fn selection(&self) -> Option<&Selection> {
        self.editor().selection()
    }

    pub(crate) fn overlay(&self) -> Option<&Overlay> {
        self.overlay.as_ref()
    }

    pub(crate) fn find_settings(&self) -> &FindSettings {
        &self.find
    }

    pub(crate) fn buffer_count(&self) -> usize {
        self.docs.len()
    }

    pub(crate) fn active_buffer(&self) -> usize {
        self.active
    }

    pub(crate) fn buffer_names(&self) -> Vec<String> {
        let lang = self.lang();
        self.docs
            .iter()
            .map(|document| display_name(&document.file, lang))
            .collect()
    }

    /// Encoding of the active buffer (tests and, later, a status indicator).
    #[cfg(test)]
    pub(crate) fn encoding(&self) -> Encoding {
        self.document().encoding
    }

    /// Display name of the active buffer; the translated sentinel when unnamed.
    pub(crate) fn file_name(&self) -> &str {
        let name = &self.document().file;
        if name.is_empty() {
            t(self.lang(), "[No Name]")
        } else {
            name
        }
    }

    /// Interface language of the session, from the `language` option.
    pub(crate) fn lang(&self) -> Lang {
        self.config.language()
    }

    /// `tab_size` from the registry; the renderer and every editor share it.
    pub(crate) fn tab_width(&self) -> usize {
        let width = self.config.int(Opt::TabSize);
        usize::try_from(width.max(1)).unwrap_or(DEFAULT_TAB_WIDTH)
    }

    /// Test seam: move the cursor and mark the frame dirty.
    #[cfg(test)]
    pub(crate) fn set_cursor(&mut self, line: usize, col: usize) {
        self.editor_mut().set_cursor(line, col);
        self.redraw = true;
    }

    /// Test seam: the one status-line message.
    #[cfg(test)]
    pub(crate) fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Left of the status bar: a message if there is one, otherwise identity,
    /// dirty marker, buffer position and the multiplexer notice. `^B`/`^A` are
    /// documented, never rebound: the multiplexer keeps its prefix.
    fn status_left(&self) -> String {
        if let Some(message) = &self.message {
            return format!(" {message}");
        }
        let lang = self.lang();
        let mut text = format!(" nnano {}", env!("CARGO_PKG_VERSION"));
        text.push_str("  ");
        text.push_str(t(lang, "^H: help"));
        if self.any_dirty() {
            text.push_str("  ");
            text.push_str(t(lang, "[Modified]"));
        }
        if self.docs.len() > 1 {
            text.push_str(&format!("  {}/{}", self.active + 1, self.docs.len()));
        }
        if self.caps.tmux {
            text.push_str("   ");
            text.push_str(t(lang, "^B: tmux prefix"));
        } else if self.caps.screen {
            text.push_str("   ");
            text.push_str(t(lang, "^A: screen prefix"));
        }
        text
    }

    /// Right of the status bar: `line:column/total  file`, one-based and visual,
    /// as nano prints the position plus the line count of the whole document.
    fn status_right(&self) -> String {
        let (line, col) = self
            .editor()
            .cursor()
            .display_position(self.editor().buffer(), self.tab_width());
        let total = self.editor().buffer().line_count();
        format!("{line}:{col}/{total}  {} ", self.file_name())
    }

    /// Mouse: click places the cursor, drag marks, wheel scrolls. Alt+drag is a
    /// rectangular mark. The buffer bar switches buffers; panels take row clicks.
    fn on_mouse(&mut self, event: MouseEvent) {
        if !self.config.bool(Opt::Mouse) {
            return;
        }
        if self.overlay.is_some() {
            self.on_overlay_mouse(event);
            return;
        }
        if let Some(tabs) = self.tabs_rect()
            && self.hits(event.column, event.row, tabs)
        {
            if matches!(event.kind, MouseEventKind::Down(MouseButton::Left))
                && let Some(index) = self.tab_at(event.column)
            {
                self.switch_to(index);
            }
            return;
        }
        let body = self.body_rect();
        let bar = self.scrollbar_rect();
        if matches!(event.kind, MouseEventKind::Down(MouseButton::Left))
            && bar.width > 0
            && self.hits(event.column, event.row, bar)
        {
            self.scroll_to_bar(event.row.saturating_sub(bar.y), bar.height);
            return;
        }
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let Some((line, col)) = self.hit_test(event.column, event.row, body) else {
                    return;
                };
                if event.modifiers.contains(KeyModifiers::ALT) && !self.editor().block_mode() {
                    self.apply(Action::ToggleBlockSelection);
                }
                self.apply(Action::ClickAt(line, col));
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some((line, col)) = self.hit_test(event.column, event.row, body) {
                    self.apply(Action::ExtendTo(line, col));
                }
            }
            MouseEventKind::ScrollDown => self.apply(Action::ScrollView(3)),
            MouseEventKind::ScrollUp => self.apply(Action::ScrollView(-3)),
            _ => {}
        }
    }

    fn on_overlay_mouse(&mut self, event: MouseEvent) {
        if !matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            return;
        }
        let body = self.body_rect();
        let Some(overlay) = self.overlay.take() else {
            return;
        };
        match overlay {
            Overlay::Settings(mut settings) => {
                if let Some(index) = ui::overlays::settings::row_at(body, event.row) {
                    settings.select(index);
                }
                self.overlay = Some(Overlay::Settings(settings));
            }
            Overlay::Palette(mut palette) => {
                let buffers = self.buffer_names();
                if let Some(index) =
                    ui::overlays::palette::row_at(body, event.row, &palette, &buffers)
                {
                    palette.select(index);
                    if let Some(row) = palette.selected_row(&buffers) {
                        match row {
                            crate::ui::overlays::palette::Row::Command(command) => {
                                self.apply(command.action.clone());
                                return;
                            }
                            crate::ui::overlays::palette::Row::Buffer(index) => {
                                self.switch_to(index);
                                return;
                            }
                        }
                    }
                }
                self.overlay = Some(Overlay::Palette(palette));
            }
            Overlay::Help(mut help) => {
                let page = self.help_viewport();
                match event.kind {
                    MouseEventKind::ScrollDown => help.scroll_by(3, page),
                    MouseEventKind::ScrollUp => help.scroll_by(-3, page),
                    _ => {}
                }
                self.overlay = Some(Overlay::Help(help));
            }
            other => self.overlay = Some(other),
        }
    }

    /// Screen cell -> `(line, char col)` inside the body, or nothing outside it.
    fn hit_test(&self, x: u16, y: u16, body: ratatui::layout::Rect) -> Option<(usize, usize)> {
        if !self.hits(x, y, body) {
            return None;
        }
        let buffer = self.editor().buffer();
        let line = self.viewport().top_line + usize::from(y - body.y);
        if line >= buffer.line_count() {
            return None;
        }
        let target = self.viewport().left_col + usize::from(x - body.x);
        let col = ui::text::column_to_char(buffer, line, target, self.tab_width());
        Some((line, col))
    }

    fn hits(&self, x: u16, y: u16, area: ratatui::layout::Rect) -> bool {
        x >= area.x
            && y >= area.y
            && x < area.x.saturating_add(area.width)
            && y < area.y.saturating_add(area.height)
    }

    /// Buffer under a click on the tab bar.
    fn tab_at(&self, x: u16) -> Option<usize> {
        let mut left = 1u16;
        for (index, name) in self.buffer_names().iter().enumerate() {
            let width = u16::try_from(name.chars().count()).unwrap_or(u16::MAX);
            if x >= left && x < left.saturating_add(width + 2) {
                return Some(index);
            }
            left = left.saturating_add(width + 3);
        }
        None
    }

    fn screen_rect(&self) -> ratatui::layout::Rect {
        ratatui::layout::Rect::new(0, 0, self.screen.0, self.screen.1)
    }

    fn body_rect(&self) -> ratatui::layout::Rect {
        let (_, body, _, _) = ui::areas(self.screen_rect(), self.buffer_count());
        body
    }

    /// The one-column scrollbar on the right edge; width zero when the screen is
    /// too narrow to spare a column.
    fn scrollbar_rect(&self) -> ratatui::layout::Rect {
        let (_, _, bar, _) = ui::areas(self.screen_rect(), self.buffer_count());
        bar
    }

    /// Cells in the text area: the wrap width `^J` uses. Zero before the first
    /// frame has been measured.
    fn body_width(&self) -> usize {
        usize::from(self.body_rect().width)
    }

    fn tabs_rect(&self) -> Option<ratatui::layout::Rect> {
        let (tabs, _, _, _) = ui::areas(self.screen_rect(), self.buffer_count());
        tabs
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// One target of `^G`: `12`, `+3` or `-2`, in the same one-based units as the
/// input. `current` is the one-based position the relative form counts from.
fn parse_target(text: &str, current: usize) -> Option<usize> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Some(rest) = text.strip_prefix('+') {
        return rest
            .trim()
            .parse::<usize>()
            .ok()
            .map(|delta| current.saturating_add(delta));
    }
    if let Some(rest) = text.strip_prefix('-') {
        return rest
            .trim()
            .parse::<usize>()
            .ok()
            .map(|delta| current.saturating_sub(delta));
    }
    text.parse().ok()
}

/// One page is the visible area minus the row the cursor already sits on.
fn page_step(body_height: u16) -> isize {
    isize::try_from(body_height.saturating_sub(1).max(1)).unwrap_or(isize::MAX)
}

/// Entry point used by the `nnano` binary: read args, then run with the merged
/// configuration the binary resolved from the environment.
pub fn run(file: Option<String>, config: Config, syntax: SyntaxSet) -> io::Result<()> {
    let mut app = App::open(file)?;
    app.apply_config(config);
    app.apply_syntax(syntax);
    // The recovery directory is a runtime concern: a test or a build without a
    // state directory must never touch the disk.
    app.recovery_dir = config::detect_state_dir().map(|dir| dir.join("recovery"));
    app.restore_recovery();
    app.run()
}
#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
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

    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn a_click_places_the_cursor_and_a_drag_marks() {
        let mut app = App::from_text("s.txt", "uno\ndos\n");
        app.screen = (80, 24);
        app.on_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 2, 1));
        assert_eq!(
            (app.editor().cursor().line(), app.editor().cursor().col()),
            (1, 2)
        );

        app.on_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 0, 0));
        app.on_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), 2, 0));
        let selection = app.selection().unwrap();
        assert_eq!(selection.text(app.editor().buffer()), "un");
    }

    #[test]
    fn clicking_the_scrollbar_scrolls_without_moving_the_cursor() {
        let text = (0..100).map(|n| format!("line {n}\n")).collect::<String>();
        let mut app = App::from_text("long.txt", &text);
        app.screen = (80, 24);
        let bar = app.scrollbar_rect();
        assert_eq!(bar.width, 1, "the bar owns the right edge");
        let before = (app.editor().cursor().line(), app.editor().cursor().col());
        // The last cell of the bar maps to the last page.
        app.on_mouse(mouse(
            MouseEventKind::Down(MouseButton::Left),
            bar.x,
            bar.y + bar.height - 1,
        ));
        assert!(app.viewport().top_line > 0);
        assert_eq!(
            (app.editor().cursor().line(), app.editor().cursor().col()),
            before,
            "the wheel and the bar scroll the view, not the cursor"
        );
    }

    #[test]
    fn a_click_outside_the_text_area_is_ignored() {
        let mut app = App::from_text("s.txt", "uno\n");
        app.screen = (20, 5);
        app.on_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 5, 4));
        assert_eq!(app.editor().cursor().col(), 0);
        app.on_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 30, 1));
        assert_eq!(app.editor().cursor().col(), 0);
    }

    #[test]
    fn alt_drag_marks_a_rectangle() {
        let mut app = App::from_text("s.txt", "abc\ndef\nghi\n");
        app.screen = (80, 24);
        app.on_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::ALT,
        });
        app.on_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), 2, 2));
        let selection = app.selection().unwrap();
        assert_eq!(selection.mode(), nnano_core::buffer::SelectionMode::Block);
        assert_eq!(selection.text(app.editor().buffer()), "ab\nde\ngh");
    }

    #[test]
    fn the_wheel_scrolls_without_moving_the_cursor() {
        let text = (0..100).map(|n| format!("line {n}\n")).collect::<String>();
        let mut app = App::from_text("long.txt", &text);
        app.screen = (40, 10);
        app.on_mouse(mouse(MouseEventKind::ScrollDown, 1, 1));
        assert_eq!(app.viewport().top_line, 3);
        assert_eq!(app.editor().cursor().line(), 0);
        app.on_mouse(mouse(MouseEventKind::ScrollUp, 1, 1));
        assert_eq!(app.viewport().top_line, 0);
    }

    #[test]
    fn the_mouse_option_turns_clicks_off() {
        let mut app = App::from_text("s.txt", "uno\n");
        app.apply_config(config(&[(Opt::Mouse, Value::Bool(false))]));
        app.screen = (80, 24);
        app.on_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 2, 0));
        assert_eq!(app.editor().cursor().col(), 0);
    }

    #[test]
    fn a_bracketed_paste_can_be_undone_in_one_step() {
        let mut app = App::from_text("s.txt", "");
        app.apply(Action::PasteText("uno\ndos".to_string()));
        assert_eq!(app.editor().buffer().line_count(), 2);
        app.apply(Action::Undo);
        assert!(app.editor().buffer().is_empty());
    }

    #[test]
    fn clicking_a_settings_row_moves_the_highlight() {
        let mut app = App::from_text("s.txt", "");
        app.screen = (80, 24);
        app.apply(Action::OpenSettings);
        // Find the screen row the renderer puts the Mouse row on, using the same
        // hit test the click goes through: the panel geometry is not hard-coded
        // here, so adding an option cannot silently desync this test.
        let target = nnano_core::config::OPTIONS
            .iter()
            .position(|row| row.id == Opt::Mouse)
            .unwrap();
        let body = app.body_rect();
        let y = (body.y..body.y + body.height)
            .find(|y| super::ui::overlays::settings::row_at(body, *y) == Some(target))
            .expect("the Mouse row is on screen");
        app.on_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 10, y));
        let Some(super::Overlay::Settings(settings)) = app.overlay() else {
            panic!("settings should still be open");
        };
        assert_eq!(settings.selected(), target);
    }

    #[test]
    fn shift_arrows_move_the_highlight_instead_of_marking_a_panel() {
        let mut app = App::from_text("s.txt", "uno\n");
        app.apply(Action::OpenSettings);
        app.apply(Action::SelectDown);
        let Some(super::Overlay::Settings(settings)) = app.overlay() else {
            panic!("settings should still be open");
        };
        assert_eq!(settings.selected(), 1, "a panel row moves, it has no mark");
        // The buffer was not touched: no mark leaked out of the overlay.
        assert!(!app.editor().has_selection());
    }

    #[test]
    fn caret_h_opens_the_shortcut_reference() {
        let mut app = App::from_text("s.txt", "uno\n");
        app.apply(Action::OpenHelp);
        assert!(matches!(app.overlay(), Some(super::Overlay::Help(_))));
        app.apply(Action::Cancel);
        assert!(app.overlay().is_none());
    }

    #[test]
    fn the_shortcut_reference_scrolls_and_clamps() {
        let mut app = App::from_text("s.txt", "");
        app.body_height = 24;
        let viewport = 24 - 3;
        app.apply(Action::OpenHelp);
        app.apply(Action::PageDown);
        let Some(super::Overlay::Help(help)) = app.overlay() else {
            panic!("help should stay open");
        };
        assert!(help.scroll() > 0);

        app.apply(Action::MoveFileEnd);
        let Some(super::Overlay::Help(help)) = app.overlay() else {
            panic!("help should stay open");
        };
        assert_eq!(
            help.scroll(),
            super::ui::overlays::help::content_len() - viewport
        );

        app.apply(Action::MoveFileStart);
        let Some(super::Overlay::Help(help)) = app.overlay() else {
            panic!("help should stay open");
        };
        assert_eq!(help.scroll(), 0);
        app.apply(Action::Cancel);
        assert!(app.overlay().is_none());
    }

    #[test]
    fn the_status_bar_advertises_the_help_key() {
        let app = App::from_text("s.txt", "");
        assert!(
            app.status_left().contains("^H: help"),
            "{}",
            app.status_left()
        );
    }

    #[test]
    fn clicking_a_palette_row_runs_the_command() {
        let mut app = App::from_text("s.txt", "");
        app.screen = (80, 24);
        app.apply(Action::OpenPalette);
        // Rows = commands + buffers; panel height = rows + 3, centered in the
        // 23-row body, with one border row and one filter row above the list.
        let rows = super::ui::overlays::palette::COMMANDS.len() + app.buffer_count();
        let height = u16::try_from(rows + 3).unwrap();
        let first_row = (23 - height) / 2 + 2;
        app.on_mouse(mouse(
            MouseEventKind::Down(MouseButton::Left),
            10,
            first_row,
        ));
        // The first command is Save file, and this buffer has no path: it opens
        // the save-as prompt instead of refusing.
        let Some(super::Overlay::Line(line)) = app.overlay() else {
            panic!("save on an unnamed buffer should ask for a path");
        };
        assert_eq!(line.kind, super::ui::overlays::line::Kind::SaveAs);
        assert!(app.message().is_none(), "a prompt needs no extra message");
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
        assert!(app.file_name().contains("missing"));
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
        assert_eq!(app.encoding(), Encoding::Latin1);
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
        // No path: rather than refusing, `^S` asks for one. The buffer stays
        // dirty until the write actually lands.
        assert!(app.editor().is_dirty());
        let Some(super::Overlay::Line(line)) = app.overlay() else {
            panic!("save-as should be open");
        };
        assert_eq!(line.kind, super::ui::overlays::line::Kind::SaveAs);
    }

    #[test]
    fn no_argument_opens_an_unnamed_buffer() {
        let app = App::open(None).unwrap();
        assert_eq!(app.file_name(), "[No Name]");
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
            Action::OpenReplace,
            Action::OpenPalette,
            Action::OpenSettings,
            Action::OpenHelp,
        ] {
            let mut app = App::from_text("s.txt", "");
            app.apply(open.clone());
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

    fn syntax_set() -> nnano_core::syntax::SyntaxSet {
        nnano_core::syntax::SyntaxSet::from_grammars(nnano_core::syntax::parse(
            "syntax \"rust\" \"\\\\.rs$\"\ncolor red \"fn\"\ncolor green \"[0-9]+\"\n",
        ))
    }

    #[test]
    fn the_grammar_follows_the_file_name() {
        let mut app = App::from_text("main.rs", "fn 42");
        app.apply_syntax(syntax_set());
        let spans = app.syntax_spans(0);
        assert_eq!(spans.len(), 2, "{spans:?}");

        let mut other = App::from_text("notes.txt", "fn 42");
        other.apply_syntax(syntax_set());
        assert!(other.syntax_spans(0).is_empty(), "no grammar for .txt");
    }

    #[test]
    fn m_y_turns_highlighting_off_for_the_session() {
        let mut app = App::from_text("main.rs", "fn 42");
        app.apply_syntax(syntax_set());
        assert!(!app.syntax_spans(0).is_empty());
        app.apply(Action::ToggleSyntax);
        assert!(app.syntax_spans(0).is_empty());
        assert!(app.message().unwrap().contains("off"));
        app.apply(Action::ToggleSyntax);
        assert!(!app.syntax_spans(0).is_empty());
    }

    #[test]
    fn the_syntax_option_sets_the_default() {
        let mut app = App::from_text("main.rs", "fn 42");
        app.apply_syntax(syntax_set());
        app.apply_config(config(&[(Opt::Syntax, Value::Bool(false))]));
        assert!(app.syntax_spans(0).is_empty());
    }

    fn open_replace(app: &mut App, search: &str, replacement: &str) {
        app.apply(Action::OpenReplace);
        for ch in search.chars() {
            app.apply(Action::Insert(ch));
        }
        app.apply(Action::Newline);
        for ch in replacement.chars() {
            app.apply(Action::Insert(ch));
        }
        app.apply(Action::Newline);
    }

    #[test]
    fn replace_replaces_one_match_and_then_skips() {
        let mut app = App::from_text("s.txt", "uno dos uno\n");
        open_replace(&mut app, "uno", "X");
        assert_eq!(app.message(), None);
        // The first match is in front of the cursor.
        app.apply(Action::Insert('y'));
        assert_eq!(app.editor().buffer().line_text(0), "X dos uno");
        app.apply(Action::Insert('n'));
        assert!(app.overlay().is_none(), "no more matches");
    }

    #[test]
    fn replace_all_is_one_undo_step() {
        let mut app = App::from_text("s.txt", "uno dos uno\n");
        open_replace(&mut app, "uno", "X");
        app.apply(Action::Insert('a'));
        assert!(app.overlay().is_none());
        assert_eq!(app.editor().buffer().line_text(0), "X dos X");
        assert!(app.message().unwrap().contains("replaced 2"));

        app.apply(Action::Undo);
        assert_eq!(app.editor().buffer().line_text(0), "uno dos uno");
    }

    #[test]
    fn replace_can_be_cancelled_at_each_stage() {
        let mut app = App::from_text("s.txt", "uno\n");
        app.apply(Action::OpenReplace);
        app.apply(Action::Cancel);
        assert!(app.overlay().is_none());

        open_replace(&mut app, "uno", "X");
        app.apply(Action::Insert('q'));
        assert!(app.overlay().is_none());
        assert_eq!(app.editor().buffer().line_text(0), "uno");
    }

    #[test]
    fn an_invalid_replacement_pattern_keeps_the_prompt_open() {
        let mut app = App::from_text("s.txt", "uno\n");
        app.apply(Action::OpenReplace);
        app.apply(Action::ToggleRegexp);
        app.apply(Action::Insert('a'));
        app.apply(Action::Insert('('));
        app.apply(Action::Newline);
        assert!(app.overlay().is_some());
        assert!(app.message().unwrap().contains("invalid pattern"));
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
        // Save file, Save as, Open file, Reload from disk, Insert file, then Find.
        for _ in 0..5 {
            app.apply(Action::MoveDown);
        }
        app.apply(Action::Newline);
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

    #[test]
    fn new_buffers_are_added_and_become_active() {
        let mut app = App::from_text("uno.txt", "uno\n");
        app.apply(Action::NewBuffer);
        assert_eq!(app.buffer_count(), 2);
        assert_eq!(app.active_buffer(), 1);
        assert_eq!(app.file_name(), "[No Name]");
        assert!(app.editor().buffer().is_empty());

        app.apply(Action::SwitchBuffer(0));
        assert_eq!(app.file_name(), "uno.txt");
        assert_eq!(app.editor().buffer().line_text(0), "uno");
    }

    #[test]
    fn each_buffer_has_its_own_undo_history_and_cursor() {
        let mut app = App::from_text("uno.txt", "");
        app.apply(Action::Insert('a'));
        app.apply(Action::NewBuffer);
        app.apply(Action::Insert('b'));
        app.apply(Action::Undo);
        assert!(
            app.editor().buffer().is_empty(),
            "second buffer undoes alone"
        );
        app.apply(Action::SwitchBuffer(0));
        assert_eq!(app.editor().buffer().line_text(0), "a");
    }

    #[test]
    fn closing_a_dirty_buffer_asks_once_and_then_closes() {
        let mut app = App::from_text("uno.txt", "uno\n");
        app.apply(Action::NewBuffer);
        app.apply(Action::Insert('x'));
        app.apply(Action::CloseBuffer);
        assert!(!app.quit, "closing was not confirmed");
        assert_eq!(app.buffer_count(), 2);
        assert!(app.message().unwrap().contains("^W again"));

        app.apply(Action::CloseBuffer);
        assert_eq!(app.buffer_count(), 1);
        assert_eq!(app.file_name(), "uno.txt");
    }

    #[test]
    fn closing_a_clean_buffer_does_not_ask() {
        let mut app = App::from_text("uno.txt", "uno\n");
        app.apply(Action::NewBuffer);
        app.apply(Action::CloseBuffer);
        assert_eq!(app.buffer_count(), 1);
    }

    #[test]
    fn closing_the_last_buffer_leaves_the_editor() {
        let mut app = App::from_text("uno.txt", "uno\n");
        app.apply(Action::CloseBuffer);
        assert!(app.quit);
    }

    #[test]
    fn closing_a_dirty_last_buffer_asks_and_then_quits() {
        let mut app = App::from_text("uno.txt", "uno\n");
        app.apply(Action::Insert('x'));
        app.apply(Action::CloseBuffer);
        assert!(!app.quit);
        app.apply(Action::CloseBuffer);
        assert!(app.quit);
    }

    #[test]
    fn switching_buffers_keeps_the_scroll_offset() {
        let text = (0..80).map(|n| format!("line {n}\n")).collect::<String>();
        let mut app = App::from_text("long.txt", &text);
        app.apply(Action::NewBuffer);
        app.apply(Action::SwitchBuffer(1));
        assert_eq!(app.viewport().top_line, 0);
        app.set_cursor(60, 0);
        app.apply(Action::PageDown);
        let top = app.viewport().top_line;
        app.apply(Action::SwitchBuffer(0));
        assert_eq!(app.viewport().top_line, 0);
        app.apply(Action::SwitchBuffer(1));
        assert_eq!(app.viewport().top_line, top);
    }

    #[test]
    fn the_status_bar_counts_the_buffers() {
        let mut app = App::from_text("uno.txt", "");
        app.apply(Action::NewBuffer);
        assert!(app.status_left().contains("2/2"));
        app.apply(Action::SwitchBuffer(0));
        assert!(app.status_left().contains("1/2"));
    }

    #[test]
    fn mouse_clicks_on_the_buffer_bar_switch_buffers() {
        let mut app = App::from_text("uno.txt", "");
        app.apply(Action::NewBuffer);
        app.screen = (80, 24);
        // " [No Name] " starts at column 1; the second segment follows.
        let names = app.buffer_names();
        let layout = super::ui::tab_layout(&names);
        let (left, _) = layout[0];
        app.on_mouse(mouse(MouseEventKind::Down(MouseButton::Left), left + 1, 0));
        assert_eq!(app.active_buffer(), 0);
    }

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nnano-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn save_as_writes_the_file_and_adopts_its_name() {
        let path = temp("save-as.txt");
        let _ = std::fs::remove_file(&path);
        let mut app = App::from_text("", "hola\n");
        app.apply(Action::Save);
        let Some(super::Overlay::Line(line)) = app.overlay() else {
            panic!("save with no path asks for one");
        };
        assert_eq!(line.kind, super::ui::overlays::line::Kind::SaveAs);

        for ch in path.display().to_string().chars() {
            app.apply(Action::Insert(ch));
        }
        app.apply(Action::Newline);
        assert!(app.overlay().is_none(), "accept closes the prompt");
        assert_eq!(std::fs::read(&path).unwrap(), b"hola\n");
        assert!(!app.editor().is_dirty());
        assert_eq!(app.file_name(), path.file_name().unwrap().to_string_lossy());
        assert!(app.message().unwrap().starts_with("saved"));

        // A second `^S` now writes in place, no prompt.
        app.apply(Action::Insert('!'));
        assert!(app.editor().is_dirty());
        app.apply(Action::Save);
        assert!(app.overlay().is_none());
        assert_eq!(std::fs::read(&path).unwrap(), b"!hola\n");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_as_refuses_a_directory_and_keeps_the_old_name() {
        let dir = scratch_dir("save-as-dir");
        let mut app = App::from_text("nota.txt", "hola\n");
        app.save_as(dir.to_str().unwrap());
        assert!(app.message().unwrap().contains("is a directory"));
        assert_eq!(app.file_name(), "nota.txt");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_as_rejects_an_empty_name() {
        let mut app = App::from_text("nota.txt", "");
        app.save_as("   ");
        assert!(app.message().unwrap().contains("no file name"));
        assert_eq!(app.file_name(), "nota.txt");
    }

    #[test]
    fn save_as_switches_the_grammar_with_the_name() {
        let dir = scratch_dir("grammar");
        let path = dir.join("programa.rs");
        let mut app = App::from_text("notes.txt", "fn x\n");
        app.apply_syntax(syntax_set());
        assert!(app.syntax_spans(0).is_empty(), "no rust grammar for .txt");

        app.save_as(&path.display().to_string());
        assert!(!app.syntax_spans(0).is_empty(), "the new name is a .rs");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_path_adds_a_buffer_and_activates_it() {
        let path = temp("opened.txt");
        std::fs::write(&path, "uno\ndos\n").unwrap();
        let mut app = App::from_text("s.txt", "");
        app.open_path(&path.display().to_string());
        assert_eq!(app.buffer_count(), 2);
        assert_eq!(app.active_buffer(), 1);
        assert_eq!(app.editor().buffer().line_text(1), "dos");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn open_path_of_a_missing_file_opens_an_empty_buffer_with_that_name() {
        let path = temp("no-such-file.txt");
        let _ = std::fs::remove_file(&path);
        let mut app = App::from_text("s.txt", "");
        app.open_path(&path.display().to_string());
        assert_eq!(app.buffer_count(), 2);
        assert!(app.editor().buffer().is_empty());
        assert!(app.file_name().contains("no-such-file"));
    }

    #[test]
    fn open_path_reports_a_directory_instead_of_panicking() {
        let dir = scratch_dir("open-dir");
        let mut app = App::from_text("s.txt", "");
        app.open_path(dir.to_str().unwrap());
        assert!(app.message().unwrap().contains("cannot open"));
        assert_eq!(app.buffer_count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_open_prompt_completes_a_path_with_tab() {
        let dir = scratch_dir("complete-prompt");
        std::fs::write(dir.join("informe.txt"), "").unwrap();
        let mut app = App::from_text("s.txt", "");
        app.apply(Action::OpenPath);
        for ch in format!("{}/inf", dir.display()).chars() {
            app.apply(Action::Insert(ch));
        }
        app.apply(Action::InsertTab);
        let Some(super::Overlay::Line(line)) = app.overlay() else {
            panic!("the open prompt should stay open");
        };
        assert!(line.text().ends_with("informe.txt"), "{}", line.text());

        // Accepting opens it.
        app.apply(Action::Newline);
        assert_eq!(app.buffer_count(), 2);
        assert!(app.file_name().contains("informe"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reload_replaces_the_buffer_and_drops_the_undo_history() {
        let path = temp("reload.txt");
        std::fs::write(&path, "uno\n").unwrap();
        let mut app = open_file(&path);
        // Another program rewrites the file behind our back.
        std::fs::write(&path, "dos\ntres\n").unwrap();
        app.apply(Action::Reload);
        assert_eq!(app.editor().buffer().line_text(0), "dos");
        assert_eq!(app.editor().buffer().line_text(1), "tres");
        assert!(!app.editor().is_dirty());
        assert!(app.message().unwrap().starts_with("reloaded"));

        // The old text is gone, so the old history cannot be undone into it.
        app.apply(Action::Undo);
        assert_eq!(app.editor().buffer().line_text(0), "dos");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reload_on_a_dirty_buffer_asks_once() {
        let path = temp("reload-dirty.txt");
        std::fs::write(&path, "en disco\n").unwrap();
        let mut app = open_file(&path);
        app.apply(Action::Insert('x'));
        assert!(app.editor().is_dirty());

        app.apply(Action::Reload);
        assert!(app.editor().is_dirty(), "the edit is kept");
        assert!(app.message().unwrap().contains("^R again"));
        assert_eq!(app.editor().buffer().line_text(0), "xen disco");

        app.apply(Action::Reload);
        assert!(!app.editor().is_dirty());
        assert_eq!(app.editor().buffer().line_text(0), "en disco");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reload_without_a_path_reports_it_and_changes_nothing() {
        let mut app = App::from_text("", "hola\n");
        app.apply(Action::Reload);
        assert!(app.message().unwrap().contains("no file name"));
        assert_eq!(app.editor().buffer().line_text(0), "hola");
    }

    #[test]
    fn reload_of_a_missing_file_keeps_the_buffer() {
        let path = temp("reload-gone.txt");
        std::fs::write(&path, "hola\n").unwrap();
        let mut app = open_file(&path);
        std::fs::remove_file(&path).unwrap();
        app.apply(Action::Reload);
        assert!(app.message().unwrap().contains("cannot open"));
        assert_eq!(app.editor().buffer().line_text(0), "hola");
    }

    #[test]
    fn insert_file_inserts_at_the_cursor_as_one_undo_step() {
        let dir = scratch_dir("insert-file");
        let source = dir.join("trozo.txt");
        std::fs::write(&source, "INSERTADO\n").unwrap();
        let mut app = App::from_text("nota.txt", "uno\ndos\n");
        app.editor_mut().set_cursor(1, 0);

        app.apply(Action::InsertFile);
        let Some(super::Overlay::Line(line)) = app.overlay() else {
            panic!("insert-file asks for a path");
        };
        assert_eq!(line.kind, super::ui::overlays::line::Kind::InsertFile);
        for ch in source.display().to_string().chars() {
            app.apply(Action::Insert(ch));
        }
        app.apply(Action::Newline);

        assert!(app.overlay().is_none());
        assert_eq!(app.editor().buffer().line_text(1), "INSERTADO");
        assert_eq!(app.editor().buffer().line_text(2), "dos");
        assert!(app.editor().is_dirty());
        assert!(app.message().unwrap().starts_with("inserted"));

        app.apply(Action::Undo);
        assert_eq!(app.editor().buffer().line_text(1), "dos");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn insert_file_of_a_missing_file_changes_nothing() {
        let path = temp("insert-gone.txt");
        let _ = std::fs::remove_file(&path);
        let mut app = App::from_text("nota.txt", "uno\n");
        app.insert_file(&path.display().to_string());
        assert!(app.message().unwrap().contains("cannot open"));
        assert_eq!(app.editor().buffer().line_text(0), "uno");
        assert!(!app.editor().is_dirty());
    }

    #[test]
    fn goto_accepts_a_line_a_column_and_a_relative_offset() {
        let text = (0..20).map(|n| format!("linea {n}\n")).collect::<String>();
        let mut app = App::from_text("long.txt", &text);
        app.goto("5");
        assert_eq!(app.editor().cursor().line(), 4);
        app.goto("7,3");
        assert_eq!(
            (app.editor().cursor().line(), app.editor().cursor().col()),
            (6, 2)
        );
        app.goto("+2");
        assert_eq!(app.editor().cursor().line(), 8);
        app.goto("-1");
        assert_eq!(app.editor().cursor().line(), 7);
    }

    #[test]
    fn goto_clamps_and_rejects_garbage() {
        let mut app = App::from_text("x.txt", "uno\ndos\n");
        app.goto("9999");
        assert_eq!(app.editor().cursor().line(), 2, "clamped to the last line");
        app.goto("1,999");
        assert_eq!(
            app.editor().cursor().col(),
            app.editor().buffer().line_len(0)
        );
        app.goto("dos");
        assert!(app.message().unwrap().contains("not a line"));
        app.goto("1,dos");
        assert!(app.message().unwrap().contains("not a column"));
        app.goto("");
        assert!(app.message().unwrap().contains("no line"));
    }

    #[test]
    fn justify_wraps_to_the_body_width() {
        let mut app = App::from_text("s.txt", "uno dos tres cuatro cinco\n");
        app.screen = (14, 10);
        app.apply(Action::Justify);
        assert_eq!(app.editor().buffer().line_text(0), "uno dos tres");
        assert_eq!(app.editor().buffer().line_text(1), "cuatro cinco");
    }

    #[test]
    fn execute_inserts_the_command_output_as_one_undo_step() {
        let mut app = App::from_text("s.txt", "");
        app.execute("printf 'hola\\nchau'");
        assert_eq!(app.editor().buffer().line_text(0), "hola");
        assert!(app.editor().is_dirty());
        app.apply(Action::Undo);
        assert!(app.editor().buffer().is_empty());
    }

    #[test]
    fn execute_reports_a_failing_command_without_touching_the_buffer() {
        let mut app = App::from_text("s.txt", "x\n");
        app.execute("echo boom >&2; exit 3");
        assert_eq!(app.editor().buffer().line_text(0), "x");
        let message = app.message().unwrap();
        assert!(message.contains("exit 3"), "{message}");
        assert!(message.contains("boom"), "{message}");
    }

    #[test]
    fn execute_with_no_output_says_so() {
        let mut app = App::from_text("s.txt", "");
        app.execute("true");
        assert!(app.message().unwrap().contains("no output"));
        app.execute("   ");
        assert!(app.message().unwrap().contains("no command"));
    }

    #[test]
    fn an_overlay_edit_still_arms_the_snapshot_clock() {
        // Replace-all happens inside an overlay; it edits the buffer, so the
        // recovery clock must be re-armed on that path too.
        let dir = scratch_dir("recovery-overlay");
        let mut app = App::from_text("s.txt", "uno dos uno\n");
        app.recovery_dir = Some(dir.clone());
        open_replace(&mut app, "uno", "X");
        app.apply(Action::Insert('a'));
        assert!(app.overlay().is_none(), "replace-all closes");
        assert!(app.editor().is_dirty());
        assert!(
            app.recovery_due.is_some(),
            "an overlay edit must schedule a snapshot"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_dirty_buffer_is_snapshotted_after_the_debounce() {
        let dir = scratch_dir("recovery-due");
        let path = temp("recovered.txt");
        std::fs::write(&path, "viejo\n").unwrap();
        let mut app = open_file(&path);
        app.recovery_dir = Some(dir.clone());
        app.apply(Action::Insert('x'));
        assert!(app.recovery_due.is_some(), "an edit arms the debounce");

        // Not yet due: nothing on disk.
        app.flush_recovery(std::time::Instant::now());
        assert!(nnano_core::recovery::load(&dir, &path).is_none());

        // After the debounce the snapshot holds the unsaved text.
        app.flush_recovery(std::time::Instant::now() + super::recovery::DEBOUNCE);
        let snapshot = nnano_core::recovery::load(&dir, &path).unwrap();
        assert!(snapshot.text.starts_with('x'), "{:?}", snapshot.text);
        assert_eq!(snapshot.path, path);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_clean_buffer_writes_no_snapshot_and_saving_removes_it() {
        let dir = scratch_dir("recovery-clean");
        let path = temp("clean.txt");
        std::fs::write(&path, "uno\n").unwrap();
        let mut app = open_file(&path);
        app.recovery_dir = Some(dir.clone());

        app.flush_recovery(std::time::Instant::now() + super::recovery::DEBOUNCE);
        assert!(
            nnano_core::recovery::load(&dir, &path).is_none(),
            "a clean buffer has nothing to rescue"
        );

        app.apply(Action::Insert('!'));
        app.flush_recovery(std::time::Instant::now() + super::recovery::DEBOUNCE);
        assert!(nnano_core::recovery::load(&dir, &path).is_some());

        app.apply(Action::Save);
        assert!(
            nnano_core::recovery::load(&dir, &path).is_none(),
            "a successful save drops the snapshot"
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"!uno\n");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_clean_exit_removes_every_snapshot() {
        let dir = scratch_dir("recovery-exit");
        let path = temp("exit.txt");
        std::fs::write(&path, "uno\n").unwrap();
        let mut app = open_file(&path);
        app.recovery_dir = Some(dir.clone());
        app.apply(Action::Insert('x'));
        app.flush_recovery(std::time::Instant::now() + super::recovery::DEBOUNCE);
        assert!(nnano_core::recovery::load(&dir, &path).is_some());

        app.forget_all_recovery();
        assert!(nnano_core::recovery::load(&dir, &path).is_none());
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_snapshot_is_restored_dirty_at_startup() {
        let dir = scratch_dir("recovery-restore");
        let path = temp("restore.txt");
        std::fs::write(&path, "viejo\n").unwrap();
        let mut app = open_file(&path);
        app.recovery_dir = Some(dir.clone());
        nnano_core::recovery::write(
            &dir,
            &nnano_core::recovery::Snapshot {
                path: app.document().path.clone().unwrap(),
                encoding: Encoding::Utf8,
                text: "nuevo\nsin guardar\n".to_string(),
            },
        )
        .unwrap();

        app.restore_recovery();
        assert_eq!(app.editor().buffer().line_text(0), "nuevo");
        assert!(app.editor().is_dirty(), "recovered text is unsaved");
        assert!(app.message().unwrap().contains("recovered"));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_snapshot_that_matches_the_file_is_dropped() {
        let dir = scratch_dir("recovery-stale");
        let path = temp("stale.txt");
        std::fs::write(&path, "igual\n").unwrap();
        let mut app = open_file(&path);
        app.recovery_dir = Some(dir.clone());
        let target = app.document().path.clone().unwrap();
        nnano_core::recovery::write(
            &dir,
            &nnano_core::recovery::Snapshot {
                path: target.clone(),
                encoding: Encoding::Utf8,
                text: "igual\n".to_string(),
            },
        )
        .unwrap();

        app.restore_recovery();
        assert!(!app.editor().is_dirty(), "nothing was recovered");
        assert!(
            nnano_core::recovery::load(&dir, &target).is_none(),
            "the stale snapshot is removed"
        );
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn copying_out_is_off_until_the_option_is_on() {
        let mut app = App::from_text("s.txt", "uno\n");
        app.apply(Action::Copy);
        assert!(
            app.clipboard_sequence().is_none(),
            "OSC 52 is opt-in, and off by default"
        );

        app.apply_config(config(&[(Opt::Osc52, Value::Bool(true))]));
        app.apply(Action::Copy);
        let (sequence, truncated) = app.clipboard_sequence().unwrap();
        assert!(sequence.starts_with("\x1b]52;c;"), "{sequence:?}");
        assert!(!truncated);

        let mut empty = App::from_text("s.txt", "");
        empty.apply_config(config(&[(Opt::Osc52, Value::Bool(true))]));
        empty.apply(Action::Copy);
        assert!(empty.clipboard_sequence().is_none(), "nothing to send");
    }
}
