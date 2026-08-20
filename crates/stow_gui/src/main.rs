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

/// Setup comprehensive Unicode system fonts for Arabic, Cyrillic, CJK, and Latin path rendering
fn configure_unicode_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    #[cfg(target_os = "windows")]
    {
        // 1. Segoe UI (Primary system UI font with extensive Unicode coverage)
        if let Ok(font_data) = std::fs::read("C:\\Windows\\Fonts\\segoeui.ttf") {
            fonts.font_data.insert("segoe_ui".to_owned(), egui::FontData::from_owned(font_data));
            fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "segoe_ui".to_owned());
        }
        // 2. Tahoma (Excellent Arabic, Hebrew, and East European glyph rendering)
        if let Ok(font_data) = std::fs::read("C:\\Windows\\Fonts\\tahoma.ttf") {
            fonts.font_data.insert("tahoma".to_owned(), egui::FontData::from_owned(font_data));
            fonts.families.entry(egui::FontFamily::Proportional).or_default().push("tahoma".to_owned());
        }
        // 3. Arial (Universal fallback)
        if let Ok(font_data) = std::fs::read("C:\\Windows\\Fonts\\arial.ttf") {
            fonts.font_data.insert("arial".to_owned(), egui::FontData::from_owned(font_data));
            fonts.families.entry(egui::FontFamily::Proportional).or_default().push("arial".to_owned());
            fonts.families.entry(egui::FontFamily::Monospace).or_default().push("arial".to_owned());
        }
        // 4. Segoe UI Symbol (Extended symbols and math)
        if let Ok(font_data) = std::fs::read("C:\\Windows\\Fonts\\seguisym.ttf") {
            fonts.font_data.insert("segoe_sym".to_owned(), egui::FontData::from_owned(font_data));
            fonts.families.entry(egui::FontFamily::Proportional).or_default().push("segoe_sym".to_owned());
        }
    }

    #[cfg(target_os = "macos")]
    {
        for path in &[
            "/System/Library/Fonts/SFPro.ttf",
            "/System/Library/Fonts/HelveticaNeue.ttc",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/Library/Fonts/Arial Unicode.ttf",
        ] {
            if let Ok(font_data) = std::fs::read(path) {
                fonts.font_data.insert("macos_sys_font".to_owned(), egui::FontData::from_owned(font_data));
                fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "macos_sys_font".to_owned());
                break;
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        for path in &[
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        ] {
            if let Ok(font_data) = std::fs::read(path) {
                fonts.font_data.insert("linux_sys_font".to_owned(), egui::FontData::from_owned(font_data));
                fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "linux_sys_font".to_owned());
                break;
            }
        }
    }

    ctx.set_fonts(fonts);
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
        Box::new(|cc| {
            configure_unicode_fonts(&cc.egui_ctx);
            Ok(Box::new(StowApp::new(&cc.egui_ctx)))
        }),
    )
}
