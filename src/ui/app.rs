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
use crate::ui::sidebar::{ScenarioSort, view_sidebar};
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
            ActiveTab::SingleRun => "单局复盘",
            ActiveTab::LongTermTrend => "长期趋势",
            ActiveTab::Overview => "训练总览",
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    // Navigation & Sidebar
    SelectTab(ActiveTab),
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
            "正在启动实时目录监听...".to_string()
        } else {
            "未自动检测到 KovaaK's 目录，请在右上角「设置」中指定".to_string()
        };

        let tray = TrayController::new();

        let mut app = Self {
            config,
            db_path,
            watched_dirs,
            watcher_generation: 0,
            active_tab: ActiveTab::SingleRun,
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

        // Load existing DB immediately so UI renders in < 20ms
        app.reload_all_data(None);

        // Kick off non-blocking background sync of local KovaaK's files
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
                if tab == ActiveTab::Overview {
                    self.reload_overview();
                } else if tab == ActiveTab::LongTermTrend {
                    self.reload_trend_data();
                }
                Task::none()
            }
            Message::SelectScenario(scenario) => {
                self.selected_scenario = Some(scenario);
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
                        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' })
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
                                format!("已导出 {} 条「{}」记录至 {}", count, scenario, out_path.display()),
                                theme::ACCENT_GREEN,
                            ));
                        }
                        Err(e) => {
                            self.banner = Some((format!("导出失败: {}", e), theme::ACCENT_RED));
                        }
                    }
                } else {
                    self.banner = Some(("请先在左侧选择一个训练项目再导出 CSV".to_string(), theme::ACCENT_AMBER));
                }
                Task::none()
            }
            Message::JumpToRun { scenario, run_id } => {
                self.selected_scenario = Some(scenario);
                self.active_tab = ActiveTab::SingleRun;
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
                                format!("同步完成：新增 {} 局，跳过 {} 局（耗时 {} ms）", imported, skipped, elapsed_ms),
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
                self.banner = Some(("设置已保存，正在重新同步并监听目录".to_string(), theme::ACCENT_CYAN));
                self.spawn_sync_task()
            }
            Message::DismissBanner => {
                self.banner = None;
                Task::none()
            }
            Message::Watcher(ev) => match ev {
                WatcherEvent::WatcherReady { stats_dir, .. } => {
                    self.watcher_online = true;
                    self.watcher_status = format!("实时监听中 ({})", short_path(&stats_dir));
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
                    self.watcher_status = format!(
                        "实时监听中 · 最近出图 {:.2}s",
                        elapsed_ms as f64 / 1000.0
                    );
                    self.selected_scenario = Some(scenario.clone());
                    self.reload_all_data(Some(run_id));

                    let pb_tag = if is_new_pb { " ★ NEW PB!" } else { "" };
                    let summary = format!(
                        "新对局已入库：{} · 得分 {:.1} · 准度 {:.1}% · KPS {:.2}{}（{:.2}s）",
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
                    self.watcher_status = format!("监听异常: {}", err);
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
            self.scenario_runs = db.list_runs_for_scenario(&scenario, 40).unwrap_or_default();
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
            self.overview_stats = db.get_overview_stats(182).unwrap_or_default();
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
        let topbar = self.view_topbar();
        let sidebar = view_sidebar(
            &self.scenarios,
            self.selected_scenario.as_deref(),
            &self.search_query,
            self.favorites_only,
            self.sort_by,
            Message::SearchChanged,
            Message::ToggleFavoriteFilter,
            Message::ChangeSort,
            Message::SelectScenario,
            Message::ToggleScenarioStar,
        );

        let mut main_col = Column::new().spacing(12).width(Length::Fill);

        if self.show_settings {
            main_col = main_col.push(self.view_settings_panel());
        }

        if let Some((msg, accent)) = &self.banner {
            let banner = container(
                row![
                    text(msg).size(12.5).color(theme::TEXT_PRIMARY).width(Length::Fill),
                    button(text("✕").size(12))
                        .padding([2, 8])
                        .style(theme::ghost_button)
                        .on_press(Message::DismissBanner),
                ]
                .align_y(Alignment::Center),
            )
            .padding([8, 12])
            .width(Length::Fill)
            .style(theme::banner_container(*accent));
            main_col = main_col.push(banner);
        }

        let tab_content = match self.active_tab {
            ActiveTab::SingleRun => self.view_single_run_tab(),
            ActiveTab::LongTermTrend => self.view_long_term_trend_tab(),
            ActiveTab::Overview => self.view_overview_tab(),
        };
        main_col = main_col.push(tab_content);

        let content_scroll = scrollable(container(main_col).padding(16).width(Length::Fill))
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new().width(6).scroller_width(6),
            ))
            .style(theme::subtle_scrollbar)
            .width(Length::Fill)
            .height(Length::Fill);

        let body = row![sidebar, content_scroll]
            .width(Length::Fill)
            .height(Length::Fill);

        container(column![topbar, body].width(Length::Fill).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::base_container)
            .into()
    }

    fn view_topbar(&self) -> Element<'_, Message> {
        let dot_color = if self.watcher_online {
            theme::ACCENT_GREEN
        } else {
            theme::ACCENT_AMBER
        };

        let brand = row![
            text("◎")
                .size(18)
                .font(theme::FONT_BOLD)
                .color(theme::ACCENT_CYAN),
            text("KovaaK Pulse")
                .size(16)
                .font(theme::FONT_BOLD)
                .color(theme::TEXT_PRIMARY),
            container(
                row![
                    text("●").size(10).color(dot_color),
                    text(&self.watcher_status).size(11.5).color(theme::TEXT_SECONDARY),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([4, 10])
            .style(theme::badge_container(theme::BG_CARD, theme::BORDER_SUBTLE)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let mut tabs = Row::new().spacing(6).align_y(Alignment::Center);
        for tab in ActiveTab::ALL {
            tabs = tabs.push(
                button(
                    text(tab.label())
                        .size(13)
                        .font(if *tab == self.active_tab {
                            theme::FONT_BOLD
                        } else {
                            theme::FONT_REGULAR
                        }),
                )
                .padding([7, 16])
                .style(theme::tab_button(*tab == self.active_tab))
                .on_press(Message::SelectTab(*tab)),
            );
        }

        let sync_label = if self.is_syncing { "同步中..." } else { "↻ 同步" };
        let actions = row![
            button(text(sync_label).size(12.5))
                .padding([6, 12])
                .style(theme::pill_button(false))
                .on_press(Message::TriggerManualSync),
            button(text("⚙ 设置").size(12.5))
                .padding([6, 12])
                .style(theme::pill_button(self.show_settings))
                .on_press(Message::ToggleSettings),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        container(
            row![
                brand,
                Space::new().width(Length::Fill),
                tabs,
                Space::new().width(Length::Fill),
                actions,
            ]
            .align_y(Alignment::Center)
            .width(Length::Fill),
        )
        .padding([10, 16])
        .width(Length::Fill)
        .style(theme::topbar_container)
        .into()
    }

    fn view_settings_panel(&self) -> Element<'_, Message> {
        let stats_input = text_input("自动检测或手动填写 stats 目录路径...", &self.stats_dir_input)
            .on_input(Message::StatsDirInputChanged)
            .padding([6, 10])
            .size(12.5)
            .style(theme::search_input_style);

        let perf_input = text_input(
            "自动检测或手动填写 performances 目录路径...",
            &self.perf_dir_input,
        )
        .on_input(Message::PerfDirInputChanged)
        .padding([6, 10])
        .size(12.5)
        .style(theme::search_input_style);

        let toggles = row![
            button(
                text(if self.config.single_monitor_mode {
                    "✓ 单屏友好模式 (不抢焦点)"
                } else {
                    "○ 双屏常驻模式"
                })
                .size(12),
            )
            .padding([5, 10])
            .style(theme::pill_button(self.config.single_monitor_mode))
            .on_press(Message::ToggleSingleMonitor),
            button(
                text(if self.config.minimize_to_tray {
                    "✓ 关闭时最小化到托盘"
                } else {
                    "○ 关闭时直接退出"
                })
                .size(12),
            )
            .padding([5, 10])
            .style(theme::pill_button(self.config.minimize_to_tray))
            .on_press(Message::ToggleMinimizeToTray),
            button(
                text(if self.config.notifications_enabled {
                    "✓ 完赛横幅提示"
                } else {
                    "○ 静默入库"
                })
                .size(12),
            )
            .padding([5, 10])
            .style(theme::pill_button(self.config.notifications_enabled))
            .on_press(Message::ToggleNotifications),
            Space::new().width(Length::Fill),
            button(text("保存并重新监听").size(12.5).font(theme::FONT_BOLD))
                .padding([6, 14])
                .style(theme::primary_button)
                .on_press(Message::SaveSettings),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        container(
            column![
                text("目录与单屏偏好设置")
                    .size(13.5)
                    .font(theme::FONT_BOLD)
                    .color(theme::ACCENT_CYAN),
                row![
                    text("Stats 目录:").size(12).color(theme::TEXT_SECONDARY).width(Length::Fixed(105.0)),
                    stats_input,
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                row![
                    text("Performances:").size(12).color(theme::TEXT_SECONDARY).width(Length::Fixed(105.0)),
                    perf_input,
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                toggles,
            ]
            .spacing(10),
        )
        .padding(14)
        .width(Length::Fill)
        .style(theme::card_accent_container(theme::ACCENT_CYAN))
        .into()
    }

    fn view_single_run_tab(&self) -> Element<'_, Message> {
        let Some(detail) = &self.selected_run_detail else {
            return container(
                column![
                    text("暂无可展示的对局数据")
                        .size(16)
                        .font(theme::FONT_BOLD)
                        .color(theme::TEXT_PRIMARY),
                    text("在 KovaaK's 中完成任意一局 Challenge，或点击右上角「同步」导入历史 stats / performances 文件。")
                        .size(13)
                        .color(theme::TEXT_SECONDARY),
                ]
                .spacing(8),
            )
            .padding(28)
            .width(Length::Fill)
            .style(theme::card_container)
            .into();
        };

        let run = &detail.run;

        // 1. Quick run selector bar
        let mut run_pills = Row::new().spacing(6).align_y(Alignment::Center);
        for r in self.scenario_runs.iter().take(12) {
            let active = r.id == run.id;
            let short_ts = if r.played_at.len() >= 16 {
                &r.played_at[5..16]
            } else {
                &r.played_at
            };
            let pb_mark = if r.is_pb { " ★" } else { "" };
            run_pills = run_pills.push(
                button(
                    text(format!("{} · {:.1}{}", short_ts, r.score, pb_mark))
                        .size(11.5)
                        .font(if active { theme::FONT_BOLD } else { theme::FONT_REGULAR }),
                )
                .padding([5, 10])
                .style(theme::pill_button(active))
                .on_press(Message::SelectRun(r.id)),
            );
        }
        let run_selector = scrollable(run_pills)
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new().width(4).scroller_width(4),
            ))
            .style(theme::subtle_scrollbar)
            .width(Length::Fill);

        // 2. Top KPI cards
        let delta_text = match detail.delta_vs_last10 {
            Some(d) => format!("{:+.1} vs 近10局均值", d),
            None => format!("{} · {}", run.mode, run.played_at),
        };
        let delta_color = match detail.delta_vs_last10 {
            Some(d) if d >= 0.0 => theme::ACCENT_GREEN,
            Some(_) => theme::ACCENT_RED,
            None => theme::TEXT_MUTED,
        };

        let score_badge = if run.is_pb {
            Some(("NEW PB", theme::ACCENT_GREEN))
        } else {
            None
        };

        let cm_str = match run.cm_per_360 {
            Some(cm) => format!("{:.2} cm", cm),
            None => "—".to_string(),
        };
        let sens_sub = format!("{} {:.3} @ {} DPI", run.sens_scale, run.horiz_sens, run.dpi);

        let kpi_row = row![
            kpi_card(
                "综合得分 SCORE",
                format!("{:.1}", run.score),
                delta_text,
                theme::ACCENT_CYAN,
                delta_color,
                score_badge,
            ),
            kpi_card(
                "命中率 ACCURACY",
                format!("{:.2}%", run.accuracy),
                format!("命中 {}/{} · 失误 {}", run.hits, run.shots, run.misses),
                theme::ACCENT_GREEN,
                theme::TEXT_SECONDARY,
                None,
            ),
            kpi_card(
                "击杀速率 KPS",
                format!("{:.2} /s", run.avg_kps),
                format!("峰值 {:.1}/s · {} 杀", run.peak_kps, run.kills),
                theme::ACCENT_CYAN,
                theme::TEXT_SECONDARY,
                None,
            ),
            kpi_card(
                "平均 TTK",
                format!("{:.0} ms", run.avg_ttk_s * 1000.0),
                format!("击杀间隔 {:.0} ms", run.avg_kill_interval_ms),
                theme::ACCENT_AMBER,
                theme::TEXT_SECONDARY,
                None,
            ),
            kpi_card(
                "一枪毙命 1-SHOT",
                format!("{:.1}%", run.one_shot_pct),
                format!("{}/{} 杀 · 补枪 {:.0}ms", run.one_shot_kills, run.kills, run.multi_shot_avg_ttk_ms),
                theme::ACCENT_GREEN,
                theme::TEXT_SECONDARY,
                None,
            ),
            kpi_card(
                "灵敏度 cm/360",
                cm_str,
                sens_sub,
                theme::ACCENT_PURPLE,
                theme::TEXT_SECONDARY,
                None,
            ),
            kpi_card(
                "平均帧率 FPS",
                format!("{:.0}", run.avg_fps),
                format!("{} · FOV {:.0}", run.resolution, run.fov),
                theme::TEXT_PRIMARY,
                theme::TEXT_SECONDARY,
                None,
            ),
        ]
        .spacing(10)
        .width(Length::Fill);

        // 3. Chart 1: Per-Second Dual-Axis Canvas
        let mut mode_row = Row::new().spacing(6).align_y(Alignment::Center);
        for m in PerSecondBarMode::ALL {
            mode_row = mode_row.push(
                button(text(m.label()).size(11.5))
                    .padding([4, 10])
                    .style(theme::pill_button(*m == self.per_second_mode))
                    .on_press(Message::SetPerSecondMode(*m)),
            );
        }

        let mut callouts = Row::new().spacing(8).align_y(Alignment::Center);
        if let Some(best) = &detail.best_kps_tick {
            callouts = callouts.push(
                container(
                    text(format!("最高爆发: 第{}s ({}杀 / {:.0}分)", best.time_offset_s.round() as i32, best.kills, best.score))
                        .size(11)
                        .color(theme::ACCENT_GREEN),
                )
                .padding([3, 8])
                .style(theme::badge_container(theme::ACCENT_GREEN_DIM, theme::ACCENT_GREEN)),
            );
        }
        if let Some(worst) = &detail.worst_score_tick {
            if worst.score < 0.0 {
                callouts = callouts.push(
                    container(
                        text(format!("最大掉分秒: 第{}s ({:+.0}分 / {}失误)", worst.time_offset_s.round() as i32, worst.score, worst.shots_missed))
                            .size(11)
                            .color(theme::ACCENT_RED),
                    )
                    .padding([3, 8])
                    .style(theme::badge_container(theme::ACCENT_RED_DIM, theme::ACCENT_RED)),
                );
            }
        }

        let per_second_canvas = canvas::Canvas::new(PerSecondChart {
            ticks: &detail.ticks,
            mode: self.per_second_mode,
            avg_accuracy: run.accuracy,
            cache: &self.per_second_cache,
        })
        .width(Length::Fill)
        .height(Length::Fixed(255.0));

        let chart1_card = container(
            column![
                row![
                    text("逐秒表现双轴图")
                        .size(14)
                        .font(theme::FONT_BOLD)
                        .color(theme::TEXT_PRIMARY),
                    Space::new().width(10),
                    callouts,
                    Space::new().width(Length::Fill),
                    mode_row,
                ]
                .align_y(Alignment::Center),
                per_second_canvas,
            ]
            .spacing(10),
        )
        .padding(14)
        .width(Length::Fill)
        .style(theme::card_container);

        // 4. Bottom row: Chart 2 (Kill Interval Scatter) + Half-Split Diagnosis Card
        let kill_canvas = canvas::Canvas::new(KillIntervalChart {
            kills: &detail.kills,
            avg_interval_ms: run.avg_kill_interval_ms,
            cache: &self.kill_interval_cache,
        })
        .width(Length::Fill)
        .height(Length::Fixed(225.0));

        let chart2_card = container(
            column![
                row![
                    text("击杀节奏散点图")
                        .size(14)
                        .font(theme::FONT_BOLD)
                        .color(theme::TEXT_PRIMARY),
                    Space::new().width(Length::Fill),
                    text(format!(
                        "最快 {:.0}ms · 均值 {:.0}ms · 最慢 {:.0}ms",
                        run.fastest_kill_interval_ms,
                        run.avg_kill_interval_ms,
                        run.slowest_kill_interval_ms
                    ))
                    .size(11.5)
                    .color(theme::TEXT_SECONDARY),
                ]
                .align_y(Alignment::Center),
                kill_canvas,
            ]
            .spacing(10),
        )
        .padding(14)
        .width(Length::FillPortion(3))
        .style(theme::card_container);

        let half_split_card = self.view_half_split_card(detail);

        let bottom_row = row![chart2_card, half_split_card]
            .spacing(12)
            .width(Length::Fill);

        column![run_selector, kpi_row, chart1_card, bottom_row]
            .spacing(12)
            .width(Length::Fill)
            .into()
    }

    fn view_half_split_card<'a>(&self, detail: &'a RunDetail) -> Element<'a, Message> {
        let content: Element<'a, Message> = match &detail.half_split {
            Some(hs) => {
                let score_delta_color = if hs.score_delta_pct >= 0.0 {
                    theme::ACCENT_GREEN
                } else {
                    theme::ACCENT_RED
                };
                let acc_delta_color = if hs.accuracy_delta_pp >= 0.0 {
                    theme::ACCENT_GREEN
                } else {
                    theme::ACCENT_RED
                };
                let kps_delta_color = if hs.kps_delta >= 0.0 {
                    theme::ACCENT_GREEN
                } else {
                    theme::ACCENT_RED
                };

                column![
                    text("前后半程稳定性诊断")
                        .size(14)
                        .font(theme::FONT_BOLD)
                        .color(theme::TEXT_PRIMARY),
                    half_metric_row(
                        "半程得分",
                        format!("{:.0}", hs.first_half_score),
                        format!("{:.0}", hs.second_half_score),
                        format!("{:+.1}%", hs.score_delta_pct),
                        score_delta_color,
                    ),
                    half_metric_row(
                        "半程准度",
                        format!("{:.1}%", hs.first_half_accuracy),
                        format!("{:.1}%", hs.second_half_accuracy),
                        format!("{:+.1}pp", hs.accuracy_delta_pp),
                        acc_delta_color,
                    ),
                    half_metric_row(
                        "半程 KPS",
                        format!("{:.2}/s", hs.first_half_kps),
                        format!("{:.2}/s", hs.second_half_kps),
                        format!("{:+.2}", hs.kps_delta),
                        kps_delta_color,
                    ),
                    container(
                        column![
                            text("自动诊断")
                                .size(11.5)
                                .font(theme::FONT_BOLD)
                                .color(theme::ACCENT_CYAN),
                            text(&hs.diagnosis)
                                .size(12.5)
                                .color(theme::TEXT_PRIMARY),
                        ]
                        .spacing(4),
                    )
                    .padding(10)
                    .width(Length::Fill)
                    .style(theme::badge_container(theme::BG_BASE, theme::BORDER_ACTIVE)),
                ]
                .spacing(12)
                .into()
            }
            None => column![
                text("前后半程稳定性诊断")
                    .size(14)
                    .font(theme::FONT_BOLD)
                    .color(theme::TEXT_PRIMARY),
                text("需要 .perf 逐秒数据或更多击杀记录才能进行前后半程对比。")
                    .size(12.5)
                    .color(theme::TEXT_MUTED),
            ]
            .spacing(10)
            .into(),
        };

        container(content)
            .padding(14)
            .width(Length::FillPortion(2))
            .style(theme::card_container)
            .into()
    }

    fn view_long_term_trend_tab(&self) -> Element<'_, Message> {
        let scenario_title = self
            .selected_scenario
            .as_deref()
            .unwrap_or("未选择项目");

        // Control bar
        let mut range_pills = Row::new().spacing(5);
        for r in TimeRange::ALL {
            range_pills = range_pills.push(
                button(text(r.label()).size(11.5))
                    .padding([5, 10])
                    .style(theme::pill_button(*r == self.time_range))
                    .on_press(Message::SetTimeRange(*r)),
            );
        }

        let mut metric_pills = Row::new().spacing(5);
        for m in TrendMetric::ALL {
            metric_pills = metric_pills.push(
                button(text(m.label()).size(11.5))
                    .padding([5, 10])
                    .style(theme::pill_button(*m == self.trend_metric))
                    .on_press(Message::SetTrendMetric(*m)),
            );
        }

        let mut ma_pills = Row::new().spacing(5);
        for w in [5usize, 10, 20] {
            ma_pills = ma_pills.push(
                button(text(format!("MA{}", w)).size(11.5))
                    .padding([5, 9])
                    .style(theme::pill_button(w == self.ma_window))
                    .on_press(Message::SetMaWindow(w)),
            );
        }

        let controls = container(
            column![
                row![
                    text(format!("{} — 长期趋势", scenario_title))
                        .size(14.5)
                        .font(theme::FONT_BOLD)
                        .color(theme::TEXT_PRIMARY),
                    Space::new().width(Length::Fill),
                    range_pills,
                    Space::new().width(10),
                    ma_pills,
                ]
                .align_y(Alignment::Center),
                metric_pills,
            ]
            .spacing(10),
        )
        .padding(12)
        .width(Length::Fill)
        .style(theme::card_container);

        // Summary KPI strip
        let a = &self.trend_analysis;
        let recent_str = match a.recent_delta_pct {
            Some(d) => format!("{:+.1}%", d),
            None => "—".to_string(),
        };
        let recent_color = match a.recent_delta_pct {
            Some(d) if (d >= 0.0) == self.trend_metric.higher_is_better() => theme::ACCENT_GREEN,
            Some(_) => theme::ACCENT_RED,
            None => theme::TEXT_MUTED,
        };

        let unit = self.trend_metric.unit();
        let summary_row = row![
            kpi_card(
                "样本局数",
                format!("{} 局", a.points.len()),
                format!("时间范围: {}", self.time_range.label()),
                theme::TEXT_PRIMARY,
                theme::TEXT_MUTED,
                None,
            ),
            kpi_card(
                "区间均值 MEAN",
                format!("{:.1}{}", a.mean, unit),
                format!("标准差 σ {:.1}", a.std_dev),
                theme::ACCENT_CYAN,
                theme::TEXT_SECONDARY,
                None,
            ),
            kpi_card(
                "区间最佳 PB",
                format!("{:.1}{}", a.pb_value, unit),
                format!("最低 {:.1}{}", a.min_val, unit),
                theme::ACCENT_GREEN,
                theme::TEXT_SECONDARY,
                None,
            ),
            kpi_card(
                "稳定性系数 CV",
                format!("{:.1}%", a.cv_pct),
                "越低越稳定".to_string(),
                theme::ACCENT_AMBER,
                theme::TEXT_MUTED,
                None,
            ),
            kpi_card(
                "近期变化趋势",
                recent_str,
                "后段 vs 前段均值".to_string(),
                recent_color,
                theme::TEXT_MUTED,
                None,
            ),
        ]
        .spacing(10)
        .width(Length::Fill);

        // Main trend canvas
        let trend_canvas = canvas::Canvas::new(TrendChart {
            analysis: &self.trend_analysis,
            metric: self.trend_metric,
            ma_window: self.ma_window,
            cache: &self.trend_cache,
        })
        .width(Length::Fill)
        .height(Length::Fixed(300.0));

        let trend_card = container(trend_canvas)
            .padding(14)
            .width(Length::Fill)
            .style(theme::card_container);

        // Config Group Comparison Table
        let mut table_col = Column::new().spacing(6).width(Length::Fill);
        table_col = table_col.push(
            row![
                text("灵敏度 / 配置分组 (cm/360 · FOV · 准星)")
                    .size(12)
                    .font(theme::FONT_BOLD)
                    .color(theme::TEXT_SECONDARY)
                    .width(Length::FillPortion(4)),
                text("局数")
                    .size(12)
                    .font(theme::FONT_BOLD)
                    .color(theme::TEXT_SECONDARY)
                    .width(Length::FillPortion(1)),
                text("均分")
                    .size(12)
                    .font(theme::FONT_BOLD)
                    .color(theme::TEXT_SECONDARY)
                    .width(Length::FillPortion(1)),
                text("PB")
                    .size(12)
                    .font(theme::FONT_BOLD)
                    .color(theme::TEXT_SECONDARY)
                    .width(Length::FillPortion(1)),
                text("均准度")
                    .size(12)
                    .font(theme::FONT_BOLD)
                    .color(theme::TEXT_SECONDARY)
                    .width(Length::FillPortion(1)),
                text("均 KPS")
                    .size(12)
                    .font(theme::FONT_BOLD)
                    .color(theme::TEXT_SECONDARY)
                    .width(Length::FillPortion(1)),
                text("1-Shot %")
                    .size(12)
                    .font(theme::FONT_BOLD)
                    .color(theme::TEXT_SECONDARY)
                    .width(Length::FillPortion(1)),
            ]
            .padding([6, 10]),
        );

        if self.config_groups.is_empty() {
            table_col = table_col.push(
                text("暂无不同灵敏度 / FOV 分组数据")
                    .size(12.5)
                    .color(theme::TEXT_MUTED),
            );
        } else {
            for g in &self.config_groups {
                let cm_label = match g.cm_per_360 {
                    Some(cm) => format!("{:.2} cm/360", cm),
                    None => format!("{} {:.3}", g.sens_scale, g.horiz_sens),
                };
                let desc = format!("{}  |  FOV {:.0}  |  {}", cm_label, g.fov, g.crosshair);
                table_col = table_col.push(
                    container(
                        row![
                            text(desc)
                                .size(12)
                                .color(theme::TEXT_PRIMARY)
                                .width(Length::FillPortion(4)),
                            text(format!("{}", g.run_count))
                                .size(12)
                                .color(theme::TEXT_SECONDARY)
                                .width(Length::FillPortion(1)),
                            text(format!("{:.1}", g.avg_score))
                                .size(12)
                                .color(theme::ACCENT_CYAN)
                                .width(Length::FillPortion(1)),
                            text(format!("{:.1}", g.pb_score))
                                .size(12)
                                .font(theme::FONT_BOLD)
                                .color(theme::ACCENT_GREEN)
                                .width(Length::FillPortion(1)),
                            text(format!("{:.1}%", g.avg_accuracy))
                                .size(12)
                                .color(theme::TEXT_PRIMARY)
                                .width(Length::FillPortion(1)),
                            text(format!("{:.2}", g.avg_kps))
                                .size(12)
                                .color(theme::TEXT_PRIMARY)
                                .width(Length::FillPortion(1)),
                            text(format!("{:.1}%", g.avg_one_shot_pct))
                                .size(12)
                                .color(theme::ACCENT_AMBER)
                                .width(Length::FillPortion(1)),
                        ]
                        .align_y(Alignment::Center),
                    )
                    .padding([8, 10])
                    .width(Length::Fill)
                    .style(theme::badge_container(theme::BG_BASE, theme::BORDER_SUBTLE)),
                );
            }
        }

        let group_card = container(
            column![
                text("灵敏度 / FOV / 准星 配置分组对比")
                    .size(14)
                    .font(theme::FONT_BOLD)
                    .color(theme::TEXT_PRIMARY),
                table_col,
            ]
            .spacing(10),
        )
        .padding(14)
        .width(Length::Fill)
        .style(theme::card_container);

        column![controls, summary_row, trend_card, group_card]
            .spacing(12)
            .width(Length::Fill)
            .into()
    }

    fn view_overview_tab(&self) -> Element<'_, Message> {
        let ov = &self.overview_stats;
        let today_mins = ov.today_duration_s / 60.0;

        // Today summary cards
        let summary_row = row![
            kpi_card(
                "今日训练局数",
                format!("{} 局", ov.today_runs),
                "含全部 Challenge / 练习".to_string(),
                theme::ACCENT_CYAN,
                theme::TEXT_MUTED,
                None,
            ),
            kpi_card(
                "今日有效时长",
                format!("{:.1} 分钟", today_mins),
                format!("{:.0} 秒实际射击时长", ov.today_duration_s),
                theme::TEXT_PRIMARY,
                theme::TEXT_SECONDARY,
                None,
            ),
            kpi_card(
                "今日刷新 PB",
                format!("{} 次", ov.today_new_pbs),
                "单日突破记录".to_string(),
                theme::ACCENT_GREEN,
                theme::TEXT_SECONDARY,
                None,
            ),
            kpi_card(
                "累计本地归档",
                format!("{} 局", ov.total_runs),
                format!("覆盖 {} 个训练项目", ov.total_scenarios),
                theme::ACCENT_AMBER,
                theme::TEXT_SECONDARY,
                None,
            ),
        ]
        .spacing(10)
        .width(Length::Fill);

        // GitHub-style Heatmap Canvas
        let heatmap_canvas = canvas::Canvas::new(HeatmapChart {
            cells: &ov.heatmap,
            cache: &self.heatmap_cache,
        })
        .width(Length::Fill)
        .height(Length::Fixed(185.0));

        let heatmap_card = container(
            column![
                row![
                    text("近半年训练日历热力图")
                        .size(14)
                        .font(theme::FONT_BOLD)
                        .color(theme::TEXT_PRIMARY),
                    Space::new().width(Length::Fill),
                    text("悬停查看当日局数、时长与新 PB")
                        .size(11.5)
                        .color(theme::TEXT_MUTED),
                ]
                .align_y(Alignment::Center),
                heatmap_canvas,
            ]
            .spacing(10),
        )
        .padding(14)
        .width(Length::Fill)
        .style(theme::card_container);

        // Recent PB list + CSV export card
        let mut pb_list = Column::new().spacing(6).width(Length::Fill);
        if ov.recent_pbs.is_empty() {
            pb_list = pb_list.push(
                text("暂无 PB 记录").size(12.5).color(theme::TEXT_MUTED),
            );
        } else {
            for pb in ov.recent_pbs.iter().take(10) {
                let row_btn = button(
                    row![
                        text("★").size(12).color(theme::ACCENT_AMBER),
                        text(&pb.scenario)
                            .size(12.5)
                            .font(theme::FONT_BOLD)
                            .color(theme::TEXT_PRIMARY)
                            .width(Length::Fill),
                        text(format!("{:.1}", pb.score))
                            .size(13)
                            .font(theme::FONT_BOLD)
                            .color(theme::ACCENT_GREEN),
                        text(format!("Acc {:.1}%", pb.accuracy))
                            .size(11.5)
                            .color(theme::TEXT_SECONDARY),
                        text(&pb.played_at)
                            .size(11)
                            .color(theme::TEXT_MUTED),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                )
                .width(Length::Fill)
                .padding([8, 10])
                .style(theme::sidebar_item_button(false))
                .on_press(Message::JumpToRun {
                    scenario: pb.scenario.clone(),
                    run_id: pb.id,
                });
                pb_list = pb_list.push(row_btn);
            }
        }

        let pb_card = container(
            column![
                text("近期突破 PB 列表（点击跳转复盘）")
                    .size(14)
                    .font(theme::FONT_BOLD)
                    .color(theme::TEXT_PRIMARY),
                pb_list,
            ]
            .spacing(10),
        )
        .padding(14)
        .width(Length::FillPortion(3))
        .style(theme::card_container);

        let selected_label = self
            .selected_scenario
            .as_deref()
            .unwrap_or("未选择项目");
        let export_card = container(
            column![
                text("数据导出与备份")
                    .size(14)
                    .font(theme::FONT_BOLD)
                    .color(theme::TEXT_PRIMARY),
                text(format!("当前选中项目：{}", selected_label))
                    .size(12.5)
                    .color(theme::ACCENT_CYAN),
                text("可将当前项目的全部历史对局（得分、准度、KPS、TTK、1-Shot%、cm/360、FPS 等）导出为标准 UTF-8 CSV 文件，方便在 Excel 或自定义脚本中进一步分析。")
                    .size(12)
                    .color(theme::TEXT_SECONDARY),
                Space::new().height(8),
                button(text("导出当前项目 CSV").size(13).font(theme::FONT_BOLD))
                    .padding([9, 16])
                    .style(theme::primary_button)
                    .on_press(Message::ExportCurrentScenarioCsv),
            ]
            .spacing(10),
        )
        .padding(14)
        .width(Length::FillPortion(2))
        .style(theme::card_container);

        let bottom = row![pb_card, export_card].spacing(12).width(Length::Fill);

        column![summary_row, heatmap_card, bottom]
            .spacing(12)
            .width(Length::Fill)
            .into()
    }
}

