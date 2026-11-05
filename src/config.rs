//! Konfigürasyon modeli, profiller ve disk kalıcılığı.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Crosshair'in temel geometrik biçimi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    /// Dört kollu artı.
    Cross,
    /// Sadece merkez noktası.
    Dot,
    /// Sadece daire.
    Circle,
    /// Üst kol olmadan artı (T biçimi).
    TShape,
    /// 45 derece döndürülmüş dört kol.
    XShape,
    /// Artı + daire birlikte.
    CrossCircle,
}

impl Style {
    pub const ALL: [Style; 6] = [
        Style::Cross,
        Style::Dot,
        Style::Circle,
        Style::TShape,
        Style::XShape,
        Style::CrossCircle,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Style::Cross => "Artı (Cross)",
            Style::Dot => "Nokta (Dot)",
            Style::Circle => "Daire (Circle)",
            Style::TShape => "T Biçimi",
            Style::XShape => "X Biçimi",
            Style::CrossCircle => "Artı + Daire",
        }
    }

    /// Bu biçimde kollar çiziliyor mu?
    pub fn has_arms(self) -> bool {
        matches!(
            self,
            Style::Cross | Style::TShape | Style::XShape | Style::CrossCircle
        )
    }

    /// Bu biçimde daire çiziliyor mu?
    pub fn has_circle(self) -> bool {
        matches!(self, Style::Circle | Style::CrossCircle)
    }
}

/// Renk: çarpılmamış sRGB bileşenleri ve ayrı saydamlık.
///
/// Alfa, renkten ayrı tutulur: egui'nin renk seçicileri alfayı önceden
/// çarptığı için tek bir `[u8; 4]` üzerinde düzenleme yapmak düşük alfa
/// değerlerinde renk bilgisini bozar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rgba8 {
    pub rgb: [u8; 3],
    pub alpha: u8,
}

impl Rgba8 {
    pub const fn new(r: u8, g: u8, b: u8, alpha: u8) -> Self {
        Self {
            rgb: [r, g, b],
            alpha,
        }
    }
}

impl Default for Rgba8 {
    fn default() -> Self {
        Self::new(255, 255, 255, 255)
    }
}

/// Tek bir crosshair profilinin tüm görsel parametreleri.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Crosshair {
    pub style: Style,
    /// Her bir kolun uzunluğu (piksel).
    pub arm_length: f32,
    /// Kol kalınlığı (piksel).
    pub thickness: f32,
    /// Merkezden kolların başlangıcına kadar olan boşluk (piksel).
    pub gap: f32,
    /// Kolların dönüşü (derece).
    pub rotation: f32,
    /// Ana renk.
    pub color: Rgba8,

    pub outline: bool,
    pub outline_thickness: f32,
    pub outline_color: Rgba8,

    pub center_dot: bool,
    pub center_dot_radius: f32,
    pub center_dot_color: Rgba8,

    pub circle_radius: f32,
    pub circle_filled: bool,

    /// Ekran merkezine göre kayma (piksel): [x, y].
    pub offset: [f32; 2],
    /// Tüm çizime uygulanan genel opaklık çarpanı (0.0 - 1.0).
    pub opacity: f32,
    /// Koordinatları tam piksele yuvarla (daha keskin görüntü).
    pub pixel_snap: bool,
}

impl Default for Crosshair {
    fn default() -> Self {
        Self {
            style: Style::Cross,
            arm_length: 10.0,
            thickness: 2.0,
            gap: 4.0,
            rotation: 0.0,
            color: Rgba8::new(0, 255, 0, 255),
            outline: true,
            outline_thickness: 1.0,
            outline_color: Rgba8::new(0, 0, 0, 200),
            center_dot: true,
            center_dot_radius: 1.5,
            center_dot_color: Rgba8::new(0, 255, 0, 255),
            circle_radius: 16.0,
            circle_filled: false,
            offset: [0.0, 0.0],
            opacity: 1.0,
            pixel_snap: true,
        }
    }
}

/// Global kısayollar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hotkeys {
    /// Crosshair'i göster/gizle.
    pub toggle_overlay: String,
    /// Sıradaki profile geç.
    pub next_profile: String,
    /// Ayarlar penceresini aç/kapat.
    pub toggle_settings: String,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            toggle_overlay: "Alt+KeyX".to_string(),
            next_profile: "Alt+KeyC".to_string(),
            toggle_settings: "Alt+KeyS".to_string(),
        }
    }
}

/// Overlay penceresinin yerleşimi. Birden fazla monitörde doğru ekranı
/// seçmek için elle konum/boyut verilebilir.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OverlayPlacement {
    /// `true` ise aşağıdaki konum/boyut kullanılır, aksi halde birincil
    /// monitör otomatik olarak kaplanır.
    pub manual: bool,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for OverlayPlacement {
    fn default() -> Self {
        Self {
            manual: false,
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        }
    }
}

/// Diske yazılan kök konfigürasyon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Etkin profilin adı.
    pub active_profile: String,
    /// Ada göre sıralı profiller.
    pub profiles: BTreeMap<String, Crosshair>,
    pub hotkeys: Hotkeys,
    pub placement: OverlayPlacement,
    /// Program açılışında ayarlar penceresi görünsün mü?
    pub open_settings_on_start: bool,
    /// Değişiklikler otomatik kaydedilsin mi?
    pub autosave: bool,
}

