use egui::{
    vec2, Color32, FontFamily, FontId, Pos2, Rect, RichText, Rounding, Sense, Stroke, Ui,
};

use crate::bridge::{DeviceBridge, GuiCommand};
use crate::theme::{self, card_frame, sunken_frame};
use hsm200_driver::{Button, LedMode, RgbColor};

pub struct LightingView {
    pub lcd_brightness: u8,
    pub button_modes: [LedMode; 14],
    pub meter_top: [RgbColor; 4],
    pub meter_mid: [RgbColor; 4],
    pub meter_bot: [RgbColor; 4],
    pub test_vu_level: u8,
}

impl Default for LightingView {
    fn default() -> Self {
        Self {
            lcd_brightness: 100,
            button_modes: [LedMode::Solid; 14],
            meter_top: [RgbColor::new(0xFF, 0x00, 0x80); 4],     // Neon pink
            meter_mid: [RgbColor::new(0x9D, 0x00, 0xFF); 4],     // Electric violet
            meter_bot: [RgbColor::new(0x2D, 0x12, 0x36); 4],     // Deep plum
            test_vu_level: 180,
        }
    }
}

impl LightingView {
    pub fn ui(&mut self, ui: &mut Ui, bridge: &DeviceBridge) {
        ui.vertical(|ui| {
            // Header Bar
            ui.horizontal(|ui| {
                theme::section_header(
                    ui,
                    "LIGHTING & HARDWARE LED STUDIO",
                    "Configure button backlights (0x0601), LCD brightness, and 3-point RGB meter gradients (0x0501)",
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(
                            RichText::new("⚡ SYNC ALL LEDS")
                                .color(Color32::WHITE)
                                .font(FontId::new(12.0, FontFamily::Proportional))
                                .strong(),
                        )
                        .clicked()
                    {
                        bridge.send(GuiCommand::SyncRgbMeters);
                        bridge.send(GuiCommand::SetLcdBrightness(self.lcd_brightness));
                    }
                });
            });

            ui.add_space(10.0);

            ui.horizontal_top(|ui| {
                // Left Column: Physical Button Backlights
                ui.vertical(|ui| {
                    self.render_button_leds(ui, bridge);
                });

                ui.add_space(12.0);

                // Right Column: LCD Backlight + 4-Channel RGB VU Meters
                ui.vertical(|ui| {
                    self.render_lcd_backlight_card(ui, bridge);
                    ui.add_space(10.0);
                    self.render_rgb_meters_card(ui, bridge);
                });
            });
        });
    }

    fn render_button_leds(&mut self, ui: &mut Ui, bridge: &DeviceBridge) {
        card_frame(false).show(ui, |ui| {
            ui.set_width(450.0);
            ui.label(
                RichText::new("PHYSICAL BUTTON BACKLIGHTS")
                    .color(theme::COLOR_PRIMARY)
                    .font(FontId::new(14.0, FontFamily::Proportional))
                    .strong(),
            );
            ui.add_space(4.0);

            // Master buttons row
            ui.horizontal(|ui| {
                if ui.button("ALL SOLID PINK").clicked() {
                    for m in &mut self.button_modes {
                        *m = LedMode::Solid;
                    }
                    bridge.send(GuiCommand::SetAllButtonsLed(LedMode::Solid));
                }
                if ui.button("ALL BLINK").clicked() {
                    for m in &mut self.button_modes {
                        *m = LedMode::Blink;
                    }
                    bridge.send(GuiCommand::SetAllButtonsLed(LedMode::Blink));
                }
                if ui.button("ALL OFF").clicked() {
                    for m in &mut self.button_modes {
                        *m = LedMode::Off;
                    }
                    bridge.send(GuiCommand::SetAllButtonsLed(LedMode::Off));
                }
            });

            ui.add_space(10.0);

            // 4 Lower Action Buttons (Action 1..4)
            ui.group(|ui| {
                ui.label(
                    RichText::new("LOWER 4 ACTION BUTTONS (MUTE / TRIGGER)")
                        .color(theme::COLOR_ACCENT_LAVENDER)
                        .font(FontId::new(11.0, FontFamily::Proportional))
                        .strong(),
                );
                ui.add_space(4.0);

                let action_btns = [
                    (Button::Action1, 0, "ACTION 1"),
                    (Button::Action2, 1, "ACTION 2"),
                    (Button::Action3, 2, "ACTION 3"),
                    (Button::Action4, 3, "ACTION 4"),
                ];

                ui.horizontal(|ui| {
                    for (btn, idx, name) in action_btns {
                        self.render_button_pill(ui, btn, idx, name, bridge);
                    }
                });
            });

            ui.add_space(8.0);

            // 4 Encoder Push Buttons (Enc 1..4)
            ui.group(|ui| {
                ui.label(
                    RichText::new("ROTARY ENCODER PUSH BUTTONS")
                        .color(theme::COLOR_ACCENT_LAVENDER)
                        .font(FontId::new(11.0, FontFamily::Proportional))
                        .strong(),
                );
                ui.add_space(4.0);

                let enc_btns = [
                    (Button::Encoder1Push, 4, "ENC 1"),
                    (Button::Encoder2Push, 5, "ENC 2"),
                    (Button::Encoder3Push, 6, "ENC 3"),
                    (Button::Encoder4Push, 7, "ENC 4"),
                ];

                ui.horizontal(|ui| {
                    for (btn, idx, name) in enc_btns {
                        self.render_button_pill(ui, btn, idx, name, bridge);
                    }
                });
            });

            ui.add_space(8.0);

            // 6 Control Buttons (Ctrl A..F)
            ui.group(|ui| {
                ui.label(
                    RichText::new("CONTROL / SYSTEM BUTTONS (A - F)")
                        .color(theme::COLOR_ACCENT_LAVENDER)
                        .font(FontId::new(11.0, FontFamily::Proportional))
                        .strong(),
                );
                ui.add_space(4.0);

                let ctrl_btns = [
                    (Button::ControlA, 8, "CTRL A"),
                    (Button::ControlB, 9, "CTRL B"),
                    (Button::ControlC, 10, "CTRL C"),
                    (Button::ControlD, 11, "CTRL D"),
                    (Button::ControlE, 12, "CTRL E"),
                    (Button::ControlF, 13, "CTRL F"),
                ];

                ui.horizontal_wrapped(|ui| {
                    for (btn, idx, name) in ctrl_btns {
                        self.render_button_pill(ui, btn, idx, name, bridge);
                    }
                });
            });
        });
    }

    fn render_button_pill(
        &mut self,
        ui: &mut Ui,
        btn: Button,
        idx: usize,
        name: &str,
        bridge: &DeviceBridge,
    ) {
        let current_mode = self.button_modes[idx];
        let (fill, stroke, text_color) = match current_mode {
            LedMode::Solid => (
                theme::COLOR_PRIMARY,
                Stroke::new(1.0_f32, theme::COLOR_PRIMARY_HOVER),
                Color32::WHITE,
            ),
            LedMode::Blink => (
                theme::COLOR_ACCENT_PURPLE,
                Stroke::new(1.0_f32, Color32::WHITE),
                Color32::WHITE,
            ),
            LedMode::Off => (
                theme::COLOR_BG_SUNKEN,
                Stroke::new(1.0_f32, theme::COLOR_BORDER_SUBTLE),
                theme::COLOR_TEXT_MUTED,
            ),
        };

        let btn_ui = egui::Button::new(
            RichText::new(name)
                .color(text_color)
                .font(FontId::new(10.0, FontFamily::Proportional))
                .strong(),
        )
        .fill(fill)
        .stroke(stroke)
        .rounding(Rounding::same(6.0));

        if ui.add(btn_ui).clicked() {
            let next_mode = match current_mode {
                LedMode::Off => LedMode::Solid,
                LedMode::Solid => LedMode::Blink,
                LedMode::Blink => LedMode::Off,
            };
            self.button_modes[idx] = next_mode;
            bridge.send(GuiCommand::SetButtonLed {
                button: btn,
                mode: next_mode,
            });
        }
    }

    fn render_lcd_backlight_card(&mut self, ui: &mut Ui, bridge: &DeviceBridge) {
        card_frame(false).show(ui, |ui| {
            ui.set_width(420.0);
            ui.label(
                RichText::new("LCD SCREEN BACKLIGHT")
                    .color(theme::COLOR_PRIMARY)
                    .font(FontId::new(13.0, FontFamily::Proportional))
                    .strong(),
            );
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.label("BRIGHTNESS:");
                let mut b = self.lcd_brightness;
                if ui
                    .add(egui::Slider::new(&mut b, 0..=100).suffix("%"))
                    .changed()
                {
                    self.lcd_brightness = b;
                    bridge.send(GuiCommand::SetLcdBrightness(b));
                }
            });
        });
    }

    fn render_rgb_meters_card(&mut self, ui: &mut Ui, bridge: &DeviceBridge) {
        card_frame(true).show(ui, |ui| {
            ui.set_width(420.0);
            ui.label(
                RichText::new("3-POINT RGB VU METERS (CHANNELS 1 - 4)")
                    .color(theme::COLOR_PRIMARY)
                    .font(FontId::new(14.0, FontFamily::Proportional))
                    .strong(),
            );
            ui.add_space(4.0);

            // Palette presets
            ui.horizontal(|ui| {
                if ui.button("NEON PINK / VIOLET").clicked() {
                    for i in 0..4 {
                        self.meter_top[i] = RgbColor::new(0xFF, 0x00, 0x80);
                        self.meter_mid[i] = RgbColor::new(0x9D, 0x00, 0xFF);
                        self.meter_bot[i] = RgbColor::new(0x2D, 0x12, 0x36);
                    }
                    bridge.send(GuiCommand::SyncRgbMeters);
                }
                if ui.button("SYNTHWAVE").clicked() {
                    for i in 0..4 {
                        self.meter_top[i] = RgbColor::new(0xFF, 0x00, 0x55);
                        self.meter_mid[i] = RgbColor::new(0xFF, 0x77, 0x00);
                        self.meter_bot[i] = RgbColor::new(0x00, 0xE6, 0x99);
                    }
                    bridge.send(GuiCommand::SyncRgbMeters);
                }
                if ui.button("STUDIO CLASSIC").clicked() {
                    for i in 0..4 {
                        self.meter_top[i] = RgbColor::new(0xFF, 0x22, 0x22);
                        self.meter_mid[i] = RgbColor::new(0xFF, 0xCC, 0x00);
                        self.meter_bot[i] = RgbColor::new(0x00, 0xCC, 0x44);
                    }
                    bridge.send(GuiCommand::SyncRgbMeters);
                }
            });

            ui.add_space(10.0);

            // Live visual meters preview
            sunken_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    for ch in 0..4 {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new(format!("CH {}", ch + 1))
                                    .color(theme::COLOR_TEXT_SECONDARY)
                                    .font(FontId::new(10.0, FontFamily::Proportional))
                                    .strong(),
                            );

                            let (rect, _) = ui.allocate_exact_size(vec2(28.0, 100.0), Sense::hover());
                            let top_c = Color32::from_rgb(
                                self.meter_top[ch].r,
                                self.meter_top[ch].g,
                                self.meter_top[ch].b,
                            );
                            let mid_c = Color32::from_rgb(
                                self.meter_mid[ch].r,
                                self.meter_mid[ch].g,
                                self.meter_mid[ch].b,
                            );
                            let bot_c = Color32::from_rgb(
                                self.meter_bot[ch].r,
                                self.meter_bot[ch].g,
                                self.meter_bot[ch].b,
                            );

                            // Segmented meter preview
                            let seg_h = rect.height() / 3.0;
                            let r_top = Rect::from_min_size(rect.min, vec2(rect.width(), seg_h));
                            let r_mid = Rect::from_min_size(Pos2::new(rect.min.x, rect.min.y + seg_h), vec2(rect.width(), seg_h));
                            let r_bot = Rect::from_min_size(Pos2::new(rect.min.x, rect.min.y + seg_h * 2.0), vec2(rect.width(), seg_h));

                            ui.painter().rect_filled(r_top, Rounding::same(2.0), top_c);
                            ui.painter().rect_filled(r_mid, Rounding::same(2.0), mid_c);
                            ui.painter().rect_filled(r_bot, Rounding::same(2.0), bot_c);
                        });
                        ui.add_space(12.0);
                    }
                });
            });

            ui.add_space(8.0);

            // VU Test Level Slider
            ui.horizontal(|ui| {
                ui.label("VU TEST LEVEL:");
                let mut lvl = self.test_vu_level;
                if ui.add(egui::Slider::new(&mut lvl, 0..=255)).changed() {
                    self.test_vu_level = lvl;
                    bridge.send(GuiCommand::SyncRgbMeters);
                }
            });
        });
    }
}
