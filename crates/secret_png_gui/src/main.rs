#![windows_subsystem = "windows"]

mod app;

use app::SecretPngApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 780.0])
            .with_min_inner_size([540.0, 600.0])
            .with_title("Secret PNG — Video in Image Carrier Engine"),
        ..Default::default()
    };

    eframe::run_native(
        "Secret PNG",
        options,
        Box::new(|_cc| Ok(Box::new(SecretPngApp::default()))),
    )
}
