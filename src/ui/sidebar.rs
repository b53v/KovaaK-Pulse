use iced::widget::{Column, Row, Space, button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length};

use crate::model::{Language, ScenarioSummary};
use crate::ui::i18n::{sort_label, t};
use crate::ui::theme::{self, UiPalette};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScenarioSort {
    #[default]
    Recent,
    RunCount,
    Name,
}

impl ScenarioSort {
    pub const ALL: &'static [ScenarioSort] = &[
        ScenarioSort::Recent,
        ScenarioSort::RunCount,
        ScenarioSort::Name,
    ];
}

/// Clean neutral scenario picker bar + collapsible dropdown list.
#[allow(clippy::too_many_arguments)]
pub fn view_scenario_picker<'a, Message: Clone + 'a>(
    p: UiPalette,
    lang: Language,
    scenarios: &'a [ScenarioSummary],
    selected_scenario: Option<&'a str>,
    is_open: bool,
    search_query: &'a str,
    favorites_only: bool,
    sort_by: ScenarioSort,
    on_toggle_open: Message,
    on_search: impl Fn(String) -> Message + 'a,
    on_toggle_fav_filter: Message,
    on_sort: impl Fn(ScenarioSort) -> Message + 'a,
    on_select: impl Fn(String) -> Message + 'a,
    on_toggle_star: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    let active_summary = selected_scenario.and_then(|name| {
        scenarios.iter().find(|s| s.scenario == name)
    });

    let arrow = if is_open { "▴" } else { "▾" };
    let title_str = active_summary
        .map(|s| s.scenario.as_str())
        .unwrap_or_else(|| t(lang, "选择训练项目", "Select Scenario"));
    let meta_str = match active_summary {
        Some(s) => match lang {
            Language::Zh => format!("PB {:.0} · {}局", s.pb_score, s.run_count),
            Language::En => format!("PB {:.0} · {} runs", s.pb_score, s.run_count),
        },
        None => match lang {
            Language::Zh => format!("{}个项目", scenarios.len()),
            Language::En => format!("{} scenarios", scenarios.len()),
        },
    };

    let mut header_row = row![
        text(arrow).size(11).color(p.text_muted),
        text(title_str)
            .size(12.5)
            .font(theme::FONT_BOLD)
            .color(p.text_primary)
            .width(Length::Fill),
        text(meta_str).size(11).color(p.text_muted),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    if let Some(active) = active_summary {
        let star_char = if active.is_favorite { "★" } else { "☆" };
        let star_color = if active.is_favorite {
            p.semantic_amber
        } else {
            p.text_muted
        };
        header_row = header_row.push(
            button(text(star_char).size(12).color(star_color))
                .padding([0, 3])
                .style(theme::ghost_button(p))
                .on_press(on_toggle_star(active.scenario.clone())),
        );
    }

    let picker_bar = button(header_row)
        .width(Length::Fill)
        .padding([6, 9])
        .style(theme::sidebar_item_button(p, is_open))
        .on_press(on_toggle_open);

    if !is_open {
        return container(picker_bar)
            .padding([6, 10])
            .width(Length::Fill)
            .style(theme::topbar_container(p))
            .into();
    }

    let query_lower = search_query.trim().to_lowercase();
    let mut filtered: Vec<&ScenarioSummary> = scenarios
        .iter()
        .filter(|s| {
            if favorites_only && !s.is_favorite {
                return false;
            }
            if !query_lower.is_empty() && !s.scenario.to_lowercase().contains(&query_lower) {
                return false;
            }
            true
        })
        .collect();

    filtered.sort_by(|a, b| {
        b.is_favorite.cmp(&a.is_favorite).then_with(|| match sort_by {
            ScenarioSort::Recent => b.latest_played_at_ms.cmp(&a.latest_played_at_ms),
            ScenarioSort::RunCount => b.run_count.cmp(&a.run_count),
            ScenarioSort::Name => a.scenario.to_lowercase().cmp(&b.scenario.to_lowercase()),
        })
    });

    let search_box = text_input(t(lang, "搜索项目名称...", "Search scenarios..."), search_query)
        .on_input(on_search)
        .padding([5, 8])
        .size(12)
        .style(theme::search_input_style(p));

    let fav_label = if favorites_only {
        t(lang, "★ 仅收藏", "★ Favs")
    } else {
        t(lang, "☆ 收藏", "☆ Favs")
    };
    let fav_btn = button(text(fav_label).size(11))
        .padding([3, 7])
        .style(theme::pill_button(p, favorites_only))
        .on_press(on_toggle_fav_filter);

    let mut sort_row = Row::new().spacing(4).align_y(Alignment::Center);
    for s in ScenarioSort::ALL {
        sort_row = sort_row.push(
            button(text(sort_label(*s, lang)).size(11))
                .padding([3, 7])
                .style(theme::pill_button(p, *s == sort_by))
                .on_press(on_sort(*s)),
        );
    }

    let filter_bar = row![fav_btn, Space::new().width(Length::Fill), sort_row]
        .align_y(Alignment::Center);

    let mut list_col = Column::new().spacing(4).width(Length::Fill);

    if filtered.is_empty() {
        list_col = list_col.push(
            container(
                text(if scenarios.is_empty() {
                    t(lang, "暂无本地记录，完成一局后将自动入库", "No local runs yet. Finish a run to ingest.")
                } else {
                    t(lang, "无匹配项目", "No matching scenarios")
                })
                .size(11.5)
                .color(p.text_muted),
            )
            .padding(10)
            .width(Length::Fill),
        );
    } else {
        for item in filtered {
            let is_selected = selected_scenario == Some(item.scenario.as_str());
            let star_label = if item.is_favorite { "★" } else { "☆" };
            let star_color = if item.is_favorite {
                p.semantic_amber
            } else {
                p.text_muted
            };

            let star_btn = button(text(star_label).size(11.5).color(star_color))
                .padding([0, 3])
                .style(theme::ghost_button(p))
                .on_press(on_toggle_star(item.scenario.clone()));

            let top_row = row![
                text(&item.scenario)
                    .size(12)
                    .font(if is_selected { theme::FONT_BOLD } else { theme::FONT_REGULAR })
                    .color(p.text_primary)
                    .width(Length::Fill),
                star_btn,
            ]
            .align_y(Alignment::Center)
            .spacing(4);

            let latest_txt = match lang {
                Language::Zh => format!("最近 {:.1}", item.latest_score),
                Language::En => format!("Last {:.1}", item.latest_score),
            };
            let count_txt = match lang {
                Language::Zh => format!("{}局", item.run_count),
                Language::En => format!("{}r", item.run_count),
            };
            let sub_row = row![
                text(format!("PB {:.1}", item.pb_score))
                    .size(10.5)
                    .color(p.text_secondary),
                text("·").size(10.5).color(p.text_muted),
                text(latest_txt).size(10.5).color(p.text_muted),
                Space::new().width(Length::Fill),
                text(count_txt).size(10.5).color(p.text_muted),
            ]
            .spacing(4)
            .align_y(Alignment::Center);

            let item_btn = button(column![top_row, sub_row].spacing(3).width(Length::Fill))
                .width(Length::Fill)
                .padding([6, 8])
                .style(theme::sidebar_item_button(p, is_selected))
                .on_press(on_select(item.scenario.clone()));

            list_col = list_col.push(item_btn);
        }
    }

    let list_scroll = scrollable(list_col)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new().width(4).scroller_width(4),
        ))
        .style(theme::subtle_scrollbar(p))
        .height(Length::Fixed(180.0));

    let dropdown = container(
        column![search_box, filter_bar, list_scroll]
            .spacing(7)
            .width(Length::Fill),
    )
    .padding(8)
    .width(Length::Fill)
    .style(theme::card_container(p));

    container(column![picker_bar, dropdown].spacing(6).width(Length::Fill))
        .padding([6, 10])
        .width(Length::Fill)
        .style(theme::topbar_container(p))
        .into()
}
