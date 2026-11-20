use iced::widget::{button, container, scrollable, text_input};
use iced::{Background, Border, Color, Font, Shadow, Theme, font};

/// Core esports dark palette for KovaaK Pulse.
pub const BG_BASE: Color = Color::from_rgb(0x0D as f32 / 255.0, 0x10 as f32 / 255.0, 0x17 as f32 / 255.0);
pub const BG_SURFACE: Color = Color::from_rgb(0x11 as f32 / 255.0, 0x16 as f32 / 255.0, 0x22 as f32 / 255.0);
pub const BG_CARD: Color = Color::from_rgb(0x16 as f32 / 255.0, 0x1B as f32 / 255.0, 0x26 as f32 / 255.0);
pub const BG_CARD_HOVER: Color = Color::from_rgb(0x1C as f32 / 255.0, 0x23 as f32 / 255.0, 0x33 as f32 / 255.0);
pub const BG_ELEVATED: Color = Color::from_rgb(0x1E as f32 / 255.0, 0x26 as f32 / 255.0, 0x38 as f32 / 255.0);

pub const BORDER_SUBTLE: Color = Color::from_rgb(0x1E as f32 / 255.0, 0x25 as f32 / 255.0, 0x36 as f32 / 255.0);
pub const BORDER_ACTIVE: Color = Color::from_rgb(0x2D as f32 / 255.0, 0x3A as f32 / 255.0, 0x52 as f32 / 255.0);

pub const TEXT_PRIMARY: Color = Color::from_rgb(0xF1 as f32 / 255.0, 0xF5 as f32 / 255.0, 0xF9 as f32 / 255.0);
pub const TEXT_SECONDARY: Color = Color::from_rgb(0x94 as f32 / 255.0, 0xA3 as f32 / 255.0, 0xB8 as f32 / 255.0);
pub const TEXT_MUTED: Color = Color::from_rgb(0x64 as f32 / 255.0, 0x74 as f32 / 255.0, 0x8B as f32 / 255.0);

pub const ACCENT_CYAN: Color = Color::from_rgb(0x00 as f32 / 255.0, 0xD4 as f32 / 255.0, 0xFF as f32 / 255.0);
pub const ACCENT_CYAN_DIM: Color = Color::from_rgba(0.0, 0xD4 as f32 / 255.0, 1.0, 0.16);
pub const ACCENT_GREEN: Color = Color::from_rgb(0x22 as f32 / 255.0, 0xC5 as f32 / 255.0, 0x5E as f32 / 255.0);
pub const ACCENT_GREEN_DIM: Color = Color::from_rgba(0x22 as f32 / 255.0, 0xC5 as f32 / 255.0, 0x5E as f32 / 255.0, 0.16);
pub const ACCENT_AMBER: Color = Color::from_rgb(0xF5 as f32 / 255.0, 0x9E as f32 / 255.0, 0x0B as f32 / 255.0);
pub const ACCENT_RED: Color = Color::from_rgb(0xEF as f32 / 255.0, 0x44 as f32 / 255.0, 0x44 as f32 / 255.0);
pub const ACCENT_RED_DIM: Color = Color::from_rgba(0xEF as f32 / 255.0, 0x44 as f32 / 255.0, 0x44 as f32 / 255.0, 0.18);
pub const ACCENT_PURPLE: Color = Color::from_rgb(0xA8 as f32 / 255.0, 0x55 as f32 / 255.0, 0xF7 as f32 / 255.0);

pub const GRID_LINE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.07);
pub const CROSSHAIR_LINE: Color = Color::from_rgba(0.0, 0.83, 1.0, 0.45);
pub const TOOLTIP_BG: Color = Color::from_rgba(0x0B as f32 / 255.0, 0x0F as f32 / 255.0, 0x19 as f32 / 255.0, 0.95);

pub const FONT_REGULAR: Font = Font {
    family: font::Family::Name("Microsoft YaHei"),
    weight: font::Weight::Normal,
    stretch: font::Stretch::Normal,
    style: font::Style::Normal,
};

pub const FONT_BOLD: Font = Font {
    family: font::Family::Name("Microsoft YaHei"),
    weight: font::Weight::Bold,
    stretch: font::Stretch::Normal,
    style: font::Style::Normal,
};

pub fn app_theme() -> Theme {
    Theme::custom(
        "KovaaK Pulse".to_string(),
        iced::theme::Palette {
            background: BG_BASE,
            text: TEXT_PRIMARY,
            primary: ACCENT_CYAN,
            success: ACCENT_GREEN,
            warning: ACCENT_AMBER,
            danger: ACCENT_RED,
        },
    )
}

pub fn base_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_BASE)),
        text_color: Some(TEXT_PRIMARY),
        ..Default::default()
    }
}