fn kpi_card<'a>(
    label: &'a str,
    value: String,
    subtitle: String,
    value_color: Color,
    sub_color: Color,
    badge: Option<(&'a str, Color)>,
) -> Element<'a, Message> {
    let mut top_row = Row::new()
        .spacing(6)
        .align_y(Alignment::Center)
        .push(text(label).size(11).color(theme::TEXT_MUTED));

    if let Some((badge_text, badge_color)) = badge {
        top_row = top_row.push(Space::new().width(Length::Fill)).push(
            container(
                text(badge_text)
                    .size(10)
                    .font(theme::FONT_BOLD)
                    .color(badge_color),
            )
            .padding([2, 6])
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
                .size(21)
                .font(theme::FONT_BOLD)
                .color(value_color),
            text(subtitle).size(11).color(sub_color),
        ]
        .spacing(4),
    )
    .padding([10, 12])
    .width(Length::Fill)
    .style(theme::card_container)
    .into()
}

fn half_metric_row<'a>(
    label: &'a str,
    first: String,
    second: String,
    delta: String,
    delta_color: Color,
) -> Element<'a, Message> {
    container(
        row![
            text(label)
                .size(12)
                .color(theme::TEXT_SECONDARY)
                .width(Length::Fixed(72.0)),
            column![
                text("前半程").size(10).color(theme::TEXT_MUTED),
                text(first).size(13).font(theme::FONT_BOLD).color(theme::TEXT_PRIMARY),
            ]
            .width(Length::Fill),
            column![
                text("后半程").size(10).color(theme::TEXT_MUTED),
                text(second).size(13).font(theme::FONT_BOLD).color(theme::TEXT_PRIMARY),
            ]
            .width(Length::Fill),
            container(text(delta).size(11.5).font(theme::FONT_BOLD).color(delta_color))
                .padding([3, 8])
                .style(theme::badge_container(
                    Color { a: 0.16, ..delta_color },
                    delta_color,
                )),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([8, 10])
    .width(Length::Fill)
    .style(theme::badge_container(theme::BG_BASE, theme::BORDER_SUBTLE))
    .into()
}

fn short_path(path: &str) -> String {
    let p = PathBuf::from(path);
    let mut parts: Vec<String> = p
        .components()
        .rev()
        .take(2)
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    parts.reverse();
    if parts.is_empty() {
        path.to_string()
    } else {
        format!("...\\{}", parts.join("\\"))
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
