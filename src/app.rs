//! Uygulama durumu.
//!
//! Kök pencere ayarlar penceresidir; kapatıldığında gizlenir ve program arka
//! planda çalışmaya devam eder. Crosshair ayrı bir overlay'de çizilir:
//! Windows'ta yerel katmanlı pencere (`overlay_win`), diğer platformlarda
//! saydam bir egui penceresi.

use crate::config::{self, Config};
use crate::hotkeys::{Action, HotkeyService};
use crate::ui::{self, Request, SettingsState};
use egui::{Context, ViewportCommand};
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[cfg(windows)]
use crate::{overlay_win, tray};

/// Otomatik kaydetme için bekleme süresi.
const AUTOSAVE_DEBOUNCE: Duration = Duration::from_millis(600);
/// Pencere gizliyken kısayol/tepsi olaylarını yoklama aralığı.
const IDLE_POLL: Duration = Duration::from_millis(50);

pub struct XCrossApp {
    cfg: Config,
    /// Diske yazılmış son hâli; değişiklik tespiti için.
    saved: Config,
    config_path: PathBuf,
    config_path_text: String,

    overlay_visible: bool,
    /// Ayarlar penceresinin istenen görünürlüğü.
    settings_visible: bool,
    /// Pencereye en son uygulanan görünürlük.
    settings_applied: bool,
    /// `logic` çağrı sayısı; eframe'in ilk gösterimini beklemek için.
    frames: u64,
    settings_state: SettingsState,
    quitting: bool,

    hotkeys: HotkeyService,
    #[cfg(windows)]
    overlay: Result<overlay_win::NativeOverlay, String>,
    #[cfg(windows)]
    tray: Result<tray::Tray, String>,

    pending_save_since: Option<Instant>,
}

impl XCrossApp {
    /// `settings_visible`, kök pencerenin hangi görünürlükle oluşturulduğudur.
    pub fn new(mut cfg: Config, config_path: PathBuf, settings_visible: bool) -> Self {
        cfg.normalize();
        let mut hotkeys = HotkeyService::new();
        hotkeys.apply(&cfg.hotkeys);
        let mut app = Self {
            saved: cfg.clone(),
            cfg,
            config_path_text: config_path.display().to_string(),
            config_path,
            overlay_visible: true,
            settings_visible,
            // eframe kök pencereyi ilk kareden sonra her koşulda gösterir
            // (`with_visible(false)` yok sayılır); gizlemeyi biz yaparız.
            settings_applied: true,
            frames: 0,
            settings_state: SettingsState::default(),
            quitting: false,
            hotkeys,
            #[cfg(windows)]
            overlay: overlay_win::NativeOverlay::spawn(),
            #[cfg(windows)]
            tray: tray::Tray::new(),
            pending_save_since: None,
        };
        // Geri dönüş yolu yoksa ayarlar penceresi gizli başlayamaz; aksi halde
        // programa ulaşmanın tek yolu Görev Yöneticisi olurdu.
        if !app.can_run_hidden() {
            app.settings_visible = true;
        }
        app
    }

    /// Ayarlar penceresi kapatılınca program arka planda yaşayabilir mi?
    /// Geri dönüş yolu (tepsi simgesi veya kısayol) yoksa kapatma = çıkış.
    fn can_run_hidden(&self) -> bool {
        #[cfg(windows)]
        if self.tray.is_ok() {
            return true;
        }
        self.hotkeys.available() && self.hotkeys.last_error().is_none()
    }

