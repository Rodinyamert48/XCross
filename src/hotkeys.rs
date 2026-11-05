//! Global (oyun içinden de çalışan) kısayol yönetimi.

use crate::config::Hotkeys;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::collections::HashMap;
use std::str::FromStr;

/// Bir kısayolun tetiklediği eylem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ToggleOverlay,
    NextProfile,
    ToggleSettings,
}

/// Kısayol servisi. Platform desteği yoksa (ör. Wayland) sessizce devre dışı
/// kalır ve `last_error` ile sebebi bildirir.
pub struct HotkeyService {
    manager: Option<GlobalHotKeyManager>,
    registered: Vec<HotKey>,
    bindings: HashMap<u32, Action>,
    last_error: Option<String>,
}

impl HotkeyService {
    pub fn new() -> Self {
        let (manager, last_error) = match GlobalHotKeyManager::new() {
            Ok(m) => (Some(m), None),
            Err(e) => (None, Some(format!("kısayol yöneticisi başlatılamadı: {e}"))),
        };
        Self {
            manager,
            registered: Vec::new(),
            bindings: HashMap::new(),
            last_error,
        }
    }

    /// Kısayol desteği bu platformda kullanılabilir mi?
    pub fn available(&self) -> bool {
        self.manager.is_some()
    }

    /// En son oluşan hata (varsa).
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Verilen yapılandırmaya göre tüm kısayolları yeniden kaydeder.
    pub fn apply(&mut self, hk: &Hotkeys) {
        let Some(manager) = &self.manager else {
            return;
        };
        if !self.registered.is_empty() {
            let _ = manager.unregister_all(&self.registered);
            self.registered.clear();
        }
        self.bindings.clear();
        self.last_error = None;

        let wanted = [
            (hk.toggle_overlay.as_str(), Action::ToggleOverlay),
            (hk.next_profile.as_str(), Action::NextProfile),
            (hk.toggle_settings.as_str(), Action::ToggleSettings),
        ];

        let mut errors = Vec::new();
        for (text, action) in wanted {
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            match HotKey::from_str(text) {
                Ok(key) => match manager.register(key) {
                    Ok(()) => {
                        self.bindings.insert(key.id(), action);
                        self.registered.push(key);
                    }
                    Err(e) => errors.push(format!("'{text}' kaydedilemedi: {e}")),
                },
                Err(e) => errors.push(format!("'{text}' çözümlenemedi: {e}")),
            }
        }
        if !errors.is_empty() {
            self.last_error = Some(errors.join(" | "));
        }
    }

    /// Bekleyen kısayol olaylarını toplar. Yalnızca tuşa basma anı sayılır.
    pub fn poll(&self) -> Vec<Action> {
        let mut out = Vec::new();
        if self.manager.is_none() {
            return out;
        }
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.state != HotKeyState::Pressed {
                continue;
            }
            if let Some(action) = self.bindings.get(&event.id) {
                out.push(*action);
            }
        }
        out
    }
}

/// Kısayol metninin geçerli olup olmadığını söyler (UI doğrulaması için).
pub fn is_valid(text: &str) -> bool {
    let text = text.trim();
    text.is_empty() || HotKey::from_str(text).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_hotkeys_parse() {
        let hk = Hotkeys::default();
        for text in [&hk.toggle_overlay, &hk.next_profile, &hk.toggle_settings] {
            assert!(HotKey::from_str(text).is_ok(), "çözümlenemedi: {text}");
        }
    }

    #[test]
    fn validation_accepts_empty_and_rejects_garbage() {
        assert!(is_valid(""));
        assert!(is_valid("Ctrl+Alt+KeyX"));
        assert!(!is_valid("bu bir kısayol değil"));
    }
}
