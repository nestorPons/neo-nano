//! Semantic colours -> terminal palette.
//!
//! `nnano-core` names colours (red, brightblue, …) and never imports ratatui; this
//! is the only place the two meet. Every mapping is total, so a grammar can never
//! make a frame fail.

use nnano_core::syntax::{Colour, SyntaxStyle};
use ratatui::style::{Color, Modifier, Style};

pub fn colour(colour: Colour) -> Color {
    match colour {
        Colour::Black => Color::Black,
        Colour::Red => Color::Red,
        Colour::Green => Color::Green,
        Colour::Yellow => Color::Yellow,
        Colour::Blue => Color::Blue,
        Colour::Magenta => Color::Magenta,
        Colour::Cyan => Color::Cyan,
        Colour::White => Color::Gray,
        Colour::BrightBlack => Color::DarkGray,
        Colour::BrightRed => Color::LightRed,
        Colour::BrightGreen => Color::LightGreen,
        Colour::BrightYellow => Color::LightYellow,
        Colour::BrightBlue => Color::LightBlue,
        Colour::BrightMagenta => Color::LightMagenta,
        Colour::BrightCyan => Color::LightCyan,
        Colour::BrightWhite => Color::White,
    }
}

pub fn span_style(style: &SyntaxStyle) -> Style {
    let mut modifiers = Modifier::empty();
    if style.attributes.bold {
        modifiers |= Modifier::BOLD;
    }
    if style.attributes.italic {
        modifiers |= Modifier::ITALIC;
    }
    if style.attributes.underline {
        modifiers |= Modifier::UNDERLINED;
    }
    if style.attributes.reverse {
        modifiers |= Modifier::REVERSED;
    }
    let mut out = Style::default().add_modifier(modifiers);
    if let Some(foreground) = style.foreground {
        out = out.fg(colour(foreground));
    }
    if let Some(background) = style.background {
        out = out.bg(colour(background));
    }
    out
}

#[cfg(test)]
mod tests {
    use nnano_core::syntax::{Colour, SyntaxStyle};
    use ratatui::style::Color;

    use super::{colour, span_style};

    #[test]
    fn every_colour_maps_to_the_palette() {
        for (named, expected) in [
            (Colour::Black, Color::Black),
            (Colour::White, Color::Gray),
            (Colour::BrightBlack, Color::DarkGray),
            (Colour::BrightWhite, Color::White),
        ] {
            assert_eq!(colour(named), expected);
        }
    }

    #[test]
    fn a_style_carries_both_sides() {
        let style = span_style(&SyntaxStyle {
            foreground: Some(Colour::Yellow),
            background: Some(Colour::Blue),
            ..SyntaxStyle::default()
        });
        assert_eq!(style.fg, Some(Color::Yellow));
        assert_eq!(style.bg, Some(Color::Blue));
        assert_eq!(
            span_style(&SyntaxStyle::default()),
            ratatui::style::Style::default()
        );
    }
}
