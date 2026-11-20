use iced::widget::canvas::{self, Action, Cache, Event, Frame, Geometry, LineDash, Path, Stroke, Style, Text};
use iced::widget::text;
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, alignment, mouse};

use crate::model::{KillEvent, Language};
use crate::ui::charts::draw_tooltip;
use crate::ui::i18n::t;
use crate::ui::theme::{self, UiPalette};

pub struct KillIntervalChart<'a> {
    pub kills: &'a [KillEvent],
    pub avg_interval_ms: f64,
    pub palette: UiPalette,
    pub lang: Language,
    pub cache: &'a Cache,
}

#[derive(Default)]
pub struct KillIntervalState {
    hovered_index: Option<usize>,
}

const PAD_LEFT: f32 = 42.0;
const PAD_RIGHT: f32 = 12.0;
const PAD_TOP: f32 = 18.0;
const PAD_BOTTOM: f32 = 22.0;

impl<'a, Message> canvas::Program<Message> for KillIntervalChart<'a> {
    type State = KillIntervalState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        if let Event::Mouse(mouse::Event::CursorMoved { .. }) = event {
            let new_hover = self.find_nearest_kill(bounds, cursor);
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
        if let Some(idx) = state.hovered_index.or_else(|| self.find_nearest_kill(bounds, cursor)) {
            if let Some(kill) = self.kills.get(idx) {
                self.draw_hover(&mut overlay, bounds.size(), idx, kill);
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
        if cursor.is_over(bounds) && !self.kills.is_empty() {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<'a> KillIntervalChart<'a> {
    fn kill_y_ms(kill: &KillEvent) -> f64 {
        kill.interval_ms.unwrap_or(kill.ttk_s * 1000.0).max(0.0)
    }

    fn max_y_ms(&self) -> f64 {
        let raw_max = self
            .kills
            .iter()
            .map(Self::kill_y_ms)
            .fold(self.avg_interval_ms, f64::max);
        (raw_max * 1.15).max(100.0)
    }

    fn point_for(&self, size: Size, idx: usize, kill: &KillEvent) -> Point {
        let plot_w = (size.width - PAD_LEFT - PAD_RIGHT).max(1.0);
        let plot_h = (size.height - PAD_TOP - PAD_BOTTOM).max(1.0);
        let n = self.kills.len().max(1);
        let x = if n == 1 {
            PAD_LEFT + plot_w * 0.5
        } else {
            PAD_LEFT + (idx as f32 / (n - 1) as f32) * plot_w
        };
        let max_y = self.max_y_ms();
        let norm = (Self::kill_y_ms(kill) / max_y).clamp(0.0, 1.0) as f32;
        let y = PAD_TOP + plot_h * (1.0 - norm);
        Point::new(x, y)
    }

    fn find_nearest_kill(&self, bounds: Rectangle, cursor: mouse::Cursor) -> Option<usize> {
        if self.kills.is_empty() {
            return None;
        }
        let pos = cursor.position_in(bounds)?;
        let size = bounds.size();
        if pos.x < PAD_LEFT - 8.0
            || pos.x > size.width - PAD_RIGHT + 8.0
            || pos.y < PAD_TOP - 8.0
            || pos.y > size.height - PAD_BOTTOM + 8.0
        {
            return None;
        }

        let mut best_idx = None;
        let mut best_dist_sq = 24.0_f32 * 24.0;
        for (i, kill) in self.kills.iter().enumerate() {
            let pt = self.point_for(size, i, kill);
            let dx = pt.x - pos.x;
            let dy = pt.y - pos.y;
            let d2 = dx * dx + dy * dy;
            if d2 < best_dist_sq {
                best_dist_sq = d2;
                best_idx = Some(i);
            }
        }
        best_idx
    }

    fn draw_base(&self, frame: &mut Frame, size: Size) {
        let p = self.palette;
        let plot_w = (size.width - PAD_LEFT - PAD_RIGHT).max(1.0);
        let plot_h = (size.height - PAD_TOP - PAD_BOTTOM).max(1.0);

        if self.kills.is_empty() {
            frame.fill_text(Text {
                content: t(
                    self.lang,
                    "当前对局无逐次击杀明细",
                    "No per-kill events in this run",
                )
                .to_string(),
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

        let max_y = self.max_y_ms();
        let grid_steps = 4;
        for i in 0..=grid_steps {
            let frac = i as f32 / grid_steps as f32;
            let y = PAD_TOP + plot_h * frac;
            let line = Path::line(Point::new(PAD_LEFT, y), Point::new(PAD_LEFT + plot_w, y));
            frame.stroke(
                &line,
                Stroke::default().with_color(p.grid_line).with_width(1.0),
            );
            let val = max_y * (1.0 - frac as f64);
            frame.fill_text(Text {
                content: format!("{:.0}ms", val),
                position: Point::new(PAD_LEFT - 6.0, y),
                color: p.text_muted,
                size: 10.0.into(),
                font: theme::FONT_REGULAR,
                align_x: text::Alignment::Right,
                align_y: alignment::Vertical::Center,
                ..Text::default()
            });
        }

        if self.avg_interval_ms > 0.0 {
            let norm = (self.avg_interval_ms / max_y).clamp(0.0, 1.0) as f32;
            let avg_y = PAD_TOP + plot_h * (1.0 - norm);
            let avg_line = Path::line(
                Point::new(PAD_LEFT, avg_y),
                Point::new(PAD_LEFT + plot_w, avg_y),
            );
            frame.stroke(
                &avg_line,
                Stroke {
                    style: Style::Solid(Color { a: 0.35, ..p.text_secondary }),
                    width: 1.0,
                    line_dash: LineDash {
                        segments: &[4.0, 4.0],
                        offset: 0,
                    },
                    ..Stroke::default()
                },
            );
        }

        let connect = Path::new(|b| {
            for (i, k) in self.kills.iter().enumerate() {
                let pt = self.point_for(size, i, k);
                if i == 0 {
                    b.move_to(pt);
                } else {
                    b.line_to(pt);
                }
            }
        });
        frame.stroke(
            &connect,
            Stroke::default().with_color(p.grid_line).with_width(1.0),
        );

        for (i, kill) in self.kills.iter().enumerate() {
            let pt = self.point_for(size, i, kill);
            let color = if kill.is_one_shot {
                Color { a: 0.60, ..p.text_secondary }
            } else {
                p.semantic_red
            };
            let radius = if kill.is_one_shot { 2.8 } else { 4.0 };
            let dot = Path::circle(pt, radius);
            frame.fill(&dot, color);
        }

        let n = self.kills.len();
        let step = (n / 5).max(1);
        for i in (0..n).step_by(step) {
            let pt = self.point_for(size, i, &self.kills[i]);
            frame.fill_text(Text {
                content: format!("#{}", self.kills[i].kill_index),
                position: Point::new(pt.x, PAD_TOP + plot_h + 6.0),
                color: p.text_muted,
                size: 9.5.into(),
                font: theme::FONT_REGULAR,
                align_x: text::Alignment::Center,
                align_y: alignment::Vertical::Top,
                ..Text::default()
            });
        }

        let legend = match self.lang {
            Language::Zh => format!("● 1-Shot  ● 补射  -- 均 {:.0}ms", self.avg_interval_ms),
            Language::En => format!("● 1-Shot  ● Multi  -- Avg {:.0}ms", self.avg_interval_ms),
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

    fn draw_hover(&self, frame: &mut Frame, size: Size, idx: usize, kill: &KillEvent) {
        let p = self.palette;
        let pt = self.point_for(size, idx, kill);
        let color = if kill.is_one_shot {
            p.text_primary
        } else {
            p.semantic_red
        };

        let ring = Path::circle(pt, 6.0);
        frame.stroke(&ring, Stroke::default().with_color(color).with_width(1.8));

        let (title, lines) = match self.lang {
            Language::Zh => {
                let interval_str = match kill.interval_ms {
                    Some(ms) => format!("{:.0} ms", ms),
                    None => "首杀".to_string(),
                };
                let shot_label = if kill.is_one_shot { "一枪秒杀" } else { "多枪补射" };
                (
                    format!("第 #{} 杀 ({})", kill.kill_index, kill.timestamp),
                    vec![
                        (shot_label.to_string(), color),
                        (format!("间隔: {}  TTK: {:.0}ms", interval_str, kill.ttk_s * 1000.0), p.text_primary),
                        (format!("开火/命中: {}/{}  准度: {:.1}%", kill.shots, kill.hits, kill.accuracy), p.text_secondary),
                        (format!("过射: {}  目标: {}", kill.overshots, kill.bot), p.text_muted),
                    ],
                )
            }
            Language::En => {
                let interval_str = match kill.interval_ms {
                    Some(ms) => format!("{:.0} ms", ms),
                    None => "First".to_string(),
                };
                let shot_label = if kill.is_one_shot { "1-Shot Kill" } else { "Multi-Shot" };
                (
                    format!("Kill #{} ({})", kill.kill_index, kill.timestamp),
                    vec![
                        (shot_label.to_string(), color),
                        (format!("Interval: {}  TTK: {:.0}ms", interval_str, kill.ttk_s * 1000.0), p.text_primary),
                        (format!("Shots/Hits: {}/{}  Acc: {:.1}%", kill.shots, kill.hits, kill.accuracy), p.text_secondary),
                        (format!("Overshots: {}  Bot: {}", kill.overshots, kill.bot), p.text_muted),
                    ],
                )
            }
        };

        draw_tooltip(frame, p, size, pt, &title, &lines);
    }
}
