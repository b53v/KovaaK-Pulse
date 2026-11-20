use std::collections::BTreeMap;

use crate::model::{
    ConfigGroupStats, DeepInsights, HalfSplitComparison, KillEvent, PacingProfile, PerfTick,
    RunRecord, TrendAnalysis, TrendMetric, TrendPoint,
};

/// Computes linear-interpolated percentile `p` (`0.0..=1.0`) from an ascending-sorted slice.
fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let clamped = p.clamp(0.0, 1.0);
    let pos = clamped * (sorted.len() - 1) as f64;
    let lower = pos.floor() as usize;
    let upper = pos.ceil() as usize;
    if lower == upper {
        sorted[lower]
    } else {
        let weight = pos - lower as f64;
        sorted[lower] * (1.0 - weight) + sorted[upper] * weight
    }
}

/// Computes hidden deep-telemetry insights for a single run (zero-miss score ceiling,
/// 1-shot streak, miss cascade, P10/P50/P90 reaction percentiles, metronome CV%, and 3-phase pacing).
pub fn compute_deep_insights(
    run: &RunRecord,
    kills: &[KillEvent],
    ticks: &[PerfTick],
) -> DeepInsights {
    let mut max_one_shot_streak = 0u32;
    let mut cur_streak = 0u32;
    let mut multi_shot_count = 0u32;
    let mut clustered_miss_count = 0u32;
    let mut prev_was_multi = false;
    let mut miss_recovery_time_lost_s = 0.0f64;

    let mut all_intervals: Vec<f64> = Vec::with_capacity(kills.len());
    let mut one_shot_intervals: Vec<f64> = Vec::with_capacity(kills.len());

    for k in kills {
        if k.is_one_shot {
            cur_streak += 1;
            if cur_streak > max_one_shot_streak {
                max_one_shot_streak = cur_streak;
            }
            prev_was_multi = false;
            if let Some(iv) = k.interval_ms {
                if iv > 0.0 {
                    one_shot_intervals.push(iv);
                }
            }
        } else {
            cur_streak = 0;
            if k.shots > 1 {
                multi_shot_count += 1;
                miss_recovery_time_lost_s += k.ttk_s.max(0.0);
                if prev_was_multi {
                    clustered_miss_count += 1;
                }
                prev_was_multi = true;
            } else {
                prev_was_multi = false;
            }
        }

        if let Some(iv) = k.interval_ms {
            if iv > 0.0 {
                all_intervals.push(iv);
            }
        }
    }

    all_intervals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    one_shot_intervals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let p10_interval_ms = percentile_sorted(&all_intervals, 0.10);
    let p50_interval_ms = percentile_sorted(&all_intervals, 0.50);
    let p90_interval_ms = percentile_sorted(&all_intervals, 0.90);

    let rhythm_basis = if one_shot_intervals.len() >= 2 {
        &one_shot_intervals
    } else {
        &all_intervals
    };
    let rhythm_cv_pct = if rhythm_basis.len() >= 2 {
        let n = rhythm_basis.len() as f64;
        let mean = rhythm_basis.iter().sum::<f64>() / n;
        if mean > 1e-6 {
            let var = rhythm_basis.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
            (var.sqrt() / mean) * 100.0
        } else {
            0.0
        }
    } else {
        0.0
    };

    let p50_one_shot_ms = if !one_shot_intervals.is_empty() {
        percentile_sorted(&one_shot_intervals, 0.50)
    } else {
        p50_interval_ms
    };

    let estimated_lost_kills = if p50_one_shot_ms > 1.0 && miss_recovery_time_lost_s > 0.0 {
        (miss_recovery_time_lost_s * 1000.0) / p50_one_shot_ms
    } else {
        0.0
    };

    let score_per_kill = if run.kills > 0 && run.score > 0.0 {
        run.score / run.kills as f64
    } else {
        0.0
    };
    let potential_ceiling_score = if run.score > 0.0 {
        run.score + estimated_lost_kills * score_per_kill
    } else {
        0.0
    };

    // 3-phase split (Early 0-33%, Mid 33-66%, Late 66-100%)
    let (early_kps, mid_kps, late_kps, early_acc, mid_acc, late_acc) = if ticks.len() >= 3 {
        let len = ticks.len();
        let n1 = (len / 3).max(1);
        let n2 = ((2 * len) / 3).max(n1 + 1).min(len - 1);
        let phase_stats = |slice: &[PerfTick]| -> (f64, f64) {
            let count = slice.len() as f64;
            let k: u32 = slice.iter().map(|t| t.kills).sum();
            let fired: u32 = slice.iter().map(|t| t.shots_fired).sum();
            let hit: u32 = slice.iter().map(|t| t.shots_hit).sum();
            let kps = if count > 0.0 { k as f64 / count } else { 0.0 };
            let acc = if fired > 0 {
                (hit as f64 / fired as f64) * 100.0
            } else {
                run.accuracy
            };
            (kps, acc)
        };
        let (ek, ea) = phase_stats(&ticks[..n1]);
        let (mk, ma) = phase_stats(&ticks[n1..n2]);
        let (lk, la) = phase_stats(&ticks[n2..]);
        (ek, mk, lk, ea, ma, la)
    } else if kills.len() >= 3 && run.duration_s > 0.0 {
        let len = kills.len();
        let n1 = (len / 3).max(1);
        let n2 = ((2 * len) / 3).max(n1 + 1).min(len - 1);
        let phase_dur = (run.duration_s / 3.0).max(0.5);
        let kill_phase = |slice: &[KillEvent]| -> (f64, f64) {
            let kps = slice.len() as f64 / phase_dur;
            let shots: u32 = slice.iter().map(|k| k.shots).sum();
            let hits: u32 = slice.iter().map(|k| k.hits).sum();
            let acc = if shots > 0 {
                (hits as f64 / shots as f64) * 100.0
            } else {
                run.accuracy
            };
            (kps, acc)
        };
        let (ek, ea) = kill_phase(&kills[..n1]);
        let (mk, ma) = kill_phase(&kills[n1..n2]);
        let (lk, la) = kill_phase(&kills[n2..]);
        (ek, mk, lk, ea, ma, la)
    } else {
        (
            run.avg_kps,
            run.avg_kps,
            run.avg_kps,
            run.accuracy,
            run.accuracy,
            run.accuracy,
        )
    };

    let eff_e = early_kps * (early_acc / 100.0).max(0.25);
    let eff_m = mid_kps * (mid_acc / 100.0).max(0.25);
    let eff_l = late_kps * (late_acc / 100.0).max(0.25);
    let avg_eff = (eff_e + eff_m + eff_l) / 3.0;

    let pacing_profile = if avg_eff > 1e-6 {
        if (eff_l < eff_e * 0.88 && eff_l < eff_m * 0.90)
            || (late_acc + 5.0 < early_acc && late_kps <= early_kps)
        {
            PacingProfile::EndgameChoke
        } else if eff_m < eff_e * 0.88 && eff_m < eff_l * 0.88 {
            PacingProfile::MidSlump
        } else if eff_e < eff_m * 0.88 && eff_e < eff_l * 0.88 {
            PacingProfile::SlowStarter
        } else if eff_l > eff_e * 1.10 && eff_l >= eff_m * 1.04 {
            PacingProfile::StrongFinisher
        } else {
            PacingProfile::SteadyMetronome
        }
    } else {
        PacingProfile::SteadyMetronome
    };

    DeepInsights {
        max_one_shot_streak,
        miss_recovery_time_lost_s,
        estimated_lost_kills,
        potential_ceiling_score,
        p10_interval_ms,
        p50_interval_ms,
        p90_interval_ms,
        rhythm_cv_pct,
        multi_shot_count,
        clustered_miss_count,
        early_kps,
        mid_kps,
        late_kps,
        early_acc,
        mid_acc,
        late_acc,
        pacing_profile,
    }
}

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

    let mut sorted_vals = values.clone();
    sorted_vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p25_floor = percentile_sorted(&sorted_vals, 0.25);
    let p90_ceiling = percentile_sorted(&sorted_vals, 0.90);

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
        p25_floor,
        p90_ceiling,
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
