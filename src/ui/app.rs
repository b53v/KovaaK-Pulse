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
    AppConfig, ConfigGroupStats, Language, OverviewStats, RunDetail, RunRecord, ScenarioSummary,
    ThemeMode, TimeRange, TrendAnalysis, TrendMetric, WatcherEvent,
};
use crate::storage::db::Database;
use crate::ui::charts::heatmap::HeatmapChart;
use crate::ui::charts::kill_interval_chart::KillIntervalChart;
use crate::ui::charts::per_second_chart::{PerSecondBarMode, PerSecondChart};
use crate::ui::charts::trend_chart::TrendChart;
use crate::ui::i18n::{
    bar_mode_label, half_split_diagnosis, metric_short, pacing_profile_label, t, tab_label,
    time_range_short,
};
use crate::ui::sidebar::{ScenarioSort, view_scenario_picker};
use crate::ui::theme::{self, UiPalette};
use crate::ui::tray::{TrayAction, TrayController, TraySubKey, poll_tray_event};
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
    SetLanguage(Language),
    SetThemeMode(ThemeMode),
    SaveSettings,
    DismissBanner,
    HideToTray,

    // Background Watcher & Tray
    Watcher(WatcherEvent),
    TrayAction(TrayAction),
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

        let tray = TrayController::new(config.language);

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
        theme::app_theme(self.config.theme)
    }

    fn palette(&self) -> UiPalette {
        UiPalette::from_mode(self.config.theme)
    }

    fn lang(&self) -> Language {
        self.config.language
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

        if let Some(tray_key) = self.tray.sub_key() {
            subs.push(Subscription::run_with(tray_key, tray_event_stream));
        }

        subs.push(window::close_requests().map(Message::WindowCloseRequested));

        Subscription::batch(subs)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let p = self.palette();
        let lang = self.lang();

        match message {
            Message::SelectTab(tab) => {
                self.active_tab = tab;
                self.scenario_picker_open = false;
                match tab {
                    ActiveTab::SingleRun => {
                        self.trend_cache.clear();
                        self.heatmap_cache.clear();
                    }
                    ActiveTab::LongTermTrend => {
                        self.per_second_cache.clear();
                        self.kill_interval_cache.clear();
                        self.heatmap_cache.clear();
                        self.reload_trend_data();
                    }
                    ActiveTab::Overview => {
                        self.per_second_cache.clear();
                        self.kill_interval_cache.clear();
                        self.trend_cache.clear();
                        self.reload_overview();
                    }
                }
                trim_working_set();
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
                            let msg = match lang {
                                Language::Zh => format!("已导出 {} 条至 {}", count, out_path.display()),
                                Language::En => format!("Exported {} runs to {}", count, out_path.display()),
                            };
                            self.banner = Some((msg, p.semantic_green));
                        }
                        Err(e) => {
                            self.banner = Some((format!("{}: {}", t(lang, "导出失败", "Export failed"), e), p.semantic_red));
                        }
                    }
                } else {
                    self.banner = Some((
                        t(lang, "请先选择一个训练项目", "Select a scenario first").to_string(),
                        p.border_active,
                    ));
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
                            let msg = match lang {
                                Language::Zh => format!("同步完成：新增 {} 局，跳过 {} 局（{}ms）", imported, skipped, elapsed_ms),
                                Language::En => format!("Synced: +{} new, {} skipped ({}ms)", imported, skipped, elapsed_ms),
                            };
                            self.banner = Some((msg, p.border_active));
                        } else if self.scenarios.is_empty() {
                            self.reload_all_data(None);
                        }
                    }
                    Err(e) => {
                        self.banner = Some((format!("{}: {}", t(lang, "同步失败", "Sync failed"), e), p.semantic_red));
                    }
                }
                trim_working_set();
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
            Message::SetLanguage(new_lang) => {
                if self.config.language != new_lang {
                    self.config.language = new_lang;
                    let _ = save_config(&self.config);
                    self.banner = None;
                    self.tray.set_language(new_lang);
                    self.clear_chart_caches();
                }
                Task::none()
            }
            Message::SetThemeMode(new_mode) => {
                if self.config.theme != new_mode {
                    self.config.theme = new_mode;
                    let _ = save_config(&self.config);
                    self.clear_chart_caches();
                }
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
                self.banner = Some((
                    t(lang, "设置已保存，正在重新监听", "Settings saved, restarting watcher").to_string(),
                    p.border_active,
                ));
                self.spawn_sync_task()
            }
            Message::DismissBanner => {
                self.banner = None;
                Task::none()
            }
            Message::HideToTray => {
                self.clear_chart_caches();
                trim_working_set();
                if self.tray.is_active() {
                    window::oldest().and_then(|id| window::set_mode(id, window::Mode::Hidden))
                } else {
                    window::oldest().and_then(|id| window::minimize(id, true))
                }
            }
            Message::Watcher(ev) => match ev {
                WatcherEvent::WatcherReady { .. } => Task::none(),
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
                    self.selected_scenario = Some(scenario.clone());
                    self.reload_all_data(Some(run_id));

                    let pb_tag = if is_new_pb { " · NEW PB" } else { "" };
                    let summary = format!(
                        "{} · {:.1} · {:.1}% · {:.2}/s{} ({:.2}s)",
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
                                p.semantic_green
                            } else {
                                p.border_active
                            },
                        ));
                    }
                    self.tray.update_tooltip(&format!("KovaaK Pulse — {}", summary));
                    Task::none()
                }
                WatcherEvent::Error(err) => {
                    self.banner = Some((
                        format!("{}: {}", t(lang, "监听异常", "Watcher error"), err),
                        p.semantic_red,
                    ));
                    Task::none()
                }
            },
            Message::TrayAction(action) => match action {
                TrayAction::ShowWindow => window::oldest().and_then(|id| {
                    Task::batch([
                        window::set_mode(id, window::Mode::Windowed),
                        window::minimize(id, false),
                        window::gain_focus(id),
                    ])
                }),
                TrayAction::SyncNow => self.spawn_sync_task(),
                TrayAction::Quit => iced::exit(),
            },
            Message::WindowCloseRequested(id) => {
                if self.config.minimize_to_tray && self.tray.is_active() {
                    self.clear_chart_caches();
                    trim_working_set();
                    // Real hide-to-tray (disappears from taskbar until restored via tray icon)
                    window::set_mode(id, window::Mode::Hidden)
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
                .map_err(|e| format!("Sync task failed: {}", e))?
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
        let p = self.palette();
        let lang = self.lang();

        let header = self.view_compact_header(p, lang);
        let picker = view_scenario_picker(
            p,
            lang,
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

        let mut body_col = Column::new().spacing(10).width(Length::Fill);

        if self.show_settings {
            body_col = body_col.push(self.view_settings_panel(p, lang));
        }

        if let Some((msg, accent)) = &self.banner {
            let banner = container(
                row![
                    text(msg).size(11.5).color(p.text_primary).width(Length::Fill),
                    button(text("✕").size(11))
                        .padding([2, 6])
                        .style(theme::ghost_button(p))
                        .on_press(Message::DismissBanner),
                ]
                .align_y(Alignment::Center),
            )
            .padding([6, 9])
            .width(Length::Fill)
            .style(theme::banner_container(p, *accent));
            body_col = body_col.push(banner);
        }

        let tab_content = match self.active_tab {
            ActiveTab::SingleRun => self.view_single_run_tab(p, lang),
            ActiveTab::LongTermTrend => self.view_long_term_trend_tab(p, lang),
            ActiveTab::Overview => self.view_overview_tab(p, lang),
        };
        body_col = body_col.push(tab_content);

        let scroll_body = scrollable(container(body_col).padding(10).width(Length::Fill))
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new().width(5).scroller_width(5),
            ))
            .style(theme::subtle_scrollbar(p))
            .width(Length::Fill)
            .height(Length::Fill);

        container(
            column![header, picker, scroll_body]
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::base_container(p))
        .into()
    }

    fn view_compact_header(&self, p: UiPalette, lang: Language) -> Element<'_, Message> {
        let sync_label = if self.is_syncing { "…" } else { "↻" };

        let mut status_row = row![
            text("KovaaK Pulse")
                .size(13.5)
                .font(theme::FONT_BOLD)
                .color(p.text_primary),
            Space::new().width(Length::Fill),
            button(text(sync_label).size(12.5))
                .padding([3, 7])
                .style(theme::ghost_button(p))
                .on_press(Message::TriggerManualSync),
            button(text("⚙").size(12.5))
                .padding([3, 7])
                .style(theme::ghost_button(p))
                .on_press(Message::ToggleSettings),
        ]
        .spacing(4)
        .align_y(Alignment::Center);

        if self.tray.is_active() {
            status_row = status_row.push(
                button(text("▾").size(12.5))
                    .padding([3, 7])
                    .style(theme::ghost_button(p))
                    .on_press(Message::HideToTray),
            );
        }

        let mut tabs_row = Row::new().spacing(3).width(Length::Fill);
        for tab in ActiveTab::ALL {
            let active = *tab == self.active_tab;
            tabs_row = tabs_row.push(
                button(
                    container(
                        text(tab_label(*tab, lang))
                            .size(12)
                            .font(if active { theme::FONT_BOLD } else { theme::FONT_REGULAR }),
                    )
                    .width(Length::Fill)
                    .center_x(Length::Fill),
                )
                .width(Length::FillPortion(1))
                .padding([5, 6])
                .style(theme::tab_button(p, active))
                .on_press(Message::SelectTab(*tab)),
            );
        }

        let segmented_bar = container(tabs_row)
            .padding(3)
            .width(Length::Fill)
            .style(theme::segmented_track_container(p));

        container(column![status_row, segmented_bar].spacing(8).width(Length::Fill))
            .padding([8, 10])
            .width(Length::Fill)
            .style(theme::topbar_container(p))
            .into()
    }

    fn view_settings_panel(&self, p: UiPalette, lang: Language) -> Element<'_, Message> {
        let lang_row = row![
            text(t(lang, "语言", "Language"))
                .size(11)
                .color(p.text_secondary)
                .width(Length::Fill),
            button(text("中文").size(11))
                .padding([3, 10])
                .style(theme::pill_button(p, self.config.language == Language::Zh))
                .on_press(Message::SetLanguage(Language::Zh)),
            button(text("EN").size(11))
                .padding([3, 10])
                .style(theme::pill_button(p, self.config.language == Language::En))
                .on_press(Message::SetLanguage(Language::En)),
        ]
        .spacing(5)
        .align_y(Alignment::Center);

        let theme_row = row![
            text(t(lang, "主题", "Theme"))
                .size(11)
                .color(p.text_secondary)
                .width(Length::Fill),
            button(text(t(lang, "深色", "Dark")).size(11))
                .padding([3, 10])
                .style(theme::pill_button(p, self.config.theme == ThemeMode::Dark))
                .on_press(Message::SetThemeMode(ThemeMode::Dark)),
            button(text(t(lang, "浅色", "Light")).size(11))
                .padding([3, 10])
                .style(theme::pill_button(p, self.config.theme == ThemeMode::Light))
                .on_press(Message::SetThemeMode(ThemeMode::Light)),
        ]
        .spacing(5)
        .align_y(Alignment::Center);

        let stats_input = text_input(t(lang, "stats 目录路径...", "stats folder path..."), &self.stats_dir_input)
            .on_input(Message::StatsDirInputChanged)
            .padding([5, 8])
            .size(11.5)
            .style(theme::search_input_style(p));

        let perf_input = text_input(
            t(lang, "performances 目录路径...", "performances folder path..."),
            &self.perf_dir_input,
        )
        .on_input(Message::PerfDirInputChanged)
        .padding([5, 8])
        .size(11.5)
        .style(theme::search_input_style(p));

        let toggle_row = row![
            button(
                text(if self.config.single_monitor_mode {
                    t(lang, "✓ 单屏", "✓ 1-Mon")
                } else {
                    t(lang, "○ 双屏", "○ 2-Mon")
                })
                .size(10.5),
            )
            .padding([4, 6])
            .style(theme::pill_button(p, self.config.single_monitor_mode))
            .on_press(Message::ToggleSingleMonitor),
            button(
                text(if self.config.minimize_to_tray {
                    t(lang, "✓ 关闭隐藏到托盘", "✓ Hide to Tray")
                } else {
                    t(lang, "○ 关闭退出", "○ Quit on X")
                })
                .size(10.5),
            )
            .padding([4, 6])
            .style(theme::pill_button(p, self.config.minimize_to_tray))
            .on_press(Message::ToggleMinimizeToTray),
            button(
                text(if self.config.notifications_enabled {
                    t(lang, "✓ 提示", "✓ Toast")
                } else {
                    t(lang, "○ 静默", "○ Quiet")
                })
                .size(10.5),
            )
            .padding([4, 6])
            .style(theme::pill_button(p, self.config.notifications_enabled))
            .on_press(Message::ToggleNotifications),
        ]
        .spacing(4)
        .align_y(Alignment::Center);

        container(
            column![
                text(t(lang, "偏好设置", "Settings"))
                    .size(12)
                    .font(theme::FONT_BOLD)
                    .color(p.text_primary),
                lang_row,
                theme_row,
                text(t(lang, "Stats 目录", "Stats Folder")).size(10.5).color(p.text_muted),
                stats_input,
                text(t(lang, "Performances 目录", "Performances Folder")).size(10.5).color(p.text_muted),
                perf_input,
                toggle_row,
                button(
                    container(
                        text(t(lang, "保存目录并重新监听", "Save Folders & Restart Watcher"))
                            .size(11.5)
                            .font(theme::FONT_BOLD),
                    )
                    .width(Length::Fill)
                    .center_x(Length::Fill),
                )
                .width(Length::Fill)
                .padding([6, 10])
                .style(theme::primary_button(p))
                .on_press(Message::SaveSettings),
            ]
            .spacing(7),
        )
        .padding(10)
        .width(Length::Fill)
        .style(theme::card_container(p))
        .into()
    }

    fn view_single_run_tab(&self, p: UiPalette, lang: Language) -> Element<'_, Message> {
        let Some(detail) = &self.selected_run_detail else {
            return container(
                column![
                    text(t(lang, "暂无对局数据", "No Run Data Yet"))
                        .size(14)
                        .font(theme::FONT_BOLD)
                        .color(p.text_primary),
                    text(t(
                        lang,
                        "完成一局 KovaaK's Challenge 或点击右上角 ↻ 同步本地记录。",
                        "Finish a KovaaK's Challenge or click ↻ above to sync local files."
                    ))
                    .size(12)
                    .color(p.text_secondary),
                ]
                .spacing(6),
            )
            .padding(16)
            .width(Length::Fill)
            .style(theme::card_container(p))
            .into();
        };

        let run = &detail.run;

        // ── TIER 1: Hero Card (Score + Accuracy) ──────────────────────────
        let (hero_sub_text, hero_sub_color) = if run.is_pb {
            match detail.delta_vs_last10 {
                Some(d) => match lang {
                    Language::Zh => (format!("NEW PB · {:+.1} vs 近10局", d), p.semantic_green),
                    Language::En => (format!("NEW PB · {:+.1} vs last 10", d), p.semantic_green),
                },
                None => ("NEW PB".to_string(), p.semantic_green),
            }
        } else if let Some(d) = detail.delta_vs_last10 {
            let color = if d >= 0.0 { p.semantic_green } else { p.semantic_red };
            let txt = match lang {
                Language::Zh => format!("{:+.1} vs 近10局均值", d),
                Language::En => format!("{:+.1} vs last 10 avg", d),
            };
            (txt, color)
        } else {
            (short_timestamp(&run.played_at), p.text_muted)
        };

        let hero_left = column![
            text(t(lang, "得分", "Score")).size(10.5).color(p.text_muted),
            text(format!("{:.1}", run.score))
                .size(26)
                .font(theme::FONT_BOLD)
                .color(p.text_primary),
            text(hero_sub_text).size(11).color(hero_sub_color),
        ]
        .spacing(2)
        .width(Length::Fill);

        let hits_sub = match lang {
            Language::Zh => format!("{}/{} 命中", run.hits, run.shots),
            Language::En => format!("{}/{} hits", run.hits, run.shots),
        };
        let hero_right = column![
            text(t(lang, "命中率", "Accuracy")).size(10.5).color(p.text_muted),
            text(format!("{:.1}%", run.accuracy))
                .size(22)
                .font(theme::FONT_BOLD)
                .color(p.text_primary),
            text(hits_sub).size(11).color(p.text_muted),
        ]
        .spacing(2)
        .align_x(Alignment::End);

        let hero_card = container(row![hero_left, hero_right].align_y(Alignment::End))
            .padding([12, 14])
            .width(Length::Fill)
            .style(theme::card_container(p));

        // ── TIER 2: 3-Column Secondary Strip (KPS / Avg TTK / 1-Shot) ────
        let secondary_strip = container(
            row![
                secondary_metric_cell(
                    p,
                    "KPS",
                    format!("{:.2}/s", run.avg_kps),
                    match lang {
                        Language::Zh => format!("峰值 {:.1}", run.peak_kps),
                        Language::En => format!("Peak {:.1}", run.peak_kps),
                    },
                ),
                secondary_metric_cell(
                    p,
                    t(lang, "平均 TTK", "Avg TTK"),
                    format!("{:.0} ms", run.avg_ttk_s * 1000.0),
                    match lang {
                        Language::Zh => format!("间隔 {:.0}ms", run.avg_kill_interval_ms),
                        Language::En => format!("Int {:.0}ms", run.avg_kill_interval_ms),
                    },
                ),
                secondary_metric_cell(
                    p,
                    t(lang, "一枪率", "1-Shot"),
                    format!("{:.1}%", run.one_shot_pct),
                    match lang {
                        Language::Zh => format!("补枪 {:.0}ms", run.multi_shot_avg_ttk_ms),
                        Language::En => format!("Multi {:.0}ms", run.multi_shot_avg_ttk_ms),
                    },
                ),
            ]
            .spacing(8)
            .width(Length::Fill),
        )
        .padding([10, 12])
        .width(Length::Fill)
        .style(theme::card_container(p));

        // ── TIER 3: Charts (Clean & Quiet) ───────────────────────────────
        let mut mode_row = Row::new().spacing(4).align_y(Alignment::Center);
        for m in PerSecondBarMode::ALL {
            mode_row = mode_row.push(
                button(text(bar_mode_label(*m, lang)).size(10.5))
                    .padding([3, 7])
                    .style(theme::pill_button(p, *m == self.per_second_mode))
                    .on_press(Message::SetPerSecondMode(*m)),
            );
        }

        let per_second_canvas = canvas::Canvas::new(PerSecondChart {
            ticks: &detail.ticks,
            mode: self.per_second_mode,
            avg_accuracy: run.accuracy,
            palette: p,
            lang,
            cache: &self.per_second_cache,
        })
        .width(Length::Fill)
        .height(Length::Fixed(185.0));

        let mut caption_parts = Vec::new();
        if let Some(best) = &detail.best_kps_tick {
            caption_parts.push(match lang {
                Language::Zh => format!("峰值 {}s ({}杀)", best.time_offset_s.round() as i32, best.kills),
                Language::En => format!("Peak {}s ({}k)", best.time_offset_s.round() as i32, best.kills),
            });
        }
        if let Some(worst) = &detail.worst_score_tick {
            if worst.score < 0.0 {
                caption_parts.push(match lang {
                    Language::Zh => format!("掉分 {}s ({:+.0})", worst.time_offset_s.round() as i32, worst.score),
                    Language::En => format!("Drop {}s ({:+.0})", worst.time_offset_s.round() as i32, worst.score),
                });
            }
        }

        let mut chart1_col = column![
            row![
                text(t(lang, "逐秒表现", "Per-Second"))
                    .size(12)
                    .font(theme::FONT_BOLD)
                    .color(p.text_primary),
                Space::new().width(Length::Fill),
                mode_row,
            ]
            .align_y(Alignment::Center),
            per_second_canvas,
        ]
        .spacing(6);

        if !caption_parts.is_empty() {
            chart1_col = chart1_col.push(
                text(caption_parts.join("  ·  "))
                    .size(10.5)
                    .color(p.text_muted),
            );
        }

        let chart1_card = container(chart1_col)
            .padding(11)
            .width(Length::Fill)
            .style(theme::card_container(p));

        let half_split_card = self.view_half_split_card(p, lang, detail);

        let kill_canvas = canvas::Canvas::new(KillIntervalChart {
            kills: &detail.kills,
            avg_interval_ms: run.avg_kill_interval_ms,
            palette: p,
            lang,
            cache: &self.kill_interval_cache,
        })
        .width(Length::Fill)
        .height(Length::Fixed(170.0));

        let chart2_card = container(
            column![
                row![
                    text(t(lang, "击杀节奏", "Kill Pacing"))
                        .size(12)
                        .font(theme::FONT_BOLD)
                        .color(p.text_primary),
                    Space::new().width(Length::Fill),
                    text(format!(
                        "{:.0} / {:.0} / {:.0} ms",
                        run.fastest_kill_interval_ms,
                        run.avg_kill_interval_ms,
                        run.slowest_kill_interval_ms
                    ))
                    .size(10.5)
                    .color(p.text_muted),
                ]
                .align_y(Alignment::Center),
                kill_canvas,
            ]
            .spacing(6),
        )
        .padding(11)
        .width(Length::Fill)
        .style(theme::card_container(p));

        let mut recent_runs_col = Column::new().spacing(4).width(Length::Fill);
        for r in self.scenario_runs.iter().take(8) {
            let active = r.id == run.id;
            let pb_mark = if r.is_pb { " PB" } else { "" };
            recent_runs_col = recent_runs_col.push(
                button(
                    row![
                        text(short_timestamp(&r.played_at))
                            .size(11)
                            .color(p.text_muted)
                            .width(Length::Fill),
                        text(format!("{:.1}", r.score))
                            .size(11.5)
                            .font(if active { theme::FONT_BOLD } else { theme::FONT_REGULAR })
                            .color(p.text_primary),
                        text(pb_mark)
                            .size(10)
                            .font(theme::FONT_BOLD)
                            .color(p.semantic_green),
                        text(format!("{:.1}%", r.accuracy))
                            .size(11)
                            .color(p.text_muted),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .width(Length::Fill)
                .padding([5, 8])
                .style(theme::sidebar_item_button(p, active))
                .on_press(Message::SelectRun(r.id)),
            );
        }

        let runs_card = container(
            column![
                row![
                    text(t(lang, "最近对局", "Recent Runs"))
                        .size(12)
                        .font(theme::FONT_BOLD)
                        .color(p.text_primary),
                    Space::new().width(Length::Fill),
                    text(match lang {
                        Language::Zh => format!("近 {} 局", self.scenario_runs.len().min(8)),
                        Language::En => format!("Last {}", self.scenario_runs.len().min(8)),
                    })
                    .size(10.5)
                    .color(p.text_muted),
                ]
                .align_y(Alignment::Center),
                recent_runs_col,
            ]
            .spacing(6),
        )
        .padding(11)
        .width(Length::Fill)
        .style(theme::card_container(p));

        // ── TIER 4: Quiet Metadata Footer ────────────────────────────────
        let sens_part = match run.cm_per_360 {
            Some(cm) => format!("{:.1} cm/360 ({} {:.2} @ {}DPI)", cm, run.sens_scale, run.horiz_sens, run.dpi),
            None => format!("{} {:.2} @ {}DPI", run.sens_scale, run.horiz_sens, run.dpi),
        };
        let meta_footer = container(
            text(format!("{} · {:.0} FPS · {}", sens_part, run.avg_fps, run.resolution))
                .size(10.5)
                .color(p.text_muted),
        )
        .padding([4, 2])
        .width(Length::Fill);

        let deep_insights_card = self.view_deep_insights_card(p, lang, detail);

        column![
            hero_card,
            secondary_strip,
            deep_insights_card,
            chart1_card,
            half_split_card,
            chart2_card,
            runs_card,
            meta_footer,
        ]
        .spacing(10)
        .width(Length::Fill)
        .into()
    }

    fn view_deep_insights_card<'a>(
        &self,
        p: UiPalette,
        lang: Language,
        detail: &'a RunDetail,
    ) -> Element<'a, Message> {
        let di = &detail.deep_insights;
        let run = &detail.run;
        let ceiling_diff = (di.potential_ceiling_score - run.score).max(0.0);

        let ceiling_sub = match lang {
            Language::Zh => format!(
                "+{:.0} · 补枪耗 {:.2}s / +{:.1}杀",
                ceiling_diff, di.miss_recovery_time_lost_s, di.estimated_lost_kills
            ),
            Language::En => format!(
                "+{:.0} · {:.2}s lost / +{:.1}k",
                ceiling_diff, di.miss_recovery_time_lost_s, di.estimated_lost_kills
            ),
        };

        let streak_val = match lang {
            Language::Zh => format!("{} 连杀", di.max_one_shot_streak),
            Language::En => format!("{} streak", di.max_one_shot_streak),
        };
        let streak_sub = match lang {
            Language::Zh => format!("连空传染 {}/{}", di.clustered_miss_count, di.multi_shot_count),
            Language::En => format!("Tilt cascade {}/{}", di.clustered_miss_count, di.multi_shot_count),
        };

        let row1 = row![
            secondary_metric_cell(
                p,
                t(lang, "零失误理论上限", "Zero-Miss Ceiling"),
                format!("~{:.0}", di.potential_ceiling_score),
                ceiling_sub,
            ),
            secondary_metric_cell(
                p,
                t(lang, "最长一枪连杀", "Max 1-Shot Streak"),
                streak_val,
                streak_sub,
            ),
        ]
        .spacing(8)
        .width(Length::Fill);

        let p10_sub = match lang {
            Language::Zh => format!("中位 P50 {:.0}ms", di.p50_interval_ms),
            Language::En => format!("Median P50 {:.0}ms", di.p50_interval_ms),
        };
        let p90_sub = match lang {
            Language::Zh => format!("节拍波动 CV {:.1}%", di.rhythm_cv_pct),
            Language::En => format!("Rhythm CV {:.1}%", di.rhythm_cv_pct),
        };

        let row2 = row![
            secondary_metric_cell(
                p,
                t(lang, "极限爆发 P10", "Peak P10"),
                format!("{:.0} ms", di.p10_interval_ms),
                p10_sub,
            ),
            secondary_metric_cell(
                p,
                t(lang, "卡壳分位 P90", "Bottleneck P90"),
                format!("{:.0} ms", di.p90_interval_ms),
                p90_sub,
            ),
        ]
        .spacing(8)
        .width(Length::Fill);

        let phase_box = container(
            row![
                column![
                    text(t(lang, "前 1/3", "Early 1/3")).size(9.5).color(p.text_muted),
                    text(format!("{:.2}/s ({:.0}%)", di.early_kps, di.early_acc))
                        .size(11)
                        .font(theme::FONT_BOLD)
                        .color(p.text_primary),
                ]
                .spacing(1)
                .width(Length::FillPortion(1)),
                column![
                    text(t(lang, "中 1/3", "Mid 1/3")).size(9.5).color(p.text_muted),
                    text(format!("{:.2}/s ({:.0}%)", di.mid_kps, di.mid_acc))
                        .size(11)
                        .font(theme::FONT_BOLD)
                        .color(p.text_primary),
                ]
                .spacing(1)
                .width(Length::FillPortion(1)),
                column![
                    text(t(lang, "后 1/3", "Late 1/3")).size(9.5).color(p.text_muted),
                    text(format!("{:.2}/s ({:.0}%)", di.late_kps, di.late_acc))
                        .size(11)
                        .font(theme::FONT_BOLD)
                        .color(p.text_primary),
                ]
                .spacing(1)
                .width(Length::FillPortion(1)),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([6, 8])
        .width(Length::Fill)
        .style(theme::subtle_row_container(p));

        container(
            column![
                row![
                    text(t(lang, "深度洞察", "Deep Telemetry"))
                        .size(12)
                        .font(theme::FONT_BOLD)
                        .color(p.text_primary),
                    Space::new().width(Length::Fill),
                    text(pacing_profile_label(di.pacing_profile, lang))
                        .size(10.5)
                        .color(p.text_secondary),
                ]
                .align_y(Alignment::Center),
                row1,
                row2,
                phase_box,
            ]
            .spacing(8),
        )
        .padding(11)
        .width(Length::Fill)
        .style(theme::card_container(p))
        .into()
    }

    fn view_half_split_card<'a>(
        &self,
        p: UiPalette,
        lang: Language,
        detail: &'a RunDetail,
    ) -> Element<'a, Message> {
        let content: Element<'a, Message> = match &detail.half_split {
            Some(hs) => {
                let score_c = if hs.score_delta_pct >= 0.0 {
                    p.semantic_green
                } else {
                    p.semantic_red
                };
                let acc_c = if hs.accuracy_delta_pp >= 0.0 {
                    p.semantic_green
                } else {
                    p.semantic_red
                };
                let kps_c = if hs.kps_delta >= 0.0 {
                    p.semantic_green
                } else {
                    p.semantic_red
                };

                column![
                    row![
                        text(t(lang, "前后半程", "Half Split"))
                            .size(12)
                            .font(theme::FONT_BOLD)
                            .color(p.text_primary),
                        Space::new().width(Length::Fill),
                        text(half_split_diagnosis(hs, lang))
                            .size(10.5)
                            .color(p.text_secondary),
                    ]
                    .align_y(Alignment::Center),
                    compact_half_row(
                        p,
                        lang,
                        t(lang, "得分", "Score"),
                        format!("{:.0}", hs.first_half_score),
                        format!("{:.0}", hs.second_half_score),
                        format!("{:+.1}%", hs.score_delta_pct),
                        score_c,
                    ),
                    compact_half_row(
                        p,
                        lang,
                        t(lang, "准度", "Acc"),
                        format!("{:.1}%", hs.first_half_accuracy),
                        format!("{:.1}%", hs.second_half_accuracy),
                        format!("{:+.1}pp", hs.accuracy_delta_pp),
                        acc_c,
                    ),
                    compact_half_row(
                        p,
                        lang,
                        "KPS",
                        format!("{:.2}", hs.first_half_kps),
                        format!("{:.2}", hs.second_half_kps),
                        format!("{:+.2}", hs.kps_delta),
                        kps_c,
                    ),
                ]
                .spacing(5)
                .into()
            }
            None => column![
                text(t(lang, "前后半程", "Half Split"))
                    .size(12)
                    .font(theme::FONT_BOLD)
                    .color(p.text_primary),
                text(t(
                    lang,
                    "需要 .perf 逐秒数据或更多击杀记录进行对比。",
                    "Needs .perf ticks or more kills for half-split comparison."
                ))
                .size(11)
                .color(p.text_muted),
            ]
            .spacing(4)
            .into(),
        };

        container(content)
            .padding(11)
            .width(Length::Fill)
            .style(theme::card_container(p))
            .into()
    }

    fn view_long_term_trend_tab(&self, p: UiPalette, lang: Language) -> Element<'_, Message> {
        let mut range_pills = Row::new().spacing(3);
        for r in TimeRange::ALL {
            range_pills = range_pills.push(
                button(text(time_range_short(*r, lang)).size(10.5))
                    .padding([3, 6])
                    .style(theme::pill_button(p, *r == self.time_range))
                    .on_press(Message::SetTimeRange(*r)),
            );
        }

        let mut ma_pills = Row::new().spacing(3);
        for w in [5usize, 10, 20] {
            ma_pills = ma_pills.push(
                button(text(format!("MA{}", w)).size(10.5))
                    .padding([3, 5])
                    .style(theme::pill_button(p, w == self.ma_window))
                    .on_press(Message::SetMaWindow(w)),
            );
        }

        let mut metric_pills = Row::new().spacing(4).width(Length::Fill);
        for m in TrendMetric::ALL {
            metric_pills = metric_pills.push(
                button(
                    container(text(metric_short(*m, lang)).size(11))
                        .width(Length::Fill)
                        .center_x(Length::Fill),
                )
                .width(Length::FillPortion(1))
                .padding([4, 4])
                .style(theme::pill_button(p, *m == self.trend_metric))
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
        .style(theme::card_container(p));

        let a = &self.trend_analysis;
        let unit = self.trend_metric.unit();
        let recent_str = match a.recent_delta_pct {
            Some(d) => format!("{:+.1}%", d),
            None => "—".to_string(),
        };
        let recent_color = match a.recent_delta_pct {
            Some(d) if (d >= 0.0) == self.trend_metric.higher_is_better() => p.semantic_green,
            Some(_) => p.semantic_red,
            None => p.text_muted,
        };

        let sum_card = container(
            column![
                row![
                    secondary_metric_cell(
                        p,
                        t(lang, "区间均值", "Mean"),
                        format!("{:.1}{}", a.mean, unit),
                        match lang {
                            Language::Zh => format!("{} 局 · σ {:.1}", a.points.len(), a.std_dev),
                            Language::En => format!("{} runs · σ {:.1}", a.points.len(), a.std_dev),
                        },
                    ),
                    secondary_metric_cell(
                        p,
                        t(lang, "区间最佳 PB", "Best PB"),
                        format!("{:.1}{}", a.pb_value, unit),
                        match lang {
                            Language::Zh => format!("最低 {:.1}{}", a.min_val, unit),
                            Language::En => format!("Min {:.1}{}", a.min_val, unit),
                        },
                    ),
                ]
                .spacing(8),
                row![
                    secondary_metric_cell(
                        p,
                        t(lang, "稳定性 CV", "Stability CV"),
                        format!("{:.1}%", a.cv_pct),
                        t(lang, "越低越稳定", "Lower is steadier").to_string(),
                    ),
                    column![
                        text(t(lang, "近期趋势", "Recent Trend")).size(10).color(p.text_muted),
                        text(recent_str)
                            .size(15)
                            .font(theme::FONT_BOLD)
                            .color(recent_color),
                        text(t(lang, "后段 vs 前段", "Late vs Early")).size(10).color(p.text_muted),
                    ]
                    .spacing(2)
                    .width(Length::FillPortion(1)),
                ]
                .spacing(8),
                row![
                    secondary_metric_cell(
                        p,
                        t(lang, "保底线 P25", "Floor P25"),
                        format!("{:.1}{}", a.p25_floor, unit),
                        t(lang, "低谷稳定下限", "25th percentile").to_string(),
                    ),
                    secondary_metric_cell(
                        p,
                        t(lang, "常态上限 P90", "Ceiling P90"),
                        format!("{:.1}{}", a.p90_ceiling, unit),
                        t(lang, "前10%高水准", "90th percentile").to_string(),
                    ),
                ]
                .spacing(8),
            ]
            .spacing(10),
        )
        .padding([10, 12])
        .width(Length::Fill)
        .style(theme::card_container(p));

        let trend_canvas = canvas::Canvas::new(TrendChart {
            analysis: &self.trend_analysis,
            metric: self.trend_metric,
            ma_window: self.ma_window,
            palette: p,
            lang,
            cache: &self.trend_cache,
        })
        .width(Length::Fill)
        .height(Length::Fixed(205.0));

        let trend_card = container(trend_canvas)
            .padding(10)
            .width(Length::Fill)
            .style(theme::card_container(p));

        let mut group_col = Column::new().spacing(5).width(Length::Fill);
        if self.config_groups.is_empty() {
            group_col = group_col.push(
                text(t(lang, "暂无不同灵敏度 / FOV 分组数据", "No sensitivity / FOV groups yet"))
                    .size(11.5)
                    .color(p.text_muted),
            );
        } else {
            for g in &self.config_groups {
                let cm_label = match g.cm_per_360 {
                    Some(cm) => format!("{:.1} cm/360", cm),
                    None => format!("{} {:.2}", g.sens_scale, g.horiz_sens),
                };
                let header = row![
                    text(format!("{} · FOV {:.0}", cm_label, g.fov))
                        .size(11.5)
                        .font(theme::FONT_BOLD)
                        .color(p.text_primary)
                        .width(Length::Fill),
                    text(match lang {
                        Language::Zh => format!("{}局", g.run_count),
                        Language::En => format!("{}r", g.run_count),
                    })
                    .size(10.5)
                    .color(p.text_muted),
                ]
                .align_y(Alignment::Center);

                let metrics = row![
                    text(match lang {
                        Language::Zh => format!("均 {:.1}", g.avg_score),
                        Language::En => format!("Avg {:.1}", g.avg_score),
                    })
                    .size(11)
                    .color(p.text_secondary),
                    text(format!("PB {:.1}", g.pb_score))
                        .size(11)
                        .color(p.text_primary),
                    text(match lang {
                        Language::Zh => format!("准 {:.1}%", g.avg_accuracy),
                        Language::En => format!("Acc {:.1}%", g.avg_accuracy),
                    })
                    .size(11)
                    .color(p.text_muted),
                    Space::new().width(Length::Fill),
                    text(format!("1S {:.0}%", g.avg_one_shot_pct))
                        .size(11)
                        .color(p.text_muted),
                ]
                .spacing(8)
                .align_y(Alignment::Center);

                group_col = group_col.push(
                    container(column![header, metrics].spacing(3))
                        .padding([6, 8])
                        .width(Length::Fill)
                        .style(theme::subtle_row_container(p)),
                );
            }
        }

        let group_card = container(
            column![
                text(t(lang, "配置分组对比", "Config Groups"))
                    .size(12)
                    .font(theme::FONT_BOLD)
                    .color(p.text_primary),
                group_col,
            ]
            .spacing(7),
        )
        .padding(11)
        .width(Length::Fill)
        .style(theme::card_container(p));

        column![controls, sum_card, trend_card, group_card]
            .spacing(10)
            .width(Length::Fill)
            .into()
    }

    fn view_overview_tab(&self, p: UiPalette, lang: Language) -> Element<'_, Message> {
        let ov = &self.overview_stats;
        let today_mins = ov.today_duration_s / 60.0;

        let summary_card = container(
            column![
                row![
                    secondary_metric_cell(
                        p,
                        t(lang, "今日局数", "Today Runs"),
                        match lang {
                            Language::Zh => format!("{} 局", ov.today_runs),
                            Language::En => format!("{}", ov.today_runs),
                        },
                        t(lang, "当日完成对局", "Completed today").to_string(),
                    ),
                    secondary_metric_cell(
                        p,
                        t(lang, "今日时长", "Today Time"),
                        match lang {
                            Language::Zh => format!("{:.1} 分", today_mins),
                            Language::En => format!("{:.1} min", today_mins),
                        },
                        match lang {
                            Language::Zh => format!("{:.0}s 射击", ov.today_duration_s),
                            Language::En => format!("{:.0}s active", ov.today_duration_s),
                        },
                    ),
                ]
                .spacing(8),
                row![
                    column![
                        text(t(lang, "今日新 PB", "Today PBs")).size(10).color(p.text_muted),
                        text(match lang {
                            Language::Zh => format!("{} 次", ov.today_new_pbs),
                            Language::En => format!("{}", ov.today_new_pbs),
                        })
                        .size(15)
                        .font(theme::FONT_BOLD)
                        .color(if ov.today_new_pbs > 0 {
                            p.semantic_green
                        } else {
                            p.text_primary
                        }),
                        text(t(lang, "单日突破", "Personal bests")).size(10).color(p.text_muted),
                    ]
                    .spacing(2)
                    .width(Length::FillPortion(1)),
                    secondary_metric_cell(
                        p,
                        t(lang, "累计归档", "Total Archived"),
                        match lang {
                            Language::Zh => format!("{} 局", ov.total_runs),
                            Language::En => format!("{} runs", ov.total_runs),
                        },
                        match lang {
                            Language::Zh => format!("{} 个项目", ov.total_scenarios),
                            Language::En => format!("{} scenarios", ov.total_scenarios),
                        },
                    ),
                ]
                .spacing(8),
            ]
            .spacing(10),
        )
        .padding([10, 12])
        .width(Length::Fill)
        .style(theme::card_container(p));

        let heatmap_canvas = canvas::Canvas::new(HeatmapChart {
            cells: &ov.heatmap,
            palette: p,
            lang,
            cache: &self.heatmap_cache,
        })
        .width(Length::Fill)
        .height(Length::Fixed(152.0));

        let heatmap_card = container(
            column![
                row![
                    text(t(lang, "训练日历 (近18周)", "Activity (18w)"))
                        .size(12)
                        .font(theme::FONT_BOLD)
                        .color(p.text_primary),
                    Space::new().width(Length::Fill),
                    text(t(lang, "悬停看明细", "Hover for details")).size(10).color(p.text_muted),
                ]
                .align_y(Alignment::Center),
                heatmap_canvas,
            ]
            .spacing(6),
        )
        .padding(11)
        .width(Length::Fill)
        .style(theme::card_container(p));

        let mut pb_list = Column::new().spacing(4).width(Length::Fill);
        if ov.recent_pbs.is_empty() {
            pb_list = pb_list.push(
                text(t(lang, "暂无 PB 记录", "No PBs yet"))
                    .size(11.5)
                    .color(p.text_muted),
            );
        } else {
            for pb in ov.recent_pbs.iter().take(8) {
                pb_list = pb_list.push(
                    button(
                        column![
                            row![
                                text(&pb.scenario)
                                    .size(11.5)
                                    .font(theme::FONT_BOLD)
                                    .color(p.text_primary)
                                    .width(Length::Fill),
                                text(format!("{:.1}", pb.score))
                                    .size(12)
                                    .font(theme::FONT_BOLD)
                                    .color(p.semantic_green),
                            ]
                            .spacing(5)
                            .align_y(Alignment::Center),
                            row![
                                text(format!("Acc {:.1}% · KPS {:.2}", pb.accuracy, pb.avg_kps))
                                    .size(10.5)
                                    .color(p.text_muted),
                                Space::new().width(Length::Fill),
                                text(short_timestamp(&pb.played_at))
                                    .size(10.5)
                                    .color(p.text_muted),
                            ]
                            .align_y(Alignment::Center),
                        ]
                        .spacing(2),
                    )
                    .width(Length::Fill)
                    .padding([6, 8])
                    .style(theme::sidebar_item_button(p, false))
                    .on_press(Message::JumpToRun {
                        scenario: pb.scenario.clone(),
                        run_id: pb.id,
                    }),
                );
            }
        }

        let pb_card = container(
            column![
                text(t(lang, "近期突破 PB", "Recent PBs"))
                    .size(12)
                    .font(theme::FONT_BOLD)
                    .color(p.text_primary),
                pb_list,
            ]
            .spacing(6),
        )
        .padding(11)
        .width(Length::Fill)
        .style(theme::card_container(p));

        let selected_label = self
            .selected_scenario
            .as_deref()
            .unwrap_or_else(|| t(lang, "未选择项目", "None"));
        let export_card = container(
            column![
                row![
                    text(t(lang, "导出 CSV", "Export CSV"))
                        .size(12)
                        .font(theme::FONT_BOLD)
                        .color(p.text_primary),
                    Space::new().width(Length::Fill),
                    text(selected_label).size(10.5).color(p.text_muted),
                ]
                .align_y(Alignment::Center),
                button(
                    container(
                        text(t(lang, "导出当前项目全部历史 CSV", "Export Selected Scenario CSV"))
                            .size(11.5)
                            .font(theme::FONT_BOLD),
                    )
                    .width(Length::Fill)
                    .center_x(Length::Fill),
                )
                .width(Length::Fill)
                .padding([7, 10])
                .style(theme::primary_button(p))
                .on_press(Message::ExportCurrentScenarioCsv),
            ]
            .spacing(7),
        )
        .padding(11)
        .width(Length::Fill)
        .style(theme::card_container(p));

        column![summary_card, heatmap_card, pb_card, export_card]
            .spacing(10)
            .width(Length::Fill)
            .into()
    }
}

