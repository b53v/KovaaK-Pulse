use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::Path;

use chrono::{Duration, Local};
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::analytics::{
    compute_config_groups, compute_deep_insights, compute_half_split, compute_trend_analysis,
    find_best_kps_tick, find_worst_score_tick,
};
use crate::config::default_db_path;
use crate::model::{
    ConfigGroupStats, DailyHeatmapCell, KillEvent, OverviewStats, ParsedRun, PerfTick, RunDetail,
    RunRecord, ScenarioSummary, TimeRange, TrendAnalysis, TrendMetric,
};
use crate::parser::csv_parser::extract_file_key;
use crate::parser::parse_run_pair;

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_key TEXT NOT NULL UNIQUE,
    scenario TEXT NOT NULL,
    scenario_hash TEXT NOT NULL DEFAULT '',
    mode TEXT NOT NULL DEFAULT 'Challenge',
    played_at TEXT NOT NULL DEFAULT '',
    played_at_ms INTEGER NOT NULL DEFAULT 0,
    date_key TEXT NOT NULL DEFAULT '',
    challenge_start TEXT NOT NULL DEFAULT '',
    game_version TEXT NOT NULL DEFAULT '',
    duration_s REAL NOT NULL DEFAULT 0.0,
    score REAL NOT NULL DEFAULT 0.0,
    kills INTEGER NOT NULL DEFAULT 0,
    deaths INTEGER NOT NULL DEFAULT 0,
    shots INTEGER NOT NULL DEFAULT 0,
    hits INTEGER NOT NULL DEFAULT 0,
    misses INTEGER NOT NULL DEFAULT 0,
    accuracy REAL NOT NULL DEFAULT 0.0,
    damage_done REAL NOT NULL DEFAULT 0.0,
    damage_possible REAL NOT NULL DEFAULT 0.0,
    damage_efficiency REAL NOT NULL DEFAULT 0.0,
    avg_ttk_s REAL NOT NULL DEFAULT 0.0,
    fight_time_s REAL NOT NULL DEFAULT 0.0,
    time_remaining_s REAL NOT NULL DEFAULT 0.0,
    total_overshots INTEGER NOT NULL DEFAULT 0,
    reloads INTEGER NOT NULL DEFAULT 0,
    pause_count INTEGER NOT NULL DEFAULT 0,
    pause_duration_s REAL NOT NULL DEFAULT 0.0,
    distance_traveled REAL NOT NULL DEFAULT 0.0,
    mbs_points REAL NOT NULL DEFAULT 0.0,
    avg_target_scale REAL NOT NULL DEFAULT 1.0,
    avg_time_dilation REAL NOT NULL DEFAULT 1.0,
    avg_kps REAL NOT NULL DEFAULT 0.0,
    peak_kps REAL NOT NULL DEFAULT 0.0,
    one_shot_kills INTEGER NOT NULL DEFAULT 0,
    one_shot_pct REAL NOT NULL DEFAULT 0.0,
    avg_kill_interval_ms REAL NOT NULL DEFAULT 0.0,
    fastest_kill_interval_ms REAL NOT NULL DEFAULT 0.0,
    slowest_kill_interval_ms REAL NOT NULL DEFAULT 0.0,
    multi_shot_avg_ttk_ms REAL NOT NULL DEFAULT 0.0,
    sens_scale TEXT NOT NULL DEFAULT '',
    horiz_sens REAL NOT NULL DEFAULT 0.0,
    vert_sens REAL NOT NULL DEFAULT 0.0,
    dpi INTEGER NOT NULL DEFAULT 0,
    cm_per_360 REAL,
    fov REAL NOT NULL DEFAULT 0.0,
    fov_scale TEXT NOT NULL DEFAULT '',
    hide_gun INTEGER NOT NULL DEFAULT 0,
    crosshair TEXT NOT NULL DEFAULT '',
    crosshair_scale REAL NOT NULL DEFAULT 1.0,
    crosshair_color TEXT NOT NULL DEFAULT '',
    resolution TEXT NOT NULL DEFAULT '',
    avg_fps REAL NOT NULL DEFAULT 0.0,
    max_fps_config REAL NOT NULL DEFAULT 0.0,
    input_lag_ms REAL NOT NULL DEFAULT 0.0,
    resolution_scale REAL NOT NULL DEFAULT 100.0,
    has_perf INTEGER NOT NULL DEFAULT 0,
    is_pb INTEGER NOT NULL DEFAULT 0,
    csv_path TEXT NOT NULL DEFAULT '',
    perf_path TEXT
);

