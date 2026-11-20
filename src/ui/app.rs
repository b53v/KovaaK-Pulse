use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::futures::SinkExt;
use iced::widget::canvas::{self, Cache};
use iced::widget::{
    Column, Row, Space, button, column, container, row, scrollable, text, text_input,
};
use iced::{Alignment, Color, Element, Length, Subscription, Task, Theme, window};

use crate::config::{default_db_path, load_config, resolve_kovaak_dirs, save_config};
use crate::model::{
    AppConfig, ConfigGroupStats, OverviewStats, RunDetail, RunRecord, ScenarioSummary, TimeRange,
    TrendAnalysis, TrendMetric, WatcherEvent,
};
use crate::storage::db::Database;
use crate::ui::charts::heatmap::HeatmapChart;
use crate::ui::charts::kill_interval_chart::KillIntervalChart;
use crate::ui::charts::per_second_chart::{PerSecondBarMode, PerSecondChart};
use crate::ui::charts::trend_chart::TrendChart;
use crate::ui::sidebar::{ScenarioSort, view_scenario_picker};
use crate::ui::theme;
use crate::ui::tray::{TrayAction, TrayController};
use crate::watcher::{WatcherConfig, spawn_watcher};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveTab {
    #[default]
    SingleRun,
    LongTermTrend,
    Overview,
}

impl ActiveTab {
    pub const ALL: &'static [ActiveTab] = &[
        ActiveTab::SingleRun,
        ActiveTab::LongTermTrend,
        ActiveTab::Overview,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ActiveTab::SingleRun => "复盘",
            ActiveTab::LongTermTrend => "趋势",
            ActiveTab::Overview => "总览",
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    // Navigation & Scenario Picker
    SelectTab(ActiveTab),
    ToggleScenarioPicker,
    SelectScenario(String),
    SelectRun(i64),
    SearchChanged(String),
    ToggleFavoriteFilter,
    ChangeSort(ScenarioSort),
    ToggleScenarioStar(String),

    // Single-Run Review controls
    SetPerSecondMode(PerSecondBarMode),

    // Long-Term Trend controls
    SetTimeRange(TimeRange),
    SetTrendMetric(TrendMetric),
    SetMaWindow(usize),

    // Overview & Export
    ExportCurrentScenarioCsv,
    JumpToRun { scenario: String, run_id: i64 },

    // Sync & Settings
    TriggerManualSync,
    SyncFinished(Result<(usize, usize, u64), String>),
    ToggleSettings,
    StatsDirInputChanged(String),
    PerfDirInputChanged(String),
    ToggleNotifications,
    ToggleSingleMonitor,
    ToggleMinimizeToTray,
    SaveSettings,
    DismissBanner,

    // Background Watcher & Tray
    Watcher(WatcherEvent),
    TrayTick,
    WindowCloseRequested(window::Id),
}

#[derive(Clone, Hash)]
struct WatcherKey {
    stats_dir: PathBuf,
    perf_dir: PathBuf,
    db_path: PathBuf,
    generation: u64,
}

pub struct App {
    config: AppConfig,
    db_path: PathBuf,
    watched_dirs: Option<(PathBuf, PathBuf)>,
    watcher_generation: u64,

    // UI state
    active_tab: ActiveTab,
    scenario_picker_open: bool,
    search_query: String,
    favorites_only: bool,
    sort_by: ScenarioSort,
    per_second_mode: PerSecondBarMode,
    time_range: TimeRange,
    trend_metric: TrendMetric,
    ma_window: usize,

    // Settings panel
    show_settings: bool,
    stats_dir_input: String,
    perf_dir_input: String,

    // Status & notifications (non-focus-stealing)
    watcher_online: bool,
    watcher_status: String,
    last_ingest_ms: Option<u64>,
    banner: Option<(String, Color)>,
    is_syncing: bool,

    // Loaded data
    scenarios: Vec<ScenarioSummary>,
    selected_scenario: Option<String>,
    scenario_runs: Vec<RunRecord>,
    selected_run_detail: Option<RunDetail>,
    trend_analysis: TrendAnalysis,
    config_groups: Vec<ConfigGroupStats>,
    overview_stats: OverviewStats,

    // Canvas geometry caches
    per_second_cache: Cache,
    kill_interval_cache: Cache,
    trend_cache: Cache,
    heatmap_cache: Cache,

    // System tray
    tray: TrayController,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let config = load_config();
        let db_path = default_db_path();
        let watched_dirs = resolve_kovaak_dirs(&config);
        let ma_window = if matches!(config.default_ma_window, 5 | 10 | 20) {
            config.default_ma_window
        } else {
            10
        };

        let stats_dir_input = config
            .stats_dir
            .clone()
            .or_else(|| watched_dirs.as_ref().map(|(s, _)| s.display().to_string()))
            .unwrap_or_default();
        let perf_dir_input = config
            .performances_dir
            .clone()
            .or_else(|| watched_dirs.as_ref().map(|(_, p)| p.display().to_string()))
            .unwrap_or_default();

        let watcher_status = if watched_dirs.is_some() {
            "监听中".to_string()
        } else {
            "未配置目录".to_string()
        };

        let tray = TrayController::new();

        let mut app = Self {
            config,
            db_path,
            watched_dirs,
            watcher_generation: 0,
            active_tab: ActiveTab::SingleRun,
            scenario_picker_open: false,
            search_query: String::new(),
            favorites_only: false,
            sort_by: ScenarioSort::Recent,
            per_second_mode: PerSecondBarMode::Score,
            time_range: TimeRange::All,
            trend_metric: TrendMetric::Score,
            ma_window,
            show_settings: false,
            stats_dir_input,
            perf_dir_input,
            watcher_online: false,
            watcher_status,
            last_ingest_ms: None,
            banner: None,
            is_syncing: false,
            scenarios: Vec::new(),
            selected_scenario: None,
            scenario_runs: Vec::new(),
            selected_run_detail: None,
            trend_analysis: TrendAnalysis::default(),
            config_groups: Vec::new(),
            overview_stats: OverviewStats::default(),
            per_second_cache: Cache::new(),
            kill_interval_cache: Cache::new(),
            trend_cache: Cache::new(),
            heatmap_cache: Cache::new(),
            tray,
        };

