#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub use kovaak_pulse::{analytics, config, model, parser, storage, watcher};

mod ui;

use iced::{Size, window};
use ui::app::App;
use ui::theme;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .theme(App::theme)
        .default_font(theme::FONT_REGULAR)
        .window(window::Settings {
            size: Size::new(380.0, 560.0),
            min_size: Some(Size::new(340.0, 400.0)),
            icon: ui::tray::build_window_icon(),
            resizable: true,
            exit_on_close_request: false,
            ..window::Settings::default()
        })
        .run()
}
