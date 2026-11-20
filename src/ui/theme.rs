use iced::widget::{button, container, scrollable, text_input};
use iced::{Background, Border, Color, Font, Shadow, Theme, font};

use crate::model::ThemeMode;

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

/// Complete palette supporting both Dark (Linear / Win11 Dark) and Light (Win11 Light) modes.
#[derive(Debug, Clone, Copy)]
pub struct UiPalette {
    pub bg_base: Color,
    pub bg_surface: Color,
    pub bg_card: Color,
    pub bg_card_hover: Color,
    pub bg_elevated: Color,
    pub border_subtle: Color,
    pub border_active: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub accent_blue: Color,
    pub accent_blue_dim: Color,
    pub semantic_green: Color,
    pub semantic_red: Color,
    pub semantic_amber: Color,
    pub chart_line_secondary: Color,
    pub grid_line: Color,
    pub crosshair_line: Color,
    pub tooltip_bg: Color,
    pub heatmap_empty: Color,
}

impl UiPalette {
    pub fn from_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Light => Self::light(),
        }
    }

    pub const fn dark() -> Self {
        Self {
            bg_base: Color::from_rgb(0x0F as f32 / 255.0, 0x11 as f32 / 255.0, 0x15 as f32 / 255.0),
            bg_surface: Color::from_rgb(0x15 as f32 / 255.0, 0x18 as f32 / 255.0, 0x1E as f32 / 255.0),
            bg_card: Color::from_rgb(0x18 as f32 / 255.0, 0x1C as f32 / 255.0, 0x23 as f32 / 255.0),
            bg_card_hover: Color::from_rgb(0x1E as f32 / 255.0, 0x23 as f32 / 255.0, 0x2D as f32 / 255.0),
            bg_elevated: Color::from_rgb(0x22 as f32 / 255.0, 0x27 as f32 / 255.0, 0x31 as f32 / 255.0),
            border_subtle: Color::from_rgb(0x25 as f32 / 255.0, 0x2A as f32 / 255.0, 0x34 as f32 / 255.0),
            border_active: Color::from_rgb(0x36 as f32 / 255.0, 0x3E as f32 / 255.0, 0x4D as f32 / 255.0),
            text_primary: Color::from_rgb(0xF3 as f32 / 255.0, 0xF4 as f32 / 255.0, 0xF6 as f32 / 255.0),
            text_secondary: Color::from_rgb(0x9C as f32 / 255.0, 0xA3 as f32 / 255.0, 0xAF as f32 / 255.0),
            text_muted: Color::from_rgb(0x6B as f32 / 255.0, 0x72 as f32 / 255.0, 0x80 as f32 / 255.0),
            accent_blue: Color::from_rgb(0x60 as f32 / 255.0, 0xA5 as f32 / 255.0, 0xFA as f32 / 255.0),
            accent_blue_dim: Color::from_rgba(0x60 as f32 / 255.0, 0xA5 as f32 / 255.0, 0xFA as f32 / 255.0, 0.16),
            semantic_green: Color::from_rgb(0x34 as f32 / 255.0, 0xD3 as f32 / 255.0, 0x99 as f32 / 255.0),
            semantic_red: Color::from_rgb(0xF8 as f32 / 255.0, 0x71 as f32 / 255.0, 0x71 as f32 / 255.0),
            semantic_amber: Color::from_rgb(0xD9 as f32 / 255.0, 0x9B as f32 / 255.0, 0x26 as f32 / 255.0),
            chart_line_secondary: Color::from_rgb(0xE5 as f32 / 255.0, 0xE7 as f32 / 255.0, 0xEB as f32 / 255.0),
            grid_line: Color::from_rgba(1.0, 1.0, 1.0, 0.055),
            crosshair_line: Color::from_rgba(1.0, 1.0, 1.0, 0.28),
            tooltip_bg: Color::from_rgba(0x11 as f32 / 255.0, 0x13 as f32 / 255.0, 0x18 as f32 / 255.0, 0.96),
            heatmap_empty: Color::from_rgb(0x1E as f32 / 255.0, 0x22 as f32 / 255.0, 0x2B as f32 / 255.0),
        }
    }

    pub const fn light() -> Self {
        Self {
            bg_base: Color::from_rgb(0xF4 as f32 / 255.0, 0xF6 as f32 / 255.0, 0xF9 as f32 / 255.0),
            bg_surface: Color::from_rgb(0xFB as f32 / 255.0, 0xFC as f32 / 255.0, 0xFE as f32 / 255.0),
            bg_card: Color::from_rgb(1.0, 1.0, 1.0),
            bg_card_hover: Color::from_rgb(0xF1 as f32 / 255.0, 0xF5 as f32 / 255.0, 0xF9 as f32 / 255.0),
            bg_elevated: Color::from_rgb(0xE8 as f32 / 255.0, 0xEE as f32 / 255.0, 0xF6 as f32 / 255.0),
            border_subtle: Color::from_rgb(0xE2 as f32 / 255.0, 0xE8 as f32 / 255.0, 0xF0 as f32 / 255.0),
            border_active: Color::from_rgb(0xCB as f32 / 255.0, 0xD5 as f32 / 255.0, 0xE1 as f32 / 255.0),
            text_primary: Color::from_rgb(0x11 as f32 / 255.0, 0x18 as f32 / 255.0, 0x27 as f32 / 255.0),
            text_secondary: Color::from_rgb(0x4B as f32 / 255.0, 0x55 as f32 / 255.0, 0x63 as f32 / 255.0),
            text_muted: Color::from_rgb(0x6B as f32 / 255.0, 0x72 as f32 / 255.0, 0x80 as f32 / 255.0),
            accent_blue: Color::from_rgb(0x25 as f32 / 255.0, 0x63 as f32 / 255.0, 0xEB as f32 / 255.0),
            accent_blue_dim: Color::from_rgba(0x25 as f32 / 255.0, 0x63 as f32 / 255.0, 0xEB as f32 / 255.0, 0.14),
            semantic_green: Color::from_rgb(0x05 as f32 / 255.0, 0x96 as f32 / 255.0, 0x69 as f32 / 255.0),
            semantic_red: Color::from_rgb(0xDC as f32 / 255.0, 0x26 as f32 / 255.0, 0x26 as f32 / 255.0),
            semantic_amber: Color::from_rgb(0xB4 as f32 / 255.0, 0x53 as f32 / 255.0, 0x09 as f32 / 255.0),
            chart_line_secondary: Color::from_rgb(0x1F as f32 / 255.0, 0x29 as f32 / 255.0, 0x37 as f32 / 255.0),
            grid_line: Color::from_rgba(0.0, 0.0, 0.0, 0.07),
            crosshair_line: Color::from_rgba(0.0, 0.0, 0.0, 0.25),
            tooltip_bg: Color::from_rgba(1.0, 1.0, 1.0, 0.96),
            heatmap_empty: Color::from_rgb(0xE5 as f32 / 255.0, 0xE7 as f32 / 255.0, 0xEB as f32 / 255.0),
        }
    }
}

