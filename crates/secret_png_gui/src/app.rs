use crossbeam_channel::{unbounded, Receiver, Sender};
use egui::{
    vec2, Align, Button, Color32, Layout, Margin, RichText, Rounding, Stroke,
    TextureHandle, Ui,
};
use image::GenericImageView;
use secret_png_core::{
    embed_files, extract_payload, inspect_carrier, strip_payload_to_file, EmbedOptions,
    EmbedReport, ExtractionReport, PayloadMetadata, ProgressUpdate, SanitizeReport, TrailerIndex,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

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

pub struct SecretPngApp {
    active_tab: ActiveTab,

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

impl Default for SecretPngApp {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            active_tab: ActiveTab::Embed,
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

impl SecretPngApp {
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
        let (_w, _h) = img.dimensions();
        // Resize to maximum 240x160 thumbnail to save GPU memory
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
                                    "Successfully embedded '{}' into carrier image!",
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
                                    "Image sanitized! Removed {} of hidden payload.",
                                    Self::format_bytes(report.payload_bytes_removed)
                                ),
                                false,
                            ));
                            self.last_sanitize_report = Some(report);
                        }
                        Err(e) => {
                            self.status_banner = Some((format!("Sanitizing Failed: {}", e), true));
                        }
                    }
                }
            }
        }
    }

    // --- UI Renderers ---

    fn render_header(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.heading(
                RichText::new("🛡️ SECRET PNG")
                    .size(24.0)
                    .color(Color32::from_rgb(56, 189, 248))
                    .strong(),
            );
            ui.label(
                RichText::new("Carrier Engine v1.0")
                    .size(13.0)
                    .color(Color32::from_rgb(148, 163, 184)),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("Buffered Streaming • ChaCha20-Poly1305 • BLAKE3")
                        .size(11.0)
                        .color(Color32::from_rgb(100, 116, 139)),
                );
            });
        });

        ui.add_space(10.0);

        // Tab Navigation Bar
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let tabs = [
                (ActiveTab::Embed, "📦 Embed Video into Image"),
                (ActiveTab::Extract, "🔓 Extract Video from Image"),
                (ActiveTab::InspectSanitize, "🔍 Inspect & Sanitize"),
            ];

            for (tab, label) in tabs {
                let is_active = self.active_tab == tab;
                let bg_color = if is_active {
                    Color32::from_rgb(30, 41, 59)
                } else {
                    Color32::from_rgb(15, 23, 42)
                };
                let text_color = if is_active {
                    Color32::from_rgb(56, 189, 248)
                } else {
                    Color32::from_rgb(148, 163, 184)
                };

                let btn = Button::new(RichText::new(label).size(14.0).color(text_color).strong())
                    .fill(bg_color)
                    .stroke(Stroke::new(
                        1.0_f32,
                        if is_active {
                            Color32::from_rgb(56, 189, 248)
                        } else {
                            Color32::from_rgb(51, 65, 85)
                        },
                    ))
                    .rounding(Rounding::same(6.0));

                if ui.add(btn).clicked() {
                    self.active_tab = tab;
                    self.status_banner = None;
                }
                ui.add_space(6.0);
            }
        });

        ui.add_space(12.0);
        ui.separator();
    }

    fn render_status_banner(&mut self, ui: &mut Ui) {
        if let Some((ref msg, is_err)) = self.status_banner {
            let (bg, border, text_color, icon) = if is_err {
                (
                    Color32::from_rgb(69, 10, 10),
                    Color32::from_rgb(239, 68, 68),
                    Color32::from_rgb(254, 202, 202),
                    "⚠️",
                )
            } else {
                (
                    Color32::from_rgb(6, 78, 59),
                    Color32::from_rgb(16, 185, 129),
                    Color32::from_rgb(167, 243, 208),
                    "✅",
                )
            };

            egui::Frame::none()
                .fill(bg)
                .stroke(Stroke::new(1.0_f32, border))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(10.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(icon).size(16.0));
                        ui.label(RichText::new(msg).color(text_color).strong().size(13.0));
                    });
                });
            ui.add_space(8.0);
        }
    }

    fn render_progress_card(&mut self, ui: &mut Ui) {
        if let Some(ref progress) = self.current_progress {
            egui::Frame::none()
                .fill(Color32::from_rgb(15, 23, 42))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248)))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(12.0))
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&progress.phase)
                                    .color(Color32::from_rgb(56, 189, 248))
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
                            .fill(Color32::from_rgb(56, 189, 248))
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

        egui::ScrollArea::vertical().show(ui, |ui| {
            // 1. Host Cover Image Selector
            egui::Frame::none()
                .fill(Color32::from_rgb(22, 27, 34))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(12.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("🖼️ Host Cover Image")
                                .size(15.0)
                                .color(Color32::WHITE)
                                .strong(),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add_enabled(!is_busy, Button::new("Browse Image..."))
                                .clicked()
                            {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Image Files", &["png", "jpg", "jpeg", "webp", "gif", "bmp"])
                                    .pick_file()
                                {
                                    self.embed_host_path = Some(path.clone());
                                    self.host_thumbnail = Self::load_thumbnail(ctx, &path, "host_thumb");
                                    // Auto configure default output carrier path
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
                                        .color(Color32::from_rgb(56, 189, 248))
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
                            RichText::new("Select any PNG, JPEG, WebP, GIF or BMP image to act as the visual cover.")
                                .color(Color32::from_rgb(100, 116, 139))
                                .italics(),
                        );
                    }
                });

            ui.add_space(10.0);

            // 2. Secret Video Payload Selector
            egui::Frame::none()
                .fill(Color32::from_rgb(22, 27, 34))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(12.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("🎬 Secret Video Payload")
                                .size(15.0)
                                .color(Color32::WHITE)
                                .strong(),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add_enabled(!is_busy, Button::new("Browse Video / Media..."))
                                .clicked()
                            {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter(
                                        "Video / Media Files",
                                        &["mp4", "mkv", "mov", "webm", "avi", "flv", "wmv", "ts", "zip", "bin", "*"],
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
                            RichText::new("Select any video file (e.g. MP4, MKV, MOV, WebM, multi-GB 4K videos).")
                                .color(Color32::from_rgb(100, 116, 139))
                                .italics(),
                        );
                    }
                });

            ui.add_space(10.0);

            // 3. Security & Destination Options
            egui::Frame::none()
                .fill(Color32::from_rgb(22, 27, 34))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(12.0))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("🔐 Security & Output Options")
                            .size(15.0)
                            .color(Color32::WHITE)
                            .strong(),
                    );
                    ui.add_space(6.0);

                    // Output file path
                    ui.horizontal(|ui| {
                        ui.label("Carrier Output:");
                        if let Some(ref p) = self.embed_output_path {
                            ui.label(
                                RichText::new(p.display().to_string())
                                    .color(Color32::from_rgb(56, 189, 248))
                                    .size(12.0),
                            );
                        } else {
                            ui.label(RichText::new("Not set").color(Color32::GRAY).italics());
                        }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Choose Save Location...").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Carrier Image", &["png", "jpg", "jpeg", "webp"])
                                    .save_file()
                                {
                                    self.embed_output_path = Some(path);
                                }
                            }
                        });
                    });

                    ui.add_space(6.0);
                    ui.checkbox(
                        &mut self.embed_enable_encryption,
                        "Enable ChaCha20-Poly1305 + Argon2id Password Encryption",
                    );

                    if self.embed_enable_encryption {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label("Password:");
                            let edit = egui::TextEdit::singleline(&mut self.embed_password)
                                .password(!self.embed_show_password)
                                .desired_width(220.0);
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
                "⏳ Processing Streaming Carrier..."
            } else {
                "🚀 Embed & Conceal Video into Image"
            };

            let embed_btn = Button::new(
                RichText::new(btn_text)
                    .size(16.0)
                    .color(if can_embed { Color32::BLACK } else { Color32::GRAY })
                    .strong(),
            )
            .fill(if can_embed {
                Color32::from_rgb(56, 189, 248)
            } else {
                Color32::from_rgb(51, 65, 85)
            })
            .min_size(vec2(ui.available_width(), 44.0))
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
                    .inner_margin(Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("🎉 Embedding Report")
                                .size(15.0)
                                .color(Color32::from_rgb(52, 211, 153))
                                .strong(),
                        );
                        ui.add_space(4.0);
                        ui.label(format!("• Carrier Output: {}", self.embed_output_path.as_ref().unwrap().display()));
                        ui.label(format!("• Host Cover Size: {}", Self::format_bytes(report.host_image_size)));
                        ui.label(format!("• Video Payload Size: {}", Self::format_bytes(report.payload_size)));
                        ui.label(format!("• Total Carrier Size: {}", Self::format_bytes(report.total_carrier_size)));
                        ui.label(format!("• BLAKE3 Checksum: {}", report.blake3_hex));
                        ui.label(format!("• CRC32: 0x{:08X}", report.crc32));
                        ui.label(format!("• Elapsed Time: {:.2}s", report.elapsed_millis as f64 / 1000.0));
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

        egui::ScrollArea::vertical().show(ui, |ui| {
            // 1. Carrier Image Selector
            egui::Frame::none()
                .fill(Color32::from_rgb(22, 27, 34))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(12.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("🖼️ Carrier Image to Extract From")
                                .size(15.0)
                                .color(Color32::WHITE)
                                .strong(),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add_enabled(!is_busy, Button::new("Browse Carrier Image..."))
                                .clicked()
                            {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif", "bmp"])
                                    .pick_file()
                                {
                                    self.extract_carrier_path = Some(path.clone());
                                    self.carrier_thumbnail = Self::load_thumbnail(ctx, &path, "carrier_thumb");

                                    // Instant O(1) metadata inspection
                                    match inspect_carrier(&path) {
                                        Ok(meta) => {
                                            self.extract_inspected_meta = Some(meta.clone());
                                            let parent = path.parent().unwrap_or_else(|| Path::new("."));
                                            self.extract_output_path = Some(parent.join(&meta.1.original_filename));
                                            self.status_banner = Some((
                                                format!(
                                                    "Carrier payload found! Embedded file: '{}' ({})",
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
                                        .color(Color32::from_rgb(56, 189, 248))
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
                            RichText::new("Choose a carrier image file containing an embedded video.")
                                .color(Color32::from_rgb(100, 116, 139))
                                .italics(),
                        );
                    }
                });

            ui.add_space(10.0);

            // 2. Detected Payload Info
            if let Some((ref trailer, ref meta)) = self.extract_inspected_meta {
                egui::Frame::none()
                    .fill(Color32::from_rgb(15, 23, 42))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248)))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("📦 Detected Embedded Payload")
                                .size(15.0)
                                .color(Color32::from_rgb(56, 189, 248))
                                .strong(),
                        );
                        ui.add_space(4.0);
                        ui.label(format!("• Original Filename: {}", meta.original_filename));
                        ui.label(format!("• Format / MIME: .{} ({})", meta.file_extension, meta.mime_type));
                        ui.label(format!("• Video File Size: {}", Self::format_bytes(meta.original_file_size)));
                        ui.label(format!("• Host Image Size: {}", Self::format_bytes(trailer.host_image_size)));
                        ui.label(format!(
                            "• Encryption: {}",
                            if meta.is_encrypted {
                                "🔒 Password Protected (ChaCha20-Poly1305)"
                            } else {
                                "🔓 Unencrypted Raw Stream"
                            }
                        ));
                        ui.label(format!("• BLAKE3 Checksum: {}", meta.blake3_hex));
                        ui.label(format!("• CRC32: 0x{:08X}", meta.crc32));
                    });

                ui.add_space(10.0);

                // Password input if encrypted
                if meta.is_encrypted {
                    egui::Frame::none()
                        .fill(Color32::from_rgb(30, 27, 75))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(129, 140, 248)))
                        .rounding(Rounding::same(8.0))
                        .inner_margin(Margin::same(12.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("🔐 Decryption Required")
                                    .color(Color32::from_rgb(199, 210, 254))
                                    .strong(),
                            );
                            ui.add_space(4.0);
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
                    .fill(Color32::from_rgb(22, 27, 34))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Save Extracted Video As:");
                            if let Some(ref p) = self.extract_output_path {
                                ui.label(
                                    RichText::new(p.display().to_string())
                                        .color(Color32::from_rgb(56, 189, 248))
                                        .size(12.0),
                            );
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button("Change Output Location...").clicked() {
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
                    "⏳ Extracting & Verifying Checksum..."
                } else {
                    "🔓 Extract Video Payload to Standalone File"
                };

                let extract_btn = Button::new(
                    RichText::new(btn_text)
                        .size(16.0)
                        .color(if can_extract { Color32::BLACK } else { Color32::GRAY })
                        .strong(),
                )
                .fill(if can_extract {
                    Color32::from_rgb(52, 211, 153)
                } else {
                    Color32::from_rgb(51, 65, 85)
                })
                .min_size(vec2(ui.available_width(), 44.0))
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
                    .inner_margin(Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("🎉 Video Extracted & Verified!")
                                .size(15.0)
                                .color(Color32::from_rgb(52, 211, 153))
                                .strong(),
                        );
                        ui.add_space(4.0);
                        ui.label(format!("• Output Path: {}", report.output_path.display()));
                        ui.label(format!("• Original Filename: {}", report.original_filename));
                        ui.label(format!("• Payload Size: {}", Self::format_bytes(report.file_size)));
                        ui.label(format!("• Verified BLAKE3: {}", report.blake3_hex));
                        ui.label(format!("• Elapsed Time: {:.2}s", report.elapsed_millis as f64 / 1000.0));
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

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Frame::none()
                .fill(Color32::from_rgb(22, 27, 34))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(12.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("🔍 Inspect & Sanitize Image")
                                .size(15.0)
                                .color(Color32::WHITE)
                                .strong(),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add_enabled(!is_busy, Button::new("Select Image...")).clicked() {
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
                                            self.sanitize_output_path = Some(parent.join(format!("{}_sanitized.{}", stem, ext)));
                                        }
                                        Err(e) => {
                                            self.inspect_inspected_meta = None;
                                            self.status_banner = Some((format!("No carrier payload detected: {}", e), false));
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

            if let Some((ref trailer, ref meta)) = self.inspect_inspected_meta {
                egui::Frame::none()
                    .fill(Color32::from_rgb(15, 23, 42))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248)))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("📊 Carrier Geometry & Details")
                                .size(15.0)
                                .color(Color32::from_rgb(56, 189, 248))
                                .strong(),
                        );
                        ui.add_space(6.0);
                        ui.label(format!("• Protocol Version: v{}", meta.protocol_version));
                        ui.label(format!("• Original File: {}", meta.original_filename));
                        ui.label(format!("• Video Size: {}", Self::format_bytes(meta.original_file_size)));
                        ui.label(format!("• Host Image Size: {}", Self::format_bytes(trailer.host_image_size)));
                        ui.label(format!("• Payload Stream Size: {}", Self::format_bytes(trailer.payload_length)));
                        ui.label(format!("• Metadata Block Size: {} bytes", trailer.metadata_length));
                        ui.label(format!("• Fixed Trailer Size: 64 bytes"));
                        ui.label(format!("• Host Format: {}", meta.host_image_format));
                    });

                ui.add_space(10.0);

                egui::Frame::none()
                    .fill(Color32::from_rgb(22, 27, 34))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("🧹 Sanitize Image (Remove Payload)")
                                .size(15.0)
                                .color(Color32::WHITE)
                                .strong(),
                        );
                        ui.add_space(6.0);
                        ui.label("This will strip the embedded payload, restoring the exact pristine original cover image.");
                        ui.add_space(6.0);

                        ui.horizontal(|ui| {
                            ui.label("Clean Output File:");
                            if let Some(ref p) = self.sanitize_output_path {
                                ui.label(RichText::new(p.display().to_string()).color(Color32::from_rgb(56, 189, 248)));
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button("Choose Location...").clicked() {
                                    if let Some(path) = rfd::FileDialog::new().save_file() {
                                        self.sanitize_output_path = Some(path);
                                    }
                                }
                            });
                        });

                        ui.add_space(10.0);
                        let can_sanitize = !is_busy && self.inspect_path.is_some() && self.sanitize_output_path.is_some();
                        let sanitize_btn = Button::new(
                            RichText::new("🧹 Clean & Sanitize Image")
                                .size(15.0)
                                .color(if can_sanitize { Color32::BLACK } else { Color32::GRAY })
                                .strong(),
                        )
                        .fill(if can_sanitize {
                            Color32::from_rgb(248, 113, 113)
                        } else {
                            Color32::from_rgb(51, 65, 85)
                        })
                        .min_size(vec2(ui.available_width(), 38.0))
                        .rounding(Rounding::same(6.0));

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

impl eframe::App for SecretPngApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker_messages(ctx);

        // Dark modern style
        let mut style = (*ctx.style()).clone();
        style.visuals.dark_mode = true;
        style.visuals.override_text_color = Some(Color32::from_rgb(241, 245, 249));
        style.visuals.panel_fill = Color32::from_rgb(13, 17, 23);
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
