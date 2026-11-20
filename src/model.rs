use serde::{Deserialize, Serialize};

/// Complete metadata and summary metrics for a single KovaaK's run.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunRecord {
    pub id: i64,
    /// Common stem shared by `... Stats.csv` and `... Performance.perf`
    /// e.g. `Micro Flick - Challenge - 2026.11.20-21.58.16`
    pub file_key: String,
    pub scenario: String,
    pub scenario_hash: String,
    pub mode: String,
    /// Sortable local timestamp string: `YYYY-MM-DD HH:MM:SS`
    pub played_at: String,
    /// Epoch milliseconds (UTC if available from `.perf`, else local parsed timestamp)
    pub played_at_ms: i64,
    /// `YYYY-MM-DD`
    pub date_key: String,
    pub challenge_start: String,
    pub game_version: String,
    pub duration_s: f64,

    // Core performance metrics
    pub score: f64,
    pub kills: u32,
    pub deaths: u32,
    pub shots: u32,
    pub hits: u32,
    pub misses: u32,
    /// 0.0 ..= 100.0
    pub accuracy: f64,
    pub damage_done: f64,
    pub damage_possible: f64,
    /// 0.0 ..= 100.0
    pub damage_efficiency: f64,
    /// Seconds (directly from CSV `Avg TTK:`)
    pub avg_ttk_s: f64,
    pub fight_time_s: f64,
    pub time_remaining_s: f64,
    pub total_overshots: u32,
    pub reloads: u32,
    pub pause_count: u32,
    pub pause_duration_s: f64,
    pub distance_traveled: f64,
    pub mbs_points: f64,
    pub avg_target_scale: f64,
    pub avg_time_dilation: f64,

    // Derived rhythm / pacing metrics
    pub avg_kps: f64,
    pub peak_kps: f64,
    pub one_shot_kills: u32,
    /// 0.0 ..= 100.0
    pub one_shot_pct: f64,
    pub avg_kill_interval_ms: f64,
    pub fastest_kill_interval_ms: f64,
    pub slowest_kill_interval_ms: f64,
    /// Average TTK in ms for kills that required > 1 shot
    pub multi_shot_avg_ttk_ms: f64,

    // Hardware & sensitivity settings
    pub sens_scale: String,
    pub horiz_sens: f64,
    pub vert_sens: f64,
    pub dpi: u32,
    pub cm_per_360: Option<f64>,
    pub fov: f64,
    pub fov_scale: String,
    pub hide_gun: bool,
    pub crosshair: String,
    pub crosshair_scale: f64,
    pub crosshair_color: String,
    pub resolution: String,
    pub avg_fps: f64,
    pub max_fps_config: f64,
    pub input_lag_ms: f64,
    pub resolution_scale: f64,

    // Provenance & flags
    pub has_perf: bool,
    pub is_pb: bool,
    pub csv_path: String,
    pub perf_path: Option<String>,
}

/// Single kill row from section 1 of `Stats.csv`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KillEvent {
    pub kill_index: u32,
    pub timestamp: String,
    /// Milliseconds elapsed since challenge start (or first kill if start unavailable)
    pub elapsed_ms: f64,
    /// Milliseconds since previous kill (`None` for the first kill)
    pub interval_ms: Option<f64>,
    pub bot: String,
    pub weapon: String,
    pub ttk_s: f64,
    pub shots: u32,
    pub hits: u32,
    /// 0.0 ..= 100.0
    pub accuracy: f64,
    pub damage_done: f64,
    pub damage_possible: f64,
    /// 0.0 ..= 100.0
    pub efficiency: f64,
    pub overshots: u32,
    pub cheated: bool,
    pub is_one_shot: bool,
}

/// Aggregated ~1Hz sample from `Performance.perf`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerfTick {
    pub tick_index: u32,
    pub time_offset_s: f64,
    pub shots_fired: u32,
    pub shots_hit: u32,
    pub shots_missed: u32,
    /// 0.0 ..= 100.0
    pub accuracy: f64,
    pub damage_done: f64,
    pub damage_possible: f64,
    pub score: f64,
    pub kills: u32,
    pub deaths: u32,
    pub overshots: u32,
    pub reloads: u32,
    pub pauses: u32,
    pub mbs_points: f64,
    pub distance_traveled: f64,
    pub target_size: f64,
    pub target_speed: f64,
    pub random_sens_scale: f64,
}

/// Header decoded from a `.perf` protobuf file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerfHeader {
    pub scenario_name: String,
    pub scenario_hash: String,
    pub challenge_start_utc_ms: i64,
    pub schema_version: u32,
    pub duration_s: f64,
    pub scenario_type: String,
    pub map_name: String,
    pub bots: Vec<String>,
}