CREATE INDEX IF NOT EXISTS idx_runs_scenario_played ON runs(scenario, played_at_ms);
CREATE INDEX IF NOT EXISTS idx_runs_date_key ON runs(date_key);
CREATE INDEX IF NOT EXISTS idx_runs_played_at_ms ON runs(played_at_ms DESC);

CREATE TABLE IF NOT EXISTS perf_ticks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    tick_index INTEGER NOT NULL,
    time_offset_s REAL NOT NULL,
    shots_fired INTEGER NOT NULL,
    shots_hit INTEGER NOT NULL,
    shots_missed INTEGER NOT NULL,
    accuracy REAL NOT NULL,
    damage_done REAL NOT NULL,
    damage_possible REAL NOT NULL,
    score REAL NOT NULL,
    kills INTEGER NOT NULL,
    deaths INTEGER NOT NULL,
    overshots INTEGER NOT NULL,
    reloads INTEGER NOT NULL,
    pauses INTEGER NOT NULL,
    mbs_points REAL NOT NULL,
    distance_traveled REAL NOT NULL,
    target_size REAL NOT NULL,
    target_speed REAL NOT NULL,
    random_sens_scale REAL NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_perf_ticks_run_id ON perf_ticks(run_id, tick_index);

CREATE TABLE IF NOT EXISTS kill_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    kill_index INTEGER NOT NULL,
    timestamp TEXT NOT NULL,
    elapsed_ms REAL NOT NULL,
    interval_ms REAL,
    bot TEXT NOT NULL,
    weapon TEXT NOT NULL,
    ttk_s REAL NOT NULL,
    shots INTEGER NOT NULL,
    hits INTEGER NOT NULL,
    accuracy REAL NOT NULL,
    damage_done REAL NOT NULL,
    damage_possible REAL NOT NULL,
    efficiency REAL NOT NULL,
    overshots INTEGER NOT NULL,
    cheated INTEGER NOT NULL,
    is_one_shot INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_kill_events_run_id ON kill_events(run_id, kill_index);
"#;

const RUN_SELECT_COLS: &str = r#"
    id, file_key, scenario, scenario_hash, mode, played_at, played_at_ms, date_key,
    challenge_start, game_version, duration_s, score, kills, deaths, shots, hits, misses,
    accuracy, damage_done, damage_possible, damage_efficiency, avg_ttk_s, fight_time_s,
    time_remaining_s, total_overshots, reloads, pause_count, pause_duration_s,
    distance_traveled, mbs_points, avg_target_scale, avg_time_dilation, avg_kps, peak_kps,
    one_shot_kills, one_shot_pct, avg_kill_interval_ms, fastest_kill_interval_ms,
    slowest_kill_interval_ms, multi_shot_avg_ttk_ms, sens_scale, horiz_sens, vert_sens,
    dpi, cm_per_360, fov, fov_scale, hide_gun, crosshair, crosshair_scale, crosshair_color,
    resolution, avg_fps, max_fps_config, input_lag_ms, resolution_scale, has_perf, is_pb,
    csv_path, perf_path
"#;

