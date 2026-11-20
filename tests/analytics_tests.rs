use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{Duration, Local};

use kovaak_pulse::model::{ParsedRun, RunRecord, TimeRange, TrendMetric};
use kovaak_pulse::parser::parse_run_pair;
use kovaak_pulse::storage::db::Database;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_db_path(label: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("kovaak_pulse_test_{label}_{pid}_{n}.db"))
}

fn make_synthetic_run(
    file_key: &str,
    scenario: &str,
    days_ago: i64,
    minute_offset: i64,
    score: f64,
    accuracy: f64,
    cm_per_360: f64,
    horiz_sens: f64,
    crosshair: &str,
) -> ParsedRun {
    let dt = Local::now() - Duration::days(days_ago) + Duration::minutes(minute_offset);
    let played_at = dt.format("%Y-%m-%d %H:%M:%S").to_string();
    let date_key = dt.format("%Y-%m-%d").to_string();
    let played_at_ms = dt.timestamp_millis();

    let run = RunRecord {
        file_key: file_key.to_string(),
        scenario: scenario.to_string(),
        mode: "Challenge".to_string(),
        played_at,
        played_at_ms,
        date_key,
        duration_s: 60.0,
        score,
        kills: 60,
        shots: 75,
        hits: 60,
        misses: 15,
        accuracy,
        avg_kps: 1.0,
        peak_kps: 2.0,
        one_shot_kills: 50,
        one_shot_pct: 83.33,
        avg_ttk_s: 0.28,
        sens_scale: "Counter-Strike".to_string(),
        horiz_sens,
        vert_sens: horiz_sens,
        dpi: 400,
        cm_per_360: Some(cm_per_360),
        fov: 103.0,
        fov_scale: "Overwatch".to_string(),
        crosshair: crosshair.to_string(),
        resolution: "1280x960".to_string(),
        avg_fps: 900.0,
        ..RunRecord::default()
    };

    ParsedRun {
        run,
        kills: Vec::new(),
        ticks: Vec::new(),
        weapons: Vec::new(),
    }
}

#[test]
fn test_sqlite_deduplication_and_pb_tracking() {
    let db_path = temp_db_path("dedup");
    let db = Database::open(&db_path).expect("open db failed");

    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let csv_path = base.join("Micro Flick - Challenge - 2026.11.20-21.58.16 Stats.csv");
    let perf_path = base.join("Micro Flick - Challenge - 2026.11.20-21.58.16 Performance.perf");

    let parsed = parse_run_pair(&csv_path, Some(&perf_path)).expect("parse fixture failed");

    let (id1, pb1) = db.ingest_parsed_run(&parsed).expect("first ingest failed");
    assert!(id1 > 0);
    assert!(pb1, "first run for scenario should be a new PB");

    // Duplicate ingest of exact same file_key must deduplicate cleanly
    let (id2, pb2) = db.ingest_parsed_run(&parsed).expect("second ingest failed");
    assert_eq!(id1, id2, "duplicate ingest should return existing run_id");
    assert!(!pb2, "duplicate ingest must not mark new PB");

    let runs = db
        .list_runs_for_scenario("Micro Flick", 50)
        .expect("list_runs_for_scenario failed");
    assert_eq!(runs.len(), 1);

    let detail = db
        .get_run_detail(id1)
        .expect("get_run_detail failed")
        .expect("run detail should exist");
    assert_eq!(detail.kills.len(), 81);
    assert_eq!(detail.ticks.len(), 24);
    assert!(detail.half_split.is_some());

    drop(db);
    let _ = fs::remove_file(&db_path);
}

