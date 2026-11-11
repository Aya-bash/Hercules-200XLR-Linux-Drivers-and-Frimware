use std::path::PathBuf;

use egui::{
    vec2, Align2, Color32, FontFamily, FontId, Pos2, Rect, RichText, Rounding, Sense, Stroke,
    Ui,
};
use image::imageops::FilterType;

use crate::bridge::{DeviceBridge, GuiCommand};
use crate::theme::{self, card_frame};
use hsm200_driver::{
    display, make_test_pattern_rgb565_frame, rgb888_to_rgb565, RgbColor, LCD_HEIGHT, LCD_WIDTH,
};

pub struct DisplayView {
    pub active_page: u8,
    pub target_slot: u32,
    pub selected_image_path: Option<PathBuf>,
    pub preview_solid_color: [u8; 3],
    pub preview_mode: PreviewMode,
    pub status_message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewMode {
    SolidColor,
    TestPattern,
    CustomImage,
}

impl Default for DisplayView {
    fn default() -> Self {
        Self {
            active_page: 1,
            target_slot: display::LIVE_BACKGROUND_SLOT_DEFAULT, // 6
            selected_image_path: None,
            preview_solid_color: [0xFF, 0x00, 0x80], // #FF0080
            preview_mode: PreviewMode::SolidColor,
            status_message: None,
        }
    }
}

impl DisplayView {
    pub fn ui(&mut self, ui: &mut Ui, bridge: &DeviceBridge) {
        ui.vertical(|ui| {
            // Header Bar
            ui.horizontal(|ui| {
                theme::section_header(
                    ui,
                    "480×272 LCD DISPLAY STUDIO",
                    "Manage live mixer wallpaper (0x0801), standby screen, boot splash, and active page switching",
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🌙 PUT LCD TO SLEEP").clicked() {
                        bridge.send(GuiCommand::SetScreenPower(false));
                        self.status_message = Some("Sent Screen Sleep command (0x0003)".to_string());
                    }
                    if ui.button("☀️ WAKE LCD SCREEN").clicked() {
                        bridge.send(GuiCommand::SetScreenPower(true));
                        self.status_message = Some("Sent Screen Wake command (0x0001)".to_string());
                    }
                });
            });

            ui.add_space(10.0);

            ui.horizontal_top(|ui| {
                // Left Column: Virtual Screen Simulator Preview
                ui.vertical(|ui| {
                    self.render_screen_preview(ui, bridge);
                });

                ui.add_space(12.0);

                // Right Column: Wallpaper & Image Tools
                ui.vertical(|ui| {
                    self.render_page_switcher_card(ui, bridge);
                    ui.add_space(10.0);
                    self.render_wallpaper_uploader_card(ui, bridge);
                });
            });

            if let Some(msg) = &self.status_message {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("INFO: {msg}"))
                        .color(theme::COLOR_PRIMARY)
                        .font(FontId::new(11.0, FontFamily::Proportional)),
                );
            }
        });
    }

    fn render_screen_preview(&mut self, ui: &mut Ui, _bridge: &DeviceBridge) {
        card_frame(true).show(ui, |ui| {
            ui.set_width(500.0);
            ui.label(
                RichText::new("VIRTUAL LCD SCREEN CANVAS (480 × 272)")
                    .color(theme::COLOR_PRIMARY)
                    .font(FontId::new(14.0, FontFamily::Proportional))
                    .strong(),
            );
            ui.add_space(6.0);

            // Bezel frame around the LCD
            let bezel_frame = egui::Frame::group(&egui::Style::default())
                .fill(Color32::from_rgb(0x0C, 0x0A, 0x14))
                .stroke(Stroke::new(2.0_f32, theme::COLOR_PRIMARY))
                .rounding(Rounding::same(12.0))
                .inner_margin(egui::Margin::same(10.0));

            bezel_frame.show(ui, |ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(480.0, 272.0), Sense::hover());

                // Draw background according to preview mode
                match self.preview_mode {
                    PreviewMode::SolidColor => {
                        let c = Color32::from_rgb(
                            self.preview_solid_color[0],
                            self.preview_solid_color[1],
                            self.preview_solid_color[2],
                        );
                        ui.painter().rect_filled(rect, Rounding::same(4.0), c);
                    }
                    PreviewMode::TestPattern => {
                        // Draw 8 color bars
                        let bars = [
                            Color32::WHITE,
                            Color32::from_rgb(255, 255, 0),
                            Color32::from_rgb(0, 255, 255),
                            Color32::from_rgb(0, 255, 0),
                            Color32::from_rgb(255, 0, 255),
                            Color32::from_rgb(255, 0, 0),
                            Color32::from_rgb(0, 0, 255),
                            Color32::BLACK,
                        ];
                        let bar_w = rect.width() / 8.0;
                        for (i, bar_c) in bars.iter().enumerate() {
                            let bar_rect = Rect::from_min_size(
                                Pos2::new(rect.min.x + (i as f32) * bar_w, rect.min.y),
                                vec2(bar_w, rect.height() * 0.75),
                            );
                            ui.painter().rect_filled(bar_rect, Rounding::ZERO, *bar_c);
                        }
                        // Bottom grayscale ramp
                        let ramp_rect = Rect::from_min_max(
                            Pos2::new(rect.min.x, rect.min.y + rect.height() * 0.75),
                            rect.max,
                        );
                        ui.painter().rect_filled(ramp_rect, Rounding::ZERO, Color32::from_gray(64));
                    }
                    PreviewMode::CustomImage => {
                        ui.painter().rect_filled(
                            rect,
                            Rounding::same(4.0),
                            theme::COLOR_BG_SUNKEN,
                        );
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            "CUSTOM IMAGE LOADED",
                            FontId::new(16.0, FontFamily::Proportional),
                            theme::COLOR_PRIMARY,
                        );
                    }
                }

                // Grid overlay showing the 4 mixer channel zones on LCD
                let ch_w = rect.width() / 4.0;
                for i in 1..4 {
                    let line_x = rect.min.x + (i as f32) * ch_w;
                    ui.painter().line_segment(
                        [Pos2::new(line_x, rect.min.y), Pos2::new(line_x, rect.max.y)],
                        Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(0xFF, 0xFF, 0xFF, 0x40)),
                    );
                }

                // Channel labels overlay
                for i in 0..4 {
                    let ch_center_x = rect.min.x + (i as f32 + 0.5) * ch_w;
                    ui.painter().text(
                        Pos2::new(ch_center_x, rect.min.y + 16.0),
                        Align2::CENTER_CENTER,
                        format!("CH {}", i + 1),
                        FontId::new(11.0, FontFamily::Proportional),
                        Color32::WHITE,
                    );
                }
            });

            ui.add_space(6.0);
            ui.label(
                RichText::new("Note: Displays live wallpaper behind mixer UI. Hardware aspect ratio: 16:9 (480×272 pixels).")
                    .color(theme::COLOR_TEXT_MUTED)
                    .font(FontId::new(10.0, FontFamily::Proportional)),
            );
        });
    }

    fn render_page_switcher_card(&mut self, ui: &mut Ui, bridge: &DeviceBridge) {
        card_frame(false).show(ui, |ui| {
            ui.set_width(380.0);
            ui.label(
                RichText::new("PAGE & LAYOUT SWITCHER")
                    .color(theme::COLOR_PRIMARY)
                    .font(FontId::new(13.0, FontFamily::Proportional))
                    .strong(),
            );
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                let p1_active = self.active_page == 1;
                let btn_p1 = egui::Button::new(
                    RichText::new("📄 PAGE 1 (CH 1 - 4)")
                        .color(if p1_active { Color32::WHITE } else { theme::COLOR_TEXT_SECONDARY })
                        .strong(),
                )
                .fill(if p1_active { theme::COLOR_PRIMARY } else { theme::COLOR_BG_CARD })
                .stroke(Stroke::new(1.0_f32, if p1_active { theme::COLOR_PRIMARY } else { theme::COLOR_BORDER_SUBTLE }))
                .rounding(Rounding::same(6.0));

                if ui.add(btn_p1).clicked() {
                    self.active_page = 1;
                    bridge.send(GuiCommand::SetScreenPage(1));
                    self.status_message = Some("Switched to Page 1".to_string());
                }

                let p2_active = self.active_page == 2;
                let btn_p2 = egui::Button::new(
                    RichText::new("📄 PAGE 2 (AUX / MACRO)")
                        .color(if p2_active { Color32::WHITE } else { theme::COLOR_TEXT_SECONDARY })
                        .strong(),
                )
                .fill(if p2_active { theme::COLOR_PRIMARY } else { theme::COLOR_BG_CARD })
                .stroke(Stroke::new(1.0_f32, if p2_active { theme::COLOR_PRIMARY } else { theme::COLOR_BORDER_SUBTLE }))
                .rounding(Rounding::same(6.0));

                if ui.add(btn_p2).clicked() {
                    self.active_page = 2;
                    bridge.send(GuiCommand::SetScreenPage(2));
                    self.status_message = Some("Switched to Page 2".to_string());
                }
            });
        });
    }

    fn render_wallpaper_uploader_card(&mut self, ui: &mut Ui, bridge: &DeviceBridge) {
        card_frame(false).show(ui, |ui| {
            ui.set_width(380.0);
            ui.label(
                RichText::new("WALLPAPER & IMAGE UPLOADER")
                    .color(theme::COLOR_PRIMARY)
                    .font(FontId::new(14.0, FontFamily::Proportional))
                    .strong(),
            );
            ui.add_space(6.0);

            // Target slot selector
            ui.horizontal(|ui| {
                ui.label("TARGET SLOT:");
                ui.selectable_value(&mut self.target_slot, 6, "SLOT 6 (LIVE MIXER)");
                ui.selectable_value(&mut self.target_slot, 0, "SLOT 0 (STANDBY)");
            });

            ui.add_space(8.0);

            // Solid Color Fill Tools
            ui.group(|ui| {
                ui.label(
                    RichText::new("SOLID COLOR FILL")
                        .color(theme::COLOR_ACCENT_LAVENDER)
                        .font(FontId::new(11.0, FontFamily::Proportional))
                        .strong(),
                );
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    if ui.button("NEON PINK (#FF0080)").clicked() {
                        self.preview_solid_color = [0xFF, 0x00, 0x80];
                        self.preview_mode = PreviewMode::SolidColor;
                        bridge.send(GuiCommand::FillScreenBackground {
                            color: RgbColor::new(0xFF, 0x00, 0x80),
                            slot: self.target_slot,
                        });
                        self.status_message = Some(format!("Uploaded solid #FF0080 to slot {}", self.target_slot));
                    }
                    if ui.button("DEEP VIOLET").clicked() {
                        self.preview_solid_color = [0x3D, 0x10, 0x38];
                        self.preview_mode = PreviewMode::SolidColor;
                        bridge.send(GuiCommand::FillScreenBackground {
                            color: RgbColor::new(0x3D, 0x10, 0x38),
                            slot: self.target_slot,
                        });
                        self.status_message = Some(format!("Uploaded solid #3D1038 to slot {}", self.target_slot));
                    }
                    if ui.button("CYBER BLUE").clicked() {
                        self.preview_solid_color = [0x0A, 0x19, 0x2F];
                        self.preview_mode = PreviewMode::SolidColor;
                        bridge.send(GuiCommand::FillScreenBackground {
                            color: RgbColor::new(0x0A, 0x19, 0x2F),
                            slot: self.target_slot,
                        });
                        self.status_message = Some(format!("Uploaded solid #0A192F to slot {}", self.target_slot));
                    }
                });

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.color_edit_button_srgb(&mut self.preview_solid_color);
                    if ui.button("UPLOAD CUSTOM COLOR").clicked() {
                        self.preview_mode = PreviewMode::SolidColor;
                        let c = RgbColor::new(
                            self.preview_solid_color[0],
                            self.preview_solid_color[1],
                            self.preview_solid_color[2],
                        );
                        bridge.send(GuiCommand::FillScreenBackground {
                            color: c,
                            slot: self.target_slot,
                        });
                        self.status_message = Some(format!(
                            "Uploaded solid #{:02X}{:02X}{:02X} to slot {}",
                            c.r, c.g, c.b, self.target_slot
                        ));
                    }
                });
            });

            ui.add_space(8.0);

            // Test Pattern & Image Upload
            ui.group(|ui| {
                ui.label(
                    RichText::new("PATTERN & IMAGE FILES")
                        .color(theme::COLOR_ACCENT_LAVENDER)
                        .font(FontId::new(11.0, FontFamily::Proportional))
                        .strong(),
                );
                ui.add_space(4.0);

                if ui.button("🎨 UPLOAD COLOR-BARS TEST PATTERN").clicked() {
                    self.preview_mode = PreviewMode::TestPattern;
                    let frame = make_test_pattern_rgb565_frame();
                    bridge.send(GuiCommand::UploadImage {
                        rgb565_le: frame,
                        slot: self.target_slot,
                        splash: false,
                    });
                    self.status_message = Some(format!("Uploaded test pattern to slot {}", self.target_slot));
                }

                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    if ui.button("📁 BROWSE IMAGE (PNG/JPG/BMP)...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Images", &["png", "jpg", "jpeg", "bmp"])
                            .pick_file()
                        {
                            self.selected_image_path = Some(path.clone());
                            self.preview_mode = PreviewMode::CustomImage;

                            // Load, resize, convert and upload
                            if let Ok(img) = image::open(&path) {
                                let resized = img.resize_exact(
                                    LCD_WIDTH as u32,
                                    LCD_HEIGHT as u32,
                                    FilterType::Lanczos3,
                                );
                                let rgb8 = resized.to_rgb8();
                                let mut rgb565_le = Vec::with_capacity((LCD_WIDTH as usize) * (LCD_HEIGHT as usize) * 2);
                                for pixel in rgb8.pixels() {
                                    let p565 = rgb888_to_rgb565(pixel[0], pixel[1], pixel[2]);
                                    rgb565_le.extend_from_slice(&p565.to_le_bytes());
                                }

                                bridge.send(GuiCommand::UploadImage {
                                    rgb565_le,
                                    slot: self.target_slot,
                                    splash: false,
                                });
                                self.status_message = Some(format!(
                                    "Uploaded '{}' to slot {}",
                                    path.file_name().unwrap_or_default().to_string_lossy(),
                                    self.target_slot
                                ));
                            }
                        }
                    }

                    if let Some(path) = &self.selected_image_path {
                        ui.label(
                            RichText::new(path.file_name().unwrap_or_default().to_string_lossy())
                                .color(theme::COLOR_TEXT_PRIMARY)
                                .font(FontId::new(10.0, FontFamily::Proportional)),
                        );
                    }
                });

                ui.add_space(4.0);
                if ui.button("🚀 UPLOAD AS BOOT SPLASH (0x0801 SUB_CMD=0x05)").clicked() {
                    let frame = if let Some(path) = &self.selected_image_path {
                        if let Ok(img) = image::open(path) {
                            let resized = img.resize_exact(
                                LCD_WIDTH as u32,
                                LCD_HEIGHT as u32,
                                FilterType::Lanczos3,
                            );
                            let rgb8 = resized.to_rgb8();
                            let mut buf = Vec::with_capacity((LCD_WIDTH as usize) * (LCD_HEIGHT as usize) * 2);
                            for pixel in rgb8.pixels() {
                                let p565 = rgb888_to_rgb565(pixel[0], pixel[1], pixel[2]);
                                buf.extend_from_slice(&p565.to_le_bytes());
                            }
                            buf
                        } else {
                            make_test_pattern_rgb565_frame()
                        }
                    } else {
                        make_test_pattern_rgb565_frame()
                    };

                    bridge.send(GuiCommand::UploadImage {
                        rgb565_le: frame,
                        slot: 0,
                        splash: true,
                    });
                    self.status_message = Some("Uploaded raw 16-bit RGB565 boot splash image (0x05)".to_string());
                }
            });
        });
    }
}