/// Local SQLite storage wrapper (WAL mode).
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Opens or creates the SQLite database at `path` and initializes the schema.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create DB parent dir: {e}"))?;
            }
        }
        let conn = Connection::open(path).map_err(|e| format!("Failed to open SQLite DB: {e}"))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )
        .map_err(|e| format!("Failed to set SQLite PRAGMAs: {e}"))?;
        conn.execute_batch(SCHEMA_SQL)
            .map_err(|e| format!("Failed to initialize SQLite schema: {e}"))?;
        Ok(Self { conn })
    }

    /// Opens the default application SQLite database.
    pub fn open_default() -> Result<Self, String> {
        Self::open(&default_db_path())
    }

    /// Inserts a parsed run (deduplicated by `file_key`), returning `(run_id, is_new_pb)`.
    /// If the run already exists without `.perf` data and the new `ParsedRun` includes `.perf`,
    /// upgrades the existing row and inserts its `perf_ticks`.
    pub fn ingest_parsed_run(&self, parsed: &ParsedRun) -> Result<(i64, bool), String> {
        let existing: Option<(i64, bool)> = self
            .conn
            .query_row(
                "SELECT id, has_perf FROM runs WHERE file_key = ?1",
                params![parsed.run.file_key],
                |row| Ok((row.get(0)?, row.get::<_, i64>(1)? != 0)),
            )
            .optional()
            .map_err(|e| format!("DB lookup failed: {e}"))?;

        if let Some((existing_id, had_perf)) = existing {
            if !had_perf && parsed.run.has_perf {
                let tx = self
                    .conn
                    .unchecked_transaction()
                    .map_err(|e| format!("Transaction start failed: {e}"))?;
                tx.execute(
                    "UPDATE runs SET duration_s = ?1, avg_kps = ?2, peak_kps = ?3,
                     damage_possible = ?4, damage_efficiency = ?5, has_perf = 1, perf_path = ?6
                     WHERE id = ?7",
                    params![
                        parsed.run.duration_s,
                        parsed.run.avg_kps,
                        parsed.run.peak_kps,
                        parsed.run.damage_possible,
                        parsed.run.damage_efficiency,
                        parsed.run.perf_path,
                        existing_id
                    ],
                )
                .map_err(|e| format!("Failed to upgrade run with perf: {e}"))?;
                tx.execute("DELETE FROM perf_ticks WHERE run_id = ?1", params![existing_id])
                    .map_err(|e| format!("Failed to clear old ticks: {e}"))?;
                insert_perf_ticks(&tx, existing_id, &parsed.ticks)?;
                tx.commit().map_err(|e| format!("Commit failed: {e}"))?;
            }
            return Ok((existing_id, false));
        }

        // Determine if this run sets a new PB for its scenario (chronologically among prior runs)
        let prev_max: Option<f64> = self
            .conn
            .query_row(
                "SELECT MAX(score) FROM runs WHERE scenario = ?1 AND played_at_ms <= ?2",
                params![parsed.run.scenario, parsed.run.played_at_ms],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| format!("PB check failed: {e}"))?
            .flatten();

        let is_new_pb = match prev_max {
            None => parsed.run.score > 0.0,
            Some(best) => parsed.run.score > best + 1e-9,
        };

        let r = &parsed.run;
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| format!("Transaction start failed: {e}"))?;

        tx.execute(
            r#"INSERT INTO runs (
                file_key, scenario, scenario_hash, mode, played_at, played_at_ms, date_key,
                challenge_start, game_version, duration_s, score, kills, deaths, shots, hits, misses,
                accuracy, damage_done, damage_possible, damage_efficiency, avg_ttk_s, fight_time_s,
                time_remaining_s, total_overshots, reloads, pause_count, pause_duration_s,
                distance_traveled, mbs_points, avg_target_scale, avg_time_dilation, avg_kps, peak_kps,
                one_shot_kills, one_shot_pct, avg_kill_interval_ms, fastest_kill_interval_ms,
                slowest_kill_interval_ms, multi_shot_avg_ttk_ms, sens_scale, horiz_sens, vert_sens,
                dpi, cm_per_360, fov, fov_scale, hide_gun, crosshair, crosshair_scale, crosshair_color,
                resolution, avg_fps, max_fps_config, input_lag_ms, resolution_scale, has_perf, is_pb,
                csv_path, perf_path
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31,
                ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44, ?45, ?46,
                ?47, ?48, ?49, ?50, ?51, ?52, ?53, ?54, ?55, ?56, ?57, ?58, ?59
            )"#,
            params![
                r.file_key,
                r.scenario,
                r.scenario_hash,
                r.mode,
                r.played_at,
                r.played_at_ms,
                r.date_key,
                r.challenge_start,
                r.game_version,
                r.duration_s,
                r.score,
                r.kills,
                r.deaths,
                r.shots,
                r.hits,
                r.misses,
                r.accuracy,
                r.damage_done,
                r.damage_possible,
                r.damage_efficiency,
                r.avg_ttk_s,
                r.fight_time_s,
                r.time_remaining_s,
                r.total_overshots,
                r.reloads,
                r.pause_count,
                r.pause_duration_s,
                r.distance_traveled,
                r.mbs_points,
                r.avg_target_scale,
                r.avg_time_dilation,
                r.avg_kps,
                r.peak_kps,
                r.one_shot_kills,
                r.one_shot_pct,
                r.avg_kill_interval_ms,
                r.fastest_kill_interval_ms,
                r.slowest_kill_interval_ms,
                r.multi_shot_avg_ttk_ms,
                r.sens_scale,
                r.horiz_sens,
                r.vert_sens,
                r.dpi,
                r.cm_per_360,
                r.fov,
                r.fov_scale,
                if r.hide_gun { 1i64 } else { 0i64 },
                r.crosshair,
                r.crosshair_scale,
                r.crosshair_color,
                r.resolution,
                r.avg_fps,
                r.max_fps_config,
                r.input_lag_ms,
                r.resolution_scale,
                if r.has_perf { 1i64 } else { 0i64 },
                if is_new_pb { 1i64 } else { 0i64 },
                r.csv_path,
                r.perf_path,
            ],
        )
        .map_err(|e| format!("Failed to insert run: {e}"))?;

        let run_id = tx.last_insert_rowid();
        insert_perf_ticks(&tx, run_id, &parsed.ticks)?;
        insert_kill_events(&tx, run_id, &parsed.kills)?;

        tx.commit().map_err(|e| format!("Commit failed: {e}"))?;
        Ok((run_id, is_new_pb))
    }

    /// Incrementally syncs all `.csv` + `.perf` pairs from `stats_dir` and `perf_dir`.
    /// Returns `(imported_count, skipped_count)`.
    pub fn sync_directories(
        &self,
        stats_dir: &Path,
        perf_dir: &Path,
    ) -> Result<(usize, usize), String> {
        if !stats_dir.exists() {
            return Ok((0, 0));
        }

        // Load existing file_key -> has_perf map
        let mut existing: HashMap<String, bool> = HashMap::new();
        {
            let mut stmt = self
                .conn
                .prepare("SELECT file_key, has_perf FROM runs")
                .map_err(|e| format!("Failed to query existing keys: {e}"))?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0))
                })
                .map_err(|e| format!("Failed to iterate existing keys: {e}"))?;
            for row in rows.flatten() {
                existing.insert(row.0, row.1);
            }
        }

        // Index available `.perf` files by file_key
        let mut perf_map: HashMap<String, std::path::PathBuf> = HashMap::new();
        if perf_dir.exists() {
            if let Ok(entries) = fs::read_dir(perf_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.eq_ignore_ascii_case("perf"))
                        .unwrap_or(false)
                    {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            perf_map.insert(extract_file_key(name), path);
                        }
                    }
                }
            }
        }

        // Collect CSV files sorted by filename (roughly chronological so PB flags are accurate)
        let mut csv_files: Vec<std::path::PathBuf> = Vec::new();
        if let Ok(entries) = fs::read_dir(stats_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("csv"))
                    .unwrap_or(false)
                {
                    csv_files.push(path);
                }
            }
        }
        csv_files.sort();

        let mut imported = 0usize;
        let mut skipped = 0usize;

        for csv_path in csv_files {
            let file_name = match csv_path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };
            let key = extract_file_key(file_name);
            let perf_opt = perf_map.get(&key).map(|p| p.as_path());

            if let Some(&had_perf) = existing.get(&key) {
                if had_perf || perf_opt.is_none() {
                    skipped += 1;
                    continue;
                }
            }

            if let Ok(parsed) = parse_run_pair(&csv_path, perf_opt) {
                if self.ingest_parsed_run(&parsed).is_ok() {
                    imported += 1;
                }
            }
        }

        Ok((imported, skipped))
    }

    /// Lists all scenarios with summary stats (PB, latest score, run count, favorite flag).
    pub fn list_scenarios(&self, favorites: &[String]) -> Result<Vec<ScenarioSummary>, String> {
        let fav_set: HashSet<&str> = favorites.iter().map(|s| s.as_str()).collect();
        let sql = r#"
            SELECT
                r.scenario,
                COUNT(*) AS run_count,
                MAX(r.score) AS pb_score,
                AVG(r.score) AS avg_score,
                MAX(r.played_at_ms) AS max_played_ms
            FROM runs r
            GROUP BY r.scenario
            ORDER BY max_played_ms DESC
        "#;

        let mut stmt = self
            .conn
            .prepare(sql)
            .map_err(|e| format!("Failed to prepare list_scenarios: {e}"))?;
        let base_rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, f64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })
            .map_err(|e| format!("list_scenarios query failed: {e}"))?;

        let mut summaries = Vec::new();
        for item in base_rows {
            let (scenario, run_count, pb_score, avg_score, latest_played_at_ms) =
                item.map_err(|e| format!("Row read error: {e}"))?;

            let (latest_run_id, latest_score, latest_accuracy, latest_played_at) = self
                .conn
                .query_row(
                    "SELECT id, score, accuracy, played_at FROM runs
                     WHERE scenario = ?1
                     ORDER BY played_at_ms DESC, id DESC
                     LIMIT 1",
                    params![&scenario],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, f64>(1)?,
                            row.get::<_, f64>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    },
                )
                .unwrap_or((0, 0.0, 0.0, String::new()));

            let is_favorite = fav_set.contains(scenario.as_str());
            summaries.push(ScenarioSummary {
                scenario,
                run_count,
                pb_score,
                latest_score,
                avg_score,
                latest_accuracy,
                latest_played_at,
                latest_played_at_ms,
                latest_run_id,
                is_favorite,
            });
        }

        Ok(summaries)
    }

    /// Lists runs for a scenario, newest first.
    pub fn list_runs_for_scenario(
        &self,
        scenario: &str,
        limit: usize,
    ) -> Result<Vec<RunRecord>, String> {
        let sql = format!(
            "SELECT {RUN_SELECT_COLS} FROM runs WHERE scenario = ?1 ORDER BY played_at_ms DESC, id DESC LIMIT ?2"
        );
        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|e| format!("Failed to prepare list_runs_for_scenario: {e}"))?;
        let rows = stmt
            .query_map(params![scenario, limit as i64], row_to_run_record)
            .map_err(|e| format!("Query failed: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("Row error: {e}"))?);
        }
        Ok(out)
    }

    /// Fetches full details (run, kills, perf ticks, half-split, delta vs last-10) for a run ID.
    pub fn get_run_detail(&self, run_id: i64) -> Result<Option<RunDetail>, String> {
        let sql = format!("SELECT {RUN_SELECT_COLS} FROM runs WHERE id = ?1");
        let run = match self
            .conn
            .query_row(&sql, params![run_id], row_to_run_record)
            .optional()
            .map_err(|e| format!("Failed to fetch run {run_id}: {e}"))?
        {
            Some(r) => r,
            None => return Ok(None),
        };

        let kills = self.get_kill_events(run_id)?;
        let ticks = self.get_perf_ticks(run_id)?;

        // Compute average of up to 10 preceding runs of the same scenario
        let mut stmt = self
            .conn
            .prepare(
                "SELECT score FROM runs
                 WHERE scenario = ?1 AND id != ?2 AND played_at_ms <= ?3
                 ORDER BY played_at_ms DESC, id DESC
                 LIMIT 10",
            )
            .map_err(|e| format!("Failed to query last 10 runs: {e}"))?;
        let prev_scores: Vec<f64> = stmt
            .query_map(params![&run.scenario, run.id, run.played_at_ms], |row| {
                row.get(0)
            })
            .map_err(|e| format!("Last 10 query failed: {e}"))?
            .filter_map(Result::ok)
            .collect();

        let (last10_avg_score, delta_vs_last10) = if !prev_scores.is_empty() {
            let avg = prev_scores.iter().sum::<f64>() / prev_scores.len() as f64;
            (Some(avg), Some(run.score - avg))
        } else {
            (None, None)
        };

        let half_split = compute_half_split(&ticks);
        let deep_insights = compute_deep_insights(&run, &kills, &ticks);
        let worst_score_tick = find_worst_score_tick(&ticks);
        let best_kps_tick = find_best_kps_tick(&ticks);

        Ok(Some(RunDetail {
            run,
            kills,
            ticks,
            half_split,
            deep_insights,
            last10_avg_score,
            delta_vs_last10,
            worst_score_tick,
            best_kps_tick,
        }))
    }

    /// Fetches the latest run detail (optionally filtered to a specific scenario).
    pub fn get_latest_run_detail(&self, scenario: Option<&str>) -> Result<Option<RunDetail>, String> {
        let id_opt: Option<i64> = match scenario {
            Some(sc) => self
                .conn
                .query_row(
                    "SELECT id FROM runs WHERE scenario = ?1 ORDER BY played_at_ms DESC, id DESC LIMIT 1",
                    params![sc],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| format!("Latest run query failed: {e}"))?,
            None => self
                .conn
                .query_row(
                    "SELECT id FROM runs ORDER BY played_at_ms DESC, id DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| format!("Latest run query failed: {e}"))?,
        };

        match id_opt {
            Some(id) => self.get_run_detail(id),
            None => Ok(None),
        }
    }

    /// Computes long-term trend analysis for a scenario within `range`.
    pub fn get_trend_analysis(
        &self,
        scenario: &str,
        metric: TrendMetric,
        range: TimeRange,
        ma_window: usize,
    ) -> Result<TrendAnalysis, String> {
        let runs = self.query_runs_in_range(scenario, range)?;
        Ok(compute_trend_analysis(&runs, metric, ma_window))
    }

    /// Computes config-group comparison table (`cm/360` + FOV + Crosshair) for a scenario.
    pub fn get_config_groups(&self, scenario: &str) -> Result<Vec<ConfigGroupStats>, String> {
        let runs = self.query_runs_in_range(scenario, TimeRange::All)?;
        Ok(compute_config_groups(&runs))
    }

    /// Computes overview stats (today's summary, recent PBs, and daily heatmap).
    pub fn get_overview_stats(&self, heatmap_days: i64) -> Result<OverviewStats, String> {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let cutoff = (Local::now() - Duration::days(heatmap_days.max(1)))
            .format("%Y-%m-%d")
            .to_string();

        let (today_runs, today_duration_s, today_new_pbs): (u32, f64, u32) = self
            .conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(duration_s), 0.0), COALESCE(SUM(is_pb), 0)
                 FROM runs WHERE date_key = ?1",
                params![&today],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap_or((0, 0.0, 0));

        let (total_runs, total_scenarios): (u32, u32) = self
            .conn
            .query_row(
                "SELECT COUNT(*), COUNT(DISTINCT scenario) FROM runs",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or((0, 0));

        // Recent PBs
        let pb_sql = format!(
            "SELECT {RUN_SELECT_COLS} FROM runs WHERE is_pb = 1 ORDER BY played_at_ms DESC, id DESC LIMIT 15"
        );
        let mut pb_stmt = self
            .conn
            .prepare(&pb_sql)
            .map_err(|e| format!("Recent PBs query failed: {e}"))?;
        let recent_pbs: Vec<RunRecord> = pb_stmt
            .query_map([], row_to_run_record)
            .map_err(|e| format!("Recent PBs iter failed: {e}"))?
            .filter_map(Result::ok)
            .collect();

        // Daily heatmap
        let mut hm_stmt = self
            .conn
            .prepare(
                "SELECT date_key, COUNT(*), COALESCE(SUM(duration_s), 0.0),
                        COALESCE(SUM(is_pb), 0), COUNT(DISTINCT scenario)
                 FROM runs
                 WHERE date_key != '' AND date_key >= ?1
                 GROUP BY date_key
                 ORDER BY date_key ASC",
            )
            .map_err(|e| format!("Heatmap query failed: {e}"))?;
        let heatmap: Vec<DailyHeatmapCell> = hm_stmt
            .query_map(params![&cutoff], |row| {
                Ok(DailyHeatmapCell {
                    date_key: row.get(0)?,
                    run_count: row.get(1)?,
                    total_duration_s: row.get(2)?,
                    new_pbs: row.get(3)?,
                    unique_scenarios: row.get(4)?,
                })
            })
            .map_err(|e| format!("Heatmap iter failed: {e}"))?
            .filter_map(Result::ok)
            .collect();

        Ok(OverviewStats {
            today_runs,
            today_duration_s,
            today_new_pbs,
            total_runs,
            total_scenarios,
            recent_pbs,
            heatmap,
        })
    }

    /// Exports all runs for a scenario to a CSV file, returning the number of rows written.
    pub fn export_scenario_csv(&self, scenario: &str, out_path: &Path) -> Result<usize, String> {
        let runs = self.query_runs_in_range(scenario, TimeRange::All)?;
        let mut file =
            fs::File::create(out_path).map_err(|e| format!("Failed to create export CSV: {e}"))?;
        writeln!(
            file,
            "played_at,scenario,mode,score,is_pb,kills,shots,hits,accuracy_pct,avg_kps,peak_kps,avg_ttk_ms,one_shot_pct,cm_per_360,sens_scale,horiz_sens,dpi,fov,crosshair,avg_fps"
        )
        .map_err(|e| format!("CSV write failed: {e}"))?;

        for r in &runs {
            writeln!(
                file,
                "\"{}\",\"{}\",\"{}\",{:.4},{},{},{},{},{:.2},{:.3},{:.2},{:.2},{:.2},{},\"{}\",{:.4},{},{:.1},\"{}\",{:.1}",
                r.played_at,
                r.scenario.replace('"', "\"\""),
                r.mode,
                r.score,
                if r.is_pb { 1 } else { 0 },
                r.kills,
                r.shots,
                r.hits,
                r.accuracy,
                r.avg_kps,
                r.peak_kps,
                r.avg_ttk_s * 1000.0,
                r.one_shot_pct,
                r.cm_per_360
                    .map(|v| format!("{v:.2}"))
                    .unwrap_or_default(),
                r.sens_scale,
                r.horiz_sens,
                r.dpi,
                r.fov,
                r.crosshair.replace('"', "\"\""),
                r.avg_fps,
            )
            .map_err(|e| format!("CSV row write failed: {e}"))?;
        }
        Ok(runs.len())
    }

    fn query_runs_in_range(
        &self,
        scenario: &str,
        range: TimeRange,
    ) -> Result<Vec<RunRecord>, String> {
        let now = Local::now();
        let cutoff_date: Option<String> = match range {
            TimeRange::Today => Some(now.format("%Y-%m-%d").to_string()),
            TimeRange::Days7 => Some((now - Duration::days(6)).format("%Y-%m-%d").to_string()),
            TimeRange::Days30 => Some((now - Duration::days(29)).format("%Y-%m-%d").to_string()),
            TimeRange::All => None,
        };

        let mut out = Vec::new();
        if let Some(date_min) = cutoff_date {
            let sql = format!(
                "SELECT {RUN_SELECT_COLS} FROM runs
                 WHERE scenario = ?1 AND date_key >= ?2
                 ORDER BY played_at_ms ASC, id ASC"
            );
            let mut stmt = self
                .conn
                .prepare(&sql)
                .map_err(|e| format!("Range query prepare failed: {e}"))?;
            let rows = stmt
                .query_map(params![scenario, date_min], row_to_run_record)
                .map_err(|e| format!("Range query failed: {e}"))?;
            for r in rows {
                out.push(r.map_err(|e| format!("Row error: {e}"))?);
            }
        } else {
            let sql = format!(
                "SELECT {RUN_SELECT_COLS} FROM runs
                 WHERE scenario = ?1
                 ORDER BY played_at_ms ASC, id ASC"
            );
            let mut stmt = self
                .conn
                .prepare(&sql)
                .map_err(|e| format!("All-range query prepare failed: {e}"))?;
            let rows = stmt
                .query_map(params![scenario], row_to_run_record)
                .map_err(|e| format!("All-range query failed: {e}"))?;
            for r in rows {
                out.push(r.map_err(|e| format!("Row error: {e}"))?);
            }
        }
        Ok(out)
    }

    fn get_perf_ticks(&self, run_id: i64) -> Result<Vec<PerfTick>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT tick_index, time_offset_s, shots_fired, shots_hit, shots_missed,
                        accuracy, damage_done, damage_possible, score, kills, deaths,
                        overshots, reloads, pauses, mbs_points, distance_traveled,
                        target_size, target_speed, random_sens_scale
                 FROM perf_ticks WHERE run_id = ?1 ORDER BY tick_index ASC",
            )
            .map_err(|e| format!("Perf ticks prepare failed: {e}"))?;
        let rows = stmt
            .query_map(params![run_id], |row| {
                Ok(PerfTick {
                    tick_index: row.get(0)?,
                    time_offset_s: row.get(1)?,
                    shots_fired: row.get(2)?,
                    shots_hit: row.get(3)?,
                    shots_missed: row.get(4)?,
                    accuracy: row.get(5)?,
                    damage_done: row.get(6)?,
                    damage_possible: row.get(7)?,
                    score: row.get(8)?,
                    kills: row.get(9)?,
                    deaths: row.get(10)?,
                    overshots: row.get(11)?,
                    reloads: row.get(12)?,
                    pauses: row.get(13)?,
                    mbs_points: row.get(14)?,
                    distance_traveled: row.get(15)?,
                    target_size: row.get(16)?,
                    target_speed: row.get(17)?,
                    random_sens_scale: row.get(18)?,
                })
            })
            .map_err(|e| format!("Perf ticks query failed: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("Tick row error: {e}"))?);
        }
        Ok(out)
    }

    fn get_kill_events(&self, run_id: i64) -> Result<Vec<KillEvent>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT kill_index, timestamp, elapsed_ms, interval_ms, bot, weapon,
                        ttk_s, shots, hits, accuracy, damage_done, damage_possible,
                        efficiency, overshots, cheated, is_one_shot
                 FROM kill_events WHERE run_id = ?1 ORDER BY kill_index ASC",
            )
            .map_err(|e| format!("Kill events prepare failed: {e}"))?;
        let rows = stmt
            .query_map(params![run_id], |row| {
                Ok(KillEvent {
                    kill_index: row.get(0)?,
                    timestamp: row.get(1)?,
                    elapsed_ms: row.get(2)?,
                    interval_ms: row.get(3)?,
                    bot: row.get(4)?,
                    weapon: row.get(5)?,
                    ttk_s: row.get(6)?,
                    shots: row.get(7)?,
                    hits: row.get(8)?,
                    accuracy: row.get(9)?,
                    damage_done: row.get(10)?,
                    damage_possible: row.get(11)?,
                    efficiency: row.get(12)?,
                    overshots: row.get(13)?,
                    cheated: row.get::<_, i64>(14)? != 0,
                    is_one_shot: row.get::<_, i64>(15)? != 0,
                })
            })
            .map_err(|e| format!("Kill events query failed: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("Kill row error: {e}"))?);
        }
        Ok(out)
    }
}

