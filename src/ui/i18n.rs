use crate::model::{HalfSplitComparison, Language, TimeRange, TrendMetric};
use crate::ui::app::ActiveTab;
use crate::ui::charts::per_second_chart::PerSecondBarMode;
use crate::ui::sidebar::ScenarioSort;

/// Helper returning `zh` when `lang == Language::Zh` else `en`.
#[inline]
pub fn t(lang: Language, zh: &'static str, en: &'static str) -> &'static str {
    match lang {
        Language::Zh => zh,
        Language::En => en,
    }
}

pub fn tab_label(tab: ActiveTab, lang: Language) -> &'static str {
    match (tab, lang) {
        (ActiveTab::SingleRun, Language::Zh) => "复盘",
        (ActiveTab::SingleRun, Language::En) => "Review",
        (ActiveTab::LongTermTrend, Language::Zh) => "趋势",
        (ActiveTab::LongTermTrend, Language::En) => "Trends",
        (ActiveTab::Overview, Language::Zh) => "总览",
        (ActiveTab::Overview, Language::En) => "Overview",
    }
}

pub fn sort_label(sort: ScenarioSort, lang: Language) -> &'static str {
    match (sort, lang) {
        (ScenarioSort::Recent, Language::Zh) => "最近",
        (ScenarioSort::Recent, Language::En) => "Recent",
        (ScenarioSort::RunCount, Language::Zh) => "局数",
        (ScenarioSort::RunCount, Language::En) => "Runs",
        (ScenarioSort::Name, Language::Zh) => "名称",
        (ScenarioSort::Name, Language::En) => "Name",
    }
}

pub fn bar_mode_label(mode: PerSecondBarMode, lang: Language) -> &'static str {
    match (mode, lang) {
        (PerSecondBarMode::Score, Language::Zh) => "得分",
        (PerSecondBarMode::Score, Language::En) => "Score",
        (PerSecondBarMode::Kps, Language::Zh) => "KPS",
        (PerSecondBarMode::Kps, Language::En) => "KPS",
        (PerSecondBarMode::DamageEff, Language::Zh) => "效率%",
        (PerSecondBarMode::DamageEff, Language::En) => "Eff%",
    }
}

pub fn time_range_short(range: TimeRange, lang: Language) -> &'static str {
    match (range, lang) {
        (TimeRange::Today, Language::Zh) => "今日",
        (TimeRange::Today, Language::En) => "Today",
        (TimeRange::Days7, Language::Zh) => "7天",
        (TimeRange::Days7, Language::En) => "7d",
        (TimeRange::Days30, Language::Zh) => "30天",
        (TimeRange::Days30, Language::En) => "30d",
        (TimeRange::All, Language::Zh) => "全部",
        (TimeRange::All, Language::En) => "All",
    }
}

pub fn metric_short(metric: TrendMetric, lang: Language) -> &'static str {
    match (metric, lang) {
        (TrendMetric::Score, Language::Zh) => "得分",
        (TrendMetric::Score, Language::En) => "Score",
        (TrendMetric::Accuracy, Language::Zh) => "准度",
        (TrendMetric::Accuracy, Language::En) => "Acc",
        (TrendMetric::Kps, Language::Zh) => "KPS",
        (TrendMetric::Kps, Language::En) => "KPS",
        (TrendMetric::AvgTtkMs, Language::Zh) => "TTK",
        (TrendMetric::AvgTtkMs, Language::En) => "TTK",
        (TrendMetric::OneShotPct, Language::Zh) => "1-Shot",
        (TrendMetric::OneShotPct, Language::En) => "1-Shot",
    }
}

pub fn half_split_diagnosis(hs: &HalfSplitComparison, lang: Language) -> String {
    match lang {
        Language::Zh => hs.diagnosis.clone(),
        Language::En => {
            if hs.accuracy_delta_pp <= -5.0 && hs.score_delta_pct <= -10.0 {
                format!(
                    "2nd-half accuracy dropped {:.1}pp — possible fatigue or rushing",
                    hs.accuracy_delta_pp
                )
            } else if hs.accuracy_delta_pp >= 3.0 {
                format!(
                    "2nd-half accuracy rose +{:.1}pp — warming up steadily",
                    hs.accuracy_delta_pp
                )
            } else if hs.kps_delta >= 0.3 {
                format!(
                    "2nd-half pace sped up (+{:.2} KPS) — strong finish",
                    hs.kps_delta
                )
            } else if hs.kps_delta <= -0.3 {
                format!(
                    "2nd-half pace slowed ({:.2} KPS) — rhythm dipped late",
                    hs.kps_delta
                )
            } else {
                "Steady pacing & accuracy across both halves".to_string()
            }
        }
    }
}
