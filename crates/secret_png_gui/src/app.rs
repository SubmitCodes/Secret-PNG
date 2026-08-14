use crossbeam_channel::{unbounded, Receiver, Sender};
use egui::{
    vec2, Align, Button, Color32, ComboBox, Layout, Margin, RichText, Rounding, Stroke,
    TextureHandle, Ui,
};
use secret_png_core::{
    embed_files, extract_payload, inspect_carrier, strip_payload_to_file, EmbedOptions,
    EmbedReport, ExtractionReport, PayloadMetadata, ProgressUpdate, SanitizeReport, TrailerIndex,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTheme {
    CyberCyan,      // Default: Slate (#0B0F19), Navy Card (#161B22), Cyan (#38BDF8)
    MidnightViolet, // Violet: Deep Obsidian (#0F0E17), Card (#1A162B), Violet (#A855F7)
    EmeraldMatrix,  // Emerald: Pitch (#09140E), Card (#112419), Mint (#10B981)
    CrimsonRuby,    // Ruby: Obsidian (#140B0E), Card (#231218), Rose (#F43F5E)
    MonochromeDark, // Minimal: Charcoal (#121212), Card (#1E1E1E), Frost (#E2E8F0)
}

impl AppTheme {
    pub fn name(&self) -> &'static str {
        match self {
            AppTheme::CyberCyan => "Cyber Cyan (Default)",
            AppTheme::MidnightViolet => "Midnight Violet",
            AppTheme::EmeraldMatrix => "Emerald Matrix",
            AppTheme::CrimsonRuby => "Crimson Ruby",
            AppTheme::MonochromeDark => "Monochrome Dark",
        }
    }

    pub fn bg(&self) -> Color32 {
        match self {
            AppTheme::CyberCyan => Color32::from_rgb(11, 15, 25),
            AppTheme::MidnightViolet => Color32::from_rgb(15, 14, 23),
            AppTheme::EmeraldMatrix => Color32::from_rgb(9, 20, 14),
            AppTheme::CrimsonRuby => Color32::from_rgb(20, 11, 14),
            AppTheme::MonochromeDark => Color32::from_rgb(18, 18, 18),
        }
    }

    pub fn card_bg(&self) -> Color32 {
        match self {
            AppTheme::CyberCyan => Color32::from_rgb(22, 27, 34),
            AppTheme::MidnightViolet => Color32::from_rgb(26, 22, 43),
            AppTheme::EmeraldMatrix => Color32::from_rgb(17, 36, 25),
            AppTheme::CrimsonRuby => Color32::from_rgb(35, 18, 24),
            AppTheme::MonochromeDark => Color32::from_rgb(30, 30, 30),
        }
    }

    pub fn card_border(&self) -> Color32 {
        match self {
            AppTheme::CyberCyan => Color32::from_rgb(48, 54, 61),
            AppTheme::MidnightViolet => Color32::from_rgb(60, 48, 86),
            AppTheme::EmeraldMatrix => Color32::from_rgb(34, 64, 48),
            AppTheme::CrimsonRuby => Color32::from_rgb(70, 32, 42),
            AppTheme::MonochromeDark => Color32::from_rgb(55, 55, 55),
        }
    }

    pub fn accent(&self) -> Color32 {
        match self {
            AppTheme::CyberCyan => Color32::from_rgb(56, 189, 248),
            AppTheme::MidnightViolet => Color32::from_rgb(168, 85, 247),
            AppTheme::EmeraldMatrix => Color32::from_rgb(52, 211, 153),
            AppTheme::CrimsonRuby => Color32::from_rgb(244, 63, 94),
            AppTheme::MonochromeDark => Color32::from_rgb(226, 232, 240),
        }
    }

    pub fn primary_btn_fill(&self) -> Color32 {
        match self {
            AppTheme::CyberCyan => Color32::from_rgb(2, 132, 199),
            AppTheme::MidnightViolet => Color32::from_rgb(124, 58, 237),
            AppTheme::EmeraldMatrix => Color32::from_rgb(5, 150, 105),
            AppTheme::CrimsonRuby => Color32::from_rgb(225, 29, 72),
            AppTheme::MonochromeDark => Color32::from_rgb(51, 65, 85),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveTab {
    Embed,
    Extract,
    InspectSanitize,
}

enum WorkerMessage {
    Progress(ProgressUpdate),
    EmbedDone(Result<EmbedReport, String>),
    ExtractDone(Result<ExtractionReport, String>),
    SanitizeDone(Result<SanitizeReport, String>),
}

pub struct StowApp {
    active_tab: ActiveTab,
    theme: AppTheme,

    // App Branding Logo
    app_logo: Option<TextureHandle>,

    // Embed tab state
    embed_host_path: Option<PathBuf>,
    embed_payload_path: Option<PathBuf>,
    embed_output_path: Option<PathBuf>,
    embed_password: String,
    embed_enable_encryption: bool,
    embed_show_password: bool,

    // Extract tab state
    extract_carrier_path: Option<PathBuf>,
    extract_output_path: Option<PathBuf>,
    extract_password: String,
    extract_show_password: bool,
    extract_inspected_meta: Option<(TrailerIndex, PayloadMetadata)>,

    // Inspect/Sanitize state
    inspect_path: Option<PathBuf>,
    inspect_inspected_meta: Option<(TrailerIndex, PayloadMetadata)>,
    sanitize_output_path: Option<PathBuf>,

    // Image preview caches
    host_thumbnail: Option<TextureHandle>,
    carrier_thumbnail: Option<TextureHandle>,

    // Async Worker & Progress State
    worker_rx: Receiver<WorkerMessage>,
    worker_tx: Sender<WorkerMessage>,
    is_working: Arc<AtomicBool>,
    current_progress: Option<ProgressUpdate>,
    status_banner: Option<(String, bool)>, // (message, is_error)

    // Result reports
    last_embed_report: Option<EmbedReport>,
    last_extract_report: Option<ExtractionReport>,
    last_sanitize_report: Option<SanitizeReport>,
}

impl Default for StowApp {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            active_tab: ActiveTab::Embed,
            theme: AppTheme::CyberCyan,
            app_logo: None,
            embed_host_path: None,
            embed_payload_path: None,
            embed_output_path: None,
            embed_password: String::new(),
            embed_enable_encryption: false,
            embed_show_password: false,

            extract_carrier_path: None,
            extract_output_path: None,
            extract_password: String::new(),
            extract_show_password: false,
            extract_inspected_meta: None,

            inspect_path: None,
            inspect_inspected_meta: None,
            sanitize_output_path: None,

            host_thumbnail: None,
            carrier_thumbnail: None,

            worker_rx: rx,
            worker_tx: tx,
            is_working: Arc::new(AtomicBool::new(false)),
            current_progress: None,
            status_banner: None,

            last_embed_report: None,
            last_extract_report: None,
            last_sanitize_report: None,
        }
    }
}

impl StowApp {
    pub fn new(ctx: &egui::Context) -> Self {
        let mut app = Self::default();
        let logo_bytes = include_bytes!("../assets/logo.png");
        if let Ok(img) = image::load_from_memory(logo_bytes) {
            let rgba = img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let color_img = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            app.app_logo = Some(ctx.load_texture("stow_logo", color_img, Default::default()));
        }
        app
    }

    fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * KB;
        const GB: u64 = 1024 * MB;

        if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} bytes", bytes)
        }
    }

    fn format_speed(speed: f64) -> String {
        const KB: f64 = 1024.0;
        const MB: f64 = 1024.0 * KB;
        const GB: f64 = 1024.0 * MB;

        if speed >= GB {
            format!("{:.2} GB/s", speed / GB)
        } else if speed >= MB {
            format!("{:.2} MB/s", speed / MB)
        } else if speed >= KB {
            format!("{:.2} KB/s", speed / KB)
        } else {
            format!("{:.0} B/s", speed)
        }
    }

    fn load_thumbnail(ctx: &egui::Context, path: &Path, name: &str) -> Option<TextureHandle> {
        let img = image::open(path).ok()?;
        let thumb = img.thumbnail(240, 160);
        let rgba = thumb.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
        Some(ctx.load_texture(name, color_image, Default::default()))
    }

    fn poll_worker_messages(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.worker_rx.try_recv() {
            ctx.request_repaint();
            match msg {
                WorkerMessage::Progress(update) => {
                    self.current_progress = Some(update);
                }
                WorkerMessage::EmbedDone(res) => {
                    self.is_working.store(false, Ordering::SeqCst);
                    self.current_progress = None;
                    match res {
                        Ok(report) => {
                            self.status_banner = Some((
                                format!(
                                    "Successfully concealed '{}' into carrier image!",
                                    report.original_file_name
                                ),
                                false,
                            ));
                            self.last_embed_report = Some(report);
                        }
                        Err(e) => {
                            self.status_banner = Some((format!("Embedding Failed: {}", e), true));
                        }
                    }
                }
                WorkerMessage::ExtractDone(res) => {
                    self.is_working.store(false, Ordering::SeqCst);
                    self.current_progress = None;
                    match res {
                        Ok(report) => {
                            self.status_banner = Some((
                                format!(
                                    "Successfully extracted '{}' ({}) with verified integrity!",
                                    report.original_filename,
                                    Self::format_bytes(report.file_size)
                                ),
                                false,
                            ));
                            self.last_extract_report = Some(report);
                        }
                        Err(e) => {
                            self.status_banner = Some((format!("Extraction Failed: {}", e), true));
                        }
                    }
                }
                WorkerMessage::SanitizeDone(res) => {
                    self.is_working.store(false, Ordering::SeqCst);
                    self.current_progress = None;
                    match res {
                        Ok(report) => {
                            self.status_banner = Some((
                                format!(
                                    "Image cleaned! Removed {} of hidden payload.",
                                    Self::format_bytes(report.payload_bytes_removed)
                                ),
                                false,
                            ));
                            self.last_sanitize_report = Some(report);
                        }
                        Err(e) => {
                            self.status_banner = Some((format!("Cleaning Failed: {}", e), true));
                        }
                    }
                }
            }
        }
    }

    // --- UI Renderers ---

    fn render_header(&mut self, ui: &mut Ui) {
        let accent = self.theme.accent();

        ui.horizontal(|ui| {
            ui.add_space(6.0);

            // Render Stow Logo
            if let Some(ref logo) = self.app_logo {
                ui.image((logo.id(), vec2(28.0, 28.0)));
                ui.add_space(4.0);
            }

            ui.heading(
                RichText::new("STOW")
                    .size(23.0)
                    .color(accent)
                    .strong(),
            );
            ui.label(
                RichText::new("v1.0")
                    .size(12.0)
                    .color(Color32::from_rgb(148, 163, 184)),
            );

            // Theme selector on top right
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ComboBox::from_id_salt("theme_selector")
                    .selected_text(RichText::new(format!("🎨 {}", self.theme.name())).size(12.0).color(accent))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.theme, AppTheme::CyberCyan, "Cyber Cyan (Default)");
                        ui.selectable_value(&mut self.theme, AppTheme::MidnightViolet, "Midnight Violet");
                        ui.selectable_value(&mut self.theme, AppTheme::EmeraldMatrix, "Emerald Matrix");
                        ui.selectable_value(&mut self.theme, AppTheme::CrimsonRuby, "Crimson Ruby");
                        ui.selectable_value(&mut self.theme, AppTheme::MonochromeDark, "Monochrome Dark");
                    });
                ui.label(RichText::new("Theme:").size(12.0).color(Color32::from_rgb(148, 163, 184)));
            });
        });

        ui.add_space(10.0);

        // Tab Navigation Bar
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            let tabs = [
                (ActiveTab::Embed, "Embed File"),
                (ActiveTab::Extract, "Extract File"),
                (ActiveTab::InspectSanitize, "Inspect & Clean"),
            ];

            for (tab, label) in tabs {
                let is_active = self.active_tab == tab;
                let bg_color = if is_active {
                    Color32::from_rgb(30, 41, 59)
                } else {
                    self.theme.bg()
                };
                let text_color = if is_active {
                    accent
                } else {
                    Color32::from_rgb(148, 163, 184)
                };

                let btn = Button::new(RichText::new(label).size(14.0).color(text_color).strong())
                    .fill(bg_color)
                    .stroke(Stroke::new(
                        1.0_f32,
                        if is_active {
                            accent
                        } else {
                            self.theme.card_border()
                        },
                    ))
                    .min_size(vec2(130.0, 34.0))
                    .rounding(Rounding::same(6.0));

                if ui.add(btn).clicked() {
                    self.active_tab = tab;
                    self.status_banner = None;
                }
                ui.add_space(6.0);
            }
        });

        ui.add_space(10.0);
        ui.separator();
    }

    fn render_status_banner(&mut self, ui: &mut Ui) {
        if let Some((ref msg, is_err)) = self.status_banner {
            let (bg, border, text_color, icon) = if is_err {
                (
                    Color32::from_rgb(69, 10, 10),
                    Color32::from_rgb(239, 68, 68),
                    Color32::from_rgb(254, 202, 202),
                    "!",
                )
            } else {
                (
                    Color32::from_rgb(6, 78, 59),
                    Color32::from_rgb(16, 185, 129),
                    Color32::from_rgb(167, 243, 208),
                    "✓",
                )
            };

            egui::Frame::none()
                .fill(bg)
                .stroke(Stroke::new(1.0_f32, border))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(10.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(icon).size(16.0).strong());
                        ui.label(RichText::new(msg).color(text_color).strong().size(13.0));
                    });
                });
            ui.add_space(8.0);
        }
    }

    fn render_progress_card(&mut self, ui: &mut Ui) {
        if let Some(ref progress) = self.current_progress {
            let accent = self.theme.accent();

            egui::Frame::none()
                .fill(Color32::from_rgb(15, 23, 42))
                .stroke(Stroke::new(1.0_f32, accent))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(12.0))
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&progress.phase)
                                    .color(accent)
                                    .strong(),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(format!("{:.1}%", progress.percentage))
                                        .color(Color32::WHITE)
                                        .strong(),
                                );
                            });
                        });

                        ui.add_space(6.0);
                        let bar = egui::ProgressBar::new(progress.percentage / 100.0)
                            .fill(accent)
                            .animate(true);
                        ui.add(bar);

                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{} / {}",
                                    Self::format_bytes(progress.bytes_processed),
                                    Self::format_bytes(progress.total_bytes)
                                ))
                                .color(Color32::from_rgb(148, 163, 184))
                                .size(12.0),
                            );

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(format!("⚡ {}", Self::format_speed(progress.speed_bytes_sec)))
                                        .color(Color32::from_rgb(52, 211, 153))
                                        .size(12.0)
                                        .strong(),
                                );
                            });
                        });
                    });
                });
            ui.add_space(10.0);
        }
    }

    fn render_embed_tab(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let is_busy = self.is_working.load(Ordering::SeqCst);
        let accent = self.theme.accent();
        let card_bg = self.theme.card_bg();
        let card_border = self.theme.card_border();

        egui::ScrollArea::vertical().show(ui, |ui| {
            // 1. Host Cover Image Selector
            egui::Frame::none()
                .fill(card_bg)
                .stroke(Stroke::new(1.0_f32, card_border))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(14.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Cover Image")
                                .size(15.0)
                                .color(Color32::WHITE)
                                .strong(),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let browse_btn = Button::new(RichText::new("Browse Image...").size(13.0))
                                .min_size(vec2(150.0, 32.0))
                                .fill(Color32::from_rgb(30, 41, 59))
                                .stroke(Stroke::new(1.0_f32, card_border))
                                .rounding(Rounding::same(6.0));

                            if ui.add_enabled(!is_busy, browse_btn).clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Image Files", &["png", "jpg", "jpeg", "webp", "gif", "bmp"])
                                    .pick_file()
                                {
                                    self.embed_host_path = Some(path.clone());
                                    self.host_thumbnail = Self::load_thumbnail(ctx, &path, "host_thumb");
                                    if self.embed_output_path.is_none() {
                                        let parent = path.parent().unwrap_or_else(|| Path::new("."));
                                        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("carrier");
                                        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("png");
                                        self.embed_output_path = Some(parent.join(format!("{}_carrier.{}", stem, ext)));
                                    }
                                }
                            }
                        });
                    });

                    ui.add_space(6.0);
                    if let Some(ref path) = self.embed_host_path {
                        ui.horizontal(|ui| {
                            if let Some(ref thumb) = self.host_thumbnail {
                                ui.image(thumb);
                                ui.add_space(10.0);
                            }
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(path.file_name().unwrap_or_default().to_string_lossy())
                                        .color(accent)
                                        .strong(),
                                );
                                ui.label(
                                    RichText::new(format!("Path: {}", path.display()))
                                        .size(11.0)
                                        .color(Color32::from_rgb(148, 163, 184)),
                                );
                                if let Ok(meta) = std::fs::metadata(path) {
                                    ui.label(
                                        RichText::new(format!("Size: {}", Self::format_bytes(meta.len())))
                                            .size(12.0)
                                            .color(Color32::from_rgb(148, 163, 184)),
                                    );
                                }
                            });
                        });
                    } else {
                        ui.label(
                            RichText::new("Select any image (PNG, JPEG, WebP, GIF, BMP) to act as the visual cover.")
                                .color(Color32::from_rgb(148, 163, 184)),
                        );
                    }
                });

            ui.add_space(10.0);

            // 2. Secret File to Conceal
            egui::Frame::none()
                .fill(card_bg)
                .stroke(Stroke::new(1.0_f32, card_border))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(14.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Secret File to Conceal")
                                .size(15.0)
                                .color(Color32::WHITE)
                                .strong(),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let browse_btn = Button::new(RichText::new("Browse File...").size(13.0))
                                .min_size(vec2(150.0, 32.0))
                                .fill(Color32::from_rgb(30, 41, 59))
                                .stroke(Stroke::new(1.0_f32, card_border))
                                .rounding(Rounding::same(6.0));

                            if ui.add_enabled(!is_busy, browse_btn).clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter(
                                        "All Files (*.*)",
                                        &["*"],
                                    )
                                    .pick_file()
                                {
                                    self.embed_payload_path = Some(path);
                                }
                            }
                        });
                    });

                    ui.add_space(6.0);
                    if let Some(ref path) = self.embed_payload_path {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(path.file_name().unwrap_or_default().to_string_lossy())
                                    .color(Color32::from_rgb(129, 140, 248))
                                    .strong(),
                            );
                            ui.label(
                                RichText::new(format!("Path: {}", path.display()))
                                    .size(11.0)
                                    .color(Color32::from_rgb(148, 163, 184)),
                            );
                            if let Ok(meta) = std::fs::metadata(path) {
                                ui.label(
                                    RichText::new(format!("Size: {}", Self::format_bytes(meta.len())))
                                        .size(12.0)
                                        .color(Color32::from_rgb(148, 163, 184)),
                                );
                            }
                        });
                    } else {
                        ui.label(
                            RichText::new("Select any file to conceal (videos, archives, documents, data, any size).")
                                .color(Color32::from_rgb(148, 163, 184)),
                        );
                    }
                });

            ui.add_space(10.0);

            // 3. Security & Destination Options
            egui::Frame::none()
                .fill(card_bg)
                .stroke(Stroke::new(1.0_f32, card_border))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(14.0))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("Options & Protection")
                            .size(15.0)
                            .color(Color32::WHITE)
                            .strong(),
                    );
                    ui.add_space(8.0);

                    // Output file path
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Save Output As:").strong());
                        if let Some(ref p) = self.embed_output_path {
                            ui.label(
                                RichText::new(p.display().to_string())
                                    .color(accent)
                                    .size(12.0),
                            );
                        } else {
                            ui.label(RichText::new("Not set").color(Color32::GRAY).italics());
                        }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let save_btn = Button::new(RichText::new("Choose Location...").size(13.0))
                                .min_size(vec2(150.0, 32.0))
                                .fill(Color32::from_rgb(30, 41, 59))
                                .stroke(Stroke::new(1.0_f32, card_border))
                                .rounding(Rounding::same(6.0));
                            if ui.add(save_btn).clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Carrier Image", &["png", "jpg", "jpeg", "webp"])
                                    .save_file()
                                {
                                    self.embed_output_path = Some(path);
                                }
                            }
                        });
                    });

                    ui.add_space(8.0);
                    ui.checkbox(
                        &mut self.embed_enable_encryption,
                        RichText::new("Protect with a Password").strong(),
                    );

                    if self.embed_enable_encryption {
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label("Password:");
                            let edit = egui::TextEdit::singleline(&mut self.embed_password)
                                .password(!self.embed_show_password)
                                .desired_width(240.0);
                            ui.add(edit);
                            ui.checkbox(&mut self.embed_show_password, "Show");
                        });
                    }
                });

            ui.add_space(14.0);

            // 4. Action Button
            let can_embed = !is_busy
                && self.embed_host_path.is_some()
                && self.embed_payload_path.is_some()
                && self.embed_output_path.is_some()
                && (!self.embed_enable_encryption || !self.embed_password.is_empty());

            let btn_text = if is_busy {
                "Processing Carrier..."
            } else {
                "Embed File into Image"
            };

            let embed_btn = Button::new(
                RichText::new(btn_text)
                    .size(16.0)
                    .color(if can_embed { Color32::WHITE } else { Color32::GRAY })
                    .strong(),
            )
            .fill(if can_embed {
                self.theme.primary_btn_fill()
            } else {
                Color32::from_rgb(30, 41, 59)
            })
            .stroke(Stroke::new(
                1.0_f32,
                if can_embed {
                    accent
                } else {
                    card_border
                },
            ))
            .min_size(vec2(ui.available_width(), 48.0))
            .rounding(Rounding::same(8.0));

            if ui.add_enabled(can_embed, embed_btn).clicked() {
                self.start_embedding();
            }

            // 5. Result Summary Card
            if let Some(ref report) = self.last_embed_report {
                ui.add_space(12.0);
                egui::Frame::none()
                    .fill(Color32::from_rgb(13, 27, 42))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(16, 185, 129)))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::same(14.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Operation Complete")
                                .size(15.0)
                                .color(Color32::from_rgb(52, 211, 153))
                                .strong(),
                        );
                        ui.add_space(4.0);
                        ui.label(format!("• Carrier Output: {}", self.embed_output_path.as_ref().unwrap().display()));
                        ui.label(format!("• Cover Image Size: {}", Self::format_bytes(report.host_image_size)));
                        ui.label(format!("• Hidden File Size: {}", Self::format_bytes(report.payload_size)));
                        ui.label(format!("• Total File Size: {}", Self::format_bytes(report.total_carrier_size)));
                        ui.label(format!("• Checksum (BLAKE3): {}", report.blake3_hex));
                        ui.label(format!("• Time Elapsed: {:.2}s", report.elapsed_millis as f64 / 1000.0));
                    });
            }
        });
    }

    fn start_embedding(&mut self) {
        let host = self.embed_host_path.clone().unwrap();
        let payload = self.embed_payload_path.clone().unwrap();
        let output = self.embed_output_path.clone().unwrap();
        let password = if self.embed_enable_encryption {
            Some(self.embed_password.clone())
        } else {
            None
        };

        self.is_working.store(true, Ordering::SeqCst);
        self.status_banner = None;
        self.last_embed_report = None;

        let tx = self.worker_tx.clone();
        let progress_cb = Box::new(move |up: ProgressUpdate| {
            let _ = tx.send(WorkerMessage::Progress(up));
        });

        let tx_done = self.worker_tx.clone();
        thread::spawn(move || {
            let res = embed_files(
                host,
                payload,
                output,
                EmbedOptions { password },
                Some(progress_cb),
            )
            .map_err(|e| e.to_string());
            let _ = tx_done.send(WorkerMessage::EmbedDone(res));
        });
    }

    fn render_extract_tab(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let is_busy = self.is_working.load(Ordering::SeqCst);
        let accent = self.theme.accent();
        let card_bg = self.theme.card_bg();
        let card_border = self.theme.card_border();

        egui::ScrollArea::vertical().show(ui, |ui| {
            // 1. Carrier Image Selector
            egui::Frame::none()
                .fill(card_bg)
                .stroke(Stroke::new(1.0_f32, card_border))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(14.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Carrier Image")
                                .size(15.0)
                                .color(Color32::WHITE)
                                .strong(),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let browse_btn = Button::new(RichText::new("Browse Image...").size(13.0))
                                .min_size(vec2(150.0, 32.0))
                                .fill(Color32::from_rgb(30, 41, 59))
                                .stroke(Stroke::new(1.0_f32, card_border))
                                .rounding(Rounding::same(6.0));

                            if ui.add_enabled(!is_busy, browse_btn).clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif", "bmp"])
                                    .pick_file()
                                {
                                    self.extract_carrier_path = Some(path.clone());
                                    self.carrier_thumbnail = Self::load_thumbnail(ctx, &path, "carrier_thumb");

                                    match inspect_carrier(&path) {
                                        Ok(meta) => {
                                            self.extract_inspected_meta = Some(meta.clone());
                                            let parent = path.parent().unwrap_or_else(|| Path::new("."));
                                            self.extract_output_path = Some(parent.join(&meta.1.original_filename));
                                            self.status_banner = Some((
                                                format!(
                                                    "Carrier payload found! Concealed file: '{}' ({})",
                                                    meta.1.original_filename,
                                                    Self::format_bytes(meta.1.original_file_size)
                                                ),
                                                false,
                                            ));
                                        }
                                        Err(e) => {
                                            self.extract_inspected_meta = None;
                                            self.status_banner = Some((format!("Inspection: {}", e), true));
                                        }
                                    }
                                }
                            }
                        });
                    });

                    ui.add_space(6.0);
                    if let Some(ref path) = self.extract_carrier_path {
                        ui.horizontal(|ui| {
                            if let Some(ref thumb) = self.carrier_thumbnail {
                                ui.image(thumb);
                                ui.add_space(10.0);
                            }
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(path.file_name().unwrap_or_default().to_string_lossy())
                                        .color(accent)
                                        .strong(),
                                );
                                ui.label(
                                    RichText::new(format!("Path: {}", path.display()))
                                        .size(11.0)
                                        .color(Color32::from_rgb(148, 163, 184)),
                                );
                                if let Ok(meta) = std::fs::metadata(path) {
                                    ui.label(
                                        RichText::new(format!("Size: {}", Self::format_bytes(meta.len())))
                                            .size(12.0)
                                            .color(Color32::from_rgb(148, 163, 184)),
                                    );
                                }
                            });
                        });
                    } else {
                        ui.label(
                            RichText::new("Choose an image file that contains an embedded file.")
                                .color(Color32::from_rgb(148, 163, 184)),
                        );
                    }
                });

            ui.add_space(10.0);

            // 2. Detected Payload Info
            if let Some((ref _trailer, ref meta)) = self.extract_inspected_meta {
                egui::Frame::none()
                    .fill(Color32::from_rgb(15, 23, 42))
                    .stroke(Stroke::new(1.0_f32, accent))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::same(14.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Detected Concealed File")
                                .size(15.0)
                                .color(accent)
                                .strong(),
                        );
                        ui.add_space(4.0);
                        ui.label(format!("• Original Filename: {}", meta.original_filename));
                        ui.label(format!("• File Size: {}", Self::format_bytes(meta.original_file_size)));
                        ui.label(format!(
                            "• Protection: {}",
                            if meta.is_encrypted {
                                "Password Protected"
                            } else {
                                "Unencrypted"
                            }
                        ));
                    });

                ui.add_space(10.0);

                // Password input if encrypted
                if meta.is_encrypted {
                    egui::Frame::none()
                        .fill(card_bg)
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(129, 140, 248)))
                        .rounding(Rounding::same(8.0))
                        .inner_margin(Margin::same(14.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Password Required")
                                    .color(Color32::from_rgb(199, 210, 254))
                                    .strong(),
                            );
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.label("Password:");
                                let edit = egui::TextEdit::singleline(&mut self.extract_password)
                                    .password(!self.extract_show_password)
                                    .desired_width(240.0);
                                ui.add(edit);
                                ui.checkbox(&mut self.extract_show_password, "Show");
                            });
                        });
                    ui.add_space(10.0);
                }

                // Output file selector
                egui::Frame::none()
                    .fill(card_bg)
                    .stroke(Stroke::new(1.0_f32, card_border))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::same(14.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Save Extracted File As:").strong());
                            if let Some(ref p) = self.extract_output_path {
                                ui.label(
                                    RichText::new(p.display().to_string())
                                        .color(accent)
                                        .size(12.0),
                                );
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let change_btn = Button::new(RichText::new("Choose Location...").size(13.0))
                                    .min_size(vec2(150.0, 32.0))
                                    .fill(Color32::from_rgb(30, 41, 59))
                                    .stroke(Stroke::new(1.0_f32, card_border))
                                    .rounding(Rounding::same(6.0));
                                if ui.add(change_btn).clicked() {
                                    if let Some(path) = rfd::FileDialog::new()
                                        .set_file_name(&meta.original_filename)
                                        .save_file()
                                    {
                                        self.extract_output_path = Some(path);
                                    }
                                }
                            });
                        });
                    });

                ui.add_space(14.0);

                // Extract Button
                let is_encrypted = meta.is_encrypted;
                let can_extract = !is_busy
                    && self.extract_carrier_path.is_some()
                    && self.extract_output_path.is_some()
                    && (!is_encrypted || !self.extract_password.is_empty());

                let btn_text = if is_busy {
                    "Extracting File..."
                } else {
                    "Extract File from Image"
                };

                let extract_btn = Button::new(
                    RichText::new(btn_text)
                        .size(16.0)
                        .color(if can_extract { Color32::WHITE } else { Color32::GRAY })
                        .strong(),
                )
                .fill(if can_extract {
                    Color32::from_rgb(5, 150, 105)
                } else {
                    Color32::from_rgb(51, 65, 85)
                })
                .stroke(Stroke::new(
                    1.0_f32,
                    if can_extract {
                        Color32::from_rgb(52, 211, 153)
                    } else {
                        card_border
                    },
                ))
                .min_size(vec2(ui.available_width(), 48.0))
                .rounding(Rounding::same(8.0));

                if ui.add_enabled(can_extract, extract_btn).clicked() {
                    self.start_extraction();
                }
            }

            // Result summary
            if let Some(ref report) = self.last_extract_report {
                ui.add_space(12.0);
                egui::Frame::none()
                    .fill(Color32::from_rgb(13, 27, 42))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(16, 185, 129)))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::same(14.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Extraction Complete")
                                .size(15.0)
                                .color(Color32::from_rgb(52, 211, 153))
                                .strong(),
                        );
                        ui.add_space(4.0);
                        ui.label(format!("• Output: {}", report.output_path.display()));
                        ui.label(format!("• Size: {}", Self::format_bytes(report.file_size)));
                        ui.label(format!("• Time Elapsed: {:.2}s", report.elapsed_millis as f64 / 1000.0));
                    });
            }
        });
    }

    fn start_extraction(&mut self) {
        let carrier = self.extract_carrier_path.clone().unwrap();
        let output = self.extract_output_path.clone();
        let is_enc = self.extract_inspected_meta.as_ref().map(|m| m.1.is_encrypted).unwrap_or(false);
        let password = if is_enc {
            Some(self.extract_password.clone())
        } else {
            None
        };

        self.is_working.store(true, Ordering::SeqCst);
        self.status_banner = None;
        self.last_extract_report = None;

        let tx = self.worker_tx.clone();
        let progress_cb = Box::new(move |up: ProgressUpdate| {
            let _ = tx.send(WorkerMessage::Progress(up));
        });

        let tx_done = self.worker_tx.clone();
        thread::spawn(move || {
            let res = extract_payload(
                carrier,
                output.as_ref(),
                password.as_deref(),
                Some(progress_cb),
            )
            .map_err(|e| e.to_string());
            let _ = tx_done.send(WorkerMessage::ExtractDone(res));
        });
    }

    fn render_inspect_sanitize_tab(&mut self, ui: &mut Ui, _ctx: &egui::Context) {
        let is_busy = self.is_working.load(Ordering::SeqCst);
        let card_bg = self.theme.card_bg();
        let card_border = self.theme.card_border();

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Frame::none()
                .fill(card_bg)
                .stroke(Stroke::new(1.0_f32, card_border))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(14.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Inspect & Clean Image")
                                .size(15.0)
                                .color(Color32::WHITE)
                                .strong(),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let select_btn = Button::new(RichText::new("Select Image...").size(13.0))
                                .min_size(vec2(150.0, 32.0))
                                .fill(Color32::from_rgb(30, 41, 59))
                                .stroke(Stroke::new(1.0_f32, card_border))
                                .rounding(Rounding::same(6.0));

                            if ui.add_enabled(!is_busy, select_btn).clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif", "bmp"])
                                    .pick_file()
                                {
                                    self.inspect_path = Some(path.clone());
                                    match inspect_carrier(&path) {
                                        Ok(meta) => {
                                            self.inspect_inspected_meta = Some(meta);
                                            let parent = path.parent().unwrap_or_else(|| Path::new("."));
                                            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("clean");
                                            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("png");
                                            self.sanitize_output_path = Some(parent.join(format!("{}_clean.{}", stem, ext)));
                                        }
                                        Err(e) => {
                                            self.inspect_inspected_meta = None;
                                            self.status_banner = Some((format!("No hidden file found: {}", e), false));
                                        }
                                    }
                                }
                            }
                        });
                    });

                    if let Some(ref path) = self.inspect_path {
                        ui.add_space(6.0);
                        ui.label(format!("Inspecting: {}", path.display()));
                    }
                });

            ui.add_space(10.0);

            if let Some((ref _trailer, ref meta)) = self.inspect_inspected_meta {
                egui::Frame::none()
                    .fill(Color32::from_rgb(15, 23, 42))
                    .stroke(Stroke::new(1.0_f32, self.theme.accent()))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::same(14.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Carrier File Details")
                                .size(15.0)
                                .color(self.theme.accent())
                                .strong(),
                        );
                        ui.add_space(4.0);
                        ui.label(format!("• Original File: {}", meta.original_filename));
                        ui.label(format!("• File Size: {}", Self::format_bytes(meta.original_file_size)));
                        ui.label(format!("• Format: {}", meta.host_image_format));
                    });

                ui.add_space(10.0);

                egui::Frame::none()
                    .fill(card_bg)
                    .stroke(Stroke::new(1.0_f32, card_border))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::same(14.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Remove Concealed File")
                                .size(15.0)
                                .color(Color32::WHITE)
                                .strong(),
                        );
                        ui.add_space(6.0);
                        ui.label("This will strip the hidden file, restoring the original untouched cover image.");
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Clean Output File:").strong());
                            if let Some(ref p) = self.sanitize_output_path {
                                ui.label(RichText::new(p.display().to_string()).color(self.theme.accent()));
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let choose_btn = Button::new(RichText::new("Choose Location...").size(13.0))
                                    .min_size(vec2(150.0, 32.0))
                                    .fill(Color32::from_rgb(30, 41, 59))
                                    .stroke(Stroke::new(1.0_f32, card_border))
                                    .rounding(Rounding::same(6.0));
                                if ui.add(choose_btn).clicked() {
                                    if let Some(path) = rfd::FileDialog::new().save_file() {
                                        self.sanitize_output_path = Some(path);
                                    }
                                }
                            });
                        });

                        ui.add_space(12.0);
                        let can_sanitize = !is_busy && self.inspect_path.is_some() && self.sanitize_output_path.is_some();
                        let sanitize_btn = Button::new(
                            RichText::new("Remove Hidden File")
                                .size(15.0)
                                .color(if can_sanitize { Color32::WHITE } else { Color32::GRAY })
                                .strong(),
                        )
                        .fill(if can_sanitize {
                            Color32::from_rgb(225, 29, 72)
                        } else {
                            Color32::from_rgb(51, 65, 85)
                        })
                        .stroke(Stroke::new(
                            1.0_f32,
                            if can_sanitize {
                                Color32::from_rgb(251, 113, 133)
                            } else {
                                card_border
                            },
                        ))
                        .min_size(vec2(ui.available_width(), 44.0))
                        .rounding(Rounding::same(8.0));

                        if ui.add_enabled(can_sanitize, sanitize_btn).clicked() {
                            self.start_sanitizing();
                        }
                    });
            }
        });
    }

    fn start_sanitizing(&mut self) {
        let carrier = self.inspect_path.clone().unwrap();
        let output = self.sanitize_output_path.clone().unwrap();

        self.is_working.store(true, Ordering::SeqCst);
        self.status_banner = None;
        self.last_sanitize_report = None;

        let tx_done = self.worker_tx.clone();
        thread::spawn(move || {
            let res = strip_payload_to_file(carrier, output).map_err(|e| e.to_string());
            let _ = tx_done.send(WorkerMessage::SanitizeDone(res));
        });
    }
}

impl eframe::App for StowApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker_messages(ctx);

        let mut style = (*ctx.style()).clone();
        style.visuals.dark_mode = true;
        style.visuals.override_text_color = Some(Color32::from_rgb(241, 245, 249));
        style.visuals.panel_fill = self.theme.bg();
        ctx.set_style(style);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(4.0);
            self.render_header(ui);
            self.render_status_banner(ui);
            self.render_progress_card(ui);

            match self.active_tab {
                ActiveTab::Embed => self.render_embed_tab(ui, ctx),
                ActiveTab::Extract => self.render_extract_tab(ui, ctx),
                ActiveTab::InspectSanitize => self.render_inspect_sanitize_tab(ui, ctx),
            }
        });
    }
}
