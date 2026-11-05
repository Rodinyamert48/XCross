//! Windows sistem tepsisi simgesi: ayarları aç, crosshair'i göster/gizle,
//! çıkış. Ayarlar penceresi kapatıldığında programa buradan ulaşılır.

use crate::raster;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    ShowSettings,
    ToggleOverlay,
    Quit,
}

pub struct Tray {
    _icon: TrayIcon,
    show: MenuId,
    toggle: MenuId,
    quit: MenuId,
}

impl Tray {
    pub fn new() -> Result<Self, String> {
        let show = MenuItem::new("Ayarlar", true, None);
        let toggle = MenuItem::new("Crosshair göster/gizle", true, None);
        let quit = MenuItem::new("Çıkış", true, None);
        let menu = Menu::new();
        menu.append_items(&[&show, &toggle, &PredefinedMenuItem::separator(), &quit])
            .map_err(|e| format!("tepsi menüsü oluşturulamadı: {e}"))?;

        let size = 32;
        let icon = Icon::from_rgba(raster::icon_rgba(size), size, size)
            .map_err(|e| format!("tepsi simgesi oluşturulamadı: {e}"))?;
        let tray = TrayIconBuilder::new()
            .with_tooltip("XCross — crosshair overlay")
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()
            .map_err(|e| format!("tepsi simgesi eklenemedi: {e}"))?;

        Ok(Self {
            _icon: tray,
            show: show.id().clone(),
            toggle: toggle.id().clone(),
            quit: quit.id().clone(),
        })
    }

    /// Bekleyen tepsi olaylarını eyleme çevirir.
    pub fn poll(&self) -> Vec<TrayAction> {
        let mut out = Vec::new();
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            if ev.id == self.show {
                out.push(TrayAction::ShowSettings);
            } else if ev.id == self.toggle {
                out.push(TrayAction::ToggleOverlay);
            } else if ev.id == self.quit {
                out.push(TrayAction::Quit);
            }
        }
        while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
            // Simgeye sol tık: ayarları aç. Sağ tık menüyü kendisi açar.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = ev
            {
                out.push(TrayAction::ShowSettings);
            }
        }
        out
    }
}
