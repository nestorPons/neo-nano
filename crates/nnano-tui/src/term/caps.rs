//! Capability detection (phase 1, consumed by input and the style map later).
//!
//! Everything here is advisory: no capability may make a run fail, and nothing
//! is enforced against the terminal. Detection is pure so it can be tested
//! without touching the process environment.

/// What the terminal, and any multiplexer above it, reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caps {
    /// `$TERM`, when set and non-empty.
    pub term: Option<String>,
    /// Running under tmux (`$TMUX`), where `^B` is the prefix.
    pub tmux: bool,
    /// Running under GNU screen (`$STY`), where `^A` is the prefix.
    pub screen: bool,
    /// Colours the terminal claims to support; 0 when unknown.
    pub colours: u16,
}

impl Caps {
    pub fn detect() -> Self {
        Self::from_env(|key| std::env::var(key).ok())
    }

    fn from_env(get: impl Fn(&str) -> Option<String>) -> Self {
        Self {
            term: get("TERM").filter(|term| !term.is_empty()),
            tmux: get("TMUX").is_some(),
            screen: get("STY").is_some(),
            colours: crossterm::style::available_color_count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Caps;

    fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |key| {
            pairs
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| (*value).to_string())
        }
    }

    #[test]
    fn detects_term_and_multiplexers() {
        let caps = Caps::from_env(env(&[
            ("TERM", "xterm-256color"),
            ("TMUX", "/tmp/tmux-1000/default,123,0"),
        ]));
        assert_eq!(caps.term.as_deref(), Some("xterm-256color"));
        assert!(caps.tmux);
        assert!(!caps.screen);
    }

    #[test]
    fn empty_term_counts_as_unknown() {
        let caps = Caps::from_env(env(&[("TERM", "")]));
        assert_eq!(caps.term, None);
    }

    #[test]
    fn reports_screen_prefix() {
        let caps = Caps::from_env(env(&[("STY", "1234.pts-0.host")]));
        assert!(caps.screen);
        assert!(!caps.tmux);
    }
}
