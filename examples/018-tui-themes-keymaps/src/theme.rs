//! Themes: semantic styles instead of hard-coded colors.
//!
//! Widgets never say "red"; they ask the theme for `error`, `ok`, `selection`... One place
//! decides what that looks like – and an accessible variant is just another theme.

use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ThemeName {
    Dark,
    Light,
    /// Maximum contrast (WCAG-friendly): pure black/white + bright accents.
    HighContrast,
    /// No colors at all – meaning carried by bold/underline/reverse. Chosen when NO_COLOR is set.
    Mono,
}

impl ThemeName {
    pub fn next(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::HighContrast,
            Self::HighContrast => Self::Mono,
            Self::Mono => Self::Dark,
        }
    }
}

/// Semantic roles used by the UI.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub name: ThemeName,
    pub base: Style,
    pub border: Style,
    pub title: Style,
    pub selection: Style,
    pub muted: Style,
    pub ok: Style,
    pub warn: Style,
    pub error: Style,
}

impl Theme {
    pub fn get(name: ThemeName) -> Self {
        let plain = Style::new();
        match name {
            ThemeName::Dark => Self {
                name,
                base: plain.fg(Color::Gray).bg(Color::Black),
                border: plain.fg(Color::DarkGray),
                title: plain.fg(Color::Cyan).add_modifier(Modifier::BOLD),
                selection: plain.fg(Color::Black).bg(Color::Cyan),
                muted: plain.fg(Color::DarkGray),
                ok: plain.fg(Color::Green),
                warn: plain.fg(Color::Yellow),
                error: plain.fg(Color::Red).add_modifier(Modifier::BOLD),
            },
            ThemeName::Light => Self {
                name,
                base: plain.fg(Color::Black).bg(Color::White),
                border: plain.fg(Color::Gray),
                title: plain.fg(Color::Blue).add_modifier(Modifier::BOLD),
                selection: plain.fg(Color::White).bg(Color::Blue),
                muted: plain.fg(Color::DarkGray),
                ok: plain.fg(Color::Green),
                warn: plain.fg(Color::Rgb(160, 100, 0)),
                error: plain.fg(Color::Red).add_modifier(Modifier::BOLD),
            },
            ThemeName::HighContrast => Self {
                name,
                base: plain.fg(Color::White).bg(Color::Black),
                border: plain.fg(Color::White),
                title: plain
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                selection: plain
                    .fg(Color::Black)
                    .bg(Color::LightYellow)
                    .add_modifier(Modifier::BOLD),
                muted: plain.fg(Color::White),
                ok: plain.fg(Color::LightGreen).add_modifier(Modifier::BOLD),
                warn: plain.fg(Color::LightYellow).add_modifier(Modifier::BOLD),
                error: plain
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            },
            ThemeName::Mono => Self {
                name,
                base: plain,
                border: plain,
                title: plain.add_modifier(Modifier::BOLD),
                selection: plain.add_modifier(Modifier::REVERSED),
                muted: plain.add_modifier(Modifier::DIM),
                ok: plain,
                warn: plain.add_modifier(Modifier::BOLD),
                error: plain.add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            },
        }
    }

    /// `NO_COLOR` (step 007) overrides the requested theme.
    pub fn resolve(requested: ThemeName, no_color: bool) -> Self {
        Self::get(if no_color { ThemeName::Mono } else { requested })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_theme_uses_no_colors() {
        let theme = Theme::resolve(ThemeName::Dark, true);

        assert_eq!(theme.name, ThemeName::Mono);
        for style in [theme.ok, theme.warn, theme.error, theme.selection] {
            assert_eq!(style.fg, None);
            assert_eq!(style.bg, None);
        }
    }
}
