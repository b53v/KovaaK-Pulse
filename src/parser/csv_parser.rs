use std::collections::HashMap;
use std::f64::consts::PI;
use std::fs;
use std::path::Path;

use chrono::{Local, NaiveDateTime, TimeZone};

use crate::model::{KillEvent, ParsedRun, RunRecord, WeaponSummary};

/// Parses a KovaaK's `... Stats.csv` file from disk.
pub fn parse_csv_file(path: &Path) -> Result<ParsedRun, String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("Failed to read CSV file {:?}: {e}", path))?;
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let mut parsed = parse_csv_str(&content, file_name)?;
    parsed.run.csv_path = path.to_string_lossy().into_owned();
    Ok(parsed)
}

/// Extracts the shared file key from a KovaaK's `.csv` or `.perf` filename.
/// Example: `Micro Flick - Challenge - 2026.11.20-21.58.16 Stats.csv`
///       -> `Micro Flick - Challenge - 2026.11.20-21.58.16`
pub fn extract_file_key(file_name: &str) -> String {
    let stripped = file_name
        .strip_suffix(" Stats.csv")
        .or_else(|| file_name.strip_suffix(" Performance.perf"))
        .or_else(|| file_name.strip_suffix(".csv"))
        .or_else(|| file_name.strip_suffix(".perf"))
        .unwrap_or(file_name);
    stripped.trim().to_string()
}

/// Extracts `(scenario_from_name, mode, played_at, played_at_ms, date_key)` from a `file_key`.
fn parse_file_key_metadata(file_key: &str) -> (String, String, String, i64, String) {
    // Expected format: "<Scenario> - <Mode> - YYYY.MM.DD-HH.MM.SS"
    let parts: Vec<&str> = file_key.rsplitn(3, " - ").collect();
    if parts.len() == 3 {
        let ts_str = parts[0].trim();
        let mode = parts[1].trim().to_string();
        let scenario = parts[2].trim().to_string();

        if let Ok(naive) = NaiveDateTime::parse_from_str(ts_str, "%Y.%m.%d-%H.%M.%S") {
            let played_at = naive.format("%Y-%m-%d %H:%M:%S").to_string();
            let date_key = naive.format("%Y-%m-%d").to_string();
            let played_at_ms = Local
                .from_local_datetime(&naive)
                .single()
                .map(|dt| dt.timestamp_millis())
                .unwrap_or_else(|| naive.and_utc().timestamp_millis());
            return (scenario, mode, played_at, played_at_ms, date_key);
        }
        return (
            scenario,
            mode,
            String::new(),
            0,
            String::new(),
        );
    }

    (
        file_key.to_string(),
        "Challenge".to_string(),
        String::new(),
        0,
        String::new(),
    )
}

/// Parses `HH:MM:SS.mmm` into milliseconds since midnight.
pub fn parse_hms_ms(s: &str) -> Option<f64> {
    let trimmed = s.trim();
    let mut parts = trimmed.split(':');
    let h: f64 = parts.next()?.parse().ok()?;
    let m: f64 = parts.next()?.parse().ok()?;
    let sec_part = parts.next()?;
    let s_val: f64 = sec_part.parse().ok()?;
    Some(((h * 3600.0) + (m * 60.0) + s_val) * 1000.0)
}

fn delta_ms_with_midnight_wrap(start_ms: f64, end_ms: f64) -> f64 {
    let mut diff = end_ms - start_ms;
    if diff < -43_200_000.0 {
        diff += 86_400_000.0;
    }
    diff.max(0.0)
}

