//! Syntax highlighting, nano's `.nanorc` model (phase 7).
//!
//! - Definitions are nano's: `syntax <name> [file patterns]`, `color <fg,bg>
//!   "regex"`, `icolor` for case-insensitive rules. No tree-sitter, no TextMate,
//!   no plugin system.
//! - A `syntax` block with no file patterns is the **fallback** grammar, as in
//!   nano's `default.nanorc`; it is only used when nothing else matched.
//! - Evaluation is **per line and stateless**: a rule never spans a line break, so
//!   there is no state to invalidate after an edit. `start=`/`end=` multi-line
//!   rules from modern nanorc files are ignored on purpose.
//! - Only viewport lines are matched, and a line longer than [`MAX_LINE`] is left
//!   plain, so a 5 MB single line cannot stall a redraw.
//! - Overlaps resolve as in nano: the earliest rule wins per char, and `icolor`
//!   compiles with a case-insensitive engine.
//! - Colours are a semantic enum mapped to the terminal palette, plus the bold /
//!   italic / underline / reverse attributes. Distro grammars use names we do not
//!   know (`pink`, `latte`, hex): those degrade to the default colour and **never**
//!   fail or warn, and a pattern `regex-lite` cannot compile is simply skipped.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use regex_lite::{Regex, RegexBuilder};

/// Longest line that gets highlighted, in chars. Beyond this a line is drawn
/// plain: nano's per-line cost would otherwise be paid on a single huge line.
pub const MAX_LINE: usize = 4096;

/// Terminal palette colour, 8 normal plus 8 bright.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Colour {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
}

impl Colour {
    fn parse(name: &str) -> Option<Self> {
        let name = name.trim().to_ascii_lowercase();
        let (bright, base) = match name.strip_prefix("bright") {
            Some(rest) => (true, rest),
            None => (false, name.as_str()),
        };
        let colour = match base {
            "black" => Colour::Black,
            "red" => Colour::Red,
            "green" => Colour::Green,
            "yellow" => Colour::Yellow,
            "blue" => Colour::Blue,
            "magenta" => Colour::Magenta,
            "cyan" => Colour::Cyan,
            "white" | "grey" | "gray" => Colour::White,
            _ => return None,
        };
        Some(if bright {
            match colour {
                Colour::Black => Colour::BrightBlack,
                Colour::Red => Colour::BrightRed,
                Colour::Green => Colour::BrightGreen,
                Colour::Yellow => Colour::BrightYellow,
                Colour::Blue => Colour::BrightBlue,
                Colour::Magenta => Colour::BrightMagenta,
                Colour::Cyan => Colour::BrightCyan,
                Colour::White | Colour::BrightWhite => Colour::BrightWhite,
                other => other,
            }
        } else {
            colour
        })
    }
}

/// Text attributes a rule can request. Unknown attributes are ignored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Attributes {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub reverse: bool,
}

/// A rule's presentation. `None` means "leave the terminal default".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyntaxStyle {
    pub foreground: Option<Colour>,
    pub background: Option<Colour>,
    pub attributes: Attributes,
}

/// One `color`/`icolor` line.
#[derive(Debug)]
struct Rule {
    regex: Regex,
    style: SyntaxStyle,
}

/// One `syntax` block plus its rules.
#[derive(Debug)]
pub struct Grammar {
    pub name: String,
    /// Regexes matched against the file name, as nano stores them (`\.rs$`).
    file_patterns: Vec<Regex>,
    /// No patterns: the grammar for files nothing else matched.
    fallback: bool,
    rules: Vec<Rule>,
}

/// A coloured char range inside one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub style: SyntaxStyle,
}

impl Grammar {
    pub fn matches(&self, file_name: &str) -> bool {
        !self.fallback
            && self
                .file_patterns
                .iter()
                .any(|pattern| pattern.is_match(file_name))
    }

    pub fn is_fallback(&self) -> bool {
        self.fallback
    }