fn insert_perf_ticks(
    tx: &rusqlite::Transaction<'_>,
    run_id: i64,
    ticks: &[PerfTick],
) -> Result<(), String> {
    if ticks.is_empty() {
        return Ok(());
    }
    let mut stmt = tx
        .prepare(
            "INSERT INTO perf_ticks (
                run_id, tick_index, time_offset_s, shots_fired, shots_hit, shots_missed,
                accuracy, damage_done, damage_possible, score, kills, deaths, overshots,
                reloads, pauses, mbs_points, distance_traveled, target_size, target_speed,
                random_sens_scale
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
        )
        .map_err(|e| format!("Failed to prepare tick insert: {e}"))?;

    for t in ticks {
        stmt.execute(params![
            run_id,
            t.tick_index,
            t.time_offset_s,
            t.shots_fired,
            t.shots_hit,
            t.shots_missed,
            t.accuracy,
            t.damage_done,
            t.damage_possible,
            t.score,
            t.kills,
            t.deaths,
            t.overshots,
            t.reloads,
            t.pauses,
            t.mbs_points,
            t.distance_traveled,
            t.target_size,
            t.target_speed,
            t.random_sens_scale,
        ])
        .map_err(|e| format!("Failed to insert perf tick: {e}"))?;
    }
    Ok(())
}

