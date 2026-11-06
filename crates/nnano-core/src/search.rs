//! nano's search semantics (phase 5).
//!
//! - literal by default; POSIX-style ERE only while the *regexp* toggle is on,
//!   compiled once per accepted prompt by `regex-lite`;
//! - case-insensitive unless the toggle (or the `case_sensitive` option) says
//!   otherwise;
//! - evaluated **per line and stateless**, exactly like nano: a pattern never
//!   spans a line break, so there is no multi-line state to invalidate;
//! - runs when the prompt is accepted, never per keystroke — no debounce, no
//!   worker thread;
//! - wraps around, and reports that it did ([`Hit::wrapped`]).
//!
//! Offsets are global char indices, the same currency as `Cursor::char_index`,
//! so a hit can be turned into a cursor without touching bytes.

use std::fmt;

use regex_lite::{Regex, RegexBuilder};

use crate::buffer::TextBuffer;

/// Which way to walk the file. The prompt's `M-B` toggle and `M-W`/`M-B` both
/// come down to this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

/// One accepted match: a char range plus whether the search had to wrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    pub start: usize,
    pub end: usize,
    pub wrapped: bool,
}

/// An unusable pattern: empty, or an ERE the engine refuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchError {
    pub pattern: String,
    pub message: String,
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SearchError {}

/// A compiled query. Building it once per accepted prompt is the whole caching
/// story; there is nothing per keystroke.
#[derive(Debug)]
pub struct Search {
    engine: Engine,
}

#[derive(Debug)]
enum Engine {
    Literal {
        needle: Vec<char>,
        case_sensitive: bool,
    },
    Pattern(Box<Regex>),
}

impl Search {
    pub fn compile(needle: &str, regexp: bool, case_sensitive: bool) -> Result<Self, SearchError> {
        if needle.is_empty() {
            return Err(SearchError {
                pattern: String::new(),
                message: "empty search pattern".to_string(),
            });
        }
        if regexp {
            let regex = RegexBuilder::new(needle)
                .case_insensitive(!case_sensitive)
                .build()
                .map_err(|err| SearchError {
                    pattern: needle.to_string(),
                    message: format!("{err}"),
                })?;
            Ok(Self {
                engine: Engine::Pattern(Box::new(regex)),
            })
        } else {
            Ok(Self {
                engine: Engine::Literal {
                    needle: needle.chars().collect(),
                    case_sensitive,
                },
            })
        }
    }

    /// First match in `direction` starting at char index `from`, wrapping once.
    ///
    /// Forward includes a match that begins exactly at `from`; backward excludes
    /// it, so pressing "find backwards" twice walks back instead of standing on
    /// the same hit.
    pub fn find(&self, buffer: &TextBuffer, from: usize, direction: Direction) -> Option<Hit> {
        let from = from.min(buffer.len_chars());
        let (line, col) = buffer.char_to_position(from);
        let last = buffer.line_count() - 1;

        let probe = |line: usize, lo: usize, hi: usize, wrapped: bool| -> Option<Hit> {
            let (start, end) = self.line_hit(buffer, line, lo, hi, direction)?;
            let base = buffer.line_start(line);
            Some(Hit {
                start: base + start,
                end: base + end,
                wrapped,
            })
        };

        match direction {
            Direction::Forward => {
                if let Some(hit) = probe(line, col, usize::MAX, false) {
                    return Some(hit);
                }
                for line in line + 1..=last {
                    if let Some(hit) = probe(line, 0, usize::MAX, false) {
                        return Some(hit);
                    }
                }
                for line in 0..line {
                    if let Some(hit) = probe(line, 0, usize::MAX, true) {
                        return Some(hit);
                    }
                }
                if col > 0 {
                    probe(line, 0, col, true)
                } else {
                    None
                }
            }
            Direction::Backward => {
                if let Some(hit) = probe(line, 0, col, false) {
                    return Some(hit);
                }
                for line in (0..line).rev() {
                    if let Some(hit) = probe(line, 0, usize::MAX, false) {
                        return Some(hit);
                    }
                }
                for line in (line + 1..=last).rev() {
                    if let Some(hit) = probe(line, 0, usize::MAX, true) {
                        return Some(hit);
                    }
                }
                probe(line, col, usize::MAX, true)
            }
        }
    }

    /// Best match inside one line, with the start constrained to `lo <= start <
    /// hi` (char offsets relative to the line).
    fn line_hit(
        &self,
        buffer: &TextBuffer,
        line: usize,
        lo: usize,
        hi: usize,
        direction: Direction,
    ) -> Option<(usize, usize)> {
        match &self.engine {
            Engine::Literal {
                needle,
                case_sensitive,
            } => {
                let chars: Vec<char> = buffer.line_chars(line).collect();
                literal_hit(&chars, needle, *case_sensitive, lo, hi, direction)
            }
            Engine::Pattern(regex) => {
                let text = buffer.line_text(line);
                pattern_hit(regex, &text, lo, hi, direction)
            }
        }
    }
}

fn literal_hit(
    chars: &[char],
    needle: &[char],
    case_sensitive: bool,
    lo: usize,
    hi: usize,
    direction: Direction,
) -> Option<(usize, usize)> {
    let count = needle.len();
    if count == 0 || chars.len() < count {
        return None;
    }
    // Last start where the needle still fits, exclusive bound.
    let upper = hi.min(chars.len() - count + 1);
    let lower = lo.min(upper);
    let matches = |start: usize| {
        needle
            .iter()
            .enumerate()
            .all(|(offset, expected)| chars_eq(chars[start + offset], *expected, case_sensitive))
    };
    let start = match direction {
        Direction::Forward => (lower..upper).find(|&start| matches(start)),
        Direction::Backward => (lower..upper).rev().find(|&start| matches(start)),
    }?;
    Some((start, start + count))
}

