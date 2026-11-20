use iced::widget::canvas::{self, Action, Cache, Event, Frame, Geometry, LineDash, Path, Stroke, Style, Text};
use iced::widget::text;
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, alignment, mouse};

use crate::model::{TrendAnalysis, TrendMetric, TrendPoint};
use crate::ui::charts::draw_tooltip;
use crate::ui::theme;

pub struct TrendChart<'a> {
    pub analysis: &'a TrendAnalysis,
    pub metric: TrendMetric,
    pub ma_window: usize,
    pub cache: &'a Cache,
}

#[derive(Default)]
pub struct TrendChartState {
    hovered_index: Option<usize>,
}

const PAD_LEFT: f32 = 58.0;
const PAD_RIGHT: f32 = 22.0;
const PAD_TOP: f32 = 26.0;
const PAD_BOTTOM: f32 = 30.0;

impl<'a, Message> canvas::Program<Message> for TrendChart<'a> {
    type State = TrendChartState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        if let Event::Mouse(mouse::Event::CursorMoved { .. }) = event {
            let new_hover = self.find_nearest_point(bounds, cursor);
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

        let mut overlay = Frame::new(renderer, bounds.size());
        if let Some(idx) = state.hovered_index.or_else(|| self.find_nearest_point(bounds, cursor)) {
            if let Some(pt) = self.analysis.points.get(idx) {
                self.draw_hover(&mut overlay, bounds.size(), idx, pt);
            }
        }

        vec![base, overlay.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) && !self.analysis.points.is_empty() {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<'a> TrendChart<'a> {
    fn y_bounds(&self) -> (f64, f64) {
        if self.analysis.points.is_empty() {
            return (0.0, 100.0);
        }
        let mut lo = self.analysis.min_val;
        let mut hi = self.analysis.max_val;
        for p in &self.analysis.points {
            lo = lo.min(p.pb_envelope);
            hi = hi.max(p.pb_envelope);
            if let Some(ma) = p.ma_value {
                lo = lo.min(ma);
                hi = hi.max(ma);
            }
        }
        if (hi - lo).abs() < 1e-6 {
            let pad = (hi.abs() * 0.1).max(10.0);
            (lo - pad, hi + pad)
        } else {
            let pad = (hi - lo) * 0.12;
            (lo - pad, hi + pad)
        }
    }

    fn point_xy(&self, size: Size, idx: usize, val: f64) -> Point {
        let plot_w = (size.width - PAD_LEFT - PAD_RIGHT).max(1.0);
        let plot_h = (size.height - PAD_TOP - PAD_BOTTOM).max(1.0);
        let n = self.analysis.points.len().max(1);
        let x = if n == 1 {
            PAD_LEFT + plot_w * 0.5
        } else {
            PAD_LEFT + (idx as f32 / (n - 1) as f32) * plot_w
        };
        let (y_min, y_max) = self.y_bounds();
        let norm = ((val - y_min) / (y_max - y_min).max(1e-6)).clamp(0.0, 1.0) as f32;
        let y = PAD_TOP + plot_h * (1.0 - norm);
        Point::new(x, y)
    }

    fn find_nearest_point(&self, bounds: Rectangle, cursor: mouse::Cursor) -> Option<usize> {
        let n = self.analysis.points.len();
        if n == 0 {
            return None;
        }
        let pos = cursor.position_in(bounds)?;
        let plot_w = (bounds.width - PAD_LEFT - PAD_RIGHT).max(1.0);
        if pos.x < PAD_LEFT - 10.0
            || pos.x > PAD_LEFT + plot_w + 10.0
            || pos.y < PAD_TOP - 8.0
            || pos.y > bounds.height - PAD_BOTTOM + 8.0
        {
            return None;
        }
        if n == 1 {
            return Some(0);
        }
        let frac = ((pos.x - PAD_LEFT) / plot_w).clamp(0.0, 1.0);
        let idx = (frac * (n - 1) as f32).round() as usize;
        Some(idx.min(n - 1))
    }

    fn format_metric_val(&self, v: f64) -> String {
        match self.metric {
            TrendMetric::Score => format!("{:.1}", v),
            TrendMetric::Accuracy | TrendMetric::OneShotPct => format!("{:.1}%", v),
            TrendMetric::Kps => format!("{:.2}", v),
            TrendMetric::AvgTtkMs => format!("{:.0}ms", v),
        }
    }

    fn draw_base(&self, frame: &mut Frame, size: Size) {
        let plot_w = (size.width - PAD_LEFT - PAD_RIGHT).max(1.0);
        let plot_h = (size.height - PAD_TOP - PAD_BOTTOM).max(1.0);

        if self.analysis.points.is_empty() {
            frame.fill_text(Text {
                content: "该时间范围内暂无对局数据".to_string(),
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

        let (y_min, y_max) = self.y_bounds();

        // Horizontal grid & Y labels
        let grid_steps = 4;
        for i in 0..=grid_steps {
            let frac = i as f32 / grid_steps as f32;
            let y = PAD_TOP + plot_h * frac;
            let line = Path::line(Point::new(PAD_LEFT, y), Point::new(PAD_LEFT + plot_w, y));
            frame.stroke(
                &line,
                Stroke::default().with_color(theme::GRID_LINE).with_width(1.0),
            );
            let v = y_max - (y_max - y_min) * frac as f64;
            frame.fill_text(Text {
                content: self.format_metric_val(v),
                position: Point::new(PAD_LEFT - 6.0, y),
                color: theme::TEXT_MUTED,
                size: 10.5.into(),
                font: theme::FONT_REGULAR,
                align_x: text::Alignment::Right,
                align_y: alignment::Vertical::Center,
                ..Text::default()
            });
        }

        // Sensitivity (cm/360) change vertical markers
        for (i, pt) in self.analysis.points.iter().enumerate() {
            if pt.sens_changed {
                let p = self.point_xy(size, i, pt.value);
                let vline = Path::line(Point::new(p.x, PAD_TOP), Point::new(p.x, PAD_TOP + plot_h));
                frame.stroke(
                    &vline,
                    Stroke {
                        style: Style::Solid(Color { a: 0.65, ..theme::ACCENT_PURPLE }),
                        width: 1.2,
                        line_dash: LineDash {
                            segments: &[4.0, 3.0],
                            offset: 0,
                        },
                        ..Stroke::default()
                    },
                );
                if let Some(cm) = pt.cm_per_360 {
                    frame.fill_text(Text {
                        content: format!("{:.1}cm", cm),
                        position: Point::new(p.x + 3.0, PAD_TOP + 2.0),
                        color: theme::ACCENT_PURPLE,
                        size: 9.5.into(),
                        font: theme::FONT_BOLD,
                        align_x: text::Alignment::Left,
                        align_y: alignment::Vertical::Top,
                        ..Text::default()
                    });
                }
            }
        }

        // PB step envelope line
        if self.analysis.points.len() >= 2 {
            let pb_path = Path::new(|b| {
                let mut prev_y = 0.0_f32;
                for (i, pt) in self.analysis.points.iter().enumerate() {
                    let p = self.point_xy(size, i, pt.pb_envelope);
                    if i == 0 {
                        b.move_to(p);
                    } else {
                        // Step horizontally first, then jump vertically
                        b.line_to(Point::new(p.x, prev_y));
                        b.line_to(p);
                    }
                    prev_y = p.y;
                }
            });
            frame.stroke(
                &pb_path,
                Stroke {
                    style: Style::Solid(Color { a: 0.72, ..theme::ACCENT_GREEN }),
                    width: 1.5,
                    line_dash: LineDash {
                        segments: &[6.0, 3.0],
                        offset: 0,
                    },
                    ..Stroke::default()
                },
            );
        }

        // Moving average polyline
        let ma_path = Path::new(|b| {
            let mut started = false;
            for (i, pt) in self.analysis.points.iter().enumerate() {
                if let Some(ma) = pt.ma_value {
                    let p = self.point_xy(size, i, ma);
                    if !started {
                        b.move_to(p);
                        started = true;
                    } else {
                        b.line_to(p);
                    }
                }
            }
        });
        frame.stroke(
            &ma_path,
            Stroke::default()
                .with_color(theme::ACCENT_AMBER)
                .with_width(2.2),
        );

        // Scatter points for each run
        for (i, pt) in self.analysis.points.iter().enumerate() {
            let p = self.point_xy(size, i, pt.value);
            if pt.is_pb {
                let halo = Path::circle(p, 5.5);
                frame.fill(&halo, theme::ACCENT_GREEN_DIM);
                let dot = Path::circle(p, 3.8);
                frame.fill(&dot, theme::ACCENT_GREEN);
            } else {
                let dot = Path::circle(p, 3.0);
                frame.fill(&dot, Color { a: 0.85, ..theme::ACCENT_CYAN });
            }
        }

        // X-axis date labels
        let n = self.analysis.points.len();
        let label_step = (n / 6).max(1);
        for i in (0..n).step_by(label_step) {
            let pt = &self.analysis.points[i];
            let p = self.point_xy(size, i, pt.value);
            let short_date = if pt.date_key.len() >= 10 {
                &pt.date_key[5..10]
            } else {
                &pt.date_key
            };
            frame.fill_text(Text {
                content: format!("#{} {}", i + 1, short_date),
                position: Point::new(p.x, PAD_TOP + plot_h + 8.0),
                color: theme::TEXT_MUTED,
                size: 10.0.into(),
                font: theme::FONT_REGULAR,
                align_x: text::Alignment::Center,
                align_y: alignment::Vertical::Top,
                ..Text::default()
            });
        }

        // Legend
        frame.fill_text(Text {
            content: format!(
                "● 单局 {}   ━ MA{} 均线   --- PB 阶梯线   ┊ cm/360 变更",
                self.metric.label(),
                self.ma_window
            ),
            position: Point::new(PAD_LEFT, 4.0),
            color: theme::TEXT_SECONDARY,
            size: 11.0.into(),
            font: theme::FONT_REGULAR,
            align_x: text::Alignment::Left,
            align_y: alignment::Vertical::Top,
            ..Text::default()
        });
    }

    fn draw_hover(&self, frame: &mut Frame, size: Size, idx: usize, pt: &TrendPoint) {
        let plot_h = (size.height - PAD_TOP - PAD_BOTTOM).max(1.0);
        let p = self.point_xy(size, idx, pt.value);

        let vline = Path::line(Point::new(p.x, PAD_TOP), Point::new(p.x, PAD_TOP + plot_h));
        frame.stroke(
            &vline,
            Stroke::default()
                .with_color(theme::CROSSHAIR_LINE)
                .with_width(1.2),
        );

        let ring = Path::circle(p, 6.0);
        frame.fill(&ring, theme::BG_BASE);
        frame.stroke(
            &ring,
            Stroke::default()
                .with_color(if pt.is_pb { theme::ACCENT_GREEN } else { theme::ACCENT_CYAN })
                .with_width(2.0),
        );

        let mut lines = vec![
            (
                format!(
                    "{}: {}{}",
                    self.metric.label(),
                    self.format_metric_val(pt.value),
                    if pt.is_pb { "  [PB]" } else { "" }
                ),
                if pt.is_pb { theme::ACCENT_GREEN } else { theme::ACCENT_CYAN },
            ),
            (
                format!("得分: {:.1}   准度: {:.1}%   KPS: {:.2}", pt.score, pt.accuracy, pt.kps),
                theme::TEXT_PRIMARY,
            ),
            (
                format!("TTK: {:.0}ms   1-Shot: {:.1}%", pt.avg_ttk_ms, pt.one_shot_pct),
                theme::TEXT_SECONDARY,
            ),
        ];

        if let Some(ma) = pt.ma_value {
            lines.push((
                format!("MA{}: {}   PB 阶梯: {}", self.ma_window, self.format_metric_val(ma), self.format_metric_val(pt.pb_envelope)),
                theme::ACCENT_AMBER,
            ));
        }

        if let Some(cm) = pt.cm_per_360 {
            let tag = if pt.sens_changed { " (灵敏度变更)" } else { "" };
            lines.push((format!("cm/360: {:.2} cm{}", cm, tag), theme::ACCENT_PURPLE));
        }

        draw_tooltip(
            frame,
            size,
            p,
            &format!("第 #{} 局  {}", idx + 1, pt.played_at),
            &lines,
        );
    }
}