    fn save_now(&mut self) {
        match config::save(&self.config_path, &self.cfg) {
            Ok(()) => {
                self.saved = self.cfg.clone();
                self.pending_save_since = None;
                self.settings_state.status = "Kaydedildi.".into();
            }
            Err(e) => self.settings_state.status = format!("Kaydedilemedi: {e}"),
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

    fn quit(&mut self, ctx: &Context) {
        self.quitting = true;
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    fn hide_settings(&mut self, ctx: &Context) {
        if self.can_run_hidden() {
            self.settings_visible = false;
        } else {
            self.quit(ctx);
        }
    }

    fn handle_events(&mut self, ctx: &Context) {
        for action in self.hotkeys.poll() {
            match action {
                Action::ToggleOverlay => self.overlay_visible = !self.overlay_visible,
                Action::NextProfile => {
                    self.cfg.cycle_profile();
                    self.settings_state.status = format!("Profil: {}", self.cfg.active_profile);
                }
                Action::ToggleSettings => {
                    if self.settings_visible {
                        self.hide_settings(ctx);
                    } else {
                        self.settings_visible = true;
                    }
                }
            }
        }

        #[cfg(windows)]
        let tray_actions = self.tray.as_ref().map(|t| t.poll()).unwrap_or_default();
        #[cfg(windows)]
        for action in tray_actions {
            match action {
                tray::TrayAction::ShowSettings => self.settings_visible = true,
                tray::TrayAction::ToggleOverlay => self.overlay_visible = !self.overlay_visible,
                tray::TrayAction::Quit => self.quit(ctx),
            }
        }
    }

    /// İstenen görünürlüğü ayarlar penceresine uygular.
    fn sync_settings_window(&mut self, ctx: &Context) {
        // eframe ilk kare çizildikten sonra pencereyi kendisi gösterir; ondan
        // önce gönderilen gizleme komutu ezilir.
        if self.frames < 2 || self.settings_visible == self.settings_applied {
            return;
        }
        ctx.send_viewport_cmd(ViewportCommand::Visible(self.settings_visible));
        if self.settings_visible {
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        }
        self.settings_applied = self.settings_visible;
    }

    /// Overlay'in durumu, ayarlarda gösterilmek üzere.
    fn overlay_status(&self) -> String {
        #[cfg(windows)]
        {
            let mut s = match &self.overlay {
                Ok(o) => format!("Overlay: {}", o.status()),
                Err(e) => format!("Overlay başlatılamadı: {e}"),
            };
            if let Err(e) = &self.tray {
                s.push_str(&format!("\nTepsi simgesi yok: {e}"));
            }
            s
        }
        #[cfg(not(windows))]
        {
            "Overlay: saydam pencere (Linux'ta birleştirici gerekir)".to_string()
        }
    }

    #[cfg(windows)]
    fn sync_overlay(&mut self, _ctx: &Context) {
        let frame = overlay_win::Frame {
            crosshair: self.cfg.active(),
            visible: self.overlay_visible,
            placement: self.cfg.placement,
        };
        if let Ok(o) = &mut self.overlay {
            o.update(frame);
        }
    }

    /// Windows dışı platformlarda overlay, saydam ve tıklama geçirgen ayrı
    /// bir egui penceresidir.
    #[cfg(not(windows))]
    fn sync_overlay(&mut self, ctx: &Context) {
        use crate::crosshair;

        let p = self.cfg.placement;
        let (pos, size) = if p.manual {
            (egui::pos2(p.x, p.y), egui::vec2(p.width, p.height))
        } else {
            match ctx.input(|i| i.viewport().monitor_size) {
                Some(size) => (egui::Pos2::ZERO, size),
                None => return,
            }
        };
        let builder = egui::ViewportBuilder::default()
            .with_title("XCross Overlay")
            .with_app_id("xcross-overlay")
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_mouse_passthrough(true)
            .with_resizable(false)
            .with_taskbar(false)
            .with_position(pos)
            .with_inner_size(size);

        let ch = self.cfg.active();
        let prims = if self.overlay_visible {
            crosshair::primitives(&ch)
        } else {
            Vec::new()
        };
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("xcross_overlay"),
            builder,
            |ui, _class| {
                let scale = 1.0 / ui.ctx().pixels_per_point();
                let offset = egui::vec2(ch.offset[0], ch.offset[1]) * scale;
                let mut center = ui.max_rect().center() + offset;
                if ch.pixel_snap {
                    let p = ui.painter();
                    center = egui::pos2(
                        p.round_to_pixel_center(center.x),
                        p.round_to_pixel_center(center.y),
                    );
                }
                crosshair::paint_egui(ui.painter(), center, &prims, scale);
            },
        );
    }
}

impl eframe::App for XCrossApp {
    /// Saydam overlay penceresi (Linux) için zemin saydam. Ayarlar penceresi
    /// kendi panel arka planını çizer.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    /// Çizim dışı iş. Ayarlar penceresi gizliyken de çağrılır.
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.frames = self.frames.saturating_add(1);
        self.handle_events(ctx);
        self.sync_settings_window(ctx);
        #[cfg(windows)]
        self.sync_overlay(ctx);
        #[cfg(not(windows))]
        if !self.settings_visible {
            // Overlay bir egui penceresi olduğu için, ayarlar gizliyken de
            // `ui` çalışmaya devam etmeli (komut yalnızca bir kare geçerli).
            ctx.send_viewport_cmd(ViewportCommand::RequestPaintWhileHidden);
        }
        self.tick_autosave();
        ctx.request_repaint_after(IDLE_POLL);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Pencerenin X düğmesi: programı kapatma, arka plana al.
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting && self.can_run_hidden()
        {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.settings_visible = false;
        }

        let hotkey_status = self.hotkeys.last_error().map(str::to_string).or_else(|| {
            (!self.hotkeys.available())
                .then(|| "Global kısayollar bu platformda kullanılamıyor.".to_string())
        });
        let overlay_status = self.overlay_status();
        let hide_label = if self.can_run_hidden() {
            "Arka plana al"
        } else {
            "Kapat"
        };

        let mut requests = Vec::new();
        egui::CentralPanel::default_margins().show(ui, |ui| {
            requests = ui::settings_ui(
                ui,
                &mut self.cfg,
                &mut self.settings_state,
                &mut self.overlay_visible,
                &ui::Info {
                    config_path: &self.config_path_text,
                    hotkey_status: hotkey_status.as_deref(),
                    overlay_status: &overlay_status,
                    hide_label,
                },
            );
        });

        for req in requests {
            match req {
                Request::Save => self.save_now(),
                Request::ReloadHotkeys => {
                    let hk = self.cfg.hotkeys.clone();
                    self.hotkeys.apply(&hk);
                }
                Request::HideSettings => self.hide_settings(&ctx),
                Request::Quit => self.quit(&ctx),
            }
        }

        #[cfg(not(windows))]
        self.sync_overlay(&ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.cfg != self.saved {
            let _ = config::save(&self.config_path, &self.cfg);
        }
    }
}
