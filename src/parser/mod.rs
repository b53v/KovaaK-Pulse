pub mod csv_parser;
pub mod perf_parser;

use std::path::Path;

use crate::model::ParsedRun;

/// Parses a KovaaK's `.csv` run and optionally merges its paired `.perf` telemetry file.
pub fn parse_run_pair(csv_path: &Path, perf_path: Option<&Path>) -> Result<ParsedRun, String> {
    let mut parsed = csv_parser::parse_csv_file(csv_path)?;

    if let Some(p_path) = perf_path {
        if p_path.exists() {
            match perf_parser::parse_perf_file(p_path) {
                Ok(perf_data) => {
                    parsed.run.has_perf = true;
                    parsed.run.perf_path = Some(p_path.to_string_lossy().into_owned());

                    if parsed.run.scenario.is_empty() && !perf_data.header.scenario_name.is_empty()
                    {
                        parsed.run.scenario = perf_data.header.scenario_name.clone();
                    }
                    if parsed.run.scenario_hash.is_empty()
                        && !perf_data.header.scenario_hash.is_empty()
                    {
                        parsed.run.scenario_hash = perf_data.header.scenario_hash.clone();
                    }
                    if parsed.run.played_at_ms == 0 && perf_data.header.challenge_start_utc_ms > 0 {
                        parsed.run.played_at_ms = perf_data.header.challenge_start_utc_ms;
                    }

                    // Refine duration, avg_kps, and peak_kps from `.perf` ticks
                    if let Some(last_tick) = perf_data.ticks.last() {
                        if last_tick.time_offset_s > 0.0 {
                            parsed.run.duration_s = last_tick.time_offset_s;
                        }
                    } else if perf_data.header.duration_s > 0.0 {
                        parsed.run.duration_s = perf_data.header.duration_s;
                    }

                    if parsed.run.duration_s > 0.0 {
                        parsed.run.avg_kps = parsed.run.kills as f64 / parsed.run.duration_s;
                    }

                    let peak_kps = perf_data
                        .ticks
                        .iter()
                        .map(|t| t.kills as f64)
                        .fold(0.0f64, f64::max);
                    parsed.run.peak_kps = peak_kps;

                    // If CSV lacked damage_possible (e.g. missing weapon table), backfill from ticks
                    if parsed.run.damage_possible <= 0.0 {
                        let sum_dp: f64 = perf_data.ticks.iter().map(|t| t.damage_possible).sum();
                        if sum_dp > 0.0 {
                            parsed.run.damage_possible = sum_dp;
                            parsed.run.damage_efficiency =
                                (parsed.run.damage_done / sum_dp) * 100.0;
                        }
                    }

                    parsed.ticks = perf_data.ticks;
                }
                Err(_) => {
                    // Gracefully degrade to CSV-only if `.perf` is corrupt or incomplete
                    parsed.run.has_perf = false;
                }
            }
        }
    }

    // If peak_kps is still 0 (no `.perf`), estimate 1-second bucket peak from kill timestamps
    if parsed.run.peak_kps <= 0.0 && !parsed.kills.is_empty() {
        let mut sec_counts: std::collections::HashMap<i64, u32> = std::collections::HashMap::new();
        for k in &parsed.kills {
            let sec = (k.elapsed_ms / 1000.0).floor() as i64;
            *sec_counts.entry(sec).or_insert(0) += 1;
        }
        parsed.run.peak_kps = sec_counts.values().copied().max().unwrap_or(0) as f64;
    }

    Ok(parsed)
}