fn secondary_metric_cell<'a>(
    p: UiPalette,
    label: &'a str,
    value: String,
    subtitle: String,
) -> Element<'a, Message> {
    column![
        text(label).size(10).color(p.text_muted),
        text(value)
            .size(15)
            .font(theme::FONT_BOLD)
            .color(p.text_primary),
        text(subtitle).size(10).color(p.text_muted),
    ]
    .spacing(2)
    .width(Length::FillPortion(1))
    .into()
}

fn compact_half_row<'a>(
    p: UiPalette,
    lang: Language,
    label: &'a str,
    first: String,
    second: String,
    delta: String,
    delta_color: Color,
) -> Element<'a, Message> {
    let (h1_prefix, h2_prefix) = match lang {
        Language::Zh => ("前", "后"),
        Language::En => ("1st", "2nd"),
    };
    container(
        row![
            text(label)
                .size(11)
                .color(p.text_muted)
                .width(Length::Fixed(38.0)),
            text(format!("{} {}", h1_prefix, first))
                .size(11)
                .color(p.text_secondary)
                .width(Length::Fill),
            text(format!("{} {}", h2_prefix, second))
                .size(11)
                .font(theme::FONT_BOLD)
                .color(p.text_primary)
                .width(Length::Fill),
            text(delta)
                .size(11)
                .font(theme::FONT_BOLD)
                .color(delta_color),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([5, 8])
    .width(Length::Fill)
    .style(theme::subtle_row_container(p))
    .into()
}

fn short_timestamp(ts: &str) -> String {
    if ts.len() >= 16 {
        ts[5..16].to_string()
    } else {
        ts.to_string()
    }
}

/// Trims unused memory-mapped font & allocator pages from the Windows process working set.
#[cfg(windows)]
pub fn trim_working_set() {
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut core::ffi::c_void;
        fn SetProcessWorkingSetSize(
            hProcess: *mut core::ffi::c_void,
            dwMinimumWorkingSetSize: usize,
            dwMaximumWorkingSetSize: usize,
        ) -> i32;
    }
    unsafe {
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}

#[cfg(not(windows))]
pub fn trim_working_set() {}

fn tray_event_stream(key: &TraySubKey) -> impl iced::futures::Stream<Item = Message> + use<> {
    let key = key.clone();
    iced::stream::channel(16, async move |mut output| {
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        loop {
            interval.tick().await;
            if let Some(action) = poll_tray_event(&key) {
                if output.send(Message::TrayAction(action)).await.is_err() {
                    break;
                }
            }
        }
    })
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
