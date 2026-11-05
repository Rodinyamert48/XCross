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

    let force_settings = if cli.settings {
        Some(true)
    } else if cli.no_settings {
        Some(false)
    } else {
        None
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("XCross Overlay")
            .with_app_id("xcross")
            .with_inner_size([640.0, 480.0])
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_mouse_passthrough(true)
            .with_resizable(false)
            .with_taskbar(false),
        // Oyun performansını etkilememesi için düşük yük: yalnızca gerektiğinde çizer.
        ..Default::default()
    };

    let app = app::XCrossApp::new(cfg, config_path, force_settings);
    eframe::run_native("XCross", options, Box::new(|_cc| Ok(Box::new(app))))
        .map_err(|e| anyhow::anyhow!("pencere oluşturulamadı: {e}"))
        .context("XCross başlatılamadı")?;
    Ok(())
}