/// Decoded contents of a `.perf` file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerfFileData {
    pub header: PerfHeader,
    pub ticks: Vec<PerfTick>,
}

/// Weapon summary row from section 2 of `Stats.csv`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WeaponSummary {
    pub weapon: String,
    pub shots: u32,
    pub hits: u32,
    pub damage_done: f64,
    pub damage_possible: f64,
}

/// Combined parse output from `.csv` + optional `.perf`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ParsedRun {
    pub run: RunRecord,
    pub kills: Vec<KillEvent>,
    pub ticks: Vec<PerfTick>,
    pub weapons: Vec<WeaponSummary>,
}

/// First-half (0-50%) vs second-half (50-100%) stability diagnosis.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HalfSplitComparison {
    pub first_half_ticks: u32,
    pub second_half_ticks: u32,
    pub first_half_score: f64,
    pub second_half_score: f64,
    pub first_half_kills: u32,
    pub second_half_kills: u32,
    pub first_half_kps: f64,
    pub second_half_kps: f64,
    pub first_half_accuracy: f64,
    pub second_half_accuracy: f64,
    /// Percentage change of second-half score vs first-half score
    pub score_delta_pct: f64,
    /// Percentage point difference: second_half_accuracy - first_half_accuracy
    pub accuracy_delta_pp: f64,
    /// KPS difference: second_half_kps - first_half_kps
    pub kps_delta: f64,
    /// Concise diagnostic summary for the UI
    pub diagnosis: String,
}

/// Full detail bundle for the Single-Run Review tab.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunDetail {
    pub run: RunRecord,
    pub kills: Vec<KillEvent>,
    pub ticks: Vec<PerfTick>,
    pub half_split: Option<HalfSplitComparison>,
    pub last10_avg_score: Option<f64>,
    pub delta_vs_last10: Option<f64>,
    pub worst_score_tick: Option<PerfTick>,
    pub best_kps_tick: Option<PerfTick>,
}

/// Sidebar entry summarizing a single scenario.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScenarioSummary {
    pub scenario: String,
    pub run_count: u32,
    pub pb_score: f64,
    pub latest_score: f64,
    pub avg_score: f64,
    pub latest_accuracy: f64,
    pub latest_played_at: String,
    pub latest_played_at_ms: i64,
    pub latest_run_id: i64,
    pub is_favorite: bool,
}

/// UI display language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Language {
    #[default]
    Zh,
    En,
}

/// UI color theme mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

/// Metric selectable on the Long-Term Trend chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TrendMetric {
    #[default]
    Score,
    Accuracy,
    Kps,
    AvgTtkMs,
    OneShotPct,
}

impl TrendMetric {
    pub const ALL: &'static [TrendMetric] = &[
        TrendMetric::Score,
        TrendMetric::Accuracy,
        TrendMetric::Kps,
        TrendMetric::AvgTtkMs,
        TrendMetric::OneShotPct,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TrendMetric::Score => "得分 Score",
            TrendMetric::Accuracy => "准度 Acc %",
            TrendMetric::Kps => "击杀速率 KPS",
            TrendMetric::AvgTtkMs => "平均 TTK (ms)",
            TrendMetric::OneShotPct => "一枪毙命率 %",
        }
    }

    pub fn label_i18n(self, lang: Language) -> &'static str {
        match (lang, self) {
            (Language::Zh, TrendMetric::Score) => "得分",
            (Language::Zh, TrendMetric::Accuracy) => "准度",
            (Language::Zh, TrendMetric::Kps) => "KPS",
            (Language::Zh, TrendMetric::AvgTtkMs) => "TTK",
            (Language::Zh, TrendMetric::OneShotPct) => "一枪率",
            (Language::En, TrendMetric::Score) => "Score",
            (Language::En, TrendMetric::Accuracy) => "Acc %",
            (Language::En, TrendMetric::Kps) => "KPS",
            (Language::En, TrendMetric::AvgTtkMs) => "TTK",
            (Language::En, TrendMetric::OneShotPct) => "1-Shot",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            TrendMetric::Score => "",
            TrendMetric::Accuracy => "%",
            TrendMetric::Kps => "/s",
            TrendMetric::AvgTtkMs => "ms",
            TrendMetric::OneShotPct => "%",
        }
    }

    pub fn higher_is_better(self) -> bool {
        !matches!(self, TrendMetric::AvgTtkMs)
    }

    pub fn extract(self, run: &RunRecord) -> f64 {
        match self {
            TrendMetric::Score => run.score,
            TrendMetric::Accuracy => run.accuracy,
            TrendMetric::Kps => run.avg_kps,
            TrendMetric::AvgTtkMs => run.avg_ttk_s * 1000.0,
            TrendMetric::OneShotPct => run.one_shot_pct,
        }
    }
}

