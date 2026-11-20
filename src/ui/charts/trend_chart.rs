use iced::widget::canvas::{self, Action, Cache, Event, Frame, Geometry, LineDash, Path, Stroke, Style, Text};
use iced::widget::text;
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, alignment, mouse};

use crate::model::{Language, TrendAnalysis, TrendMetric, TrendPoint};
use crate::ui::charts::draw_tooltip;
use crate::ui::i18n::t;
use crate::ui::theme::{self, UiPalette};

pub struct TrendChart<'a> {
    pub analysis: &'a TrendAnalysis,
    pub metric: TrendMetric,
    pub ma_window: usize,
    pub palette: UiPalette,
    pub lang: Language,
    pub cache: &'a Cache,
}

#[derive(Default)]
pub struct TrendChartState {
    hovered_index: Option<usize>,
}

const PAD_LEFT: f32 = 44.0;
const PAD_RIGHT: f32 = 12.0;
const PAD_TOP: f32 = 20.0;
const PAD_BOTTOM: f32 = 22.0;

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
        let p = self.palette;
        let plot_w = (size.width - PAD_LEFT - PAD_RIGHT).max(1.0);
        let plot_h = (size.height - PAD_TOP - PAD_BOTTOM).max(1.0);

        if self.analysis.points.is_empty() {
            frame.fill_text(Text {
                content: t(self.lang, "该时间范围内暂无对局数据", "No runs in this time range").to_string(),
                position: Point::new(size.width * 0.5, size.height * 0.5),
                color: p.text_muted,
                size: 11.5.into(),
                font: theme::FONT_REGULAR,
                align_x: text::Alignment::Center,
                align_y: alignment::Vertical::Center,
                ..Text::default()
            });
            return;
        }

        let (y_min, y_max) = self.y_bounds();

        let grid_steps = 4;
        for i in 0..=grid_steps {
            let frac = i as f32 / grid_steps as f32;
            let y = PAD_TOP + plot_h * frac;
            let line = Path::line(Point::new(PAD_LEFT, y), Point::new(PAD_LEFT + plot_w, y));
            frame.stroke(
                &line,
                Stroke::default().with_color(p.grid_line).with_width(1.0),
            );
            let v = y_max - (y_max - y_min) * frac as f64;
            frame.fill_text(Text {
                content: self.format_metric_val(v),
                position: Point::new(PAD_LEFT - 6.0, y),
                color: p.text_muted,
                size: 10.0.into(),
                font: theme::FONT_REGULAR,
                align_x: text::Alignment::Right,
                align_y: alignment::Vertical::Center,
                ..Text::default()
            });
        }

        for (i, pt) in self.analysis.points.iter().enumerate() {
            if pt.sens_changed {
                let xy = self.point_xy(size, i, pt.value);
                let vline = Path::line(Point::new(xy.x, PAD_TOP), Point::new(xy.x, PAD_TOP + plot_h));
                frame.stroke(
                    &vline,
                    Stroke {
                        style: Style::Solid(Color { a: 0.48, ..p.semantic_amber }),
                        width: 1.0,
                        line_dash: LineDash {
                            segments: &[3.0, 3.0],
                            offset: 0,
                        },
                        ..Stroke::default()
                    },
                );
                if let Some(cm) = pt.cm_per_360 {
                    frame.fill_text(Text {
                        content: format!("{:.1}cm", cm),
                        position: Point::new(xy.x + 3.0, PAD_TOP + 2.0),
                        color: p.text_muted,
                        size: 9.0.into(),
                        font: theme::FONT_REGULAR,
                        align_x: text::Alignment::Left,
                        align_y: alignment::Vertical::Top,
                        ..Text::default()
                    });
                }
            }
        }

        if self.analysis.points.len() >= 2 {
            let pb_path = Path::new(|b| {
                let mut prev_y = 0.0_f32;
                for (i, pt) in self.analysis.points.iter().enumerate() {
                    let xy = self.point_xy(size, i, pt.pb_envelope);
                    if i == 0 {
                        b.move_to(xy);
                    } else {
                        b.line_to(Point::new(xy.x, prev_y));
                        b.line_to(xy);
                    }
                    prev_y = xy.y;
                }
            });
            frame.stroke(
                &pb_path,
                Stroke {
                    style: Style::Solid(Color { a: 0.55, ..p.text_secondary }),
                    width: 1.2,
                    line_dash: LineDash {
                        segments: &[5.0, 3.0],
                        offset: 0,
                    },
                    ..Stroke::default()
                },
            );
        }

        let ma_path = Path::new(|b| {
            let mut started = false;
            for (i, pt) in self.analysis.points.iter().enumerate() {
                if let Some(ma) = pt.ma_value {
                    let xy = self.point_xy(size, i, ma);
                    if !started {
                        b.move_to(xy);
                        started = true;
                    } else {
                        b.line_to(xy);
                    }
                }
            }
        });
        frame.stroke(
            &ma_path,
            Stroke::default().with_color(p.accent_blue).with_width(2.0),
        );

        for (i, pt) in self.analysis.points.iter().enumerate() {
            let xy = self.point_xy(size, i, pt.value);
            if pt.is_pb {
                let dot = Path::circle(xy, 3.4);
                frame.fill(&dot, p.semantic_green);
            } else {
                let dot = Path::circle(xy, 2.5);
                frame.fill(&dot, Color { a: 0.55, ..p.text_secondary });
            }
        }

        let n = self.analysis.points.len();
        let label_step = (n / 4).max(1);
        for i in (0..n).step_by(label_step) {
            let pt = &self.analysis.points[i];
            let xy = self.point_xy(size, i, pt.value);
            let short_date = if pt.date_key.len() >= 10 {
                &pt.date_key[5..10]
            } else {
                &pt.date_key
            };
            frame.fill_text(Text {
                content: short_date.to_string(),
                position: Point::new(xy.x, PAD_TOP + plot_h + 6.0),
                color: p.text_muted,
                size: 9.5.into(),
                font: theme::FONT_REGULAR,
                align_x: text::Alignment::Center,
                align_y: alignment::Vertical::Top,
                ..Text::default()
            });
        }

        let legend = match self.lang {
            Language::Zh => format!("● 单局  ━ MA{}  -- PB  ┊ cm/360", self.ma_window),
            Language::En => format!("● Run  ━ MA{}  -- PB  ┊ cm/360", self.ma_window),
        };
        frame.fill_text(Text {
            content: legend,
            position: Point::new(PAD_LEFT, 2.0),
            color: p.text_secondary,
            size: 10.0.into(),
            font: theme::FONT_REGULAR,
            align_x: text::Alignment::Left,
            align_y: alignment::Vertical::Top,
            ..Text::default()
        });
    }

    fn draw_hover(&self, frame: &mut Frame, size: Size, idx: usize, pt: &TrendPoint) {
        let p = self.palette;
        let plot_h = (size.height - PAD_TOP - PAD_BOTTOM).max(1.0);
        let xy = self.point_xy(size, idx, pt.value);

        let vline = Path::line(Point::new(xy.x, PAD_TOP), Point::new(xy.x, PAD_TOP + plot_h));
        frame.stroke(
            &vline,
            Stroke::default().with_color(p.crosshair_line).with_width(1.2),
        );

        let ring = Path::circle(xy, 5.5);
        frame.fill(&ring, p.bg_base);
        frame.stroke(
            &ring,
            Stroke::default()
                .with_color(if pt.is_pb { p.semantic_green } else { p.accent_blue })
                .with_width(1.8),
        );

        let mut lines = vec![
            (
                format!(
                    "{}: {}{}",
                    self.metric.label_i18n(self.lang),
                    self.format_metric_val(pt.value),
                    if pt.is_pb { "  [PB]" } else { "" }
                ),
                if pt.is_pb { p.semantic_green } else { p.text_primary },
            ),
            (
                match self.lang {
                    Language::Zh => format!("得分: {:.1}  准度: {:.1}%  KPS: {:.2}", pt.score, pt.accuracy, pt.kps),
                    Language::En => format!("Score: {:.1}  Acc: {:.1}%  KPS: {:.2}", pt.score, pt.accuracy, pt.kps),
                },
                p.text_secondary,
            ),
            (
                format!("TTK: {:.0}ms  1-Shot: {:.1}%", pt.avg_ttk_ms, pt.one_shot_pct),
                p.text_muted,
            ),
        ];

        if let Some(ma) = pt.ma_value {
            lines.push((
                format!("MA{}: {}  PB: {}", self.ma_window, self.format_metric_val(ma), self.format_metric_val(pt.pb_envelope)),
                p.accent_blue,
            ));
        }

        if let Some(cm) = pt.cm_per_360 {
            let tag = if pt.sens_changed {
                t(self.lang, " (变更)", " (changed)")
            } else {
                ""
            };
            lines.push((format!("cm/360: {:.2} cm{}", cm, tag), p.text_muted));
        }

        let title = match self.lang {
            Language::Zh => format!("第 #{} 局  {}", idx + 1, pt.played_at),
            Language::En => format!("Run #{}  {}", idx + 1, pt.played_at),
        };

        draw_tooltip(frame, p, size, xy, &title, &lines);
    }
}