/// Computes `cm/360` using exact KovaaK's `FovSensConfig.json` formulas.
pub fn compute_cm_per_360(sens_scale: &str, horiz_sens: f64, dpi: u32, fov: f64) -> Option<f64> {
    if horiz_sens <= 0.0 || dpi == 0 {
        return None;
    }
    let dpi_f = dpi as f64;
    let scale = sens_scale.trim().to_ascii_lowercase();

    match scale.as_str() {
        "cm/360" => return Some(horiz_sens),
        "in/360" => return Some(horiz_sens * 2.54),
        "counts/360" => return Some((horiz_sens / dpi_f) * 2.54),
        _ => {}
    }

    // Increment = degrees per mouse count
    let deg_per_count = match scale.as_str() {
        "counter-strike" | "csgo" | "cs2" | "quake/source" | "quake champions" | "apex legends" => {
            horiz_sens * 0.022
        }
        "valorant" => horiz_sens * 0.06996,
        "overwatch" | "overwatch 2" | "call of duty" | "destiny 2" => horiz_sens * 0.0066,
        "fortnite" => horiz_sens * 0.005555,
        "rainbow 6: siege" | "reflex arena" => horiz_sens * 0.018 / PI,
        "diabotical" => horiz_sens / 60.0,
        "battlefield v/1/hardline" => horiz_sens * 0.0034377486 + 0.011459162,
        "battlefield 6" => horiz_sens * 0.0017362342283 + 0.002314981532,
        "splitgate" => horiz_sens * 0.104156 * fov * 0.001,
        "paladins" => horiz_sens * 0.091176 * fov * 0.001,
        "halo" => horiz_sens * 0.022222,
        "rust" => horiz_sens * 0.1125,
        "batallion" | "battalion" => horiz_sens * 0.017501,
        "ue4" => horiz_sens * 0.07,
        "pubg" => 0.00005555 * 10f64.powf(horiz_sens / 50.0) * fov,
        "hunt: showdown" => horiz_sens * 0.0429718162181364,
        "gundam evolution" => horiz_sens * 0.0003888500001,
        "the finals" => horiz_sens * 0.001,
        "roblox" => horiz_sens * 1.01061008,
        "roblox arsenal" => horiz_sens * 0.375,
        "marvel rivals" => horiz_sens * 0.0175,
        "deadlock" => horiz_sens * 0.044,
        "fragpunk" => horiz_sens * 0.05555,
        "gta 5" => horiz_sens * 0.005446 + 0.032809,
        "strinova" => horiz_sens * 0.01388194363,
        "delta force" => horiz_sens * 0.03,
        _ => return None,
    };

    if deg_per_count <= 0.0 || !deg_per_count.is_finite() {
        return None;
    }

    let inches = 360.0 / (deg_per_count * dpi_f);
    let cm = inches * 2.54;
    if cm.is_finite() && cm > 0.0 {
        Some(cm)
    } else {
        None
    }
}

