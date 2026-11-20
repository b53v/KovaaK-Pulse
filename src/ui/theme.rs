use iced::widget::{button, container, scrollable, text_input};
use iced::{Background, Border, Color, Font, Shadow, Theme, font};

/// Minimalist monochrome-dark palette (Linear / Windows 11 Dark style).
pub const BG_BASE: Color = Color::from_rgb(0x0F as f32 / 255.0, 0x11 as f32 / 255.0, 0x15 as f32 / 255.0);
pub const BG_SURFACE: Color = Color::from_rgb(0x15 as f32 / 255.0, 0x18 as f32 / 255.0, 0x1E as f32 / 255.0);
pub const BG_CARD: Color = Color::from_rgb(0x18 as f32 / 255.0, 0x1C as f32 / 255.0, 0x23 as f32 / 255.0);
pub const BG_CARD_HOVER: Color = Color::from_rgb(0x1E as f32 / 255.0, 0x23 as f32 / 255.0, 0x2D as f32 / 255.0);
pub const BG_ELEVATED: Color = Color::from_rgb(0x22 as f32 / 255.0, 0x27 as f32 / 255.0, 0x31 as f32 / 255.0);

pub const BORDER_SUBTLE: Color = Color::from_rgb(0x25 as f32 / 255.0, 0x2A as f32 / 255.0, 0x34 as f32 / 255.0);
pub const BORDER_ACTIVE: Color = Color::from_rgb(0x36 as f32 / 255.0, 0x3E as f32 / 255.0, 0x4D as f32 / 255.0);

pub const TEXT_PRIMARY: Color = Color::from_rgb(0xF3 as f32 / 255.0, 0xF4 as f32 / 255.0, 0xF6 as f32 / 255.0);
pub const TEXT_SECONDARY: Color = Color::from_rgb(0x9C as f32 / 255.0, 0xA3 as f32 / 255.0, 0xAF as f32 / 255.0);
pub const TEXT_MUTED: Color = Color::from_rgb(0x6B as f32 / 255.0, 0x72 as f32 / 255.0, 0x80 as f32 / 255.0);

/// Single calm primary accent (slate-blue #60A5FA), used sparingly.
pub const ACCENT_BLUE: Color = Color::from_rgb(0x60 as f32 / 255.0, 0xA5 as f32 / 255.0, 0xFA as f32 / 255.0);
pub const ACCENT_BLUE_DIM: Color = Color::from_rgba(0x60 as f32 / 255.0, 0xA5 as f32 / 255.0, 0xFA as f32 / 255.0, 0.16);

/// Restrained semantic colors — strictly for live status, PB/positive delta, and negative/miss/error.
pub const SEMANTIC_GREEN: Color = Color::from_rgb(0x34 as f32 / 255.0, 0xD3 as f32 / 255.0, 0x99 as f32 / 255.0);
pub const SEMANTIC_RED: Color = Color::from_rgb(0xF8 as f32 / 255.0, 0x71 as f32 / 255.0, 0x71 as f32 / 255.0);
pub const SEMANTIC_AMBER: Color = Color::from_rgb(0xD9 as f32 / 255.0, 0x9B as f32 / 255.0, 0x26 as f32 / 255.0);

pub const CHART_LINE_SILVER: Color = Color::from_rgb(0xE5 as f32 / 255.0, 0xE7 as f32 / 255.0, 0xEB as f32 / 255.0);
pub const GRID_LINE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.055);
pub const CROSSHAIR_LINE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.28);
pub const TOOLTIP_BG: Color = Color::from_rgba(0x11 as f32 / 255.0, 0x13 as f32 / 255.0, 0x18 as f32 / 255.0, 0.96);

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
            primary: ACCENT_BLUE,
            success: SEMANTIC_GREEN,
            warning: SEMANTIC_AMBER,
            danger: SEMANTIC_RED,
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

pub fn segmented_track_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_BASE)),
        border: Border {
            color: BORDER_SUBTLE,
            width: 1.0,
            radius: 7.0.into(),
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

pub fn subtle_row_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_BASE)),
        border: Border {
            color: BORDER_SUBTLE,
            width: 1.0,
            radius: 6.0.into(),
        },
        text_color: Some(TEXT_PRIMARY),
        ..Default::default()
    }
}

pub fn banner_container(accent: Color) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(Background::Color(BG_CARD)),
        border: Border {
            color: Color { a: 0.45, ..accent },
            width: 1.0,
            radius: 6.0.into(),
        },
        text_color: Some(TEXT_PRIMARY),
        ..Default::default()
    }
}

pub fn tab_button(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let (bg, text) = if active {
            (BG_ELEVATED, TEXT_PRIMARY)
        } else {
            match status {
                button::Status::Hovered => (BG_CARD_HOVER, TEXT_PRIMARY),
                _ => (Color::TRANSPARENT, TEXT_MUTED),
            }
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
}

pub fn pill_button(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let (bg, border, text) = if active {
            (BG_ELEVATED, BORDER_ACTIVE, TEXT_PRIMARY)
        } else {
            match status {
                button::Status::Hovered => (BG_CARD_HOVER, BORDER_ACTIVE, TEXT_SECONDARY),
                _ => (Color::TRANSPARENT, BORDER_SUBTLE, TEXT_MUTED),
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
            (BG_ELEVATED, BORDER_ACTIVE)
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
        button::Status::Pressed => (BG_CARD_HOVER, TEXT_PRIMARY),
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
        button::Status::Hovered => Color::from_rgb(0x75 as f32 / 255.0, 0xB3 as f32 / 255.0, 0xFB as f32 / 255.0),
        button::Status::Pressed => Color::from_rgb(0x4B as f32 / 255.0, 0x91 as f32 / 255.0, 0xEB as f32 / 255.0),
        _ => ACCENT_BLUE,
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
        text_input::Status::Focused { .. } => BORDER_ACTIVE,
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
        selection: ACCENT_BLUE_DIM,
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