    /// Spans for one line, in char offsets, earliest rule wins per char.
    pub fn highlight(&self, line: &str, limit: usize) -> Vec<Span> {
        let total = line.chars().count();
        if total == 0 || total > limit {
            return Vec::new();
        }
        let mut owner: Vec<Option<u16>> = vec![None; total];

        for (index, rule) in self.rules.iter().enumerate() {
            let mut scanned_byte = 0;
            let mut scanned_char = 0;
            for found in rule.regex.find_iter(line) {
                let Some(prefix) = line.get(scanned_byte..found.start()) else {
                    continue;
                };
                scanned_char += prefix.chars().count();
                let start = scanned_char;
                let Some(matched) = line.get(found.start()..found.end()) else {
                    continue;
                };
                let end = start + matched.chars().count();
                scanned_byte = found.end();
                scanned_char = end;
                for slot in &mut owner[start.min(total)..end.min(total)] {
                    if slot.is_none() {
                        *slot = u16::try_from(index).ok();
                    }
                }
            }
        }

        let mut spans = Vec::new();
        let mut index = 0;
        while index < total {
            let Some(style_index) = owner[index] else {
                index += 1;
                continue;
            };
            let start = index;
            while index < total && owner[index] == Some(style_index) {
                index += 1;
            }
            let Some(rule) = self.rules.get(usize::from(style_index)) else {
                continue;
            };
            spans.push(Span {
                start,
                end: index,
                style: rule.style,
            });
        }
        spans
    }
}

/// Every grammar loaded, in precedence order: the first match wins.
#[derive(Debug, Default)]
pub struct SyntaxSet {
    grammars: Vec<Grammar>,
}

impl SyntaxSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_grammars(grammars: Vec<Grammar>) -> Self {
        Self { grammars }
    }

    pub fn is_empty(&self) -> bool {
        self.grammars.is_empty()
    }

    pub fn len(&self) -> usize {
        self.grammars.len()
    }

    pub fn names(&self) -> Vec<&str> {
        self.grammars
            .iter()
            .map(|grammar| grammar.name.as_str())
            .collect()
    }

    pub fn get(&self, name: &str) -> Option<&Grammar> {
        self.grammars.iter().find(|grammar| grammar.name == name)
    }

    /// First grammar whose file patterns match this path's name, then the
    /// fallback grammar, if any.
    pub fn select(&self, path: &str) -> Option<&Grammar> {
        let name = Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(path);
        self.grammars
            .iter()
            .find(|grammar| grammar.matches(name))
            .or_else(|| self.grammars.iter().find(|grammar| grammar.fallback))
    }

    /// Read every `*.nanorc` in `dirs`, in order. Missing directories are normal
    /// (most systems have none of the first two); unreadable ones warn. Grammar
    /// content is third-party data and never produces warnings: a rule we cannot
    /// use is skipped so a distro file cannot spam the terminal.
    pub fn load(dirs: &[PathBuf]) -> (Self, Vec<String>) {
        let mut set = Self::default();
        let mut warnings = Vec::new();

        for dir in dirs {
            let Ok(entries) = fs::read_dir(dir) else {
                continue;
            };
            let mut files: Vec<PathBuf> = entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "nanorc"))
                .collect();
            files.sort();

            for path in files {
                match fs::read_to_string(&path) {
                    Ok(text) => set.grammars.extend(parse(&text)),
                    Err(err) => warnings.push(format!("{}: cannot read: {err}", path.display())),
                }
            }
        }

        (set, warnings)
    }
}

/// Grammar directories, highest precedence first: user, nnano's system dir, then
/// the distro grammars nano ships (read-only, so we inherit its set).
pub fn detect_dirs() -> Vec<PathBuf> {
    dirs_from_env(|name| std::env::var(name).ok())
}

/// Testable directory resolution without touching the process environment.
pub fn dirs_from_env(get: impl Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let home = get("HOME").filter(|value| !value.is_empty());
    let config_home = get("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            home.as_ref()
                .map(|home| PathBuf::from(home).join(".config"))
        });
    if let Some(dir) = config_home {
        dirs.push(dir.join("nnano").join("syntax"));
    }
    dirs.push(PathBuf::from("/usr/share/nnano/syntax"));
    dirs.push(PathBuf::from("/usr/share/nano"));
    dirs
}

