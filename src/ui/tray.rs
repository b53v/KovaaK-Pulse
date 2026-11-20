use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::model::Language;

/// Actions triggered by user interaction with the system tray.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    ShowWindow,
    SyncNow,
    Quit,
}

/// Hashable key holding menu IDs for the background event-driven tray subscription.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct TraySubKey {
    pub show_id: MenuId,
    pub sync_id: MenuId,
    pub quit_id: MenuId,
}

/// Non-blocking check of the global `tray-icon` channels from a background task.
pub fn poll_tray_event(key: &TraySubKey) -> Option<TrayAction> {
    if let Ok(event) = MenuEvent::receiver().try_recv() {
        if key.show_id == event.id {
            return Some(TrayAction::ShowWindow);
        }
        if key.sync_id == event.id {
            return Some(TrayAction::SyncNow);
        }
        if key.quit_id == event.id {
            return Some(TrayAction::Quit);
        }
    }

    if let Ok(event) = TrayIconEvent::receiver().try_recv() {
        if matches!(
            event,
            TrayIconEvent::DoubleClick { .. }
                | TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                }
        ) {
            return Some(TrayAction::ShowWindow);
        }
    }

    None
}

/// Lightweight Windows system tray wrapper for single-monitor UX.
/// Never steals foreground focus when updating tooltip or run summaries.
pub struct TrayController {
    _tray: Option<TrayIcon>,
    show_item: MenuItem,
    sync_item: MenuItem,
    quit_item: MenuItem,
    show_id: MenuId,
    sync_id: MenuId,
    quit_id: MenuId,
}

fn tray_labels(lang: Language) -> (&'static str, &'static str, &'static str, &'static str) {
    match lang {
        Language::Zh => ("打开 KovaaK Pulse", "立即同步本地记录", "退出", "KovaaK Pulse — 静默监测中"),
        Language::En => ("Show KovaaK Pulse", "Sync Local Runs", "Quit", "KovaaK Pulse — Monitoring"),
    }
}

impl TrayController {
    /// Initializes the system tray icon with an embedded procedural RGBA reticle icon.
    /// Gracefully degrades to a no-op controller if the platform tray is unavailable.
    pub fn new(lang: Language) -> Self {
        let (show_lbl, sync_lbl, quit_lbl, tooltip) = tray_labels(lang);

        let menu = Menu::new();
        let show_item = MenuItem::new(show_lbl, true, None);
        let sync_item = MenuItem::new(sync_lbl, true, None);
        let quit_item = MenuItem::new(quit_lbl, true, None);

        let show_id = show_item.id().clone();
        let sync_id = sync_item.id().clone();
        let quit_id = quit_item.id().clone();

        let _ = menu.append(&show_item);
        let _ = menu.append(&sync_item);
        let _ = menu.append(&quit_item);

        let icon = build_tray_icon();

        let tray = match icon {
            Some(ic) => TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_tooltip(tooltip)
                .with_icon(ic)
                .build()
                .ok(),
            None => None,
        };

        Self {
            _tray: tray,
            show_item,
            sync_item,
            quit_item,
            show_id,
            sync_id,
            quit_id,
        }
    }

    pub fn is_active(&self) -> bool {
        self._tray.is_some()
    }

    pub fn sub_key(&self) -> Option<TraySubKey> {
        if self._tray.is_some() {
            Some(TraySubKey {
                show_id: self.show_id.clone(),
                sync_id: self.sync_id.clone(),
                quit_id: self.quit_id.clone(),
            })
        } else {
            None
        }
    }

    /// Updates tray menu labels and default tooltip when the user switches UI language.
    pub fn set_language(&self, lang: Language) {
        let (show_lbl, sync_lbl, quit_lbl, tooltip) = tray_labels(lang);
        self.show_item.set_text(show_lbl);
        self.sync_item.set_text(sync_lbl);
        self.quit_item.set_text(quit_lbl);
        self.update_tooltip(tooltip);
    }

    /// Updates the tray hover tooltip with the latest run result without stealing game focus.
    pub fn update_tooltip(&self, text: &str) {
        if let Some(tray) = &self._tray {
            let _ = tray.set_tooltip(Some(text));
        }
    }
}

/// Generates a crisp `size x size` RGBA rounded-rect dark slate (`#0F1115`) icon
/// with a clean `#60A5FA` crosshair reticle and center dot.
pub fn reticle_rgba(size: u32) -> Vec<u8> {
    let s = size as usize;
    let mut rgba = vec![0u8; s * s * 4];
    let center = (s as f32 - 1.0) / 2.0;
    let half = s as f32 * 0.5;
    let corner_r = (s as f32 * 0.22).max(2.0);
    let inner_limit = half - corner_r - 0.5;

    let ring_r = s as f32 * 0.30;
    let ring_w = (s as f32 * 0.055).max(1.1);
    let tick_half_w = (s as f32 * 0.036).max(0.85);
    let tick_inner = s as f32 * 0.16;
    let tick_outer = s as f32 * 0.40;
    let dot_r = (s as f32 * 0.08).max(1.4);

    for y in 0..s {
        for x in 0..s {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let ax = dx.abs();
            let ay = dy.abs();

            let qx = (ax - inner_limit).max(0.0);
            let qy = (ay - inner_limit).max(0.0);
            let rr_dist = (qx * qx + qy * qy).sqrt() - corner_r;

            let idx = (y * s + x) * 4;
            if rr_dist > 0.5 {
                continue;
            }

            if rr_dist > -1.2 {
                rgba[idx] = 0x25;
                rgba[idx + 1] = 0x2A;
                rgba[idx + 2] = 0x34;
                rgba[idx + 3] = 0xFF;
            } else {
                rgba[idx] = 0x0F;
                rgba[idx + 1] = 0x11;
                rgba[idx + 2] = 0x15;
                rgba[idx + 3] = 0xFF;
            }

            let dist = (dx * dx + dy * dy).sqrt();
            let on_ring = (dist - ring_r).abs() <= ring_w;
            let on_h = ay <= tick_half_w && ax >= tick_inner && ax <= tick_outer;
            let on_v = ax <= tick_half_w && ay >= tick_inner && ay <= tick_outer;
            let on_dot = dist <= dot_r;

            if on_ring || on_h || on_v || on_dot {
                rgba[idx] = 0x60;
                rgba[idx + 1] = 0xA5;
                rgba[idx + 2] = 0xFA;
                rgba[idx + 3] = 0xFF;
            }
        }
    }

    rgba
}

fn build_tray_icon() -> Option<Icon> {
    let rgba = reticle_rgba(32);
    Icon::from_rgba(rgba, 32, 32).ok()
}

/// Builds the 64x64 window/taskbar icon for `iced::window::Settings`.
pub fn build_window_icon() -> Option<iced::window::Icon> {
    let rgba = reticle_rgba(64);
    iced::window::icon::from_rgba(rgba, 64, 64).ok()
}
