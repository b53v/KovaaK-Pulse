use std::collections::HashMap;

use chrono::{Datelike, Duration, Local, NaiveDate};
use iced::widget::canvas::{self, Action, Cache, Event, Frame, Geometry, Path, Stroke, Text};
use iced::widget::text;
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, alignment, mouse};

use crate::model::DailyHeatmapCell;
use crate::ui::charts::draw_tooltip;
use crate::ui::theme;

pub struct HeatmapChart<'a> {
    pub cells: &'a [DailyHeatmapCell],
    pub cache: &'a Cache,
}

#[derive(Default)]
pub struct HeatmapState {
    hovered_slot: Option<(usize, usize)>, // (week_col, day_row)
}

const WEEKS: usize = 26;
const PAD_LEFT: f32 = 34.0;
const PAD_TOP: f32 = 24.0;
const PAD_RIGHT: f32 = 16.0;
const PAD_BOTTOM: f32 = 24.0;

impl<'a, Message> canvas::Program<Message> for HeatmapChart<'a> {
    type State = HeatmapState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        if let Event::Mouse(mouse::Event::CursorMoved { .. }) = event {
            let slot = hit_test_slot(bounds, cursor);
            if slot != state.hovered_slot {
                state.hovered_slot = slot;
                return Some(Action::request_redraw());
            }
        }
        None
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let base = self.cache.draw(renderer, bounds.size(), |frame| {
            self.draw_base(frame, bounds.size());
        });

        let mut overlay = Frame::new(renderer, bounds.size());
        if let Some((col, row)) = state.hovered_slot.or_else(|| hit_test_slot(bounds, cursor)) {
            self.draw_hover(&mut overlay, bounds.size(), col, row);
        }

