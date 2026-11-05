//! Scroll offset of the text area: the first line and first visual column shown.
//!
//! Pure state plus one rule — keep the cursor on screen — so it is testable
//! without a terminal and cannot drift from the renderer.

/// Top-left corner of the visible window, in lines and visual columns.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Viewport {
    pub top_line: usize,
    pub left_col: usize,
}

impl Viewport {
    /// Move the window the minimum needed so `(line, col)` is inside a
    /// `width` x `height` area.
    pub fn ensure_visible(&mut self, line: usize, col: usize, width: u16, height: u16) {
        let width = usize::from(width);
        let height = usize::from(height);
        if height > 0 {
            if line < self.top_line {
                self.top_line = line;
            } else if line >= self.top_line + height {
                self.top_line = line + 1 - height;
            }
        }
        if width > 0 {
            if col < self.left_col {
                self.left_col = col;
            } else if col >= self.left_col + width {
                self.left_col = col + 1 - width;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Viewport;

    #[test]
    fn does_not_move_when_the_cursor_is_inside() {
        let mut viewport = Viewport::default();
        viewport.ensure_visible(5, 5, 20, 10);
        assert_eq!(
            viewport,
            Viewport {
                top_line: 0,
                left_col: 0
            }
        );
    }

    #[test]
    fn scrolls_down_one_line_at_a_time() {
        let mut viewport = Viewport::default();
        viewport.ensure_visible(10, 0, 20, 10);
        assert_eq!(viewport.top_line, 1);
        viewport.ensure_visible(11, 0, 20, 10);
        assert_eq!(viewport.top_line, 2);
    }

    #[test]
    fn scrolls_back_up() {
        let mut viewport = Viewport {
            top_line: 40,
            left_col: 0,
        };
        viewport.ensure_visible(39, 0, 20, 10);
        assert_eq!(viewport.top_line, 39);
    }

    #[test]
    fn scrolls_right_and_back_left() {
        let mut viewport = Viewport::default();
        viewport.ensure_visible(0, 20, 20, 10);
        assert_eq!(viewport.left_col, 1);
        viewport.ensure_visible(0, 80, 20, 10);
        assert_eq!(viewport.left_col, 61);
        viewport.ensure_visible(0, 0, 20, 10);
        assert_eq!(viewport.left_col, 0);
    }

    #[test]
    fn a_zero_sized_area_changes_nothing() {
        let mut viewport = Viewport::default();
        viewport.ensure_visible(100, 100, 0, 0);
        assert_eq!(viewport, Viewport::default());
    }
}