fn chars_eq(left: char, right: char, case_sensitive: bool) -> bool {
    left == right || (!case_sensitive && left.to_lowercase().eq(right.to_lowercase()))
}

fn pattern_hit(
    regex: &Regex,
    text: &str,
    lo: usize,
    hi: usize,
    direction: Direction,
) -> Option<(usize, usize)> {
    // Byte offsets from the engine, char offsets for everything else, converted
    // in one linear walk over the line.
    let mut scanned_byte = 0;
    let mut scanned_char = 0;
    let mut last: Option<(usize, usize)> = None;

    for found in regex.find_iter(text) {
        let prefix = text
            .get(scanned_byte..found.start())
            .map_or("", |slice| slice);
        scanned_char += prefix.chars().count();
        let start = scanned_char;
        let matched = text
            .get(found.start()..found.end())
            .map_or("", |slice| slice);
        let end = start + matched.chars().count();
        scanned_byte = found.end();

        if start < lo || start >= hi {
            continue;
        }
        match direction {
            Direction::Forward => return Some((start, end)),
            Direction::Backward => last = Some((start, end)),
        }
    }

    last
}

#[cfg(test)]
mod tests {
    use super::{Direction, Search};
    use crate::buffer::TextBuffer;

    fn find(
        buffer: &TextBuffer,
        from: usize,
        needle: &str,
        backwards: bool,
    ) -> Option<(usize, usize, bool)> {
        let search = Search::compile(needle, false, false).unwrap();
        let direction = if backwards {
            Direction::Backward
        } else {
            Direction::Forward
        };
        let hit = search.find(buffer, from, direction)?;
        Some((hit.start, hit.end, hit.wrapped))
    }

    #[test]
    fn finds_forwards_from_the_cursor() {
        let buffer = TextBuffer::from_text("uno dos tres\n");
        assert_eq!(find(&buffer, 0, "dos", false), Some((4, 7, false)));
        // A match that starts before the cursor is only reachable by wrapping.
        assert_eq!(find(&buffer, 5, "dos", false), Some((4, 7, true)));
        assert_eq!(find(&buffer, 8, "tres", false), Some((8, 12, false)));
    }

    #[test]
    fn wraps_around_once_and_says_so() {
        let buffer = TextBuffer::from_text("uno dos\n");
        assert_eq!(find(&buffer, 5, "uno", false), Some((0, 3, true)));
    }

    #[test]
    fn backward_finds_the_previous_match() {
        let buffer = TextBuffer::from_text("uno dos uno\n");
        assert_eq!(find(&buffer, 8, "uno", true), Some((0, 3, false)));
        // From the second match's start, the same one is skipped.
        assert_eq!(find(&buffer, 8, "uno", true), Some((0, 3, false)));
    }

    #[test]
    fn backward_wraps_from_the_top_of_the_file() {
        let buffer = TextBuffer::from_text("uno dos\n");
        assert_eq!(find(&buffer, 1, "dos", true), Some((4, 7, true)));
    }

    #[test]
    fn is_case_insensitive_by_default() {
        let buffer = TextBuffer::from_text("Hola Mundo\n");
        assert_eq!(find(&buffer, 0, "mundo", false), Some((5, 10, false)));
    }

    #[test]
    fn case_sensitive_when_asked() {
        let buffer = TextBuffer::from_text("Hola Mundo\n");
        let search = Search::compile("mundo", false, true).unwrap();
        assert!(search.find(&buffer, 0, Direction::Forward).is_none());
        let search = Search::compile("Mundo", false, true).unwrap();
        assert!(search.find(&buffer, 0, Direction::Forward).is_some());
    }

    #[test]
    fn regexp_mode_matches_patterns_and_supports_posix_classes() {
        let buffer = TextBuffer::from_text("a1 b22\n");
        let search = Search::compile(r"b[[:digit:]]+", true, true).unwrap();
        let hit = search.find(&buffer, 0, Direction::Forward).unwrap();
        assert_eq!((hit.start, hit.end), (3, 6));
    }

    #[test]
    fn an_invalid_regexp_is_an_error_not_a_panic() {
        let err = Search::compile("a(", true, false).unwrap_err();
        assert!(!err.message.is_empty());
        assert_eq!(err.pattern, "a(");
    }

    #[test]
    fn an_empty_pattern_is_refused() {
        assert!(Search::compile("", false, false).is_err());
    }

    #[test]
    fn a_match_never_spans_a_line_break() {
        let buffer = TextBuffer::from_text("uno\ndos\n");
        assert!(find(&buffer, 0, "uno\ndos", false).is_none());
    }

    #[test]
    fn crlf_lines_are_matched_without_the_terminator() {
        let buffer = TextBuffer::from_text("uno\r\ndos\r\n");
        assert_eq!(find(&buffer, 0, "dos", false), Some((5, 8, false)));
        assert!(find(&buffer, 0, "dos\r", false).is_none());
    }

    #[test]
    fn offsets_are_chars_not_bytes() {
        let buffer = TextBuffer::from_text("ñ漢 uno\n");
        assert_eq!(find(&buffer, 0, "uno", false), Some((3, 6, false)));
    }

    #[test]
    fn overlapping_literal_matches_are_found_one_start_at_a_time() {
        let buffer = TextBuffer::from_text("aaaa");
        assert_eq!(find(&buffer, 0, "aa", false), Some((0, 2, false)));
        assert_eq!(find(&buffer, 1, "aa", false), Some((1, 3, false)));
    }
}
