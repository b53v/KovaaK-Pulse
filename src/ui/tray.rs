use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

/// Actions triggered by user interaction with the system tray.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    ShowWindow,
    SyncNow,
    Quit,
}

/// Lightweight Windows system tray wrapper for single-monitor UX.
/// Never steals foreground focus when updating tooltip or run summaries.
pub struct TrayController {
    _tray: Option<TrayIcon>,
    show_id: Option<MenuId>,
    sync_id: Option<MenuId>,
    quit_id: Option<MenuId>,
}

impl TrayController {
    /// Initializes the system tray icon with an embedded procedural RGBA reticle icon.
    /// Gracefully degrades to a no-op controller if the platform tray is unavailable.
    pub fn new() -> Self {
        let menu = Menu::new();
        let show_item = MenuItem::new("打开 / Show KovaaK Pulse", true, None);
        let sync_item = MenuItem::new("立即同步 / Sync Now", true, None);
        let quit_item = MenuItem::new("退出 / Quit", true, None);

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
                .with_tooltip("KovaaK Pulse")
                .with_icon(ic)
                .build()
                .ok(),
            None => None,
        };

        Self {
            _tray: tray,
            show_id: Some(show_id),
            sync_id: Some(sync_id),
            quit_id: Some(quit_id),
        }
    }

    pub fn is_active(&self) -> bool {
        self._tray.is_some()
    }

    /// Updates the tray hover tooltip with the latest run result without stealing game focus.
    pub fn update_tooltip(&self, text: &str) {
        if let Some(tray) = &self._tray {
            let _ = tray.set_tooltip(Some(text));
        }
    }

    /// Non-blocking poll for tray menu or left-click events.
    pub fn poll_action(&self) -> Option<TrayAction> {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if self.show_id.as_ref() == Some(&event.id) {
                return Some(TrayAction::ShowWindow);
            }
            if self.sync_id.as_ref() == Some(&event.id) {
                return Some(TrayAction::SyncNow);
            }
            if self.quit_id.as_ref() == Some(&event.id) {
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

            // Signed distance to rounded rectangle
            let qx = (ax - inner_limit).max(0.0);
            let qy = (ay - inner_limit).max(0.0);
            let rr_dist = (qx * qx + qy * qy).sqrt() - corner_r;

            let idx = (y * s + x) * 4;
            if rr_dist > 0.5 {
                continue;
            }

            // Dark rounded-rect background (#0F1115) with subtle border (#252A34)
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
