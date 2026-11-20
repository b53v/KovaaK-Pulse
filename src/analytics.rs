use std::collections::BTreeMap;

use crate::model::{
    ConfigGroupStats, HalfSplitComparison, PerfTick, RunRecord, TrendAnalysis, TrendMetric,
    TrendPoint,
};

/// Computes first-half (0-50%) vs second-half (50-100%) stability diagnosis from per-second ticks.
pub fn compute_half_split(ticks: &[PerfTick]) -> Option<HalfSplitComparison> {
    if ticks.len() < 2 {
        return None;
    }

    let mid = ticks.len() / 2;
    let first = &ticks[..mid];
    let second = &ticks[mid..];

    let sum_stats = |slice: &[PerfTick]| -> (u32, f64, u32, f64, f64) {
        let count = slice.len() as u32;
        let score: f64 = slice.iter().map(|t| t.score).sum();
        let kills: u32 = slice.iter().map(|t| t.kills).sum();
        let fired: u32 = slice.iter().map(|t| t.shots_fired).sum();
        let hit: u32 = slice.iter().map(|t| t.shots_hit).sum();
        let kps = if count > 0 {
            kills as f64 / count as f64
        } else {
            0.0
        };
        let acc = if fired > 0 {
            (hit as f64 / fired as f64) * 100.0
        } else {
            0.0
        };
        (count, score, kills, kps, acc)
    };

    let (first_ticks, first_score, first_kills, first_kps, first_acc) = sum_stats(first);
    let (second_ticks, second_score, second_kills, second_kps, second_acc) = sum_stats(second);

    let score_delta_pct = if first_score.abs() > 1e-6 {
        ((second_score - first_score) / first_score.abs()) * 100.0
    } else {
        0.0
    };
    let accuracy_delta_pp = second_acc - first_acc;
    let kps_delta = second_kps - first_kps;

    let diagnosis = if accuracy_delta_pp <= -4.0 && kps_delta >= 0.15 {
        format!(
            "后半程提速导致失误增多（准度 {:.1}%，KPS {:+.2}），建议稳住节奏",
            accuracy_delta_pp, kps_delta
        )
    } else if score_delta_pct <= -10.0 {
        format!(
            "后半程得分下滑 {:.1}%（准度 {:+.1}%），注意手部疲劳与专注度保持",
            score_delta_pct, accuracy_delta_pp
        )
    } else if score_delta_pct >= 10.0 {
        format!(
            "后半程明显起势（得分 {:+.1}%，准度 {:+.1}%），开局进入状态偏慢",
            score_delta_pct, accuracy_delta_pp
        )
    } else if accuracy_delta_pp >= 3.0 {
        format!(
            "后半程准度提升 {:+.1}%，手感越打越稳",
            accuracy_delta_pp
        )
    } else {
        format!(
            "前后半程发挥均衡（得分变化 {:+.1}%，准度变化 {:+.1}%），节奏稳定",
            score_delta_pct, accuracy_delta_pp
        )
    };

    Some(HalfSplitComparison {
        first_half_ticks: first_ticks,
        second_half_ticks: second_ticks,
        first_half_score: first_score,
        second_half_score: second_score,
        first_half_kills: first_kills,
        second_half_kills: second_kills,
        first_half_kps: first_kps,
        second_half_kps: second_kps,
        first_half_accuracy: first_acc,
        second_half_accuracy: second_acc,
        score_delta_pct,
        accuracy_delta_pp,
        kps_delta,
        diagnosis,
    })
}

/// Finds the tick with the lowest score (biggest score drop / negative spike).
pub fn find_worst_score_tick(ticks: &[PerfTick]) -> Option<PerfTick> {
    ticks
        .iter()
        .min_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
}

