#![windows_subsystem = "windows"]

mod app;

use app::StowApp;
use eframe::egui;

fn load_app_icon() -> Option<egui::IconData> {
    let icon_bytes = include_bytes!("../assets/logo.png");
    let img = image::load_from_memory(icon_bytes).ok()?.to_rgba8();
    let (width, height) = (img.width(), img.height());
    Some(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([720.0, 780.0])
        .with_min_inner_size([540.0, 600.0])
        .with_title("Stow - Universal Stealth Carrier Engine");

    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Stow",
        options,
        Box::new(|cc| Ok(Box::new(StowApp::new(&cc.egui_ctx)))),
    )
}