/// Time filter for Long-Term Trend analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TimeRange {
    Today,
    Days7,
    Days30,
    #[default]
    All,
}

impl TimeRange {
    pub const ALL: &'static [TimeRange] = &[
        TimeRange::Today,
        TimeRange::Days7,
        TimeRange::Days30,
        TimeRange::All,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TimeRange::Today => "今日",
            TimeRange::Days7 => "近 7 天",
            TimeRange::Days30 => "近 30 天",
            TimeRange::All => "全部",
        }
    }

    pub fn label_i18n(self, lang: Language) -> &'static str {
        match (lang, self) {
            (Language::Zh, TimeRange::Today) => "今日",
            (Language::Zh, TimeRange::Days7) => "7天",
            (Language::Zh, TimeRange::Days30) => "30天",
            (Language::Zh, TimeRange::All) => "全部",
            (Language::En, TimeRange::Today) => "Today",
            (Language::En, TimeRange::Days7) => "7D",
            (Language::En, TimeRange::Days30) => "30D",
            (Language::En, TimeRange::All) => "All",
        }
    }
}

/// Single point on a Long-Term Trend chart.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrendPoint {
    pub run_id: i64,
    pub index: usize,
    pub played_at: String,
    pub played_at_ms: i64,
    pub date_key: String,
    pub value: f64,
    pub score: f64,
    pub accuracy: f64,
    pub kps: f64,
    pub avg_ttk_ms: f64,
    pub one_shot_pct: f64,
    pub ma_value: Option<f64>,
    pub pb_envelope: f64,
    pub is_pb: bool,
    pub cm_per_360: Option<f64>,
    pub sens_changed: bool,
}

/// Aggregated statistics for a Long-Term Trend series.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrendAnalysis {
    pub points: Vec<TrendPoint>,
    pub mean: f64,
    pub std_dev: f64,
    /// Coefficient of variation (std_dev / |mean| * 100)
    pub cv_pct: f64,
    pub min_val: f64,
    pub max_val: f64,
    pub pb_value: f64,
    pub recent_delta_pct: Option<f64>,
}

/// Performance grouped by sensitivity (`cm/360`), FOV, and crosshair.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigGroupStats {
    pub group_key: String,
    pub cm_per_360: Option<f64>,
    pub sens_scale: String,
    pub horiz_sens: f64,
    pub dpi: u32,
    pub fov: f64,
    pub crosshair: String,
    pub run_count: u32,
    pub avg_score: f64,
    pub pb_score: f64,
    pub avg_accuracy: f64,
    pub avg_kps: f64,
    pub avg_one_shot_pct: f64,
}

/// Daily training cell for the calendar heatmap.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyHeatmapCell {
    pub date_key: String,
    pub run_count: u32,
    pub total_duration_s: f64,
    pub new_pbs: u32,
    pub unique_scenarios: u32,
}

/// Summary data for the Training Overview tab.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OverviewStats {
    pub today_runs: u32,
    pub today_duration_s: f64,
    pub today_new_pbs: u32,
    pub total_runs: u32,
    pub total_scenarios: u32,
    pub recent_pbs: Vec<RunRecord>,
    pub heatmap: Vec<DailyHeatmapCell>,
}

/// Persisted user configuration (`config.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub stats_dir: Option<String>,
    pub performances_dir: Option<String>,
    #[serde(default)]
    pub favorites: Vec<String>,
    #[serde(default = "default_ma_window")]
    pub default_ma_window: usize,
    #[serde(default = "default_true")]
    pub notifications_enabled: bool,
    #[serde(default = "default_true")]
    pub single_monitor_mode: bool,
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
    #[serde(default)]
    pub language: Language,
    #[serde(default)]
    pub theme: ThemeMode,
}

fn default_ma_window() -> usize {
    10
}

fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            stats_dir: None,
            performances_dir: None,
            favorites: Vec::new(),
            default_ma_window: default_ma_window(),
            notifications_enabled: true,
            single_monitor_mode: true,
            minimize_to_tray: true,
            language: Language::default(),
            theme: ThemeMode::default(),
        }
    }
}

/// Events emitted by the background file watcher to the GUI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WatcherEvent {
    WatcherReady {
        stats_dir: String,
        performances_dir: String,
    },
    SyncCompleted {
        imported: usize,
        skipped: usize,
        elapsed_ms: u64,
    },
    RunIngested {
        run_id: i64,
        scenario: String,
        score: f64,
        accuracy: f64,
        kps: f64,
        is_new_pb: bool,
        elapsed_ms: u64,
    },
    Error(String),
}
