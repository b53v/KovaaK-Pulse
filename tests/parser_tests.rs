use std::path::PathBuf;

use kovaak_pulse::analytics::compute_half_split;
use kovaak_pulse::parser::csv_parser::{compute_cm_per_360, parse_csv_str};
use kovaak_pulse::parser::parse_run_pair;
use kovaak_pulse::parser::perf_parser::parse_perf_file;

fn fixture_paths() -> (PathBuf, PathBuf) {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    (
        base.join("Micro Flick - Challenge - 2026.11.20-21.58.16 Stats.csv"),
        base.join("Micro Flick - Challenge - 2026.11.20-21.58.16 Performance.perf"),
    )
}

#[test]
fn test_real_micro_flick_csv_and_perf() {
    let (csv_path, perf_path) = fixture_paths();
    assert!(csv_path.exists(), "CSV fixture missing at {:?}", csv_path);
    assert!(perf_path.exists(), "Perf fixture missing at {:?}", perf_path);

    let parsed = parse_run_pair(&csv_path, Some(&perf_path))
        .expect("Failed to parse real Micro Flick CSV + Perf pair");

    let run = &parsed.run;
    assert_eq!(run.scenario, "Micro Flick");
    assert_eq!(run.file_key, "Micro Flick - Challenge - 2026.11.20-21.58.16");
    assert!((run.score - 7054.838867).abs() < 1e-4, "score={}", run.score);
    assert_eq!(run.kills, 81);
    assert_eq!(run.shots, 93);
    assert_eq!(run.hits, 81);
    assert_eq!(run.misses, 12);

    let expected_acc = (81.0 / 93.0) * 100.0; // ~87.0968%
    assert!(
        (run.accuracy - expected_acc).abs() < 0.02,
        "accuracy={} expected={}",
        run.accuracy,
        expected_acc
    );

    let cm = run.cm_per_360.expect("cm_per_360 should be computed");
    assert!(
        (cm - 83.12727).abs() < 0.05,
        "cm/360={} expected ~83.13",
        cm
    );

    assert_eq!(run.one_shot_kills, 70);
    let expected_one_shot_pct = (70.0 / 81.0) * 100.0; // ~86.4198%
    assert!(
        (run.one_shot_pct - expected_one_shot_pct).abs() < 0.05,
        "one_shot_pct={} expected={}",
        run.one_shot_pct,
        expected_one_shot_pct
    );

    assert_eq!(parsed.kills.len(), 81);
    assert!(run.has_perf, "has_perf should be true");
    assert_eq!(parsed.ticks.len(), 24, "expected 24 per-second perf ticks");

    // Cross-check `.perf` tick sums against CSV totals
    let sum_shots_fired: u32 = parsed.ticks.iter().map(|t| t.shots_fired).sum();
    let sum_shots_hit: u32 = parsed.ticks.iter().map(|t| t.shots_hit).sum();
    let sum_shots_missed: u32 = parsed.ticks.iter().map(|t| t.shots_missed).sum();
    let sum_damage_done: f64 = parsed.ticks.iter().map(|t| t.damage_done).sum();
    let sum_score: f64 = parsed.ticks.iter().map(|t| t.score).sum();
    let sum_kills: u32 = parsed.ticks.iter().map(|t| t.kills).sum();

    assert_eq!(sum_shots_fired, 93);
    assert_eq!(sum_shots_hit, 81);
    assert_eq!(sum_shots_missed, 12);
    assert!((sum_damage_done - 2025.0).abs() < 1e-3, "sum_damage_done={}", sum_damage_done);
    assert!((sum_score - 7054.838867).abs() < 1e-2, "sum_score={}", sum_score);
    assert_eq!(sum_kills, 81);

    // Direct perf parser check
    let perf_only = parse_perf_file(&perf_path).expect("parse_perf_file failed");
    assert_eq!(perf_only.ticks.len(), 24);
    assert_eq!(perf_only.header.scenario_name, "Micro Flick");

    // Half-split comparison check
    let half = compute_half_split(&parsed.ticks).expect("half_split should be Some");
    assert_eq!(half.first_half_ticks, 12);
    assert_eq!(half.second_half_ticks, 12);
    assert_eq!(half.first_half_kills + half.second_half_kills, 81);
    assert!(!half.diagnosis.is_empty());
}

