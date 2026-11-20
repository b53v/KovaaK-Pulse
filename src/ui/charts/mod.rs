pub mod heatmap;
pub mod kill_interval_chart;
pub mod per_second_chart;
pub mod trend_chart;

use iced::widget::canvas::{Frame, Path, Stroke, Text};
use iced::widget::text;
use iced::{Color, Point, Size, alignment};

use crate::ui::theme::{self, UiPalette};

/// Shared helper to draw a floating tooltip box that stays inside `bounds`.
pub fn draw_tooltip(
    frame: &mut Frame,
    p: UiPalette,
    bounds: Size,
    anchor: Point,
    title: &str,
    lines: &[(String, Color)],
) {
    let line_h = 14.0_f32;
    let padding = 7.0_f32;
    let max_chars = lines
        .iter()
        .map(|(s, _)| s.chars().count())
        .chain(std::iter::once(title.chars().count()))
        .max()
        .unwrap_or(10);
    let max_allowed_w = (bounds.width - 12.0).max(120.0);
    let box_w = (max_chars as f32 * 6.8 + padding * 2.0).clamp(120.0, max_allowed_w.min(230.0));
    let box_h = padding * 2.0 + 15.0 + (lines.len() as f32 * line_h);

    let mut x = anchor.x + 10.0;
    if x + box_w > bounds.width - 6.0 {
        x = (anchor.x - box_w - 10.0).max(6.0);
    }
    let mut y = anchor.y - box_h * 0.5;
    if y < 6.0 {
        y = 6.0;
    }
    if y + box_h > bounds.height - 6.0 {
        y = (bounds.height - box_h - 6.0).max(6.0);
    }

    let bg_path = Path::rounded_rectangle(Point::new(x, y), Size::new(box_w, box_h), 5.0.into());
    frame.fill(&bg_path, p.tooltip_bg);
    frame.stroke(
        &bg_path,
        Stroke::default()
            .with_color(p.border_active)
            .with_width(1.0),
    );

    frame.fill_text(Text {
        content: title.to_string(),
        position: Point::new(x + padding, y + padding),
        color: p.text_primary,
        size: 10.5.into(),
        font: theme::FONT_BOLD,
        align_x: text::Alignment::Left,
        align_y: alignment::Vertical::Top,
        ..Text::default()
    });

    for (i, (line, color)) in lines.iter().enumerate() {
        frame.fill_text(Text {
            content: line.clone(),
            position: Point::new(x + padding, y + padding + 15.0 + i as f32 * line_h),
            color: *color,
            size: 10.0.into(),
            font: theme::FONT_REGULAR,
            align_x: text::Alignment::Left,
            align_y: alignment::Vertical::Top,
            ..Text::default()
        });
    }
}