/// Parses KovaaK's `Stats.csv` content string (all 4 sections).
pub fn parse_csv_str(content: &str, file_name: &str) -> Result<ParsedRun, String> {
    let file_key = extract_file_key(file_name);
    let (name_scenario, mode, played_at, played_at_ms, date_key) =
        parse_file_key_metadata(&file_key);

    let mut kills: Vec<KillEvent> = Vec::new();
    let mut weapons: Vec<WeaponSummary> = Vec::new();
    let mut kv: HashMap<String, String> = HashMap::new();

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Section {
        None,
        KillTable,
        WeaponTable,
        KeyValue,
    }

    let mut section = Section::None;

    for raw_line in content.lines() {
        let line = raw_line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() {
            if section == Section::KillTable || section == Section::WeaponTable {
                section = Section::None;
            }
            continue;
        }

        if line.starts_with("Kill #,") {
            section = Section::KillTable;
            continue;
        }
        if line.starts_with("Weapon,Shots,Hits,") {
            section = Section::WeaponTable;
            continue;
        }

        match section {
            Section::KillTable => {
                let cols: Vec<&str> = line.split(',').collect();
                if cols.len() >= 11 {
                    let kill_index = cols[0].trim().parse::<u32>().unwrap_or(0);
                    if kill_index == 0 {
                        continue;
                    }
                    let timestamp = cols[1].trim().to_string();
                    let bot = cols[2].trim().to_string();
                    let weapon = cols[3].trim().to_string();
                    let ttk_s = cols[4]
                        .trim()
                        .trim_end_matches('s')
                        .parse::<f64>()
                        .unwrap_or(0.0);
                    let shots = cols[5].trim().parse::<u32>().unwrap_or(0);
                    let hits = cols[6].trim().parse::<u32>().unwrap_or(0);
                    let acc_raw = cols[7].trim().parse::<f64>().unwrap_or(0.0);
                    let accuracy = if acc_raw <= 1.0001 {
                        acc_raw * 100.0
                    } else {
                        acc_raw
                    };
                    let damage_done = cols[8].trim().parse::<f64>().unwrap_or(0.0);
                    let damage_possible = cols[9].trim().parse::<f64>().unwrap_or(0.0);
                    let eff_raw = cols[10].trim().parse::<f64>().unwrap_or(0.0);
                    let efficiency = if eff_raw <= 1.0001 {
                        eff_raw * 100.0
                    } else {
                        eff_raw
                    };
                    let cheated = cols
                        .get(11)
                        .map(|s| matches!(s.trim(), "1" | "true" | "True"))
                        .unwrap_or(false);
                    let overshots = cols
                        .get(12)
                        .and_then(|s| s.trim().parse::<u32>().ok())
                        .unwrap_or(0);
                    let is_one_shot = shots == 1 && hits >= 1;

                    kills.push(KillEvent {
                        kill_index,
                        timestamp,
                        elapsed_ms: 0.0,
                        interval_ms: None,
                        bot,
                        weapon,
                        ttk_s,
                        shots,
                        hits,
                        accuracy,
                        damage_done,
                        damage_possible,
                        efficiency,
                        overshots,
                        cheated,
                        is_one_shot,
                    });
                }
            }
            Section::WeaponTable => {
                let cols: Vec<&str> = line.split(',').collect();
                if cols.len() >= 5 {
                    let weapon = cols[0].trim().to_string();
                    if !weapon.is_empty() {
                        weapons.push(WeaponSummary {
                            weapon,
                            shots: cols[1].trim().parse::<u32>().unwrap_or(0),
                            hits: cols[2].trim().parse::<u32>().unwrap_or(0),
                            damage_done: cols[3].trim().parse::<f64>().unwrap_or(0.0),
                            damage_possible: cols[4].trim().parse::<f64>().unwrap_or(0.0),
                        });
                    }
                }
            }
            Section::None | Section::KeyValue => {
                if let Some((k, v)) = line.split_once(',') {
                    let key = k.trim().trim_end_matches(':').trim().to_string();
                    let val = v.trim().to_string();
                    if !key.is_empty() {
                        kv.insert(key, val);
                        section = Section::KeyValue;
                    }
                }
            }
        }
    }

    let get_str = |k: &str| -> String { kv.get(k).cloned().unwrap_or_default() };
    let get_f64 = |k: &str| -> f64 {
        kv.get(k)
            .and_then(|v| v.trim_end_matches('s').parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    let get_u32 = |k: &str| -> u32 {
        kv.get(k)
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0)
    };

    let scenario = {
        let s = get_str("Scenario");
        if s.is_empty() {
            name_scenario
        } else {
            s
        }
    };

    let challenge_start = get_str("Challenge Start");

    // Compute kill elapsed_ms and interval_ms
    let start_ms_opt = parse_hms_ms(&challenge_start)
        .or_else(|| kills.first().and_then(|k| parse_hms_ms(&k.timestamp)));

    let mut prev_ms: Option<f64> = None;
    let mut intervals_ms: Vec<f64> = Vec::new();
    let mut multi_shot_ttks_ms: Vec<f64> = Vec::new();
    let mut one_shot_kills: u32 = 0;

    for kill in &mut kills {
        if let Some(k_ms) = parse_hms_ms(&kill.timestamp) {
            if let Some(s_ms) = start_ms_opt {
                kill.elapsed_ms = delta_ms_with_midnight_wrap(s_ms, k_ms);
            }
            if let Some(p_ms) = prev_ms {
                let iv = delta_ms_with_midnight_wrap(p_ms, k_ms);
                kill.interval_ms = Some(iv);
                intervals_ms.push(iv);
            }
            prev_ms = Some(k_ms);
        }
        if kill.is_one_shot {
            one_shot_kills += 1;
        } else if kill.shots > 1 {
            multi_shot_ttks_ms.push(kill.ttk_s * 1000.0);
        }
    }

    let weapon_shots: u32 = weapons.iter().map(|w| w.shots).sum();
    let weapon_hits: u32 = weapons.iter().map(|w| w.hits).sum();
    let weapon_dmg_done: f64 = weapons.iter().map(|w| w.damage_done).sum();
    let weapon_dmg_possible: f64 = weapons.iter().map(|w| w.damage_possible).sum();

    let kv_hits = get_u32("Hit Count");
    let kv_misses = get_u32("Miss Count");
    let hits = if kv_hits > 0 || weapon_hits == 0 {
        kv_hits
    } else {
        weapon_hits
    };
    let shots = if weapon_shots > 0 {
        weapon_shots
    } else {
        hits.saturating_add(kv_misses)
    };
    let misses = if kv_misses > 0 || shots == 0 {
        kv_misses
    } else {
        shots.saturating_sub(hits)
    };

    let accuracy = if shots > 0 {
        (hits as f64 / shots as f64) * 100.0
    } else {
        0.0
    };

    let damage_done = {
        let d = get_f64("Damage Done");
        if d > 0.0 { d } else { weapon_dmg_done }
    };
    let damage_possible = weapon_dmg_possible;
    let damage_efficiency = if damage_possible > 0.0 {
        (damage_done / damage_possible) * 100.0
    } else {
        0.0
    };

    let kills_count = {
        let k = get_u32("Kills");
        if k > 0 || kills.is_empty() {
            k
        } else {
            kills.len() as u32
        }
    };

    let one_shot_pct = if !kills.is_empty() {
        (one_shot_kills as f64 / kills.len() as f64) * 100.0
    } else {
        0.0
    };

    let (avg_kill_interval_ms, fastest_kill_interval_ms, slowest_kill_interval_ms) =
        if !intervals_ms.is_empty() {
            let sum: f64 = intervals_ms.iter().sum();
            let min = intervals_ms
                .iter()
                .copied()
                .fold(f64::INFINITY, f64::min);
            let max = intervals_ms
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            (sum / intervals_ms.len() as f64, min, max)
        } else {
            (0.0, 0.0, 0.0)
        };

    let multi_shot_avg_ttk_ms = if !multi_shot_ttks_ms.is_empty() {
        multi_shot_ttks_ms.iter().sum::<f64>() / multi_shot_ttks_ms.len() as f64
    } else {
        0.0
    };

    // Estimate duration from kill span if `.perf` isn't merged yet
    let fight_time_s = get_f64("Fight Time");
    let kill_span_s = kills
        .last()
        .map(|k| k.elapsed_ms / 1000.0)
        .unwrap_or(0.0);
    let duration_s = if kill_span_s > 0.5 {
        kill_span_s
    } else if fight_time_s > 0.0 {
        fight_time_s
    } else {
        0.0
    };

    let avg_kps = if duration_s > 0.0 {
        kills_count as f64 / duration_s
    } else {
        0.0
    };

    let sens_scale = get_str("Sens Scale");
    let horiz_sens = get_f64("Horiz Sens");
    let vert_sens = get_f64("Vert Sens");
    let dpi = get_u32("DPI");
    let fov = get_f64("FOV");
    let cm_per_360 = compute_cm_per_360(&sens_scale, horiz_sens, dpi, fov);

    let hide_gun = matches!(
        get_str("Hide Gun").to_ascii_lowercase().as_str(),
        "true" | "1"
    );

    let run = RunRecord {
        id: 0,
        file_key,
        scenario,
        scenario_hash: get_str("Hash"),
        mode,
        played_at,
        played_at_ms,
        date_key,
        challenge_start,
        game_version: get_str("Game Version"),
        duration_s,
        score: get_f64("Score"),
        kills: kills_count,
        deaths: get_u32("Deaths"),
        shots,
        hits,
        misses,
        accuracy,
        damage_done,
        damage_possible,
        damage_efficiency,
        avg_ttk_s: get_f64("Avg TTK"),
        fight_time_s,
        time_remaining_s: get_f64("Time Remaining"),
        total_overshots: get_u32("Total Overshots"),
        reloads: get_u32("Reloads"),
        pause_count: get_u32("Pause Count"),
        pause_duration_s: get_f64("Pause Duration"),
        distance_traveled: get_f64("Distance Traveled"),
        mbs_points: get_f64("MBS Points"),
        avg_target_scale: get_f64("Avg Target Scale"),
        avg_time_dilation: get_f64("Avg Time Dilation"),
        avg_kps,
        peak_kps: 0.0,
        one_shot_kills,
        one_shot_pct,
        avg_kill_interval_ms,
        fastest_kill_interval_ms,
        slowest_kill_interval_ms,
        multi_shot_avg_ttk_ms,
        sens_scale,
        horiz_sens,
        vert_sens,
        dpi,
        cm_per_360,
        fov,
        fov_scale: get_str("FOVScale"),
        hide_gun,
        crosshair: get_str("Crosshair"),
        crosshair_scale: get_f64("Crosshair Scale"),
        crosshair_color: get_str("Crosshair Color"),
        resolution: get_str("Resolution"),
        avg_fps: get_f64("Avg FPS"),
        max_fps_config: get_f64("Max FPS (config)"),
        input_lag_ms: get_f64("Input Lag"),
        resolution_scale: get_f64("Resolution Scale"),
        has_perf: false,
        is_pb: false,
        csv_path: String::new(),
        perf_path: None,
    };

    Ok(ParsedRun {
        run,
        kills,
        ticks: Vec::new(),
        weapons,
    })
}