fn insert_kill_events(
    tx: &rusqlite::Transaction<'_>,
    run_id: i64,
    kills: &[KillEvent],
) -> Result<(), String> {
    if kills.is_empty() {
        return Ok(());
    }
    let mut stmt = tx
        .prepare(
            "INSERT INTO kill_events (
                run_id, kill_index, timestamp, elapsed_ms, interval_ms, bot, weapon,
                ttk_s, shots, hits, accuracy, damage_done, damage_possible, efficiency,
                overshots, cheated, is_one_shot
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        )
        .map_err(|e| format!("Failed to prepare kill insert: {e}"))?;

    for k in kills {
        stmt.execute(params![
            run_id,
            k.kill_index,
            k.timestamp,
            k.elapsed_ms,
            k.interval_ms,
            k.bot,
            k.weapon,
            k.ttk_s,
            k.shots,
            k.hits,
            k.accuracy,
            k.damage_done,
            k.damage_possible,
            k.efficiency,
            k.overshots,
            if k.cheated { 1i64 } else { 0i64 },
            if k.is_one_shot { 1i64 } else { 0i64 },
        ])
        .map_err(|e| format!("Failed to insert kill event: {e}"))?;
    }
    Ok(())
}

fn row_to_run_record(row: &Row<'_>) -> rusqlite::Result<RunRecord> {
    Ok(RunRecord {
        id: row.get(0)?,
        file_key: row.get(1)?,
        scenario: row.get(2)?,
        scenario_hash: row.get(3)?,
        mode: row.get(4)?,
        played_at: row.get(5)?,
        played_at_ms: row.get(6)?,
        date_key: row.get(7)?,
        challenge_start: row.get(8)?,
        game_version: row.get(9)?,
        duration_s: row.get(10)?,
        score: row.get(11)?,
        kills: row.get(12)?,
        deaths: row.get(13)?,
        shots: row.get(14)?,
        hits: row.get(15)?,
        misses: row.get(16)?,
        accuracy: row.get(17)?,
        damage_done: row.get(18)?,
        damage_possible: row.get(19)?,
        damage_efficiency: row.get(20)?,
        avg_ttk_s: row.get(21)?,
        fight_time_s: row.get(22)?,
        time_remaining_s: row.get(23)?,
        total_overshots: row.get(24)?,
        reloads: row.get(25)?,
        pause_count: row.get(26)?,
        pause_duration_s: row.get(27)?,
        distance_traveled: row.get(28)?,
        mbs_points: row.get(29)?,
        avg_target_scale: row.get(30)?,
        avg_time_dilation: row.get(31)?,
        avg_kps: row.get(32)?,
        peak_kps: row.get(33)?,
        one_shot_kills: row.get(34)?,
        one_shot_pct: row.get(35)?,
        avg_kill_interval_ms: row.get(36)?,
        fastest_kill_interval_ms: row.get(37)?,
        slowest_kill_interval_ms: row.get(38)?,
        multi_shot_avg_ttk_ms: row.get(39)?,
        sens_scale: row.get(40)?,
        horiz_sens: row.get(41)?,
        vert_sens: row.get(42)?,
        dpi: row.get(43)?,
        cm_per_360: row.get(44)?,
        fov: row.get(45)?,
        fov_scale: row.get(46)?,
        hide_gun: row.get::<_, i64>(47)? != 0,
        crosshair: row.get(48)?,
        crosshair_scale: row.get(49)?,
        crosshair_color: row.get(50)?,
        resolution: row.get(51)?,
        avg_fps: row.get(52)?,
        max_fps_config: row.get(53)?,
        input_lag_ms: row.get(54)?,
        resolution_scale: row.get(55)?,
        has_perf: row.get::<_, i64>(56)? != 0,
        is_pb: row.get::<_, i64>(57)? != 0,
        csv_path: row.get(58)?,
        perf_path: row.get(59)?,
    })
}