/// Parse a `.nanorc` fragment into grammars. Unusable rules are dropped silently;
/// multi-line (`start=`/`end=`), `header`, `comment` and config directives are
/// outside this model and are skipped.
pub fn parse(text: &str) -> Vec<Grammar> {
    let mut grammars: Vec<Grammar> = Vec::new();
    let mut current: Option<Grammar> = None;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.contains("start=") || line.contains("end=") {
            // Multi-line rule: per-line, stateless highlighting cannot honour it.
            continue;
        }
        let mut words = line.splitn(2, char::is_whitespace);
        let directive = words.next().unwrap_or_default();
        let rest = words.next().unwrap_or_default().trim();

        match directive {
            "syntax" => {
                if let Some(grammar) = current.take() {
                    grammars.push(grammar);
                }
                current = start_grammar(rest);
            }
            "color" | "icolor" => {
                let Some(grammar) = current.as_mut() else {
                    continue;
                };
                if let Some(rule) = parse_rule(directive, rest) {
                    grammar.rules.push(rule);
                }
            }
            _ => {}
        }
    }
    if let Some(grammar) = current.take() {
        grammars.push(grammar);
    }

    grammars
}

/// `syntax "c" "\.c$"`, `syntax asm "\.(S|s|asm)$"` and `syntax default` are all
/// valid: the name may be quoted or bare, and only the quoted words after it are
/// file patterns.
fn start_grammar(rest: &str) -> Option<Grammar> {
    let (name, remainder) = if rest.starts_with('"') {
        let (name, remainder) = take_quoted(rest)?;
        (name.trim().to_string(), remainder)
    } else {
        let mut words = rest.splitn(2, char::is_whitespace);
        let name = words.next()?.to_string();
        (name, words.next().unwrap_or_default())
    };
    let patterns = quoted_words(remainder);
    let file_patterns = patterns
        .iter()
        .filter_map(|pattern| Regex::new(pattern).ok())
        .collect();
    Some(Grammar {
        name,
        file_patterns,
        fallback: patterns.is_empty(),
        rules: Vec::new(),
    })
}

fn parse_rule(directive: &str, rest: &str) -> Option<Rule> {
    let mut words = rest.splitn(2, char::is_whitespace);
    let spec = words.next().unwrap_or_default();
    let regex_text = words.next().unwrap_or_default().trim();
    let style = parse_style(spec);
    let pattern = unquote(regex_text);
    if pattern.is_empty() {
        return None;
    }
    let regex = RegexBuilder::new(&pattern)
        .case_insensitive(directive == "icolor")
        .build()
        .ok()?;
    Some(Rule { regex, style })
}

/// `yellow`, `brightred`, `yellow,blue`, `,white`, with optional leading
/// attributes (`bold,pink,red`). Unknown colours become "terminal default".
fn parse_style(spec: &str) -> SyntaxStyle {
    let mut style = SyntaxStyle::default();
    // Colour slots: an empty part still consumes one, so `,green` is a background.
    let mut slot = 0;
    for part in spec.split(',') {
        match part.trim().to_ascii_lowercase().as_str() {
            "" => slot += 1,
            "bold" => style.attributes.bold = true,
            "italic" => style.attributes.italic = true,
            "underline" => style.attributes.underline = true,
            "reverse" => style.attributes.reverse = true,
            name => {
                let colour = Colour::parse(name);
                match slot {
                    0 => style.foreground = colour,
                    1 => style.background = colour,
                    _ => {}
                }
                slot += 1;
            }
        }
    }
    style
}

/// Every double-quoted word in `text`, in order. `\"` and `\\` are unescaped;
/// other backslashes stay, because a regex like `\.rs$` needs them.
fn quoted_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('"') {
        rest = &rest[start..];
        let Some((word, remainder)) = take_quoted(rest) else {
            break;
        };
        let word = word.trim();
        if !word.is_empty() {
            words.push(word.to_string());
        }
        rest = remainder;
    }
    words
}

