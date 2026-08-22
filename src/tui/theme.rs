use ratatui::style::{Color, Modifier, Style};

/// Terminal color capability used to resolve the semantic palette.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ColorMode {
    TrueColor,
    Ansi256,
}

impl ColorMode {
    pub(crate) fn detect() -> Self {
        let colorterm = std::env::var("COLORTERM")
            .unwrap_or_default()
            .to_ascii_lowercase();
        let term = std::env::var("TERM")
            .unwrap_or_default()
            .to_ascii_lowercase();
        if colorterm.contains("truecolor")
            || colorterm.contains("24bit")
            || term.contains("truecolor")
            || term.contains("direct")
        {
            Self::TrueColor
        } else {
            Self::Ansi256
        }
    }
}

/// Central semantic palette for every TUI rendering path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Theme {
    pub(crate) background: Color,
    pub(crate) background_alt: Color,
    pub(crate) border: Color,
    pub(crate) border_focus: Color,
    pub(crate) text_primary: Color,
    pub(crate) text_secondary: Color,
    pub(crate) text_muted: Color,
    pub(crate) accent: Color,
    pub(crate) cyan: Color,
    pub(crate) success: Color,
    pub(crate) warning: Color,
    pub(crate) decorative_gold: Color,
    pub(crate) error: Color,
    pub(crate) special: Color,
}

impl Theme {
    pub(crate) fn detect() -> Self {
        Self::for_mode(ColorMode::detect())
    }

    pub(crate) const fn for_mode(mode: ColorMode) -> Self {
        match mode {
            ColorMode::TrueColor => Self {
                background: Color::Rgb(0x16, 0x1b, 0x22),
                background_alt: Color::Rgb(0x21, 0x26, 0x2d),
                border: Color::Rgb(0x3d, 0x44, 0x4d),
                border_focus: Color::Rgb(0x58, 0xa6, 0xff),
                text_primary: Color::Rgb(0xd8, 0xde, 0xe9),
                text_secondary: Color::Rgb(0x8b, 0x94, 0x9e),
                text_muted: Color::Rgb(0x6e, 0x76, 0x81),
                accent: Color::Rgb(0x58, 0xa6, 0xff),
                cyan: Color::Rgb(0x56, 0xd4, 0xdd),
                success: Color::Rgb(0x3f, 0xb9, 0x50),
                warning: Color::Rgb(0xd2, 0x99, 0x22),
                decorative_gold: Color::Rgb(0xf2, 0xcc, 0x60),
                error: Color::Rgb(0xf8, 0x51, 0x49),
                special: Color::Rgb(0xbc, 0x8c, 0xff),
            },
            ColorMode::Ansi256 => Self {
                background: Color::Indexed(233),
                background_alt: Color::Indexed(235),
                border: Color::Indexed(238),
                border_focus: Color::Indexed(75),
                text_primary: Color::Indexed(253),
                text_secondary: Color::Indexed(245),
                text_muted: Color::Indexed(242),
                accent: Color::Indexed(75),
                cyan: Color::Indexed(80),
                success: Color::Indexed(71),
                warning: Color::Indexed(178),
                decorative_gold: Color::Indexed(220),
                error: Color::Indexed(203),
                special: Color::Indexed(141),
            },
        }
    }

    pub(crate) fn style(self, color: Color) -> Style {
        Style::default().fg(color).bg(self.background)
    }

    pub(crate) fn bold(self, color: Color) -> Style {
        self.style(color).add_modifier(Modifier::BOLD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_matches_documented_truecolor_and_ansi256_values() {
        let rgb = Theme::for_mode(ColorMode::TrueColor);
        assert_eq!(rgb.background, Color::Rgb(0x16, 0x1b, 0x22));
        assert_eq!(rgb.text_primary, Color::Rgb(0xd8, 0xde, 0xe9));
        assert_eq!(rgb.accent, Color::Rgb(0x58, 0xa6, 0xff));
        assert_eq!(rgb.success, Color::Rgb(0x3f, 0xb9, 0x50));
        assert_eq!(rgb.warning, Color::Rgb(0xd2, 0x99, 0x22));
        assert_eq!(rgb.decorative_gold, Color::Rgb(0xf2, 0xcc, 0x60));
        assert_eq!(rgb.error, Color::Rgb(0xf8, 0x51, 0x49));

        let ansi = Theme::for_mode(ColorMode::Ansi256);
        assert_eq!(ansi.background, Color::Indexed(233));
        assert_eq!(ansi.text_primary, Color::Indexed(253));
        assert_eq!(ansi.accent, Color::Indexed(75));
        assert_eq!(ansi.success, Color::Indexed(71));
        assert_eq!(ansi.warning, Color::Indexed(178));
        assert_eq!(ansi.decorative_gold, Color::Indexed(220));
        assert_eq!(ansi.error, Color::Indexed(203));
    }
}