#[test]
fn test_missing_perf_fallback() {
    let (csv_path, _) = fixture_paths();
    let missing_perf = csv_path.with_file_name("does_not_exist.perf");

    let parsed_none = parse_run_pair(&csv_path, None).expect("CSV-only parse (None) failed");
    assert!(!parsed_none.run.has_perf);
    assert!(parsed_none.ticks.is_empty());
    assert_eq!(parsed_none.kills.len(), 81);
    assert!((parsed_none.run.score - 7054.838867).abs() < 1e-4);
    assert!(parsed_none.run.peak_kps > 0.0);

    let parsed_missing =
        parse_run_pair(&csv_path, Some(&missing_perf)).expect("CSV-only parse (missing) failed");
    assert!(!parsed_missing.run.has_perf);
    assert!(parsed_missing.ticks.is_empty());
    assert_eq!(parsed_missing.run.kills, 81);
}

#[test]
fn test_zero_kill_tracking_csv() {
    let tracking_csv = "\
Kill #,Timestamp,Bot,Weapon,TTK,Shots,Hits,Accuracy,Damage Done,Damage Possible,Efficiency,Cheated,OverShots

Weapon,Shots,Hits,Damage Done,Damage Possible,,Sens Scale,Horiz Sens,Vert Sens,FOV,Hide Gun,Crosshair,Crosshair Scale,Crosshair Color,ADS Sens,ADS Zoom Scale,Avg Target Scale,Avg Time Dilation
Lightning Gun,400,260,2600.0,4000.0,

Kills:,0
Deaths:,0
Fight Time:,60.0
Time Remaining:,0.0
Avg TTK:,0.0
Damage Done:,2600.0
Total Overshots:,0
Damage Taken:,0.0
Hit Count:,260
Miss Count:,140
Score:,4200.5
Scenario:,Smoothbot Invincible Goated
Hash:,abc123
Game Version:,3.9.14
Challenge Start:,20:00:00.000
Pause Count:,0
Pause Duration:,0

Sens Scale:,Valorant
Horiz Sens:,0.35
Vert Sens:,0.35
DPI:,800
FOV:,103.0
FOVScale:,Overwatch
Crosshair:,Plus.png
Resolution:,1920x1080
Avg FPS:,360.0
";

    let parsed = parse_csv_str(
        tracking_csv,
        "Smoothbot Invincible Goated - Challenge - 2026.11.20-20.01.00 Stats.csv",
    )
    .expect("0-kill Tracking CSV should parse cleanly");

    assert_eq!(parsed.run.scenario, "Smoothbot Invincible Goated");
    assert_eq!(parsed.run.kills, 0);
    assert!(parsed.kills.is_empty());
    assert_eq!(parsed.run.shots, 400);
    assert_eq!(parsed.run.hits, 260);
    assert_eq!(parsed.run.misses, 140);
    assert!((parsed.run.accuracy - 65.0).abs() < 0.01);
    assert!((parsed.run.score - 4200.5).abs() < 1e-4);
    assert_eq!(parsed.run.one_shot_kills, 0);
    assert_eq!(parsed.run.one_shot_pct, 0.0);
    assert!(parsed.run.cm_per_360.is_some());
}

#[test]
fn test_cm_per_360_formulas() {
    // Counter-Strike: 1.25 @ 400 DPI => 360 / (1.25 * 0.022 * 400) * 2.54 = 83.12727...
    let cs = compute_cm_per_360("Counter-Strike", 1.25, 400, 103.0).unwrap();
    assert!((cs - 83.12727).abs() < 0.01, "cs={}", cs);

    // Direct cm/360
    let direct = compute_cm_per_360("cm/360", 45.0, 800, 103.0).unwrap();
    assert!((direct - 45.0).abs() < 1e-6);

    // Invalid inputs
    assert!(compute_cm_per_360("Counter-Strike", 0.0, 400, 103.0).is_none());
    assert!(compute_cm_per_360("Counter-Strike", 1.25, 0, 103.0).is_none());
}