/// One quoted string at the start of `text`, with the rest after the closing
/// quote. `None` when the quote is never closed.
fn take_quoted(text: &str) -> Option<(String, &str)> {
    let text = text.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = text.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        match ch {
            '"' => return Some((out, &text[index + 1..])),
            '\\' => match chars.peek() {
                Some((_, '"')) => {
                    chars.next();
                    out.push('"');
                }
                Some((_, '\\')) => {
                    chars.next();
                    out.push('\\');
                }
                _ => out.push('\\'),
            },
            _ => out.push(ch),
        }
    }
    None
}

fn unquote(text: &str) -> String {
    let text = text.trim();
    text.strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(text)
        .to_string()
}

impl fmt::Display for Colour {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Attributes, Colour, MAX_LINE, SyntaxSet, SyntaxStyle, detect_dirs, dirs_from_env, parse,
    };

    const RUST: &str = r#"
syntax "rust" "\.rs$"
color yellow "\b(fn|let|impl)\b"
icolor cyan "todo"
"#;

    fn set() -> SyntaxSet {
        SyntaxSet::from_grammars(parse(RUST))
    }

    #[test]
    fn a_grammar_matches_by_file_name() {
        let set = set();
        assert_eq!(set.len(), 1);
        assert!(set.select("src/main.rs").is_some());
        assert!(set.select("main.py").is_none());
        assert_eq!(set.get("rust").unwrap().name, "rust");
    }

    #[test]
    fn a_rule_can_match_several_times_on_one_line() {
        // Regression: the char cursor was not advanced past a match, so the
        // second and later hits of the same rule landed over earlier text.
        let set = set();
        let grammar = set.get("rust").unwrap();
        let line = "let a = fn(b) + let;";
        let spans = grammar.highlight(line, MAX_LINE);
        let text: Vec<(&str, Option<Colour>)> = spans
            .iter()
            .map(|span| (&line[span.start..span.end], span.style.foreground))
            .collect();
        assert_eq!(
            text,
            vec![
                ("let", Some(Colour::Yellow)),
                ("fn", Some(Colour::Yellow)),
                ("let", Some(Colour::Yellow)),
            ]
        );
    }

    #[test]
    fn rules_colour_matches_with_the_earliest_rule_winning() {
        let set = set();
        let grammar = set.get("rust").unwrap();
        let line = "fn TODO";
        let spans = grammar.highlight(line, MAX_LINE);
        let text: Vec<(&str, Option<Colour>)> = spans
            .iter()
            .map(|span| (&line[span.start..span.end], span.style.foreground))
            .collect();
        assert_eq!(
            text,
            vec![("fn", Some(Colour::Yellow)), ("TODO", Some(Colour::Cyan))]
        );
    }

    #[test]
    fn icolor_is_case_insensitive_and_color_is_not() {
        let grammar = &parse("syntax \"x\" \"\\.x$\"\ncolor red \"todo\"\n")[0];
        assert!(grammar.highlight("TODO", MAX_LINE).is_empty());
        assert_eq!(grammar.highlight("todo", MAX_LINE).len(), 1);

        let grammar = &parse("syntax \"x\" \"\\.x$\"\nicolor red \"todo\"\n")[0];
        assert_eq!(grammar.highlight("TODO", MAX_LINE).len(), 1);
    }

    #[test]
    fn bright_and_background_colours_parse() {
        let grammars = parse("syntax \"x\" \"\\.x$\"\ncolor brightred,blue \"a\"\n");
        let span = grammars[0].highlight("a", MAX_LINE)[0];
        assert_eq!(
            span.style,
            SyntaxStyle {
                foreground: Some(Colour::BrightRed),
                background: Some(Colour::Blue),
                attributes: Attributes::default(),
            }
        );
    }

    #[test]
    fn a_background_only_rule_keeps_the_default_foreground() {
        let grammars = parse("syntax \"x\" \"\\.x$\"\ncolor ,green \"a\"\n");
        let span = grammars[0].highlight("a", MAX_LINE)[0];
        assert_eq!(span.style.foreground, None);
        assert_eq!(span.style.background, Some(Colour::Green));
    }

    #[test]
    fn unknown_colours_keep_the_rule_and_degrade() {
        let grammars = parse("syntax \"x\" \"\\.x$\"\ncolor bold,pink,red \"a\"\n");
        let span = grammars[0].highlight("a", MAX_LINE)[0];
        assert!(span.style.attributes.bold);
        assert_eq!(span.style.foreground, None, "pink is not in the palette");
        assert_eq!(span.style.background, Some(Colour::Red));
    }

    #[test]
    fn posix_classes_work() {
        let grammars = parse("syntax \"x\" \"\\.x$\"\ncolor red \"[[:digit:]]+\"\n");
        let spans = grammars[0].highlight("a12", MAX_LINE);
        assert_eq!(spans.len(), 1);
        assert_eq!((spans[0].start, spans[0].end), (1, 3));
    }

    #[test]
    fn an_uncompilable_pattern_is_skipped_silently() {
        let grammars = parse("syntax \"x\" \"\\.x$\"\ncolor red \"(\"\n");
        assert_eq!(grammars.len(), 1);
        assert!(grammars[0].highlight("(", MAX_LINE).is_empty());
    }

    #[test]
    fn an_unquoted_name_without_patterns_is_the_fallback() {
        let grammars = parse("syntax default\ncolor red \"x\"\n");
        assert!(grammars[0].is_fallback());
        let set = SyntaxSet::from_grammars(grammars);
        assert!(set.select("anything.txt").is_some());
        assert!(!set.select("anything.txt").unwrap().matches("anything.txt"));
    }

    #[test]
    fn a_bare_name_with_quoted_patterns_keeps_its_patterns() {
        // The shape /usr/share/nano/asm.nanorc uses.
        let grammars = parse("syntax asm \"\\.(S|s|asm)$\"\ncolor red \"mov\"\n");
        assert_eq!(grammars[0].name, "asm");
        assert!(!grammars[0].is_fallback());
        let set = SyntaxSet::from_grammars(grammars);
        assert_eq!(set.select("main.asm").unwrap().name, "asm");
        assert!(set.select("main.txt").is_none());
    }

    #[test]
    fn a_patterned_grammar_wins_over_the_fallback() {
        let set = SyntaxSet::from_grammars(parse(
            "syntax default\ncolor red \"x\"\nsyntax \"rust\" \"\\.rs$\"\ncolor blue \"fn\"\n",
        ));
        assert_eq!(set.select("main.rs").unwrap().name, "rust");
        assert_eq!(set.select("readme.md").unwrap().name, "default");
    }

    #[test]
    fn multiline_rules_and_unknown_directives_are_ignored() {
        let grammars = parse(
            "syntax \"a\" \"\\.a$\"\ncomment \"#\"\ncolor red \"x\"\ncolor yellow start=\"/*\" end=\"*/\" \"y\"\nheader \"^#!\"\n\nsyntax \"b\" \"\\.b$\"\ncolor blue \"z\"\n",
        );
        assert_eq!(grammars.len(), 2);
        assert_eq!(grammars[1].name, "b");
        assert_eq!(grammars[0].highlight("y", MAX_LINE).len(), 0);
    }

    #[test]
    fn a_huge_line_is_left_plain() {
        let set = set();
        let grammar = set.get("rust").unwrap();
        let line = "fn ".repeat(MAX_LINE);
        assert!(grammar.highlight(&line, MAX_LINE).is_empty());
    }

    #[test]
    fn grammar_directories_are_ordered_user_first() {
        let dirs = dirs_from_env(|name| match name {
            "HOME" => Some("/home/ada".to_string()),
            _ => None,
        });
        assert_eq!(
            dirs[0],
            std::path::PathBuf::from("/home/ada/.config/nnano/syntax")
        );
        assert_eq!(dirs.len(), 3);
    }

    #[test]
    fn detect_dirs_always_has_the_system_fallbacks() {
        let dirs = detect_dirs();
        assert!(dirs.iter().any(|dir| dir.ends_with("nano")));
    }
}
