//! XCross — crosshair göstermeyen oyunlar için ekran ortasına özelleştirilebilir
//! bir nişangâh çizen overlay.
//!
//! Kök pencere, tam ekranı kaplayan şeffaf ve tıklama geçirgen (click-through)
//! overlay'dir; ayarlar ayrı bir pencerede açılır ve kapatılabilir.

// Windows'ta çalışırken arkada konsol penceresi açılmasın.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod crosshair;
mod hotkeys;
#[cfg(windows)]
mod overlay_win;
mod raster;
#[cfg(windows)]
mod tray;
mod ui;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "xcross",
    about = "Oyunlar için özelleştirilebilir crosshair overlay",
    version
)]
struct Cli {
    /// Kullanılacak profilin adı (yoksa oluşturulur).
    #[arg(short, long)]
    profile: Option<String>,

    /// Alternatif konfigürasyon dosyası yolu.
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Ayarlar penceresini açmadan doğrudan overlay ile başla.
    #[arg(long, conflicts_with = "settings")]
    no_settings: bool,

    /// Ayarlar penceresini açarak başla.
    #[arg(long)]
    settings: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let config_path = match cli.config {
        Some(p) => p,
        None => config::default_config_path()?,
    };
    let mut cfg = config::load(&config_path)?;

    if let Some(profile) = cli.profile {
        cfg.profiles.entry(profile.clone()).or_default();
        cfg.active_profile = profile;
    }

    let wants_settings = if cli.settings {
        true
    } else if cli.no_settings {
        false
    } else {
        cfg.open_settings_on_start
    };

    let app = app::XCrossApp::new(cfg, config_path, wants_settings);

    let icon_size = 64;
    let icon = egui::IconData {
        rgba: raster::icon_rgba(icon_size),
        width: icon_size,
        height: icon_size,
    };
    // Kök pencere ayarlar penceresidir; crosshair ayrı bir overlay'de çizilir.
    // Başlangıçta gizli olması gerekiyorsa bunu uygulama kendisi yapar: eframe
    // ilk kareden sonra pencereyi her koşulda gösterir.
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("XCross — Ayarlar")
            .with_app_id("xcross")
            .with_icon(icon)
            .with_inner_size([440.0, 680.0])
            .with_min_inner_size([380.0, 460.0]),
        ..Default::default()
    };

    eframe::run_native("XCross", options, Box::new(|_cc| Ok(Box::new(app))))
        .map_err(|e| anyhow::anyhow!("pencere oluşturulamadı: {e}"))
        .context("XCross başlatılamadı")?;
    Ok(())
}
