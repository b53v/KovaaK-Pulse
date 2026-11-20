use iced::widget::canvas::{self, Action, Cache, Event, Frame, Geometry, LineDash, Path, Stroke, Style, Text};
use iced::widget::text;
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, alignment, mouse};

use crate::model::PerfTick;
use crate::ui::charts::draw_tooltip;
use crate::ui::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PerSecondBarMode {
    #[default]
    Score,
    Kps,
    DamageEff,
}

impl PerSecondBarMode {
    pub const ALL: &'static [PerSecondBarMode] = &[
        PerSecondBarMode::Score,
        PerSecondBarMode::Kps,
        PerSecondBarMode::DamageEff,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PerSecondBarMode::Score => "得分",
            PerSecondBarMode::Kps => "KPS",
            PerSecondBarMode::DamageEff => "效率%",
        }
    }
}

pub struct PerSecondChart<'a> {
    pub ticks: &'a [PerfTick],
    pub mode: PerSecondBarMode,
    pub avg_accuracy: f64,
    pub cache: &'a Cache,
}

#[derive(Default)]
pub struct PerSecondState {
    hovered_index: Option<usize>,
}

impl<'a, Message> canvas::Program<Message> for PerSecondChart<'a> {
    type State = PerSecondState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        if let Event::Mouse(mouse::Event::CursorMoved { .. }) = event {
            let new_hover = compute_hover_index(self.ticks.len(), bounds, cursor);
            if new_hover != state.hovered_index {
                state.hovered_index = new_hover;
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

        let mut overlay_frame = Frame::new(renderer, bounds.size());
        if let Some(idx) = state.hovered_index.or_else(|| compute_hover_index(self.ticks.len(), bounds, cursor)) {
            if let Some(tick) = self.ticks.get(idx) {
                self.draw_hover(&mut overlay_frame, bounds.size(), idx, tick);
            }
        }

        vec![base, overlay_frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) && !self.ticks.is_empty() {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

const PAD_LEFT: f32 = 38.0;
const PAD_RIGHT: f32 = 34.0;
const PAD_TOP: f32 = 18.0;
const PAD_BOTTOM: f32 = 22.0;

fn compute_hover_index(len: usize, bounds: Rectangle, cursor: mouse::Cursor) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let pos = cursor.position_in(bounds)?;
    let plot_w = (bounds.width - PAD_LEFT - PAD_RIGHT).max(1.0);
    if pos.x < PAD_LEFT || pos.x > PAD_LEFT + plot_w || pos.y < PAD_TOP || pos.y > bounds.height - PAD_BOTTOM {
        return None;
    }
    let step = plot_w / len as f32;
    let idx = ((pos.x - PAD_LEFT) / step).floor() as usize;
    Some(idx.min(len - 1))
}

impl<'a> PerSecondChart<'a> {
    fn bar_value(&self, tick: &PerfTick) -> f64 {
        match self.mode {
            PerSecondBarMode::Score => tick.score,
            PerSecondBarMode::Kps => tick.kills as f64,
            PerSecondBarMode::DamageEff => {
                if tick.damage_possible > 0.0 {
                    (tick.damage_done / tick.damage_possible * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                }
            }
        }
    }

    fn y_range(&self) -> (f64, f64) {
        if self.ticks.is_empty() {
            return (0.0, 100.0);
        }
        match self.mode {
            PerSecondBarMode::DamageEff => (0.0, 100.0),
            PerSecondBarMode::Kps => {
                let max_v = self
                    .ticks
                    .iter()
                    .map(|t| t.kills as f64)
                    .fold(1.0_f64, f64::max);
                (0.0, (max_v * 1.15).ceil().max(2.0))
            }
            PerSecondBarMode::Score => {
                let mut min_v = 0.0_f64;
                let mut max_v = 0.0_f64;
                for t in self.ticks {
                    min_v = min_v.min(t.score);
                    max_v = max_v.max(t.score);
                }
                if (max_v - min_v).abs() < 1e-6 {
                    max_v = min_v + 100.0;
                }
                let pad_top = (max_v.abs() * 0.12).max(10.0);
                let pad_bot = if min_v < 0.0 {
                    (min_v.abs() * 0.15).max(10.0)
                } else {
                    0.0
                };
                (min_v - pad_bot, max_v + pad_top)
            }
        }
    }

    fn draw_base(&self, frame: &mut Frame, size: Size) {
        let plot_w = (size.width - PAD_LEFT - PAD_RIGHT).max(1.0);
        let plot_h = (size.height - PAD_TOP - PAD_BOTTOM).max(1.0);

        if self.ticks.is_empty() {
            frame.fill_text(Text {
                content: "当前对局无 .perf 逐秒采样数据（仅 CSV 摘要模式）".to_string(),
                position: Point::new(size.width * 0.5, size.height * 0.5),
                color: theme::TEXT_MUTED,
                size: 13.0.into(),
                font: theme::FONT_REGULAR,
                align_x: text::Alignment::Center,
                align_y: alignment::Vertical::Center,
                ..Text::default()
            });
            return;
        }

        let (y_min, y_max) = self.y_range();
        let y_span = (y_max - y_min).max(1e-6);
        let map_left_y = |v: f64| -> f32 {
            let norm = ((v - y_min) / y_span) as f32;
            PAD_TOP + plot_h * (1.0 - norm.clamp(0.0, 1.0))
        };
        let map_acc_y = |acc: f64| -> f32 {
            let norm = (acc / 100.0).clamp(0.0, 1.0) as f32;
            PAD_TOP + plot_h * (1.0 - norm)
        };

        // Horizontal grid lines + Left/Right Y-axis labels
        let grid_steps = 4;
        for i in 0..=grid_steps {
            let frac = i as f32 / grid_steps as f32;
            let y = PAD_TOP + plot_h * frac;
            let grid_path = Path::line(Point::new(PAD_LEFT, y), Point::new(PAD_LEFT + plot_w, y));
            frame.stroke(
                &grid_path,
                Stroke::default().with_color(theme::GRID_LINE).with_width(1.0),
            );

            let left_val = y_max - (y_max - y_min) * frac as f64;
            let left_label = match self.mode {
                PerSecondBarMode::Score => format!("{:.0}", left_val),
                PerSecondBarMode::Kps => format!("{:.1}", left_val),
                PerSecondBarMode::DamageEff => format!("{:.0}%", left_val),
            };
            frame.fill_text(Text {
                content: left_label,
                position: Point::new(PAD_LEFT - 6.0, y),
                color: theme::TEXT_MUTED,
                size: 10.5.into(),
                font: theme::FONT_REGULAR,
                align_x: text::Alignment::Right,
                align_y: alignment::Vertical::Center,
                ..Text::default()
            });

            let right_acc = 100.0 * (1.0 - frac as f64);
            frame.fill_text(Text {
                content: format!("{:.0}%", right_acc),
                position: Point::new(PAD_LEFT + plot_w + 6.0, y),
                color: theme::TEXT_MUTED,
                size: 10.0.into(),
                font: theme::FONT_REGULAR,
                align_x: text::Alignment::Left,
                align_y: alignment::Vertical::Center,
                ..Text::default()
            });
        }

        // Zero baseline if negative values exist
        let zero_y = map_left_y(0.0);
        if y_min < 0.0 {
            let zero_path = Path::line(
                Point::new(PAD_LEFT, zero_y),
                Point::new(PAD_LEFT + plot_w, zero_y),
            );
            frame.stroke(
                &zero_path,
                Stroke::default()
                    .with_color(theme::BORDER_ACTIVE)
                    .with_width(1.0),
            );
        }

        // Bars
        let n = self.ticks.len();
        let step = plot_w / n as f32;
        let bar_w = (step * 0.62).clamp(3.0, 22.0);

        for (i, tick) in self.ticks.iter().enumerate() {
            let cx = PAD_LEFT + (i as f32 + 0.5) * step;
            let val = self.bar_value(tick);
            let vy = map_left_y(val);
            let (top, height) = if val >= 0.0 {
                (vy, (zero_y - vy).max(1.5))
            } else {
                (zero_y, (vy - zero_y).max(1.5))
            };

            let bar_color = if val < 0.0 {
                Color { a: 0.80, ..theme::SEMANTIC_RED }
            } else {
                Color { a: 0.65, ..theme::ACCENT_BLUE }
            };

            let rect = Path::rounded_rectangle(
                Point::new(cx - bar_w * 0.5, top),
                Size::new(bar_w, height),
                2.0.into(),
            );
            frame.fill(&rect, bar_color);
        }

        // Average accuracy dashed reference line
        let avg_y = map_acc_y(self.avg_accuracy);
        let avg_line = Path::line(
            Point::new(PAD_LEFT, avg_y),
            Point::new(PAD_LEFT + plot_w, avg_y),
        );
        frame.stroke(
            &avg_line,
            Stroke {
                style: Style::Solid(Color { a: 0.35, ..theme::TEXT_SECONDARY }),
                width: 1.0,
                line_dash: LineDash {
                    segments: &[4.0, 4.0],
                    offset: 0,
                },
                ..Stroke::default()
            },
        );

        // Accuracy polyline (crisp off-white/silver)
        let acc_path = Path::new(|builder| {
            for (i, tick) in self.ticks.iter().enumerate() {
                let cx = PAD_LEFT + (i as f32 + 0.5) * step;
                let ay = map_acc_y(tick.accuracy);
                if i == 0 {
                    builder.move_to(Point::new(cx, ay));
                } else {
                    builder.line_to(Point::new(cx, ay));
                }
            }
        });
        frame.stroke(
            &acc_path,
            Stroke::default()
                .with_color(theme::CHART_LINE_SILVER)
                .with_width(1.8),
        );

        // Subtle dots on accuracy line
        for (i, tick) in self.ticks.iter().enumerate() {
            let cx = PAD_LEFT + (i as f32 + 0.5) * step;
            let ay = map_acc_y(tick.accuracy);
            let dot = Path::circle(Point::new(cx, ay), 2.2);
            frame.fill(&dot, theme::CHART_LINE_SILVER);
        }

        // X-axis tick labels
        let label_every = (n / 6).max(1);
        for (i, tick) in self.ticks.iter().enumerate() {
            if i % label_every == 0 || i == n - 1 {
                let cx = PAD_LEFT + (i as f32 + 0.5) * step;
                frame.fill_text(Text {
                    content: format!("{}s", tick.time_offset_s.round() as i32),
                    position: Point::new(cx, PAD_TOP + plot_h + 6.0),
                    color: theme::TEXT_MUTED,
                    size: 9.5.into(),
                    font: theme::FONT_REGULAR,
                    align_x: text::Alignment::Center,
                    align_y: alignment::Vertical::Top,
                    ..Text::default()
                });
            }
        }

        // Top legend
        frame.fill_text(Text {
            content: format!("■ {}  ● 准度% (均{:.0}%)", self.mode.label(), self.avg_accuracy),
            position: Point::new(PAD_LEFT, 2.0),
            color: theme::TEXT_SECONDARY,
            size: 10.0.into(),
            font: theme::FONT_REGULAR,
            align_x: text::Alignment::Left,
            align_y: alignment::Vertical::Top,
            ..Text::default()
        });
    }

    fn draw_hover(&self, frame: &mut Frame, size: Size, idx: usize, tick: &PerfTick) {
        let plot_w = (size.width - PAD_LEFT - PAD_RIGHT).max(1.0);
        let plot_h = (size.height - PAD_TOP - PAD_BOTTOM).max(1.0);
        let step = plot_w / self.ticks.len() as f32;
        let cx = PAD_LEFT + (idx as f32 + 0.5) * step;

        let crosshair = Path::line(
            Point::new(cx, PAD_TOP),
            Point::new(cx, PAD_TOP + plot_h),
        );
        frame.stroke(
            &crosshair,
            Stroke::default()
                .with_color(theme::CROSSHAIR_LINE)
                .with_width(1.2),
        );

        let acc_y = PAD_TOP + plot_h * (1.0 - (tick.accuracy / 100.0).clamp(0.0, 1.0) as f32);
        let ring = Path::circle(Point::new(cx, acc_y), 4.5);
        frame.fill(&ring, theme::BG_BASE);
        frame.stroke(
            &ring,
            Stroke::default()
                .with_color(theme::CHART_LINE_SILVER)
                .with_width(1.8),
        );

        let score_color = if tick.score < 0.0 {
            theme::SEMANTIC_RED
        } else {
            theme::TEXT_PRIMARY
        };
        let lines = vec![
            (format!("得分: {:+.1}", tick.score), score_color),
            (format!("击杀: {}   命中/开火: {}/{}", tick.kills, tick.shots_hit, tick.shots_fired), theme::TEXT_PRIMARY),
            (format!("准度: {:.1}%   失误: {}", tick.accuracy, tick.shots_missed), theme::TEXT_SECONDARY),
            (format!("过射: {}   伤害: {:.0}/{:.0}", tick.overshots, tick.damage_done, tick.damage_possible), theme::TEXT_MUTED),
        ];

        draw_tooltip(
            frame,
            size,
            Point::new(cx, acc_y),
            &format!("第 {} 秒 (Tick #{})", tick.time_offset_s.round() as i32, tick.tick_index + 1),
            &lines,
        );
    }
}
