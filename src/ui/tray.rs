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
        let show_item = MenuItem::new("打开 KovaaK Pulse", true, None);
        let sync_item = MenuItem::new("立即同步本地记录", true, None);
        let quit_item = MenuItem::new("退出", true, None);

        let show_id = show_item.id().clone();
        let sync_id = sync_item.id().clone();
        let quit_id = quit_item.id().clone();

        let _ = menu.append(&show_item);
        let _ = menu.append(&sync_item);
        let _ = menu.append(&quit_item);

        let icon = build_reticle_icon();

        let tray = match icon {
            Some(ic) => TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_tooltip("KovaaK Pulse — 静默监测中")
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

/// Generates a crisp 32x32 RGBA esports reticle icon in memory (zero external assets).
fn build_reticle_icon() -> Option<Icon> {
    const SIZE: usize = 32;
    let mut rgba = vec![0u8; SIZE * SIZE * 4];
    let center = (SIZE as f32 - 1.0) / 2.0;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = (y * SIZE + x) * 4;

            // Dark circular badge background
            if dist <= 15.0 {
                rgba[idx] = 0x0D;
                rgba[idx + 1] = 0x10;
                rgba[idx + 2] = 0x17;
                rgba[idx + 3] = 0xFF;
            }

            // Outer cyan ring
            if (dist - 12.2).abs() <= 1.35 {
                rgba[idx] = 0x00;
                rgba[idx + 1] = 0xD4;
                rgba[idx + 2] = 0xFF;
                rgba[idx + 3] = 0xFF;
            }

            // Crosshair ticks
            let on_h = dy.abs() <= 1.0 && dx.abs() >= 5.0 && dx.abs() <= 14.5;
            let on_v = dx.abs() <= 1.0 && dy.abs() >= 5.0 && dy.abs() <= 14.5;
            if on_h || on_v {
                rgba[idx] = 0x00;
                rgba[idx + 1] = 0xD4;
                rgba[idx + 2] = 0xFF;
                rgba[idx + 3] = 0xFF;
            }

            // Center green pulse dot
            if dist <= 2.4 {
                rgba[idx] = 0x22;
                rgba[idx + 1] = 0xC5;
                rgba[idx + 2] = 0x5E;
                rgba[idx + 3] = 0xFF;
            }
        }
    }

    Icon::from_rgba(rgba, SIZE as u32, SIZE as u32).ok()
}
