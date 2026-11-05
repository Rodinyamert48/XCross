//! Ayarlar penceresinin arayüzü.

use crate::config::{Config, Crosshair, Style};
use crate::crosshair;
use crate::hotkeys;
use egui::{Color32, RichText, Ui, Vec2};

/// Arayüzün, uygulamanın yürütmesi gereken istekleri.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// Konfigürasyonu hemen diske yaz.
    Save,
    /// Kısayolları yeniden kaydet.
    ReloadHotkeys,
    /// Ayarlar penceresini gizle (crosshair çalışmaya devam eder).
    HideSettings,
    /// Programdan tamamen çık.
    Quit,
}

/// Arayüzün gösterdiği, uygulamadan gelen salt okunur bilgiler.
pub struct Info<'a> {
    pub config_path: &'a str,
    pub hotkey_status: Option<&'a str>,
    pub overlay_status: &'a str,
    /// Ayarları gizleme düğmesinin etiketi.
    pub hide_label: &'a str,
}

/// Arayüzün kalıcı olmayan durumu.
#[derive(Debug, Default)]
pub struct SettingsState {
    pub new_profile_name: String,
    pub status: String,
}

/// Renk seçici + ayrı saydamlık kaydırıcısı.
///
/// Alfa kasıtlı olarak seçicinin dışında tutulur: egui'nin alfalı seçicileri
/// değeri önceden çarpılmış `Color32`'ye çevirdiği için düşük saydamlıkta
/// renk tonu bozulur.
fn color_row(ui: &mut Ui, label: &str, color: &mut crate::config::Rgba8) {
    ui.horizontal(|ui| {
        egui::color_picker::color_edit_button_srgb(ui, &mut color.rgb);
        ui.label(label);
        ui.add(
            egui::DragValue::new(&mut color.alpha)
                .speed(1.0)
                .range(0..=255)
                .prefix("saydamlık "),
        );
    });
}

/// Crosshair'in canlı önizlemesi.
fn preview(ui: &mut Ui, ch: &Crosshair) {
    let size = Vec2::new(ui.available_width().min(360.0), 150.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    // Oyun görüntüsünü taklit eden koyu zemin + ızgara.
    painter.rect_filled(rect, 4.0, Color32::from_rgb(32, 34, 38));
    let grid = Color32::from_rgb(48, 51, 56);
    let step = 20.0;
    let mut x = rect.left();
    while x < rect.right() {
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            egui::Stroke::new(1.0, grid),
        );
        x += step;
    }
    let mut y = rect.top();
    while y < rect.bottom() {
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            egui::Stroke::new(1.0, grid),
        );
        y += step;
    }
    // Ölçüler piksel cinsinden; ekran ölçeklemesinde (%125, %150) önizleme
    // gerçek boyutuyla görünsün diye egui noktalarına çevrilir. Konum
    // kayması önizlemede uygulanmaz.
    let scale = 1.0 / ui.ctx().pixels_per_point();
    let c = rect.center();
    let center = egui::pos2(
        painter.round_to_pixel_center(c.x),
        painter.round_to_pixel_center(c.y),
    );
    crosshair::paint_egui(&painter, center, &crosshair::primitives(ch), scale);
}

