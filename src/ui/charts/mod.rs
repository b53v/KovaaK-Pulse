pub mod heatmap;
pub mod kill_interval_chart;
pub mod per_second_chart;
pub mod trend_chart;

use iced::widget::canvas::{Frame, Path, Stroke, Text};
use iced::widget::text;
use iced::{Color, Point, Size, alignment};

use crate::ui::theme;

/// Shared helper to draw a floating dark tooltip box that stays inside `bounds`.
pub fn draw_tooltip(
    frame: &mut Frame,
    bounds: Size,
    anchor: Point,
    title: &str,
    lines: &[(String, Color)],
) {
    let line_h = 16.0_f32;
    let padding = 10.0_f32;
    let max_chars = lines
        .iter()
        .map(|(s, _)| s.chars().count())
        .chain(std::iter::once(title.chars().count()))
        .max()
        .unwrap_or(12);
    let box_w = (max_chars as f32 * 7.6 + padding * 2.0).clamp(150.0, 290.0);
    let box_h = padding * 2.0 + 18.0 + (lines.len() as f32 * line_h);

    let mut x = anchor.x + 14.0;
    if x + box_w > bounds.width - 8.0 {
        x = (anchor.x - box_w - 14.0).max(8.0);
    }
    let mut y = anchor.y - box_h * 0.5;
    if y < 8.0 {
        y = 8.0;
    }
    if y + box_h > bounds.height - 8.0 {
        y = (bounds.height - box_h - 8.0).max(8.0);
    }

    let bg_path = Path::rounded_rectangle(Point::new(x, y), Size::new(box_w, box_h), 6.0.into());
    frame.fill(&bg_path, theme::TOOLTIP_BG);
    frame.stroke(
        &bg_path,
        Stroke::default()
            .with_color(theme::BORDER_ACTIVE)
            .with_width(1.0),
    );

    frame.fill_text(Text {
        content: title.to_string(),
        position: Point::new(x + padding, y + padding),
        color: theme::ACCENT_CYAN,
        size: 12.0.into(),
        font: theme::FONT_BOLD,
        align_x: text::Alignment::Left,
        align_y: alignment::Vertical::Top,
        ..Text::default()
    });

    for (i, (line, color)) in lines.iter().enumerate() {
        frame.fill_text(Text {
            content: line.clone(),
            position: Point::new(x + padding, y + padding + 18.0 + i as f32 * line_h),
            color: *color,
            size: 11.5.into(),
            font: theme::FONT_REGULAR,
            align_x: text::Alignment::Left,
            align_y: alignment::Vertical::Top,
            ..Text::default()
        });
    }
}