/// Finds the tick with the highest KPS (kills in that second).
pub fn find_best_kps_tick(ticks: &[PerfTick]) -> Option<PerfTick> {
    ticks
        .iter()
        .max_by(|a, b| {
            a.kills.cmp(&b.kills).then_with(|| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        })
        .cloned()
}

/// Computes long-term trend points (MA, PB step envelope, sensitivity change markers) and summary stats.
/// Expects `runs` sorted chronologically (oldest -> newest).
pub fn compute_trend_analysis(
    runs: &[RunRecord],
    metric: TrendMetric,
    ma_window: usize,
) -> TrendAnalysis {
    if runs.is_empty() {
        return TrendAnalysis::default();
    }

    let window = ma_window.max(1);
    let higher_is_better = metric.higher_is_better();

    let mut points: Vec<TrendPoint> = Vec::with_capacity(runs.len());
    let mut values: Vec<f64> = Vec::with_capacity(runs.len());
    let mut running_pb: Option<f64> = None;
    let mut prev_cm: Option<f64> = None;

    for (idx, run) in runs.iter().enumerate() {
        let val = metric.extract(run);
        values.push(val);

        let start = idx.saturating_sub(window - 1);
        let slice = &values[start..=idx];
        let ma_value = Some(slice.iter().sum::<f64>() / slice.len() as f64);

        let is_pb = match running_pb {
            None => {
                running_pb = Some(val);
                true
            }
            Some(best) => {
                let improved = if higher_is_better {
                    val > best + 1e-9
                } else {
                    val > 0.0 && (best <= 0.0 || val < best - 1e-9)
                };
                if improved {
                    running_pb = Some(val);
                    true
                } else {
                    false
                }
            }
        };

        let pb_envelope = running_pb.unwrap_or(val);

        let sens_changed = match (prev_cm, run.cm_per_360) {
            (Some(a), Some(b)) => (a - b).abs() > 0.5,
            _ => false,
        };
        if run.cm_per_360.is_some() {
            prev_cm = run.cm_per_360;
        }

        points.push(TrendPoint {
            run_id: run.id,
            index: idx,
            played_at: run.played_at.clone(),
            played_at_ms: run.played_at_ms,
            date_key: run.date_key.clone(),
            value: val,
            score: run.score,
            accuracy: run.accuracy,
            kps: run.avg_kps,
            avg_ttk_ms: run.avg_ttk_s * 1000.0,
            one_shot_pct: run.one_shot_pct,
            ma_value,
            pb_envelope,
            is_pb,
            cm_per_360: run.cm_per_360,
            sens_changed,
        });
    }

    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let std_dev = variance.sqrt();
    let cv_pct = if mean.abs() > 1e-6 {
        (std_dev / mean.abs()) * 100.0
    } else {
        0.0
    };
    let min_val = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max_val = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let pb_value = running_pb.unwrap_or(0.0);

    let recent_delta_pct = if values.len() >= 4 {
        let half = (values.len() / 2).min(5);
        let recent_slice = &values[values.len() - half..];
        let prev_slice = &values[values.len() - 2 * half..values.len() - half];
        let recent_avg = recent_slice.iter().sum::<f64>() / recent_slice.len() as f64;
        let prev_avg = prev_slice.iter().sum::<f64>() / prev_slice.len() as f64;
        if prev_avg.abs() > 1e-6 {
            Some(((recent_avg - prev_avg) / prev_avg.abs()) * 100.0)
        } else {
            None
        }
    } else {
        None
    };

    TrendAnalysis {
        points,
        mean,
        std_dev,
        cv_pct,
        min_val,
        max_val,
        pb_value,
        recent_delta_pct,
    }
}

/// Groups runs by `(cm/360, FOV, Crosshair)` to compare performance across sensitivity/config setups.
pub fn compute_config_groups(runs: &[RunRecord]) -> Vec<ConfigGroupStats> {
    struct Acc {
        cm_per_360: Option<f64>,
        sens_scale: String,
        horiz_sens: f64,
        dpi: u32,
        fov: f64,
        crosshair: String,
        count: u32,
        score_sum: f64,
        pb_score: f64,
        acc_sum: f64,
        kps_sum: f64,
        one_shot_sum: f64,
    }

    let mut groups: BTreeMap<String, Acc> = BTreeMap::new();

    for run in runs {
        let cm_label = match run.cm_per_360 {
            Some(cm) => format!("{:.1} cm/360", cm),
            None => format!("{} {:.2} @ {} DPI", run.sens_scale, run.horiz_sens, run.dpi),
        };
        let fov_label = format!("FOV {:.0}", run.fov);
        let ch_label = if run.crosshair.is_empty() {
            "Default".to_string()
        } else {
            run.crosshair.clone()
        };
        let key = format!("{cm_label} | {fov_label} | {ch_label}");

        let entry = groups.entry(key).or_insert_with(|| Acc {
            cm_per_360: run.cm_per_360,
            sens_scale: run.sens_scale.clone(),
            horiz_sens: run.horiz_sens,
            dpi: run.dpi,
            fov: run.fov,
            crosshair: ch_label.clone(),
            count: 0,
            score_sum: 0.0,
            pb_score: f64::NEG_INFINITY,
            acc_sum: 0.0,
            kps_sum: 0.0,
            one_shot_sum: 0.0,
        });

        entry.count += 1;
        entry.score_sum += run.score;
        if run.score > entry.pb_score {
            entry.pb_score = run.score;
        }
        entry.acc_sum += run.accuracy;
        entry.kps_sum += run.avg_kps;
        entry.one_shot_sum += run.one_shot_pct;
    }

    let mut result: Vec<ConfigGroupStats> = groups
        .into_iter()
        .map(|(group_key, acc)| {
            let n = acc.count.max(1) as f64;
            ConfigGroupStats {
                group_key,
                cm_per_360: acc.cm_per_360,
                sens_scale: acc.sens_scale,
                horiz_sens: acc.horiz_sens,
                dpi: acc.dpi,
                fov: acc.fov,
                crosshair: acc.crosshair,
                run_count: acc.count,
                avg_score: acc.score_sum / n,
                pb_score: if acc.pb_score.is_finite() {
                    acc.pb_score
                } else {
                    0.0
                },
                avg_accuracy: acc.acc_sum / n,
                avg_kps: acc.kps_sum / n,
                avg_one_shot_pct: acc.one_shot_sum / n,
            }
        })
        .collect();

    result.sort_by(|a, b| {
        b.avg_score
            .partial_cmp(&a.avg_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    result
}