/// Ayarlar arayüzünü çizer ve yürütülmesi gereken istekleri döndürür.
pub fn settings_ui(
    ui: &mut Ui,
    cfg: &mut Config,
    state: &mut SettingsState,
    overlay_visible: &mut bool,
    info: &Info<'_>,
) -> Vec<Request> {
    let mut requests = Vec::new();

    ui.horizontal(|ui| {
        ui.heading("XCross");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button("Çıkış")
                .on_hover_text("Programı tamamen kapatır")
                .clicked()
            {
                requests.push(Request::Quit);
            }
            if ui
                .button(info.hide_label)
                .on_hover_text("Ayarları gizler; crosshair çalışmaya devam eder")
                .clicked()
            {
                requests.push(Request::HideSettings);
            }
            ui.checkbox(overlay_visible, "Crosshair açık");
        });
    });
    let problem = [
        "başarısız",
        "başlatılamadı",
        "oluşturulamadı",
        "hata",
        "yok",
    ]
    .iter()
    .any(|w| info.overlay_status.contains(w));
    let status = RichText::new(info.overlay_status).small();
    if problem {
        ui.colored_label(Color32::from_rgb(230, 120, 80), status);
    } else {
        ui.label(status.weak());
    }
    ui.separator();

    // --- Profiller ---
    ui.horizontal(|ui| {
        ui.label("Profil:");
        let current = cfg.active_profile.clone();
        egui::ComboBox::from_id_salt("profil_secimi")
            .selected_text(current.as_str())
            .show_ui(ui, |ui| {
                for name in cfg.profile_names() {
                    if ui
                        .selectable_label(name == current, name.as_str())
                        .clicked()
                    {
                        cfg.active_profile = name;
                    }
                }
            });

        if ui
            .button("Kopyala")
            .on_hover_text("Etkin profili yeni bir ad altında çoğaltır")
            .clicked()
        {
            let base = format!("{current}-kopya");
            let mut name = base.clone();
            let mut n = 2;
            while cfg.profiles.contains_key(&name) {
                name = format!("{base}-{n}");
                n += 1;
            }
            let ch = cfg.active();
            cfg.profiles.insert(name.clone(), ch);
            cfg.active_profile = name;
            state.status = "Profil kopyalandı.".into();
        }

        let can_delete = cfg.profiles.len() > 1;
        if ui
            .add_enabled(can_delete, egui::Button::new("Sil"))
            .on_disabled_hover_text("En az bir profil kalmalı")
            .clicked()
        {
            cfg.profiles.remove(&current);
            cfg.normalize();
            state.status = format!("'{current}' silindi.");
        }
    });

    ui.horizontal(|ui| {
        ui.label("Yeni profil:");
        ui.add(
            egui::TextEdit::singleline(&mut state.new_profile_name)
                .desired_width(140.0)
                .hint_text("ad"),
        );
        let name = state.new_profile_name.trim().to_string();
        let ok = !name.is_empty() && !cfg.profiles.contains_key(&name);
        if ui.add_enabled(ok, egui::Button::new("Ekle")).clicked() {
            cfg.profiles.insert(name.clone(), Crosshair::default());
            cfg.active_profile = name;
            state.new_profile_name.clear();
            state.status = "Profil eklendi.".into();
        }
    });

    ui.add_space(6.0);
    preview(ui, &cfg.active());
    ui.add_space(6.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        let ch = cfg.active_mut();

        ui.collapsing("Biçim", |ui| {
            egui::ComboBox::from_label("Şekil")
                .selected_text(ch.style.label())
                .show_ui(ui, |ui| {
                    for style in Style::ALL {
                        ui.selectable_value(&mut ch.style, style, style.label());
                    }
                });

            ui.add_enabled_ui(ch.style.has_arms(), |ui| {
                ui.add(egui::Slider::new(&mut ch.arm_length, 0.0..=100.0).text("Kol uzunluğu"));
                ui.add(egui::Slider::new(&mut ch.gap, 0.0..=60.0).text("Merkez boşluğu"));
                ui.add(egui::Slider::new(&mut ch.rotation, -180.0..=180.0).text("Dönüş (°)"));
            });
            ui.add(egui::Slider::new(&mut ch.thickness, 0.5..=20.0).text("Kalınlık"));

            ui.add_enabled_ui(ch.style.has_circle(), |ui| {
                ui.add(
                    egui::Slider::new(&mut ch.circle_radius, 0.0..=200.0).text("Daire yarıçapı"),
                );
                ui.checkbox(&mut ch.circle_filled, "Daireyi doldur");
            });

            ui.checkbox(&mut ch.center_dot, "Merkez noktası");
            ui.add_enabled_ui(ch.center_dot || ch.style == Style::Dot, |ui| {
                ui.add(
                    egui::Slider::new(&mut ch.center_dot_radius, 0.0..=20.0).text("Nokta yarıçapı"),
                );
            });
        });

        ui.collapsing("Renk", |ui| {
            color_row(ui, "Ana renk", &mut ch.color);
            color_row(ui, "Merkez noktası rengi", &mut ch.center_dot_color);
            ui.checkbox(&mut ch.outline, "Kontur (dış çizgi)");
            ui.add_enabled_ui(ch.outline, |ui| {
                ui.add(
                    egui::Slider::new(&mut ch.outline_thickness, 0.0..=10.0)
                        .text("Kontur kalınlığı"),
                );
                color_row(ui, "Kontur rengi", &mut ch.outline_color);
            });
            ui.add(egui::Slider::new(&mut ch.opacity, 0.0..=1.0).text("Genel opaklık"));
        });

        ui.collapsing("Konum", |ui| {
            ui.add(egui::Slider::new(&mut ch.offset[0], -400.0..=400.0).text("Yatay kayma (X)"));
            ui.add(egui::Slider::new(&mut ch.offset[1], -400.0..=400.0).text("Dikey kayma (Y)"));
            if ui.button("Kaymayı sıfırla").clicked() {
                ch.offset = [0.0, 0.0];
            }
            ui.checkbox(&mut ch.pixel_snap, "Piksele hizala (daha keskin)")
                .on_hover_text("Tek sayılı kalınlıklarda bulanıklığı önler");
        });

        ui.collapsing("Overlay penceresi", |ui| {
            ui.label(
                RichText::new(
                    "Birden fazla monitörde crosshair yanlış ekrana düşerse, \
                     konumu ve boyutu elle verin.",
                )
                .weak(),
            );
            ui.checkbox(&mut cfg.placement.manual, "Elle konumlandır");
            ui.add_enabled_ui(cfg.placement.manual, |ui| {
                ui.horizontal(|ui| {
                    ui.label("X");
                    ui.add(egui::DragValue::new(&mut cfg.placement.x).speed(1.0));
                    ui.label("Y");
                    ui.add(egui::DragValue::new(&mut cfg.placement.y).speed(1.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Genişlik");
                    ui.add(egui::DragValue::new(&mut cfg.placement.width).range(64.0..=16384.0));
                    ui.label("Yükseklik");
                    ui.add(egui::DragValue::new(&mut cfg.placement.height).range(64.0..=16384.0));
                });
            });
        });

        ui.collapsing("Kısayollar", |ui| {
            let mut changed = false;
            for (label, value) in [
                ("Göster/Gizle", &mut cfg.hotkeys.toggle_overlay),
                ("Sıradaki profil", &mut cfg.hotkeys.next_profile),
                ("Ayarlar", &mut cfg.hotkeys.toggle_settings),
            ] {
                ui.horizontal(|ui| {
                    ui.label(label);
                    let valid = hotkeys::is_valid(value);
                    let edit = egui::TextEdit::singleline(value)
                        .desired_width(160.0)
                        .text_color_opt(if valid {
                            None
                        } else {
                            Some(Color32::from_rgb(220, 90, 90))
                        });
                    changed |= ui.add(edit).changed();
                });
            }
            ui.label(
                RichText::new("Biçim: Ctrl+Alt+KeyX, Shift+F1, Alt+Numpad5 ...")
                    .weak()
                    .small(),
            );
            if changed || ui.button("Kısayolları uygula").clicked() {
                requests.push(Request::ReloadHotkeys);
            }
            if let Some(err) = info.hotkey_status {
                ui.colored_label(Color32::from_rgb(220, 150, 80), err);
            }
        });

        ui.collapsing("Genel", |ui| {
            ui.checkbox(&mut cfg.open_settings_on_start, "Açılışta ayarları göster");
            ui.checkbox(&mut cfg.autosave, "Değişiklikleri otomatik kaydet");
            ui.horizontal(|ui| {
                ui.label("Konfigürasyon:");
                ui.label(RichText::new(info.config_path).monospace().small());
            });
            if ui.button("Profili varsayılana döndür").clicked() {
                *cfg.active_mut() = Crosshair::default();
                state.status = "Profil sıfırlandı.".into();
            }
        });
    });

    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("Kaydet").clicked() {
            requests.push(Request::Save);
        }
        ui.label(RichText::new(&state.status).weak());
    });

    requests
}

#[cfg(test)]
mod tests {

    /// Renk seçicisinin kullandığı dönüşüm (`[u8;3]` -> `Hsva` -> `[u8;3]`)
    /// kayıpsız olmalı; aksi halde seçiciye her dokunuş rengi kaydırır.
    #[test]
    fn opaque_color_roundtrip_is_lossless() {
        for rgb in [
            [12, 34, 56],
            [0, 255, 0],
            [255, 128, 0],
            [0, 0, 0],
            [7, 7, 7],
        ] {
            let back = egui::ecolor::Hsva::from_srgb(rgb).to_srgb();
            assert_eq!(back, rgb, "kayma: {rgb:?} -> {back:?}");
        }
    }
}