        app.reload_all_data(None);
        let sync_task = app.spawn_sync_task();
        (app, sync_task)
    }

    pub fn title(&self) -> String {
        match &self.selected_scenario {
            Some(sc) => format!("KovaaK Pulse — {}", sc),
            None => "KovaaK Pulse".to_string(),
        }
    }

    pub fn theme(&self) -> Theme {
        theme::app_theme()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = Vec::new();

        if let Some((stats_dir, perf_dir)) = &self.watched_dirs {
            let key = WatcherKey {
                stats_dir: stats_dir.clone(),
                perf_dir: perf_dir.clone(),
                db_path: self.db_path.clone(),
                generation: self.watcher_generation,
            };
            subs.push(Subscription::run_with(key, watcher_stream));
        }

        if self.tray.is_active() {
            subs.push(
                iced::time::every(Duration::from_millis(250)).map(|_| Message::TrayTick),
            );
        }

        subs.push(window::close_requests().map(Message::WindowCloseRequested));

        Subscription::batch(subs)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectTab(tab) => {
                self.active_tab = tab;
                self.scenario_picker_open = false;
                if tab == ActiveTab::Overview {
                    self.reload_overview();
                } else if tab == ActiveTab::LongTermTrend {
                    self.reload_trend_data();
                }
                Task::none()
            }
            Message::ToggleScenarioPicker => {
                self.scenario_picker_open = !self.scenario_picker_open;
                Task::none()
            }
            Message::SelectScenario(scenario) => {
                self.selected_scenario = Some(scenario);
                self.scenario_picker_open = false;
                self.reload_selected_scenario_data(None);
                Task::none()
            }
            Message::SelectRun(run_id) => {
                self.load_run_detail(run_id);
                Task::none()
            }
            Message::SearchChanged(q) => {
                self.search_query = q;
                Task::none()
            }
            Message::ToggleFavoriteFilter => {
                self.favorites_only = !self.favorites_only;
                Task::none()
            }
            Message::ChangeSort(sort) => {
                self.sort_by = sort;
                Task::none()
            }
            Message::ToggleScenarioStar(scenario) => {
                if let Some(pos) = self.config.favorites.iter().position(|s| s == &scenario) {
                    self.config.favorites.remove(pos);
                } else {
                    self.config.favorites.push(scenario);
                }
                let _ = save_config(&self.config);
                self.reload_scenarios_only();
                Task::none()
            }
            Message::SetPerSecondMode(mode) => {
                self.per_second_mode = mode;
                self.per_second_cache.clear();
                Task::none()
            }
            Message::SetTimeRange(range) => {
                self.time_range = range;
                self.reload_trend_data();
                Task::none()
            }
            Message::SetTrendMetric(metric) => {
                self.trend_metric = metric;
                self.reload_trend_data();
                Task::none()
            }
            Message::SetMaWindow(w) => {
                self.ma_window = w;
                self.config.default_ma_window = w;
                let _ = save_config(&self.config);
                self.reload_trend_data();
                Task::none()
            }
            Message::ExportCurrentScenarioCsv => {
                if let Some(scenario) = &self.selected_scenario {
                    let safe_name: String = scenario
                        .chars()
                        .map(|c| {
                            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                                c
                            } else {
                                '_'
                            }
                        })
                        .collect();
                    let export_dir = directories::UserDirs::new()
                        .and_then(|u| u.document_dir().map(|d| d.join("KovaaK-Pulse-Exports")))
                        .unwrap_or_else(|| PathBuf::from("."));
                    let _ = std::fs::create_dir_all(&export_dir);
                    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
                    let out_path = export_dir.join(format!("{}_{}.csv", safe_name.trim(), ts));
                    match Database::open(&self.db_path)
                        .and_then(|db| db.export_scenario_csv(scenario, &out_path))
                    {
                        Ok(count) => {
                            self.banner = Some((
                                format!("已导出 {} 条至 {}", count, out_path.display()),
                                theme::ACCENT_GREEN,
                            ));
                        }
                        Err(e) => {
                            self.banner = Some((format!("导出失败: {}", e), theme::ACCENT_RED));
                        }
                    }
                } else {
                    self.banner = Some(("请先选择一个训练项目".to_string(), theme::ACCENT_AMBER));
                }
                Task::none()
            }
            Message::JumpToRun { scenario, run_id } => {
                self.selected_scenario = Some(scenario);
                self.active_tab = ActiveTab::SingleRun;
                self.scenario_picker_open = false;
                self.reload_selected_scenario_data(Some(run_id));
                Task::none()
            }
            Message::TriggerManualSync => self.spawn_sync_task(),
            Message::SyncFinished(res) => {
                self.is_syncing = false;
                match res {
                    Ok((imported, skipped, elapsed_ms)) => {
                        if imported > 0 {
                            self.reload_all_data(None);
                            self.banner = Some((
                                format!("同步完成：新增 {} 局，跳过 {} 局（{}ms）", imported, skipped, elapsed_ms),
                                theme::ACCENT_CYAN,
                            ));
                        } else if self.scenarios.is_empty() {
                            self.reload_all_data(None);
                        }
                    }
                    Err(e) => {
                        self.banner = Some((format!("同步失败: {}", e), theme::ACCENT_RED));
                    }
                }
                Task::none()
            }
            Message::ToggleSettings => {
                self.show_settings = !self.show_settings;
                Task::none()
            }
            Message::StatsDirInputChanged(v) => {
                self.stats_dir_input = v;
                Task::none()
            }
            Message::PerfDirInputChanged(v) => {
                self.perf_dir_input = v;
                Task::none()
            }
            Message::ToggleNotifications => {
                self.config.notifications_enabled = !self.config.notifications_enabled;
                let _ = save_config(&self.config);
                Task::none()
            }
            Message::ToggleSingleMonitor => {
                self.config.single_monitor_mode = !self.config.single_monitor_mode;
                let _ = save_config(&self.config);
                Task::none()
            }
            Message::ToggleMinimizeToTray => {
                self.config.minimize_to_tray = !self.config.minimize_to_tray;
                let _ = save_config(&self.config);
                Task::none()
            }
            Message::SaveSettings => {
                let s_trim = self.stats_dir_input.trim();
                let p_trim = self.perf_dir_input.trim();
                self.config.stats_dir = if s_trim.is_empty() {
                    None
                } else {
                    Some(s_trim.to_string())
                };
                self.config.performances_dir = if p_trim.is_empty() {
                    None
                } else {
                    Some(p_trim.to_string())
                };
                let _ = save_config(&self.config);
                self.watched_dirs = resolve_kovaak_dirs(&self.config);
                self.watcher_generation = self.watcher_generation.wrapping_add(1);
                self.show_settings = false;
                self.banner = Some(("设置已保存，正在重新监听".to_string(), theme::ACCENT_CYAN));
                self.spawn_sync_task()
            }
            Message::DismissBanner => {
                self.banner = None;
                Task::none()
            }
            Message::Watcher(ev) => match ev {
                WatcherEvent::WatcherReady { .. } => {
                    self.watcher_online = true;
                    self.watcher_status = match self.last_ingest_ms {
                        Some(ms) => format!("{:.2}s", ms as f64 / 1000.0),
                        None => "监听中".to_string(),
                    };
                    Task::none()
                }
                WatcherEvent::SyncCompleted { imported, .. } => {
                    if imported > 0 {
                        self.reload_all_data(None);
                    }
                    Task::none()
                }
                WatcherEvent::RunIngested {
                    run_id,
                    scenario,
                    score,
                    accuracy,
                    kps,
                    is_new_pb,
                    elapsed_ms,
                } => {
                    // Never steal foreground focus over KovaaK's!
                    self.watcher_online = true;
                    self.last_ingest_ms = Some(elapsed_ms);
                    self.watcher_status = format!("{:.2}s", elapsed_ms as f64 / 1000.0);
                    self.selected_scenario = Some(scenario.clone());
                    self.reload_all_data(Some(run_id));

                    let pb_tag = if is_new_pb { " ★PB!" } else { "" };
                    let summary = format!(
                        "{} · {:.1}分 · {:.1}% · {:.2}/s{} ({:.2}s)",
                        scenario,
                        score,
                        accuracy,
                        kps,
                        pb_tag,
                        elapsed_ms as f64 / 1000.0
                    );
                    if self.config.notifications_enabled {
                        self.banner = Some((
                            summary.clone(),
                            if is_new_pb {
                                theme::ACCENT_GREEN
                            } else {
                                theme::ACCENT_CYAN
                            },
                        ));
                    }
                    self.tray.update_tooltip(&format!("KovaaK Pulse — {}", summary));
                    Task::none()
                }
                WatcherEvent::Error(err) => {
                    self.watcher_online = false;
                    self.watcher_status = "异常".to_string();
                    self.banner = Some((format!("监听异常: {}", err), theme::ACCENT_RED));
                    Task::none()
                }
            },
            Message::TrayTick => {
                if let Some(action) = self.tray.poll_action() {
                    match action {
                        TrayAction::ShowWindow => {
                            return window::oldest().and_then(|id| {
                                Task::batch([
                                    window::minimize(id, false),
                                    window::gain_focus(id),
                                ])
                            });
                        }
                        TrayAction::SyncNow => {
                            return self.spawn_sync_task();
                        }
                        TrayAction::Quit => {
                            return iced::exit();
                        }
                    }
                }
                Task::none()
            }
            Message::WindowCloseRequested(id) => {
                if self.config.minimize_to_tray && self.tray.is_active() {
                    window::minimize(id, true)
                } else {
                    iced::exit()
                }
            }
        }
    }

    fn spawn_sync_task(&mut self) -> Task<Message> {
        let Some((stats_dir, perf_dir)) = self.watched_dirs.clone() else {
            return Task::none();
        };
        if self.is_syncing {
            return Task::none();
        }
        self.is_syncing = true;
        let db_path = self.db_path.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let start = Instant::now();
                    let db = Database::open(&db_path)?;
                    let (imported, skipped) = db.sync_directories(&stats_dir, &perf_dir)?;
                    Ok((imported, skipped, start.elapsed().as_millis() as u64))
                })
                .await
                .map_err(|e| format!("后台同步任务失败: {}", e))?
            },
            Message::SyncFinished,
        )
    }

    fn reload_all_data(&mut self, focus_run_id: Option<i64>) {
        self.reload_scenarios_only();
        if self.selected_scenario.is_none() {
            self.selected_scenario = self.scenarios.first().map(|s| s.scenario.clone());
        }
        self.reload_selected_scenario_data(focus_run_id);
        self.reload_overview();
    }

    fn reload_scenarios_only(&mut self) {
        if let Ok(db) = Database::open(&self.db_path) {
            if let Ok(list) = db.list_scenarios(&self.config.favorites) {
                self.scenarios = list;
            }
        }
    }

    fn reload_selected_scenario_data(&mut self, focus_run_id: Option<i64>) {
        let Some(scenario) = self.selected_scenario.clone() else {
            self.scenario_runs.clear();
            self.selected_run_detail = None;
            self.trend_analysis = TrendAnalysis::default();
            self.config_groups.clear();
            self.clear_chart_caches();
            return;
        };

        if let Ok(db) = Database::open(&self.db_path) {
            self.scenario_runs = db.list_runs_for_scenario(&scenario, 25).unwrap_or_default();
            self.selected_run_detail = match focus_run_id {
                Some(id) => db.get_run_detail(id).ok().flatten(),
                None => db.get_latest_run_detail(Some(&scenario)).ok().flatten(),
            };
            self.trend_analysis = db
                .get_trend_analysis(&scenario, self.trend_metric, self.time_range, self.ma_window)
                .unwrap_or_default();
            self.config_groups = db.get_config_groups(&scenario).unwrap_or_default();
        }

        self.clear_chart_caches();
    }

    fn load_run_detail(&mut self, run_id: i64) {
        if let Ok(db) = Database::open(&self.db_path) {
            self.selected_run_detail = db.get_run_detail(run_id).ok().flatten();
        }
        self.per_second_cache.clear();
        self.kill_interval_cache.clear();
    }

    fn reload_trend_data(&mut self) {
        let Some(scenario) = self.selected_scenario.clone() else {
            return;
        };
        if let Ok(db) = Database::open(&self.db_path) {
            self.trend_analysis = db
                .get_trend_analysis(&scenario, self.trend_metric, self.time_range, self.ma_window)
                .unwrap_or_default();
            self.config_groups = db.get_config_groups(&scenario).unwrap_or_default();
        }
        self.trend_cache.clear();
    }

    fn reload_overview(&mut self) {
        if let Ok(db) = Database::open(&self.db_path) {
            self.overview_stats = db.get_overview_stats(126).unwrap_or_default();
        }
        self.heatmap_cache.clear();
    }

    fn clear_chart_caches(&mut self) {
        self.per_second_cache.clear();
        self.kill_interval_cache.clear();
        self.trend_cache.clear();
        self.heatmap_cache.clear();
    }

    pub fn view(&self) -> Element<'_, Message> {
        let header = self.view_compact_header();
        let picker = view_scenario_picker(
            &self.scenarios,
            self.selected_scenario.as_deref(),
            self.scenario_picker_open,
            &self.search_query,
            self.favorites_only,
            self.sort_by,
            Message::ToggleScenarioPicker,
            Message::SearchChanged,
            Message::ToggleFavoriteFilter,
            Message::ChangeSort,
            Message::SelectScenario,
            Message::ToggleScenarioStar,
        );

        let mut body_col = Column::new().spacing(9).width(Length::Fill);

        if self.show_settings {
            body_col = body_col.push(self.view_settings_panel());
        }

        if let Some((msg, accent)) = &self.banner {
            let banner = container(
                row![
                    text(msg).size(11.5).color(theme::TEXT_PRIMARY).width(Length::Fill),
                    button(text("✕").size(11))
                        .padding([2, 6])
                        .style(theme::ghost_button)
                        .on_press(Message::DismissBanner),
                ]
                .align_y(Alignment::Center),
            )
            .padding([6, 9])
            .width(Length::Fill)
            .style(theme::banner_container(*accent));
            body_col = body_col.push(banner);
        }

        let tab_content = match self.active_tab {
            ActiveTab::SingleRun => self.view_single_run_tab(),
            ActiveTab::LongTermTrend => self.view_long_term_trend_tab(),
            ActiveTab::Overview => self.view_overview_tab(),
        };
        body_col = body_col.push(tab_content);

        let scroll_body = scrollable(container(body_col).padding(10).width(Length::Fill))
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new().width(5).scroller_width(5),
            ))
            .style(theme::subtle_scrollbar)
            .width(Length::Fill)
            .height(Length::Fill);

        container(
            column![header, picker, scroll_body]
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::base_container)
        .into()
    }

    fn view_compact_header(&self) -> Element<'_, Message> {
        let dot_color = if self.watcher_online {
            theme::ACCENT_GREEN
        } else {
            theme::ACCENT_AMBER
        };

        let sync_label = if self.is_syncing { "…" } else { "↻" };

        let status_row = row![
            text("◎")
                .size(15)
                .font(theme::FONT_BOLD)
                .color(theme::ACCENT_CYAN),
            text("KovaaK Pulse")
                .size(13.5)
                .font(theme::FONT_BOLD)
                .color(theme::TEXT_PRIMARY),
            container(
                row![
                    text("●").size(8.5).color(dot_color),
                    text(&self.watcher_status).size(10.5).color(theme::TEXT_SECONDARY),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .padding([2, 7])
            .style(theme::badge_container(theme::BG_CARD, theme::BORDER_SUBTLE)),
            Space::new().width(Length::Fill),
            button(text(sync_label).size(12.5))
                .padding([3, 8])
                .style(theme::pill_button(false))
                .on_press(Message::TriggerManualSync),
            button(text("⚙").size(12.5))
                .padding([3, 8])
                .style(theme::pill_button(self.show_settings))
                .on_press(Message::ToggleSettings),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let mut tabs_row = Row::new().spacing(5).width(Length::Fill);
        for tab in ActiveTab::ALL {
            let active = *tab == self.active_tab;
            tabs_row = tabs_row.push(
                button(
                    container(
                        text(tab.label())
                            .size(12)
                            .font(if active { theme::FONT_BOLD } else { theme::FONT_REGULAR }),
                    )
                    .width(Length::Fill)
                    .center_x(Length::Fill),
                )
                .width(Length::FillPortion(1))
                .padding([5, 6])
                .style(theme::tab_button(active))
                .on_press(Message::SelectTab(*tab)),
            );
        }

        container(column![status_row, tabs_row].spacing(7).width(Length::Fill))
            .padding([8, 10])
            .width(Length::Fill)
            .style(theme::topbar_container)
            .into()
    }

    fn view_settings_panel(&self) -> Element<'_, Message> {
        let stats_input = text_input("stats 目录路径...", &self.stats_dir_input)
            .on_input(Message::StatsDirInputChanged)
            .padding([5, 8])
            .size(11.5)
            .style(theme::search_input_style);

        let perf_input = text_input("performances 目录路径...", &self.perf_dir_input)
            .on_input(Message::PerfDirInputChanged)
            .padding([5, 8])
            .size(11.5)
            .style(theme::search_input_style);

        let toggle_row = row![
            button(
                text(if self.config.single_monitor_mode {
                    "✓ 单屏模式"
                } else {
                    "○ 双屏模式"
                })
                .size(11),
            )
            .padding([4, 7])
            .style(theme::pill_button(self.config.single_monitor_mode))
            .on_press(Message::ToggleSingleMonitor),
            button(
                text(if self.config.minimize_to_tray {
                    "✓ 最小化托盘"
                } else {
                    "○ 直接退出"
                })
                .size(11),
            )
            .padding([4, 7])
            .style(theme::pill_button(self.config.minimize_to_tray))
            .on_press(Message::ToggleMinimizeToTray),
            button(
                text(if self.config.notifications_enabled {
                    "✓ 横幅"
                } else {
                    "○ 静默"
                })
                .size(11),
            )
            .padding([4, 7])
            .style(theme::pill_button(self.config.notifications_enabled))
            .on_press(Message::ToggleNotifications),
        ]
        .spacing(5)
        .align_y(Alignment::Center);

        container(
            column![
                text("目录与单屏设置")
                    .size(12.5)
                    .font(theme::FONT_BOLD)
                    .color(theme::ACCENT_CYAN),
                text("Stats 目录:").size(10.5).color(theme::TEXT_MUTED),
                stats_input,
                text("Performances 目录:").size(10.5).color(theme::TEXT_MUTED),
                perf_input,
                toggle_row,
                button(
                    container(text("保存并重新监听").size(12).font(theme::FONT_BOLD))
                        .width(Length::Fill)
                        .center_x(Length::Fill),
                )
                .width(Length::Fill)
                .padding([6, 10])
                .style(theme::primary_button)
                .on_press(Message::SaveSettings),
            ]
            .spacing(6),
        )
        .padding(10)
        .width(Length::Fill)
        .style(theme::card_accent_container(theme::ACCENT_CYAN))
        .into()
    }

    fn view_single_run_tab(&self) -> Element<'_, Message> {
        let Some(detail) = &self.selected_run_detail else {
            return container(
                column![
                    text("暂无对局数据")
                        .size(14)
                        .font(theme::FONT_BOLD)
                        .color(theme::TEXT_PRIMARY),
                    text("完成一局 KovaaK's Challenge 或点击右上角 ↻ 同步本地记录。")
                        .size(12)
                        .color(theme::TEXT_SECONDARY),
                ]
                .spacing(6),
            )
            .padding(16)
            .width(Length::Fill)
            .style(theme::card_container)
            .into();
        };

        let run = &detail.run;

        // 1. Compact 2-column x 3-row KPI Grid
        let delta_text = match detail.delta_vs_last10 {
            Some(d) => format!("{:+.1} vs 近10局", d),
            None => short_timestamp(&run.played_at),
        };
        let delta_color = match detail.delta_vs_last10 {
            Some(d) if d >= 0.0 => theme::ACCENT_GREEN,
            Some(_) => theme::ACCENT_RED,
            None => theme::TEXT_MUTED,
        };
        let pb_badge = if run.is_pb {
            Some(("NEW PB", theme::ACCENT_GREEN))
        } else {
            None
        };
        let cm_str = match run.cm_per_360 {
            Some(cm) => format!("{:.1}cm · {:.0}fps", cm, run.avg_fps),
            None => format!("{:.0} FPS", run.avg_fps),
        };
        let sens_sub = format!("{} {:.2} @ {}DPI", run.sens_scale, run.horiz_sens, run.dpi);

        let kpi_row1 = row![
            compact_kpi_card(
                "得分 SCORE",
                format!("{:.1}", run.score),
                delta_text,
                theme::ACCENT_CYAN,
                delta_color,
                pb_badge,
            ),
            compact_kpi_card(
                "命中率 ACC",
                format!("{:.2}%", run.accuracy),
                format!("{}/{} · 失误 {}", run.hits, run.shots, run.misses),
                theme::ACCENT_GREEN,
                theme::TEXT_SECONDARY,
                None,
            ),
        ]
        .spacing(7)
        .width(Length::Fill);

        let kpi_row2 = row![
            compact_kpi_card(
                "击杀速率 KPS",
                format!("{:.2} /s", run.avg_kps),
                format!("峰值 {:.1}/s · {}杀", run.peak_kps, run.kills),
                theme::ACCENT_CYAN,
                theme::TEXT_SECONDARY,
                None,
            ),
            compact_kpi_card(
                "平均 TTK",
                format!("{:.0} ms", run.avg_ttk_s * 1000.0),
                format!("间隔 {:.0}ms", run.avg_kill_interval_ms),
                theme::ACCENT_AMBER,
                theme::TEXT_SECONDARY,
                None,
            ),
        ]
        .spacing(7)
        .width(Length::Fill);

        let kpi_row3 = row![
            compact_kpi_card(
                "一枪毙命 1-SHOT",
                format!("{:.1}%", run.one_shot_pct),
                format!("{}/{} · 补枪{:.0}ms", run.one_shot_kills, run.kills, run.multi_shot_avg_ttk_ms),
                theme::ACCENT_GREEN,
                theme::TEXT_SECONDARY,
                None,
            ),
            compact_kpi_card(
                "灵敏度 & FPS",
                cm_str,
                sens_sub,
                theme::ACCENT_PURPLE,
                theme::TEXT_SECONDARY,
                None,
            ),
        ]
        .spacing(7)
        .width(Length::Fill);

        // 2. Chart 1: Per-Second Dual-Axis Canvas
        let mut mode_row = Row::new().spacing(4).align_y(Alignment::Center);
        for m in PerSecondBarMode::ALL {
            mode_row = mode_row.push(
                button(text(m.label()).size(10.5))
                    .padding([3, 7])
                    .style(theme::pill_button(*m == self.per_second_mode))
                    .on_press(Message::SetPerSecondMode(*m)),
            );
        }

        let per_second_canvas = canvas::Canvas::new(PerSecondChart {
            ticks: &detail.ticks,
            mode: self.per_second_mode,
            avg_accuracy: run.accuracy,
            cache: &self.per_second_cache,
        })
        .width(Length::Fill)
        .height(Length::Fixed(190.0));

        let mut chart1_col = column![
            row![
                text("逐秒表现")
                    .size(12.5)
                    .font(theme::FONT_BOLD)
                    .color(theme::TEXT_PRIMARY),
                Space::new().width(Length::Fill),
                mode_row,
            ]
            .align_y(Alignment::Center),
            per_second_canvas,
        ]
        .spacing(6);

        let mut callouts = Row::new().spacing(6).align_y(Alignment::Center);
        let mut has_callout = false;
        if let Some(best) = &detail.best_kps_tick {
            has_callout = true;
            callouts = callouts.push(
                container(
                    text(format!("峰值 {}s: {}杀", best.time_offset_s.round() as i32, best.kills))
                        .size(10)
                        .color(theme::ACCENT_GREEN),
                )
                .padding([2, 6])
                .style(theme::badge_container(theme::ACCENT_GREEN_DIM, theme::ACCENT_GREEN)),
            );
        }
        if let Some(worst) = &detail.worst_score_tick {
            if worst.score < 0.0 {
                has_callout = true;
                callouts = callouts.push(
                    container(
                        text(format!("掉分 {}s: {:+.0}", worst.time_offset_s.round() as i32, worst.score))
                            .size(10)
                            .color(theme::ACCENT_RED),
                    )
                    .padding([2, 6])
                    .style(theme::badge_container(theme::ACCENT_RED_DIM, theme::ACCENT_RED)),
                );
            }
        }
        if has_callout {
            chart1_col = chart1_col.push(callouts);
        }

        let chart1_card = container(chart1_col)
            .padding(10)
            .width(Length::Fill)
            .style(theme::card_container);

        // 3. Compact Half-Split Card
        let half_split_card = self.view_half_split_card(detail);

        // 4. Chart 2: Kill Interval Scatter Canvas
        let kill_canvas = canvas::Canvas::new(KillIntervalChart {
            kills: &detail.kills,
            avg_interval_ms: run.avg_kill_interval_ms,
            cache: &self.kill_interval_cache,
        })
        .width(Length::Fill)
        .height(Length::Fixed(175.0));

        let chart2_card = container(
            column![
                row![
                    text("击杀节奏散点")
                        .size(12.5)
                        .font(theme::FONT_BOLD)
                        .color(theme::TEXT_PRIMARY),
                    Space::new().width(Length::Fill),
                    text(format!(
                        "{:.0}/{:.0}/{:.0}ms",
                        run.fastest_kill_interval_ms,
                        run.avg_kill_interval_ms,
                        run.slowest_kill_interval_ms
                    ))
                    .size(10.5)
                    .color(theme::TEXT_MUTED),
                ]
                .align_y(Alignment::Center),
                kill_canvas,
            ]
            .spacing(6),
        )
        .padding(10)
        .width(Length::Fill)
        .style(theme::card_container);

        // 5. Compact Quick Run Selector at bottom
        let mut recent_runs_col = Column::new().spacing(4).width(Length::Fill);
        for r in self.scenario_runs.iter().take(8) {
            let active = r.id == run.id;
            let pb_mark = if r.is_pb { " ★" } else { "" };
            recent_runs_col = recent_runs_col.push(
                button(
                    row![
                        text(short_timestamp(&r.played_at))
                            .size(11)
                            .color(theme::TEXT_SECONDARY)
                            .width(Length::Fill),
                        text(format!("{:.1}{}", r.score, pb_mark))
                            .size(11.5)
                            .font(theme::FONT_BOLD)
                            .color(if r.is_pb {
                                theme::ACCENT_GREEN
                            } else if active {
                                theme::ACCENT_CYAN
                            } else {
                                theme::TEXT_PRIMARY
                            }),
                        text(format!("{:.1}%", r.accuracy))
                            .size(11)
                            .color(theme::TEXT_MUTED),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .width(Length::Fill)
                .padding([5, 8])
                .style(theme::sidebar_item_button(active))
                .on_press(Message::SelectRun(r.id)),
            );
        }

        let runs_card = container(
            column![
                row![
                    text("最近对局快速切换")
                        .size(12.5)
                        .font(theme::FONT_BOLD)
                        .color(theme::TEXT_PRIMARY),
                    Space::new().width(Length::Fill),
                    text(format!("近 {} 局", self.scenario_runs.len().min(8)))
                        .size(10.5)
                        .color(theme::TEXT_MUTED),
                ]
                .align_y(Alignment::Center),
                recent_runs_col,
            ]
            .spacing(6),
        )
        .padding(10)
        .width(Length::Fill)
        .style(theme::card_container);

        column![
            kpi_row1,
            kpi_row2,
            kpi_row3,
            chart1_card,
            half_split_card,
            chart2_card,
            runs_card
        ]
        .spacing(9)
        .width(Length::Fill)
        .into()
    }

    fn view_half_split_card<'a>(&self, detail: &'a RunDetail) -> Element<'a, Message> {
        let content: Element<'a, Message> = match &detail.half_split {
            Some(hs) => {
                let score_c = if hs.score_delta_pct >= 0.0 {
                    theme::ACCENT_GREEN
                } else {
                    theme::ACCENT_RED
                };
                let acc_c = if hs.accuracy_delta_pp >= 0.0 {
                    theme::ACCENT_GREEN
                } else {
                    theme::ACCENT_RED
                };
                let kps_c = if hs.kps_delta >= 0.0 {
                    theme::ACCENT_GREEN
                } else {
                    theme::ACCENT_RED
                };

                column![
                    text("前后半程诊断")
                        .size(12.5)
                        .font(theme::FONT_BOLD)
                        .color(theme::TEXT_PRIMARY),
                    compact_half_row(
                        "得分",
                        format!("{:.0}", hs.first_half_score),
                        format!("{:.0}", hs.second_half_score),
                        format!("{:+.1}%", hs.score_delta_pct),
                        score_c,
                    ),
                    compact_half_row(
                        "准度",
                        format!("{:.1}%", hs.first_half_accuracy),
                        format!("{:.1}%", hs.second_half_accuracy),
                        format!("{:+.1}pp", hs.accuracy_delta_pp),
                        acc_c,
                    ),
                    compact_half_row(
                        "KPS",
                        format!("{:.2}", hs.first_half_kps),
                        format!("{:.2}", hs.second_half_kps),
                        format!("{:+.2}", hs.kps_delta),
                        kps_c,
                    ),
                    container(text(&hs.diagnosis).size(11.5).color(theme::TEXT_PRIMARY))
                        .padding([6, 8])
                        .width(Length::Fill)
                        .style(theme::badge_container(theme::BG_BASE, theme::BORDER_ACTIVE)),
                ]
                .spacing(6)
                .into()
            }
            None => column![
                text("前后半程诊断")
                    .size(12.5)
                    .font(theme::FONT_BOLD)
                    .color(theme::TEXT_PRIMARY),
                text("需要 .perf 逐秒数据或更多击杀记录进行前后半程对比。")
                    .size(11.5)
                    .color(theme::TEXT_MUTED),
            ]
            .spacing(5)
            .into(),
        };

        container(content)
            .padding(10)
            .width(Length::Fill)
            .style(theme::card_container)
            .into()
    }

    fn view_long_term_trend_tab(&self) -> Element<'_, Message> {
        // Filter row 1: TimeRange + MA window
        let mut range_pills = Row::new().spacing(3);
        for r in TimeRange::ALL {
            let short = match r {
                TimeRange::Today => "今日",
                TimeRange::Days7 => "7天",
                TimeRange::Days30 => "30天",
                TimeRange::All => "全部",
            };
            range_pills = range_pills.push(
                button(text(short).size(10.5))
                    .padding([3, 6])
                    .style(theme::pill_button(*r == self.time_range))
                    .on_press(Message::SetTimeRange(*r)),
            );
        }

        let mut ma_pills = Row::new().spacing(3);
        for w in [5usize, 10, 20] {
            ma_pills = ma_pills.push(
                button(text(format!("MA{}", w)).size(10.5))
                    .padding([3, 5])
                    .style(theme::pill_button(w == self.ma_window))
                    .on_press(Message::SetMaWindow(w)),
            );
        }

        // Filter row 2: Metric switcher (short labels so all 5 fit cleanly on 1 line)
        let mut metric_pills = Row::new().spacing(4).width(Length::Fill);
        for m in TrendMetric::ALL {
            let short = match m {
                TrendMetric::Score => "得分",
                TrendMetric::Accuracy => "准度",
                TrendMetric::Kps => "KPS",
                TrendMetric::AvgTtkMs => "TTK",
                TrendMetric::OneShotPct => "1-Shot",
            };
            metric_pills = metric_pills.push(
                button(
                    container(text(short).size(11))
                        .width(Length::Fill)
                        .center_x(Length::Fill),
                )
                .width(Length::FillPortion(1))
                .padding([4, 4])
                .style(theme::pill_button(*m == self.trend_metric))
                .on_press(Message::SetTrendMetric(*m)),
            );
        }

        let controls = container(
            column![
                row![range_pills, Space::new().width(Length::Fill), ma_pills]
                    .align_y(Alignment::Center),
                metric_pills,
            ]
            .spacing(7),
        )
        .padding(9)
        .width(Length::Fill)
        .style(theme::card_container);

        // 2x2 Summary KPI grid
        let a = &self.trend_analysis;
        let unit = self.trend_metric.unit();
        let recent_str = match a.recent_delta_pct {
            Some(d) => format!("{:+.1}%", d),
            None => "—".to_string(),
        };
        let recent_color = match a.recent_delta_pct {
            Some(d) if (d >= 0.0) == self.trend_metric.higher_is_better() => theme::ACCENT_GREEN,
            Some(_) => theme::ACCENT_RED,
            None => theme::TEXT_MUTED,
        };

        let sum_row1 = row![
            compact_kpi_card(
                "区间均值 MEAN",
                format!("{:.1}{}", a.mean, unit),
                format!("{} 局 · σ {:.1}", a.points.len(), a.std_dev),
                theme::ACCENT_CYAN,
                theme::TEXT_SECONDARY,
                None,
            ),
            compact_kpi_card(
                "区间最佳 PB",
                format!("{:.1}{}", a.pb_value, unit),
                format!("最低 {:.1}{}", a.min_val, unit),
                theme::ACCENT_GREEN,
                theme::TEXT_SECONDARY,
                None,
            ),
        ]
        .spacing(7)
        .width(Length::Fill);

        let sum_row2 = row![
            compact_kpi_card(
                "稳定性 CV",
                format!("{:.1}%", a.cv_pct),
                "越低越稳定".to_string(),
                theme::ACCENT_AMBER,
                theme::TEXT_MUTED,
                None,
            ),
            compact_kpi_card(
                "近期趋势 Δ",
                recent_str,
                "后段 vs 前段均值".to_string(),
                recent_color,
                theme::TEXT_MUTED,
                None,
            ),
        ]
        .spacing(7)
        .width(Length::Fill);

        // Trend canvas
        let trend_canvas = canvas::Canvas::new(TrendChart {
            analysis: &self.trend_analysis,
            metric: self.trend_metric,
            ma_window: self.ma_window,
            cache: &self.trend_cache,
        })
        .width(Length::Fill)
        .height(Length::Fixed(210.0));

        let trend_card = container(trend_canvas)
            .padding(10)
            .width(Length::Fill)
            .style(theme::card_container);

        // Stacked Config Group Comparison cards (fits 340px with zero overflow)
        let mut group_col = Column::new().spacing(6).width(Length::Fill);
        if self.config_groups.is_empty() {
            group_col = group_col.push(
                text("暂无不同灵敏度 / FOV 分组数据")
                    .size(11.5)
                    .color(theme::TEXT_MUTED),
            );
        } else {
            for g in &self.config_groups {
                let cm_label = match g.cm_per_360 {
                    Some(cm) => format!("{:.2} cm/360", cm),
                    None => format!("{} {:.2}", g.sens_scale, g.horiz_sens),
                };
                let header = row![
                    text(format!("{} · FOV {:.0}", cm_label, g.fov))
                        .size(11.5)
                        .font(theme::FONT_BOLD)
                        .color(theme::ACCENT_PURPLE)
                        .width(Length::Fill),
                    text(format!("{}局", g.run_count))
                        .size(10.5)
                        .color(theme::TEXT_MUTED),
                ]
                .align_y(Alignment::Center);

                let metrics = row![
                    text(format!("均 {:.1}", g.avg_score))
                        .size(11)
                        .color(theme::ACCENT_CYAN),
                    text(format!("PB {:.1}", g.pb_score))
                        .size(11)
                        .font(theme::FONT_BOLD)
                        .color(theme::ACCENT_GREEN),
                    text(format!("准 {:.1}%", g.avg_accuracy))
                        .size(11)
                        .color(theme::TEXT_PRIMARY),
                    Space::new().width(Length::Fill),
                    text(format!("1S {:.0}%", g.avg_one_shot_pct))
                        .size(11)
                        .color(theme::ACCENT_AMBER),
                ]
                .spacing(8)
                .align_y(Alignment::Center);

                group_col = group_col.push(
                    container(column![header, metrics].spacing(4))
                        .padding([7, 9])
                        .width(Length::Fill)
                        .style(theme::badge_container(theme::BG_BASE, theme::BORDER_SUBTLE)),
                );
            }
        }

        let group_card = container(
            column![
                text("灵敏度 / FOV 配置分组对比")
                    .size(12.5)
                    .font(theme::FONT_BOLD)
                    .color(theme::TEXT_PRIMARY),
                group_col,
            ]
            .spacing(7),
        )
        .padding(10)
        .width(Length::Fill)
        .style(theme::card_container);

        column![controls, sum_row1, sum_row2, trend_card, group_card]
            .spacing(9)
            .width(Length::Fill)
            .into()
    }

    fn view_overview_tab(&self) -> Element<'_, Message> {
        let ov = &self.overview_stats;
        let today_mins = ov.today_duration_s / 60.0;

        // 2x2 Today Summary Grid
        let row1 = row![
            compact_kpi_card(
                "今日训练局数",
                format!("{} 局", ov.today_runs),
                "当日完成对局".to_string(),
                theme::ACCENT_CYAN,
                theme::TEXT_MUTED,
                None,
            ),
            compact_kpi_card(
                "今日有效时长",
                format!("{:.1} 分", today_mins),
                format!("{:.0}s 射击时长", ov.today_duration_s),
                theme::TEXT_PRIMARY,
                theme::TEXT_SECONDARY,
                None,
            ),
        ]
        .spacing(7)
        .width(Length::Fill);

        let row2 = row![
            compact_kpi_card(
                "今日新 PB",
                format!("{} 次", ov.today_new_pbs),
                "单日突破记录".to_string(),
                theme::ACCENT_GREEN,
                theme::TEXT_SECONDARY,
                None,
            ),
            compact_kpi_card(
                "累计归档",
                format!("{} 局", ov.total_runs),
                format!("{} 个项目", ov.total_scenarios),
                theme::ACCENT_AMBER,
                theme::TEXT_SECONDARY,
                None,
            ),
        ]
        .spacing(7)
        .width(Length::Fill);

        // Heatmap card (18 weeks, auto-scaled to fit 340px cleanly)
        let heatmap_canvas = canvas::Canvas::new(HeatmapChart {
            cells: &ov.heatmap,
            cache: &self.heatmap_cache,
        })
        .width(Length::Fill)
        .height(Length::Fixed(155.0));

        let heatmap_card = container(
            column![
                row![
                    text("训练日历热力图 (近18周)")
                        .size(12.5)
                        .font(theme::FONT_BOLD)
                        .color(theme::TEXT_PRIMARY),
                    Space::new().width(Length::Fill),
                    text("悬停看明细").size(10).color(theme::TEXT_MUTED),
                ]
                .align_y(Alignment::Center),
                heatmap_canvas,
            ]
            .spacing(6),
        )
        .padding(10)
        .width(Length::Fill)
        .style(theme::card_container);

        // Recent PB list
        let mut pb_list = Column::new().spacing(5).width(Length::Fill);
        if ov.recent_pbs.is_empty() {
            pb_list = pb_list.push(text("暂无 PB 记录").size(11.5).color(theme::TEXT_MUTED));
        } else {
            for pb in ov.recent_pbs.iter().take(8) {
                pb_list = pb_list.push(
                    button(
                        column![
                            row![
                                text("★").size(11).color(theme::ACCENT_AMBER),
                                text(&pb.scenario)
                                    .size(11.5)
                                    .font(theme::FONT_BOLD)
                                    .color(theme::TEXT_PRIMARY)
                                    .width(Length::Fill),
                                text(format!("{:.1}", pb.score))
                                    .size(12)
                                    .font(theme::FONT_BOLD)
                                    .color(theme::ACCENT_GREEN),
                            ]
                            .spacing(5)
                            .align_y(Alignment::Center),
                            row![
                                text(format!("Acc {:.1}% · KPS {:.2}", pb.accuracy, pb.avg_kps))
                                    .size(10.5)
                                    .color(theme::TEXT_SECONDARY),
                                Space::new().width(Length::Fill),
                                text(short_timestamp(&pb.played_at))
                                    .size(10.5)
                                    .color(theme::TEXT_MUTED),
                            ]
                            .align_y(Alignment::Center),
                        ]
                        .spacing(3),
                    )
                    .width(Length::Fill)
                    .padding([6, 8])
                    .style(theme::sidebar_item_button(false))
                    .on_press(Message::JumpToRun {
                        scenario: pb.scenario.clone(),
                        run_id: pb.id,
                    }),
                );
            }
        }

        let pb_card = container(
            column![
                text("近期突破 PB（点击复盘）")
                    .size(12.5)
                    .font(theme::FONT_BOLD)
                    .color(theme::TEXT_PRIMARY),
                pb_list,
            ]
            .spacing(7),
        )
        .padding(10)
        .width(Length::Fill)
        .style(theme::card_container);

        // CSV Export card
        let selected_label = self.selected_scenario.as_deref().unwrap_or("未选择项目");
        let export_card = container(
            column![
                row![
                    text("导出 CSV")
                        .size(12.5)
                        .font(theme::FONT_BOLD)
                        .color(theme::TEXT_PRIMARY),
                    Space::new().width(Length::Fill),
                    text(selected_label).size(11).color(theme::ACCENT_CYAN),
                ]
                .align_y(Alignment::Center),
                button(
                    container(text("导出当前项目全部历史 CSV").size(12).font(theme::FONT_BOLD))
                        .width(Length::Fill)
                        .center_x(Length::Fill),
                )
                .width(Length::Fill)
                .padding([7, 12])
                .style(theme::primary_button)
                .on_press(Message::ExportCurrentScenarioCsv),
            ]
            .spacing(8),
        )
        .padding(10)
        .width(Length::Fill)
        .style(theme::card_container);

        column![row1, row2, heatmap_card, pb_card, export_card]
            .spacing(9)
            .width(Length::Fill)
            .into()
    }
}

