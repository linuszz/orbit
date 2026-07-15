use ratatui::style::Color;

pub struct ThemeColors {
    pub bg_primary: Color,
    pub bg_secondary: Color,
    pub bg_tertiary: Color,
    pub bg_card: Color,
    pub fg_primary: Color,
    pub fg_secondary: Color,
    pub fg_muted: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_idle: Color,
    pub accent_blocked: Color,
    pub accent_error: Color,
    pub border: Color,
}

impl ThemeColors {
    pub const fn orbit() -> Self {
        Self {
            bg_primary: Color::Rgb(14, 14, 20),
            bg_secondary: Color::Rgb(18, 18, 26),
            bg_tertiary: Color::Rgb(24, 24, 33),
            bg_card: Color::Rgb(20, 20, 29),
            fg_primary: Color::Rgb(242, 242, 248),
            fg_secondary: Color::Rgb(180, 180, 196),
            fg_muted: Color::Rgb(120, 120, 140),
            accent: Color::Rgb(217, 119, 6),
            accent_hover: Color::Rgb(161, 86, 0),
            accent_idle: Color::Rgb(96, 120, 158),
            accent_blocked: Color::Rgb(217, 172, 0),
            accent_error: Color::Rgb(200, 50, 30),
            border: Color::Rgb(60, 60, 76),
        }
    }

    pub const fn tokyo_night() -> Self {
        Self {
            bg_primary: Color::Rgb(26, 27, 38),
            bg_secondary: Color::Rgb(36, 40, 59),
            bg_tertiary: Color::Rgb(43, 47, 68),
            bg_card: Color::Rgb(36, 40, 59),
            fg_primary: Color::Rgb(169, 177, 214),
            fg_secondary: Color::Rgb(148, 156, 187),
            fg_muted: Color::Rgb(90, 95, 128),
            accent: Color::Rgb(187, 154, 247),
            accent_hover: Color::Rgb(165, 130, 220),
            accent_idle: Color::Rgb(125, 207, 255),
            accent_blocked: Color::Rgb(224, 175, 104),
            accent_error: Color::Rgb(247, 118, 142),
            border: Color::Rgb(45, 48, 70),
        }
    }
}

thread_local! {
    static CURRENT: std::cell::RefCell<ThemeColors> =
        const { std::cell::RefCell::new(ThemeColors::orbit()) };
}

pub fn set_theme(name: &str) {
    let new = match name {
        "tokyo-night" => ThemeColors::tokyo_night(),
        _ => ThemeColors::orbit(),
    };
    CURRENT.with(|c| *c.borrow_mut() = new);
}

pub fn bg_primary() -> Color {
    CURRENT.with(|c| c.borrow().bg_primary)
}
pub fn bg_secondary() -> Color {
    CURRENT.with(|c| c.borrow().bg_secondary)
}
pub fn bg_tertiary() -> Color {
    CURRENT.with(|c| c.borrow().bg_tertiary)
}
pub fn bg_card() -> Color {
    CURRENT.with(|c| c.borrow().bg_card)
}
pub fn fg_primary() -> Color {
    CURRENT.with(|c| c.borrow().fg_primary)
}
pub fn fg_secondary() -> Color {
    CURRENT.with(|c| c.borrow().fg_secondary)
}
pub fn fg_muted() -> Color {
    CURRENT.with(|c| c.borrow().fg_muted)
}
pub fn accent() -> Color {
    CURRENT.with(|c| c.borrow().accent)
}
pub fn accent_hover() -> Color {
    CURRENT.with(|c| c.borrow().accent_hover)
}
pub fn accent_idle() -> Color {
    CURRENT.with(|c| c.borrow().accent_idle)
}
pub fn accent_blocked() -> Color {
    CURRENT.with(|c| c.borrow().accent_blocked)
}
pub fn accent_error() -> Color {
    CURRENT.with(|c| c.borrow().accent_error)
}
pub fn border() -> Color {
    CURRENT.with(|c| c.borrow().border)
}
