//! Uygulama durumu: overlay kök penceresi + ayarlar görünümü.

use crate::config::{self, Config};
use crate::crosshair;
use crate::hotkeys::{Action, HotkeyService};
use crate::ui::{self, Request, SettingsState};
use egui::{Context, ViewportCommand};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Otomatik kaydetme için bekleme süresi.
const AUTOSAVE_DEBOUNCE: Duration = Duration::from_millis(600);
/// Ayarlar kapalıyken kısayolları yoklama aralığı.
const IDLE_POLL: Duration = Duration::from_millis(60);

pub struct XCrossApp {
    cfg: Config,
    /// Diske yazılmış son hâli; değişiklik tespiti için.
    saved: Config,
    config_path: PathBuf,
    config_path_text: String,

    overlay_visible: bool,
    settings_open: bool,
    settings_state: SettingsState,

    hotkeys: HotkeyService,
    /// Overlay penceresine uygulanmış son yerleşim ayarı.
    applied_placement: Option<config::OverlayPlacement>,
    pending_save_since: Option<Instant>,
}

impl XCrossApp {
    pub fn new(mut cfg: Config, config_path: PathBuf, force_settings: Option<bool>) -> Self {
        cfg.normalize();
        let settings_open = force_settings.unwrap_or(cfg.open_settings_on_start);
        let mut hotkeys = HotkeyService::new();
        hotkeys.apply(&cfg.hotkeys);
        Self {
            saved: cfg.clone(),
            cfg,
            config_path_text: config_path.display().to_string(),
            config_path,
            overlay_visible: true,
            settings_open,
            settings_state: SettingsState::default(),
            hotkeys,
            applied_placement: None,
            pending_save_since: None,
        }
    }

    fn save_now(&mut self) {
        match config::save(&self.config_path, &self.cfg) {
            Ok(()) => {
                self.saved = self.cfg.clone();
                self.pending_save_since = None;
                self.settings_state.status = "Kaydedildi.".into();
            }
            Err(e) => {
                self.settings_state.status = format!("Kaydedilemedi: {e}");
            }
        }
    }

    /// Değişiklikleri izler ve `autosave` açıkken gecikmeli kaydeder.
    fn tick_autosave(&mut self) {
        if self.cfg == self.saved {
            self.pending_save_since = None;
            return;
        }
        if !self.cfg.autosave {
            return;
        }
        match self.pending_save_since {
            None => self.pending_save_since = Some(Instant::now()),
            Some(t) if t.elapsed() >= AUTOSAVE_DEBOUNCE => self.save_now(),
            Some(_) => {}
        }
    }

    fn handle_hotkeys(&mut self) {
        for action in self.hotkeys.poll() {
            match action {
                Action::ToggleOverlay => self.overlay_visible = !self.overlay_visible,
                Action::NextProfile => {
                    self.cfg.cycle_profile();
                    self.settings_state.status = format!("Profil: {}", self.cfg.active_profile);
                }
                Action::ToggleSettings => self.settings_open = !self.settings_open,
            }
        }
    }

    /// Overlay penceresini hedef ekranı kaplayacak biçimde konumlandırır.
    fn place_overlay(&mut self, ctx: &Context) {
        if self.applied_placement == Some(self.cfg.placement) {
            return;
        }
        let (pos, size) = if self.cfg.placement.manual {
            let p = self.cfg.placement;
            (egui::pos2(p.x, p.y), egui::vec2(p.width, p.height))
        } else {
            let monitor = ctx.input(|i| i.viewport().monitor_size);
            match monitor {
                Some(size) => (egui::Pos2::ZERO, size),
                // Monitör boyutu henüz bilinmiyor: sonraki karede tekrar dene.
                None => return,
            }
        };
        ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
        self.applied_placement = Some(self.cfg.placement);
    }

    fn draw_overlay(&self, ui: &mut egui::Ui) {
        if !self.overlay_visible {
            return;
        }
        let center = ui.max_rect().center();
        crosshair::draw(ui.painter(), center, &self.cfg.active());
    }

    fn draw_settings(&mut self, ctx: &Context) {
        if !self.settings_open {
            return;
        }
        let builder = egui::ViewportBuilder::default()
            .with_title("XCross — Ayarlar")
            .with_inner_size([420.0, 640.0])
            .with_min_inner_size([360.0, 420.0])
            // Overlay her zaman üstte olduğu için ayarlar penceresinin de
            // üstte olması gerekir, aksi halde overlay'in arkasında kalır.
            .with_always_on_top();

        let hotkey_status = self.hotkeys.last_error().map(str::to_string).or_else(|| {
            (!self.hotkeys.available())
                .then(|| "Global kısayollar bu platformda kullanılamıyor.".to_string())
        });

        let mut requests = Vec::new();
        let mut close = false;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("xcross_settings"),
            builder,
            |ui, _class| {
                egui::CentralPanel::default_margins().show(ui, |ui| {
                    requests = ui::settings_ui(
                        ui,
                        &mut self.cfg,
                        &mut self.settings_state,
                        &mut self.overlay_visible,
                        &self.config_path_text,
                        hotkey_status.as_deref(),
                    );
                });
                if ui.ctx().input(|i| i.viewport().close_requested()) {
                    close = true;
                }
            },
        );

        for req in requests {
            match req {
                Request::Save => self.save_now(),
                Request::ReloadHotkeys => {
                    let hk = self.cfg.hotkeys.clone();
                    self.hotkeys.apply(&hk);
                }
                Request::CloseSettings => close = true,
            }
        }
        if close {
            self.settings_open = false;
        }
    }
}

impl eframe::App for XCrossApp {
    /// Overlay'in şeffaf olabilmesi için zemin tamamen saydam.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    /// Çizim dışı iş: yerleşim, kısayollar, otomatik kayıt.
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.place_overlay(ctx);
        self.handle_hotkeys();
        self.tick_autosave();
        // Ayarlar kapalıyken olay akışı olmadığı için kısayolları yoklamak
        // adına düzenli uyanmamız gerekir.
        ctx.request_repaint_after(IDLE_POLL);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw_overlay(ui);
        let ctx = ui.ctx().clone();
        self.draw_settings(&ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.cfg != self.saved {
            let _ = config::save(&self.config_path, &self.cfg);
        }
    }
}