pub fn app_theme(mode: ThemeMode) -> Theme {
    let p = UiPalette::from_mode(mode);
    Theme::custom(
        match mode {
            ThemeMode::Dark => "KovaaK Pulse Dark".to_string(),
            ThemeMode::Light => "KovaaK Pulse Light".to_string(),
        },
        iced::theme::Palette {
            background: p.bg_base,
            text: p.text_primary,
            primary: p.accent_blue,
            success: p.semantic_green,
            warning: p.semantic_amber,
            danger: p.semantic_red,
        },
    )
}

pub fn base_container(p: UiPalette) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(Background::Color(p.bg_base)),
        text_color: Some(p.text_primary),
        ..Default::default()
    }
}

pub fn topbar_container(p: UiPalette) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(Background::Color(p.bg_surface)),
        border: Border {
            color: p.border_subtle,
            width: 1.0,
            radius: 0.0.into(),
        },
        text_color: Some(p.text_primary),
        ..Default::default()
    }
}

pub fn segmented_track_container(p: UiPalette) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(Background::Color(p.bg_base)),
        border: Border {
            color: p.border_subtle,
            width: 1.0,
            radius: 7.0.into(),
        },
        text_color: Some(p.text_primary),
        ..Default::default()
    }
}

pub fn card_container(p: UiPalette) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(Background::Color(p.bg_card)),
        border: Border {
            color: p.border_subtle,
            width: 1.0,
            radius: 8.0.into(),
        },
        text_color: Some(p.text_primary),
        ..Default::default()
    }
}

