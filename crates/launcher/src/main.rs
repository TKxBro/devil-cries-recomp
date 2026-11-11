#![windows_subsystem = "windows"]

mod app;
mod builder;
mod config;
mod theme;

use app::DevilCriesApp;
use eframe::egui;

fn main() -> Result<(), String> {
    let viewport = egui::ViewportBuilder::default()
        .with_title("Devil Cries Recomp")
        .with_inner_size([720.0, 500.0])
        .with_min_inner_size([600.0, 420.0])
        .with_active(true);

    let native = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Devil Cries Recomp",
        native,
        Box::new(|cc| {
            theme::apply_theme(&cc.egui_ctx);
            Ok(Box::new(DevilCriesApp::new()))
        }),
    )
    .map_err(|e| e.to_string())
}