impl Default for Config {
    fn default() -> Self {
        let mut profiles = BTreeMap::new();
        profiles.insert("default".to_string(), Crosshair::default());
        Self {
            active_profile: "default".to_string(),
            profiles,
            hotkeys: Hotkeys::default(),
            placement: OverlayPlacement::default(),
            open_settings_on_start: true,
            autosave: true,
        }
    }
}

impl Config {
    /// Etkin profil; yoksa ilk profile, o da yoksa varsayılana düşer.
    pub fn active(&self) -> Crosshair {
        self.profiles
            .get(&self.active_profile)
            .cloned()
            .or_else(|| self.profiles.values().next().cloned())
            .unwrap_or_default()
    }

    /// Etkin profilin değiştirilebilir referansı. Profil yoksa oluşturulur.
    pub fn active_mut(&mut self) -> &mut Crosshair {
        let name = self.active_profile.clone();
        self.profiles.entry(name).or_default()
    }

    /// Profil adlarını sıralı döndürür.
    pub fn profile_names(&self) -> Vec<String> {
        self.profiles.keys().cloned().collect()
    }

    /// Sıradaki profile geçer (alfabetik, dairesel).
    pub fn cycle_profile(&mut self) {
        let names = self.profile_names();
        if names.len() < 2 {
            return;
        }
        let idx = names
            .iter()
            .position(|n| n == &self.active_profile)
            .unwrap_or(0);
        self.active_profile = names[(idx + 1) % names.len()].clone();
    }

    /// Yapılandırmanın tutarlı olmasını sağlar (boş profil listesi vb.).
    pub fn normalize(&mut self) {
        if self.profiles.is_empty() {
            self.profiles
                .insert("default".to_string(), Crosshair::default());
        }
        if !self.profiles.contains_key(&self.active_profile) {
            self.active_profile = self
                .profiles
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| "default".to_string());
        }
        self.opacity_clamp();
    }

    fn opacity_clamp(&mut self) {
        for ch in self.profiles.values_mut() {
            ch.opacity = ch.opacity.clamp(0.0, 1.0);
            ch.thickness = ch.thickness.max(0.1);
            ch.arm_length = ch.arm_length.max(0.0);
            ch.gap = ch.gap.max(0.0);
        }
    }
}

/// Varsayılan konfigürasyon dosyasının yolu.
pub fn default_config_path() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "XCross")
        .context("platform konfigürasyon dizini bulunamadı")?;
    Ok(dirs.config_dir().join("config.toml"))
}

/// Konfigürasyonu diskten yükler; dosya yoksa varsayılanı döndürür.
pub fn load(path: &Path) -> Result<Config> {
    if !path.exists() {
        let mut cfg = Config::default();
        cfg.normalize();
        return Ok(cfg);
    }
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("konfigürasyon okunamadı: {}", path.display()))?;
    let mut cfg: Config = toml::from_str(&text)
        .with_context(|| format!("konfigürasyon ayrıştırılamadı: {}", path.display()))?;
    cfg.normalize();
    Ok(cfg)
}

/// Konfigürasyonu diske yazar (dizinleri oluşturur).
pub fn save(path: &Path, cfg: &Config) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("dizin oluşturulamadı: {}", parent.display()))?;
    }
    let text = toml::to_string_pretty(cfg).context("konfigürasyon serileştirilemedi")?;
    std::fs::write(path, text)
        .with_context(|| format!("konfigürasyon yazılamadı: {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_preserves_values() {
        let mut cfg = Config::default();
        cfg.active_mut().arm_length = 42.0;
        cfg.active_mut().color = Rgba8::new(1, 2, 3, 4);
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn normalize_restores_missing_profile() {
        let mut cfg = Config {
            active_profile: "yok".into(),
            profiles: BTreeMap::new(),
            ..Config::default()
        };
        cfg.normalize();
        assert_eq!(cfg.active_profile, "default");
        assert!(cfg.profiles.contains_key("default"));
    }

    #[test]
    fn cycle_profile_wraps_around() {
        let mut cfg = Config::default();
        cfg.profiles.insert("b".into(), Crosshair::default());
        cfg.profiles.insert("c".into(), Crosshair::default());
        // Sıralı adlar: b, c, default
        cfg.active_profile = "b".into();
        cfg.cycle_profile();
        assert_eq!(cfg.active_profile, "c");
        cfg.cycle_profile();
        assert_eq!(cfg.active_profile, "default");
        cfg.cycle_profile();
        assert_eq!(cfg.active_profile, "b");
    }

    #[test]
    fn single_profile_cycle_is_noop() {
        let mut cfg = Config::default();
        cfg.cycle_profile();
        assert_eq!(cfg.active_profile, "default");
    }

    #[test]
    fn partial_toml_fills_defaults() {
        let cfg: Config = toml::from_str("active_profile = \"p\"\n[profiles.p]\narm_length = 7.0\n").unwrap();
        assert_eq!(cfg.profiles["p"].arm_length, 7.0);
        assert_eq!(cfg.profiles["p"].thickness, Crosshair::default().thickness);
        assert_eq!(cfg.hotkeys, Hotkeys::default());
    }
}