        vec![base, overlay.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if hit_test_slot(bounds, cursor).is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

fn grid_metrics(size: Size) -> (f32, f32, f32) {
    let avail_w = (size.width - PAD_LEFT - PAD_RIGHT).max(100.0);
    let avail_h = (size.height - PAD_TOP - PAD_BOTTOM).max(70.0);
    let cell_w = avail_w / WEEKS as f32;
    let cell_h = avail_h / 7.0;
    let step = cell_w.min(cell_h).clamp(10.0, 22.0);
    let gap = (step * 0.2).clamp(2.0, 4.0);
    let cell_size = (step - gap).max(6.0);
    (step, cell_size, gap)
}

fn start_monday() -> NaiveDate {
    let today = Local::now().date_naive();
    let weekday_offset = today.weekday().num_days_from_monday() as i64;
    let this_monday = today - Duration::days(weekday_offset);
    this_monday - Duration::weeks((WEEKS - 1) as i64)
}

fn date_for_slot(col: usize, row: usize) -> NaiveDate {
    start_monday() + Duration::days((col * 7 + row) as i64)
}

fn hit_test_slot(bounds: Rectangle, cursor: mouse::Cursor) -> Option<(usize, usize)> {
    let pos = cursor.position_in(bounds)?;
    let (step, cell_size, _) = grid_metrics(bounds.size());
    let rx = pos.x - PAD_LEFT;
    let ry = pos.y - PAD_TOP;
    if rx < 0.0 || ry < 0.0 {
        return None;
    }
    let col = (rx / step).floor() as usize;
    let row = (ry / step).floor() as usize;
    if col >= WEEKS || row >= 7 {
        return None;
    }
    let in_cell_x = rx - col as f32 * step;
    let in_cell_y = ry - row as f32 * step;
    if in_cell_x <= cell_size && in_cell_y <= cell_size {
        let date = date_for_slot(col, row);
        if date <= Local::now().date_naive() {
            return Some((col, row));
        }
    }
    None
}

fn intensity_color(runs: u32) -> Color {
    match runs {
        0 => Color::from_rgb(0x18 as f32 / 255.0, 0x1E as f32 / 255.0, 0x2C as f32 / 255.0),
        1..=2 => Color::from_rgba(0.0, 0.83, 1.0, 0.25),
        3..=5 => Color::from_rgba(0.0, 0.83, 1.0, 0.48),
        6..=10 => Color::from_rgba(0.0, 0.83, 1.0, 0.74),
        _ => theme::ACCENT_GREEN,
    }
}

impl<'a> HeatmapChart<'a> {
    fn cell_map(&self) -> HashMap<&str, &DailyHeatmapCell> {
        self.cells
            .iter()
            .map(|c| (c.date_key.as_str(), c))
            .collect()
    }

    fn draw_base(&self, frame: &mut Frame, size: Size) {
        let (step, cell_size, _) = grid_metrics(size);
        let map = self.cell_map();
        let today = Local::now().date_naive();

        // Weekday labels on left
        let weekdays = [(0, "一"), (2, "三"), (4, "五"), (6, "日")];
        for (row, label) in weekdays {
            let y = PAD_TOP + row as f32 * step + cell_size * 0.5;
            frame.fill_text(Text {
                content: label.to_string(),
                position: Point::new(PAD_LEFT - 8.0, y),
                color: theme::TEXT_MUTED,
                size: 10.0.into(),
                font: theme::FONT_REGULAR,
                align_x: text::Alignment::Right,
                align_y: alignment::Vertical::Center,
                ..Text::default()
            });
        }

        // Month labels on top + cells
        let mut last_month = 0u32;
        for col in 0..WEEKS {
            let first_day = date_for_slot(col, 0);
            if first_day.month() != last_month {
                last_month = first_day.month();
                let x = PAD_LEFT + col as f32 * step;
                frame.fill_text(Text {
                    content: format!("{}月", last_month),
                    position: Point::new(x, 4.0),
                    color: theme::TEXT_MUTED,
                    size: 10.5.into(),
                    font: theme::FONT_REGULAR,
                    align_x: text::Alignment::Left,
                    align_y: alignment::Vertical::Top,
                    ..Text::default()
                });
            }

            for row in 0..7 {
                let date = date_for_slot(col, row);
                if date > today {
                    continue;
                }
                let key = date.format("%Y-%m-%d").to_string();
                let cell = map.get(key.as_str()).copied();
                let runs = cell.map(|c| c.run_count).unwrap_or(0);
                let has_pb = cell.map(|c| c.new_pbs > 0).unwrap_or(false);

                let x = PAD_LEFT + col as f32 * step;
                let y = PAD_TOP + row as f32 * step;
                let rect = Path::rounded_rectangle(
                    Point::new(x, y),
                    Size::new(cell_size, cell_size),
                    3.0.into(),
                );
                frame.fill(&rect, intensity_color(runs));

                if has_pb {
                    frame.stroke(
                        &rect,
                        Stroke::default()
                            .with_color(theme::ACCENT_AMBER)
                            .with_width(1.3),
                    );
                }
            }
        }

        // Legend at bottom
        let legend_y = PAD_TOP + 7.0 * step + 6.0;
        let levels = [0u32, 1, 4, 8, 12];
        let mut lx = PAD_LEFT;
        frame.fill_text(Text {
            content: "少".to_string(),
            position: Point::new(lx, legend_y + 1.0),
            color: theme::TEXT_MUTED,
            size: 10.0.into(),
            font: theme::FONT_REGULAR,
            align_x: text::Alignment::Left,
            align_y: alignment::Vertical::Top,
            ..Text::default()
        });
        lx += 18.0;
        for lv in levels {
            let r = Path::rounded_rectangle(Point::new(lx, legend_y + 2.0), Size::new(10.0, 10.0), 2.0.into());
            frame.fill(&r, intensity_color(lv));
            lx += 13.0;
        }
        frame.fill_text(Text {
            content: "多   (金框 = 当日刷新 PB)".to_string(),
            position: Point::new(lx + 4.0, legend_y + 1.0),
            color: theme::TEXT_MUTED,
            size: 10.0.into(),
            font: theme::FONT_REGULAR,
            align_x: text::Alignment::Left,
            align_y: alignment::Vertical::Top,
            ..Text::default()
        });
    }

    fn draw_hover(&self, frame: &mut Frame, size: Size, col: usize, row: usize) {
        let (step, cell_size, _) = grid_metrics(size);
        let x = PAD_LEFT + col as f32 * step;
        let y = PAD_TOP + row as f32 * step;

        let highlight = Path::rounded_rectangle(
            Point::new(x - 1.0, y - 1.0),
            Size::new(cell_size + 2.0, cell_size + 2.0),
            3.5.into(),
        );
        frame.stroke(
            &highlight,
            Stroke::default()
                .with_color(theme::TEXT_PRIMARY)
                .with_width(1.6),
        );

        let date = date_for_slot(col, row);
        let key = date.format("%Y-%m-%d").to_string();
        let map = self.cell_map();
        let cell = map.get(key.as_str()).copied();

        let lines = match cell {
            Some(c) if c.run_count > 0 => {
                let mins = c.total_duration_s / 60.0;
                vec![
                    (format!("训练局数: {} 局 ({} 个项目)", c.run_count, c.unique_scenarios), theme::TEXT_PRIMARY),
                    (format!("有效时长: {:.1} 分钟", mins), theme::ACCENT_CYAN),
                    (
                        format!("当日新 PB: {}", c.new_pbs),
                        if c.new_pbs > 0 { theme::ACCENT_AMBER } else { theme::TEXT_MUTED },
                    ),
                ]
            }
            _ => vec![("当日无训练记录".to_string(), theme::TEXT_MUTED)],
        };

        draw_tooltip(
            frame,
            size,
            Point::new(x + cell_size * 0.5, y + cell_size * 0.5),
            &key,
            &lines,
        );
    }
}