fn compact_kpi_card<'a>(
    label: &'a str,
    value: String,
    subtitle: String,
    value_color: Color,
    sub_color: Color,
    badge: Option<(&'a str, Color)>,
) -> Element<'a, Message> {
    let mut top_row = Row::new()
        .spacing(4)
        .align_y(Alignment::Center)
        .push(text(label).size(10).color(theme::TEXT_MUTED));

    if let Some((badge_text, badge_color)) = badge {
        top_row = top_row.push(Space::new().width(Length::Fill)).push(
            container(
                text(badge_text)
                    .size(9)
                    .font(theme::FONT_BOLD)
                    .color(badge_color),
            )
            .padding([1, 5])
            .style(theme::badge_container(
                Color { a: 0.18, ..badge_color },
                badge_color,
            )),
        );
    }

    container(
        column![
            top_row,
            text(value)
                .size(17.5)
                .font(theme::FONT_BOLD)
                .color(value_color),
            text(subtitle).size(10).color(sub_color),
        ]
        .spacing(3),
    )
    .padding([8, 10])
    .width(Length::FillPortion(1))
    .style(theme::card_container)
    .into()
}

fn compact_half_row<'a>(
    label: &'a str,
    first: String,
    second: String,
    delta: String,
    delta_color: Color,
) -> Element<'a, Message> {
    container(
        row![
            text(label)
                .size(11)
                .color(theme::TEXT_SECONDARY)
                .width(Length::Fixed(34.0)),
            text(format!("前 {}", first))
                .size(11.5)
                .color(theme::TEXT_PRIMARY)
                .width(Length::Fill),
            text(format!("后 {}", second))
                .size(11.5)
                .font(theme::FONT_BOLD)
                .color(theme::TEXT_PRIMARY)
                .width(Length::Fill),
            container(text(delta).size(10.5).font(theme::FONT_BOLD).color(delta_color))
                .padding([2, 6])
                .style(theme::badge_container(
                    Color { a: 0.16, ..delta_color },
                    delta_color,
                )),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([5, 8])
    .width(Length::Fill)
    .style(theme::badge_container(theme::BG_BASE, theme::BORDER_SUBTLE))
    .into()
}

fn short_timestamp(ts: &str) -> String {
    if ts.len() >= 16 {
        ts[5..16].to_string()
    } else {
        ts.to_string()
    }
}

fn watcher_stream(key: &WatcherKey) -> impl iced::futures::Stream<Item = Message> + use<> {
    let cfg = WatcherConfig {
        stats_dir: key.stats_dir.clone(),
        performances_dir: key.perf_dir.clone(),
        db_path: key.db_path.clone(),
        stability_wait_ms: 150,
        perf_fallback_ms: 1200,
    };

    iced::stream::channel(64, async move |mut output| {
        match spawn_watcher(cfg) {
            Ok((mut rx, _handle)) => {
                while let Some(ev) = rx.recv().await {
                    if output.send(Message::Watcher(ev)).await.is_err() {
                        break;
                    }
                }
            }
            Err(err) => {
                let _ = output.send(Message::Watcher(WatcherEvent::Error(err))).await;
                std::future::pending::<()>().await;
            }
        }
    })
}