pub fn subtle_row_container(p: UiPalette) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(Background::Color(p.bg_base)),
        border: Border {
            color: p.border_subtle,
            width: 1.0,
            radius: 6.0.into(),
        },
        text_color: Some(p.text_primary),
        ..Default::default()
    }
}

pub fn banner_container(p: UiPalette, accent: Color) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(Background::Color(p.bg_card)),
        border: Border {
            color: Color { a: 0.55, ..accent },
            width: 1.0,
            radius: 6.0.into(),
        },
        text_color: Some(p.text_primary),
        ..Default::default()
    }
}

pub fn tab_button(p: UiPalette, active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let (bg, text) = if active {
            (p.bg_elevated, p.text_primary)
        } else {
            match status {
                button::Status::Hovered => (p.bg_card_hover, p.text_primary),
                _ => (Color::TRANSPARENT, p.text_muted),
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

pub fn pill_button(p: UiPalette, active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let (bg, border, text) = if active {
            (p.bg_elevated, p.border_active, p.text_primary)
        } else {
            match status {
                button::Status::Hovered => (p.bg_card_hover, p.border_active, p.text_secondary),
                _ => (Color::TRANSPARENT, p.border_subtle, p.text_muted),
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

pub fn sidebar_item_button(
    p: UiPalette,
    selected: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let (bg, border) = if selected {
            (p.bg_elevated, p.border_active)
        } else {
            match status {
                button::Status::Hovered => (p.bg_card_hover, p.border_active),
                _ => (p.bg_card, p.border_subtle),
            }
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: p.text_primary,
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

pub fn ghost_button(p: UiPalette) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let (bg, text) = match status {
            button::Status::Hovered => (p.bg_elevated, p.text_primary),
            button::Status::Pressed => (p.bg_card_hover, p.text_primary),
            _ => (Color::TRANSPARENT, p.text_secondary),
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

pub fn primary_button(p: UiPalette) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered => Color { a: 0.9, ..p.accent_blue },
            button::Status::Pressed => Color { a: 0.8, ..p.accent_blue },
            _ => p.accent_blue,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: Color::WHITE,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 6.0.into(),
            },
            shadow: Shadow::default(),
            snap: true,
        }
    }
}

pub fn search_input_style(p: UiPalette) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |_theme: &Theme, status: text_input::Status| {
        let border_color = match status {
            text_input::Status::Focused { .. } => p.border_active,
            text_input::Status::Hovered => p.border_active,
            _ => p.border_subtle,
        };
        text_input::Style {
            background: Background::Color(p.bg_base),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 6.0.into(),
            },
            icon: p.text_muted,
            placeholder: p.text_muted,
            value: p.text_primary,
            selection: p.accent_blue_dim,
        }
    }
}

pub fn subtle_scrollbar(p: UiPalette) -> impl Fn(&Theme, scrollable::Status) -> scrollable::Style {
    move |_theme: &Theme, _status: scrollable::Status| {
        let rail = scrollable::Rail {
            background: Some(Background::Color(Color::TRANSPARENT)),
            border: Border::default(),
            scroller: scrollable::Scroller {
                background: Background::Color(p.border_active),
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
                background: Background::Color(p.bg_card),
                border: Border::default(),
                shadow: Shadow::default(),
                icon: p.text_secondary,
            },
        }
    }
}