pub fn sidebar_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_SURFACE)),
        border: Border {
            color: BORDER_SUBTLE,
            width: 1.0,
            radius: 0.0.into(),
        },
        text_color: Some(TEXT_PRIMARY),
        ..Default::default()
    }
}

pub fn topbar_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_SURFACE)),
        border: Border {
            color: BORDER_SUBTLE,
            width: 1.0,
            radius: 0.0.into(),
        },
        text_color: Some(TEXT_PRIMARY),
        ..Default::default()
    }
}

pub fn card_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_CARD)),
        border: Border {
            color: BORDER_SUBTLE,
            width: 1.0,
            radius: 8.0.into(),
        },
        text_color: Some(TEXT_PRIMARY),
        ..Default::default()
    }
}

pub fn card_accent_container(accent: Color) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(Background::Color(BG_CARD)),
        border: Border {
            color: Color { a: 0.45, ..accent },
            width: 1.0,
            radius: 8.0.into(),
        },
        text_color: Some(TEXT_PRIMARY),
        ..Default::default()
    }
}

pub fn badge_container(bg: Color, border_color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 4.0.into(),
        },
        text_color: Some(TEXT_PRIMARY),
        ..Default::default()
    }
}

pub fn banner_container(accent: Color) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(Background::Color(Color { a: 0.14, ..accent })),
        border: Border {
            color: Color { a: 0.5, ..accent },
            width: 1.0,
            radius: 6.0.into(),
        },
        text_color: Some(TEXT_PRIMARY),
        ..Default::default()
    }
}

pub fn tab_button(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let (bg, border, text) = if active {
            (ACCENT_CYAN_DIM, ACCENT_CYAN, ACCENT_CYAN)
        } else {
            match status {
                button::Status::Hovered => (BG_CARD_HOVER, BORDER_ACTIVE, TEXT_PRIMARY),
                _ => (Color::TRANSPARENT, Color::TRANSPARENT, TEXT_SECONDARY),
            }
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: text,
            border: Border {
                color: border,
                width: if active { 1.0 } else { 0.0 },
                radius: 6.0.into(),
            },
            shadow: Shadow::default(),
            snap: true,
        }
    }
}

pub fn pill_button(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let (bg, border, text) = if active {
            (ACCENT_CYAN_DIM, ACCENT_CYAN, ACCENT_CYAN)
        } else {
            match status {
                button::Status::Hovered => (BG_ELEVATED, BORDER_ACTIVE, TEXT_PRIMARY),
                _ => (BG_CARD, BORDER_SUBTLE, TEXT_SECONDARY),
            }
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: text,
            border: Border {
                color: border,
                width: 1.0,
                radius: 5.0.into(),
            },
            shadow: Shadow::default(),
            snap: true,
        }
    }
}

pub fn sidebar_item_button(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let (bg, border) = if selected {
            (ACCENT_CYAN_DIM, ACCENT_CYAN)
        } else {
            match status {
                button::Status::Hovered => (BG_CARD_HOVER, BORDER_ACTIVE),
                _ => (BG_CARD, BORDER_SUBTLE),
            }
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: TEXT_PRIMARY,
            border: Border {
                color: border,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: Shadow::default(),
            snap: true,
        }
    }
}

pub fn ghost_button(_theme: &Theme, status: button::Status) -> button::Style {
    let (bg, text) = match status {
        button::Status::Hovered => (BG_ELEVATED, TEXT_PRIMARY),
        button::Status::Pressed => (BG_CARD_HOVER, ACCENT_CYAN),
        _ => (Color::TRANSPARENT, TEXT_SECONDARY),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: text,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 5.0.into(),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

pub fn primary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Color::from_rgb(0.15, 0.88, 1.0),
        button::Status::Pressed => Color::from_rgb(0.0, 0.72, 0.88),
        _ => ACCENT_CYAN,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: BG_BASE,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 6.0.into(),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

pub fn search_input_style(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Focused { .. } => ACCENT_CYAN,
        text_input::Status::Hovered => BORDER_ACTIVE,
        _ => BORDER_SUBTLE,
    };
    text_input::Style {
        background: Background::Color(BG_BASE),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 6.0.into(),
        },
        icon: TEXT_MUTED,
        placeholder: TEXT_MUTED,
        value: TEXT_PRIMARY,
        selection: ACCENT_CYAN_DIM,
    }
}

pub fn subtle_scrollbar(_theme: &Theme, _status: scrollable::Status) -> scrollable::Style {
    let rail = scrollable::Rail {
        background: Some(Background::Color(Color::TRANSPARENT)),
        border: Border::default(),
        scroller: scrollable::Scroller {
            background: Background::Color(BORDER_ACTIVE),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 4.0.into(),
            },
        },
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: Background::Color(BG_CARD),
            border: Border::default(),
            shadow: Shadow::default(),
            icon: TEXT_SECONDARY,
        },
    }
}