#[test]
fn test_pb_step_ma_sens_change_and_config_groups() {
    let db_path = temp_db_path("trend");
    let db = Database::open(&db_path).expect("open db failed");

    // 12 chronological runs:
    // Runs 0..6: 83.1 cm/360 (1.25 @ 400 DPI, RoundDot1.png)
    // Runs 6..12: 41.6 cm/360 (2.50 @ 400 DPI, Plus.png) -> sensitivity change at index 6
    let scores = [
        6000.0, 6200.0, 6100.0, 6500.0, 6400.0, 6300.0, // group A (6 runs, avg 6250, PB 6500)
        6600.0, 6800.0, 6700.0, 7000.0, 6900.0, 7200.0, // group B (6 runs, avg 6866.67, PB 7200)
    ];

    for (i, &score) in scores.iter().enumerate() {
        let (cm, sens, ch) = if i < 6 {
            (83.1, 1.25, "RoundDot1.png")
        } else {
            (41.6, 2.50, "Plus.png")
        };
        let run = make_synthetic_run(
            &format!("Micro Flick - Challenge - 2026.11.10-10.{:02}.00", i),
            "Micro Flick",
            5 - (i as i64 / 3),
            i as i64,
            score,
            85.0 + (i as f64) * 0.5,
            cm,
            sens,
            ch,
        );
        db.ingest_parsed_run(&run).expect("ingest failed");
    }

    // MA5 verification
    let trend5 = db
        .get_trend_analysis("Micro Flick", TrendMetric::Score, TimeRange::All, 5)
        .expect("get_trend_analysis MA5 failed");
    assert_eq!(trend5.points.len(), 12);
    assert!((trend5.pb_value - 7200.0).abs() < 1e-6);

    // Check PB step progression
    let expected_pbs = [
        6000.0, 6200.0, 6200.0, 6500.0, 6500.0, 6500.0, 6600.0, 6800.0, 6800.0, 7000.0, 7000.0,
        7200.0,
    ];
    let expected_is_pb = [
        true, true, false, true, false, false, true, true, false, true, false, true,
    ];
    for (i, pt) in trend5.points.iter().enumerate() {
        assert!(
            (pt.pb_envelope - expected_pbs[i]).abs() < 1e-6,
            "idx {i} pb_envelope={} expected={}",
            pt.pb_envelope,
            expected_pbs[i]
        );
        assert_eq!(pt.is_pb, expected_is_pb[i], "idx {i} is_pb mismatch");
    }

    // Check MA5 at index 4 (avg of first 5: 6000,6200,6100,6500,6400 = 6240)
    assert!((trend5.points[4].ma_value.unwrap() - 6240.0).abs() < 1e-6);
    // Check MA5 at index 11 (avg of last 5: 6800,6700,7000,6900,7200 = 6920)
    assert!((trend5.points[11].ma_value.unwrap() - 6920.0).abs() < 1e-6);

    // Check sensitivity change detection (only at index 6)
    for (i, pt) in trend5.points.iter().enumerate() {
        assert_eq!(
            pt.sens_changed,
            i == 6,
            "idx {i} sens_changed={} expected={}",
            pt.sens_changed,
            i == 6
        );
    }

    // MA10 verification at index 11 (avg of indices 2..=11)
    let trend10 = db
        .get_trend_analysis("Micro Flick", TrendMetric::Score, TimeRange::All, 10)
        .expect("get_trend_analysis MA10 failed");
    let expected_ma10_last: f64 = scores[2..12].iter().sum::<f64>() / 10.0;
    assert!((trend10.points[11].ma_value.unwrap() - expected_ma10_last).abs() < 1e-6);

    // Config group comparison
    let groups = db
        .get_config_groups("Micro Flick")
        .expect("get_config_groups failed");
    assert_eq!(groups.len(), 2, "expected 2 config groups");
    // Sorted by avg_score descending -> group B (41.6 cm/360) first, group A (83.1 cm/360) second
    assert_eq!(groups[0].run_count, 6);
    assert!((groups[0].pb_score - 7200.0).abs() < 1e-6);
    assert!((groups[0].avg_score - (41200.0 / 6.0)).abs() < 1e-3);
    assert_eq!(groups[1].run_count, 6);
    assert!((groups[1].pb_score - 6500.0).abs() < 1e-6);
    assert!((groups[1].avg_score - 6250.0).abs() < 1e-3);

    drop(db);
    let _ = fs::remove_file(&db_path);
}

#[test]
fn test_heatmap_and_overview_aggregation() {
    let db_path = temp_db_path("heatmap");
    let db = Database::open(&db_path).expect("open db failed");

    // 2 runs today (Micro Flick: 1 PB, 1 lower), 1 run 2 days ago (1wall6targets TE: 1 PB)
    let r1 = make_synthetic_run("mf-1", "Micro Flick", 0, 1, 7000.0, 88.0, 83.1, 1.25, "Dot");
    let r2 = make_synthetic_run("mf-2", "Micro Flick", 0, 2, 6800.0, 85.0, 83.1, 1.25, "Dot");
    let r3 = make_synthetic_run("1w6t-1", "1wall6targets TE", 2, 1, 140.0, 92.0, 41.6, 2.50, "Dot");

    db.ingest_parsed_run(&r1).unwrap();
    db.ingest_parsed_run(&r2).unwrap();
    db.ingest_parsed_run(&r3).unwrap();

    let overview = db.get_overview_stats(30).expect("get_overview_stats failed");
    assert_eq!(overview.total_runs, 3);
    assert_eq!(overview.total_scenarios, 2);
    assert_eq!(overview.today_runs, 2);
    assert_eq!(overview.today_new_pbs, 1);

    let today_key = Local::now().format("%Y-%m-%d").to_string();
    let two_days_ago_key = (Local::now() - Duration::days(2))
        .format("%Y-%m-%d")
        .to_string();

    let today_cell = overview
        .heatmap
        .iter()
        .find(|c| c.date_key == today_key)
        .expect("today heatmap cell missing");
    assert_eq!(today_cell.run_count, 2);
    assert_eq!(today_cell.new_pbs, 1);
    assert_eq!(today_cell.unique_scenarios, 1);

    let older_cell = overview
        .heatmap
        .iter()
        .find(|c| c.date_key == two_days_ago_key)
        .expect("2-days-ago heatmap cell missing");
    assert_eq!(older_cell.run_count, 1);
    assert_eq!(older_cell.new_pbs, 1);

    drop(db);
    let _ = fs::remove_file(&db_path);
}
