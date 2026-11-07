#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod runner;
mod ui;

use config::Config;
use eframe::egui;

fn main() -> eframe::Result {
    let path = directories::ProjectDirs::from("", "varnholt", "clang-tidy-gui")
        .expect("A user configuration directory is required")
        .config_dir()
        .join("config.json");
    let mut notice = None;
    let source = if path.exists() {
        path.clone()
    } else {
        "config.json".into()
    };
    let mut config = if source.exists() {
        match Config::load(&source) {
            Ok(config) => config,
            Err(error) => {
                notice = Some(format!("Could not load {}: {error}. Settings will not be saved this session; repair or move that file first.", source.display()));
                Config::default()
            }
        }
    } else {
        Config::default()
    };
    let save_enabled = notice.is_none();
    config.merge_catalogue();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(window_icon())
            .with_inner_size([1180.0, 820.0])
            .with_min_inner_size([900.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native(
        "clang-tidy-gui",
        options,
        Box::new(move |cc| {
            Ok(Box::new(ui::Application::new(
                cc,
                config,
                path,
                notice,
                save_enabled,
            )))
        }),
    )
}

fn window_icon() -> egui::IconData {
    let mut rgba = Vec::with_capacity(64 * 64 * 4);
    for y in 0..64 {
        for x in 0..64 {
            let dx = x as f32 - 31.5;
            let dy = y as f32 - 31.5;
            let diamond = dx.abs() + dy.abs();
            let accent = (25.0..=29.0).contains(&diamond) || dx * dx + dy * dy < 22.0;
            rgba.extend_from_slice(if accent {
                &[91, 218, 190, 255]
            } else {
                &[13, 19, 28, 255]
            });
        }
    }
    egui::IconData {
        rgba,
        width: 64,
        height: 64,
    }
}
