//! Display width and tab stops, shared by the cursor and the renderer.
//!
//! One accumulator decides both "which column is the cursor on" and "how wide is
//! this text", so a cursor can never drift from the glyphs drawn under it. Only
//! the visible window is ever measured; nothing here scans a whole buffer.

use unicode_width::UnicodeWidthChar;

/// Width of a tab stop. Becomes the `tab_size` registry option; fixed until the
/// option registry exists.
pub const DEFAULT_TAB_WIDTH: usize = 8;

/// Column of the next tab stop strictly after `col`.
pub fn next_tab_stop(col: usize, tab_width: usize) -> usize {
    let tab_width = tab_width.max(1);
    col + tab_width - col % tab_width
}

/// Running display width of a char sequence, with tab stops and emoji sequences
/// treated as one to three cells (never one per component).
#[derive(Debug, Clone)]
pub struct Width {
    col: usize,
    prev: Option<char>,
    tab_width: usize,
}

impl Width {
    pub fn new(start_col: usize, tab_width: usize) -> Self {
        Self {
            col: start_col,
            prev: None,
            tab_width: tab_width.max(1),
        }
    }

    /// Column reached so far, counting from `start_col`.
    pub fn column(&self) -> usize {
        self.col
    }

    /// Add one char and return the cells it occupies. Zero-width chars (ZWJ,
    /// variation selectors, combining marks) add nothing but still update the
    /// sequence context.
    pub fn push(&mut self, ch: char) -> usize {
        let cells = match ch {
            '\t' => next_tab_stop(self.col, self.tab_width) - self.col,
            _ if joins_previous(self.prev, ch) => 0,
            _ => ch.width().unwrap_or(0),
        };
        self.col += cells;
        self.prev = Some(ch);
        cells
    }
}

/// True when `ch` must not add cells because it continues the previous grapheme:
/// a base after a ZWJ, an emoji skin-tone modifier, or a combining mark.
fn joins_previous(prev: Option<char>, ch: char) -> bool {
    let after_joiner = matches!(prev, Some('\u{200D}'));
    let modifier = matches!(ch, '\u{FE0E}' | '\u{FE0F}' | '\u{1F3FB}'..='\u{1F3FF}');
    after_joiner || modifier
}

/// Display column after `chars`, starting at column 0.
pub fn line_visual_column(chars: impl IntoIterator<Item = char>, tab_width: usize) -> usize {
    let mut width = Width::new(0, tab_width);
    for ch in chars {
        width.push(ch);
    }
    width.column()
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_TAB_WIDTH, Width, line_visual_column, next_tab_stop};

    #[test]
    fn tabs_snap_to_the_next_stop() {
        assert_eq!(next_tab_stop(0, 8), 8);
        assert_eq!(next_tab_stop(3, 8), 8);
        assert_eq!(next_tab_stop(8, 8), 16);
        assert_eq!(next_tab_stop(3, 4), 4);
        assert_eq!(line_visual_column("a\tb".chars(), DEFAULT_TAB_WIDTH), 9);
    }

    #[test]
    fn tabs_use_the_column_they_start_at() {
        assert_eq!(line_visual_column("\ta".chars(), 8), 9);
        assert_eq!(line_visual_column("ab\tc".chars(), 4), 5);
        // A tab that starts exactly on a stop still moves one full tab.
        assert_eq!(next_tab_stop(4, 4), 8);
    }

    #[test]
    fn cjk_is_two_cells() {
        assert_eq!(line_visual_column("漢字".chars(), 8), 4);
        assert_eq!(line_visual_column("a漢b".chars(), 8), 4);
    }

    #[test]
    fn combining_marks_add_nothing() {
        assert_eq!(line_visual_column("e\u{301}".chars(), 8), 1);
    }

    #[test]
    fn emoji_sequences_are_one_glyph_wide() {
        // Family emoji: four people joined with ZWJ, two cells total.
        assert_eq!(
            line_visual_column("👨\u{200D}👩\u{200D}👧\u{200D}👦".chars(), 8),
            2
        );
        // Skin-tone modifier on a base: still one glyph.
        assert_eq!(line_visual_column("👍\u{1F3FD}".chars(), 8), 2);
        // Variation selector does not widen the base.
        assert_eq!(line_visual_column("❤\u{FE0F}".chars(), 8), 1);
    }

    #[test]
    fn accumulator_reports_each_step() {
        let mut width = Width::new(0, 4);
        assert_eq!(width.push('a'), 1);
        assert_eq!(width.push('\t'), 3);
        assert_eq!(width.column(), 4);
        assert_eq!(width.push('漢'), 2);
        assert_eq!(width.column(), 6);
    }
}
