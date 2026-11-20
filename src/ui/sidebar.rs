use iced::widget::{Column, Row, button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length};

use crate::model::ScenarioSummary;
use crate::ui::theme;

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

    pub fn label(self) -> &'static str {
        match self {
            ScenarioSort::Recent => "最近",
            ScenarioSort::RunCount => "局数",
            ScenarioSort::Name => "名称",
        }
    }
}

pub fn view_sidebar<'a, Message: Clone + 'a>(
    scenarios: &'a [ScenarioSummary],
    selected_scenario: Option<&'a str>,
    search_query: &'a str,
    favorites_only: bool,
    sort_by: ScenarioSort,
    on_search: impl Fn(String) -> Message + 'a,
    on_toggle_fav_filter: Message,
    on_sort: impl Fn(ScenarioSort) -> Message + 'a,
    on_select: impl Fn(String) -> Message + 'a,
    on_toggle_star: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
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
        // Favorites always float to top
        b.is_favorite.cmp(&a.is_favorite).then_with(|| match sort_by {
            ScenarioSort::Recent => b.latest_played_at_ms.cmp(&a.latest_played_at_ms),
            ScenarioSort::RunCount => b.run_count.cmp(&a.run_count),
            ScenarioSort::Name => a.scenario.to_lowercase().cmp(&b.scenario.to_lowercase()),
        })
    });

    let search_box = text_input("搜索项目名称...", search_query)
        .on_input(on_search)
        .padding([7, 10])
        .size(13)
        .style(theme::search_input_style);

    let fav_btn = button(
        text(if favorites_only { "★ 仅收藏" } else { "☆ 收藏" })
            .size(11.5)
            .font(theme::FONT_BOLD),
    )
    .padding([4, 8])
    .style(theme::pill_button(favorites_only))
    .on_press(on_toggle_fav_filter);

    let mut sort_row = Row::new().spacing(4).align_y(Alignment::Center);
    for s in ScenarioSort::ALL {
        sort_row = sort_row.push(
            button(text(s.label()).size(11.5))
                .padding([4, 8])
                .style(theme::pill_button(*s == sort_by))
                .on_press(on_sort(*s)),
        );
    }

    let filter_bar = row![fav_btn, iced::widget::Space::new().width(Length::Fill), sort_row]
        .align_y(Alignment::Center);

    let mut list_col = Column::new().spacing(6).width(Length::Fill);

    if filtered.is_empty() {
        list_col = list_col.push(
            container(
                text(if scenarios.is_empty() {
                    "暂无本地记录\n打完一局 KovaaK's 将自动入库"
                } else {
                    "无匹配项目"
                })
                .size(12.5)
                .color(theme::TEXT_MUTED),
            )
            .padding(16)
            .width(Length::Fill),
        );
    } else {
        for item in filtered {
            let is_selected = selected_scenario == Some(item.scenario.as_str());
            let star_label = if item.is_favorite { "★" } else { "☆" };
            let star_color = if item.is_favorite {
                theme::ACCENT_AMBER
            } else {
                theme::TEXT_MUTED
            };

            let star_btn = button(text(star_label).size(13).color(star_color))
                .padding([0, 4])
                .style(theme::ghost_button)
                .on_press(on_toggle_star(item.scenario.clone()));

            let header_row = row![
                text(&item.scenario)
                    .size(13)
                    .font(theme::FONT_BOLD)
                    .color(if is_selected {
                        theme::ACCENT_CYAN
                    } else {
                        theme::TEXT_PRIMARY
                    })
                    .width(Length::Fill),
                star_btn,
            ]
            .align_y(Alignment::Center)
            .spacing(4);

            let stats_row = row![
                text(format!("PB {:.1}", item.pb_score))
                    .size(11.5)
                    .color(theme::ACCENT_GREEN),
                text("·").size(11.5).color(theme::TEXT_MUTED),
                text(format!("最近 {:.1}", item.latest_score))
                    .size(11.5)
                    .color(theme::TEXT_SECONDARY),
                iced::widget::Space::new().width(Length::Fill),
                text(format!("{} 局", item.run_count))
                    .size(11)
                    .color(theme::TEXT_MUTED),
            ]
            .spacing(5)
            .align_y(Alignment::Center);

            let card = button(column![header_row, stats_row].spacing(5).width(Length::Fill))
                .width(Length::Fill)
                .padding([9, 10])
                .style(theme::sidebar_item_button(is_selected))
                .on_press(on_select(item.scenario.clone()));

            list_col = list_col.push(card);
        }
    }

    let list_scroll = scrollable(list_col)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new().width(5).scroller_width(5),
        ))
        .style(theme::subtle_scrollbar)
        .height(Length::Fill);

    let header = row![
        text("训练项目")
            .size(13.5)
            .font(theme::FONT_BOLD)
            .color(theme::TEXT_PRIMARY),
        iced::widget::Space::new().width(Length::Fill),
        text(format!("{} 个", scenarios.len()))
            .size(11.5)
            .color(theme::TEXT_MUTED),
    ]
    .align_y(Alignment::Center);

    container(
        column![header, search_box, filter_bar, list_scroll]
            .spacing(10)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fixed(272.0))
    .height(Length::Fill)
    .padding(12)
    .style(theme::sidebar_container)
    .into()
}
