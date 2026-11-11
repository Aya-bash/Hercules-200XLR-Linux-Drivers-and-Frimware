use std::collections::VecDeque;
use std::time::Instant;

use egui::{
    vec2, Align2, Color32, FontFamily, FontId, Pos2, RichText, Rounding, Sense, Stroke, Ui,
};

use crate::theme::{self, card_frame, sunken_frame};
use hsm200_driver::{DeviceInfo, HardwareEvent, InputReport};

pub struct MonitorView {
    pub start_time: Instant,
    pub event_log: VecDeque<(f32, String)>,
    pub max_log_entries: usize,
}

impl Default for MonitorView {
    fn default() -> Self {
        Self {
            start_time: Instant::now(),
            event_log: VecDeque::with_capacity(200),
            max_log_entries: 100,
        }
    }
}

impl MonitorView {
    pub fn push_event(&mut self, ev: HardwareEvent) {
        let elapsed = self.start_time.elapsed().as_secs_f32();
        let desc = match ev {
            HardwareEvent::ButtonPressed(b) => format!("BUTTON PRESSED  : {}", b.name()),
            HardwareEvent::ButtonReleased(b) => format!("BUTTON RELEASED : {}", b.name()),
            HardwareEvent::EncoderTurned {
                encoder,
                delta,
                counter,
            } => {
                let dir = if delta > 0 { "CW (+)" } else { "CCW (-)" };
                format!("ENCODER {encoder} TURNED : {dir:<7} delta={delta:+} (counter={counter})")
            }
            HardwareEvent::AuxEncoderChanged {
                old_value,
                new_value,
            } => format!("AUX ENCODER     : {old_value} -> {new_value}"),
            HardwareEvent::HwFlagsChanged {
                old_flags,
                new_flags,
            } => format!("HW FLAGS        : 0x{old_flags:02X} -> 0x{new_flags:02X}"),
        };

        if self.event_log.len() >= self.max_log_entries {
            self.event_log.pop_front();
        }
        self.event_log.push_back((elapsed, desc));
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        report_opt: Option<&InputReport>,
        dev_info_opt: Option<&DeviceInfo>,
    ) {
        ui.vertical(|ui| {
            // Header Bar
            ui.horizontal(|ui| {
                theme::section_header(
                    ui,
                    "HARDWARE EVENT MONITOR & TELEMETRY",
                    "Live USB report inspector, action button bits, rotary encoder counters, and diagnostic event feed",
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🗑️ CLEAR LOG").clicked() {
                        self.event_log.clear();
                    }
                });
            });

            ui.add_space(10.0);

            ui.horizontal_top(|ui| {
                // Left Column: Button Bitmask Indicators + Encoder Counters + Device Telemetry
                ui.vertical(|ui| {
                    self.render_action_buttons_card(ui, report_opt);
                    ui.add_space(10.0);
                    self.render_encoders_card(ui, report_opt);
                    ui.add_space(10.0);
                    self.render_device_telemetry_card(ui, dev_info_opt);
                });

                ui.add_space(12.0);

                // Right Column: Live Event Stream Feed
                ui.vertical(|ui| {
                    self.render_event_log_card(ui);
                });
            });
        });
    }

    fn render_action_buttons_card(&self, ui: &mut Ui, report_opt: Option<&InputReport>) {
        card_frame(false).show(ui, |ui| {
            ui.set_width(400.0);
            ui.label(
                RichText::new("LOWER 4 ACTION BUTTON BITMASKS")
                    .color(theme::COLOR_PRIMARY)
                    .font(FontId::new(13.0, FontFamily::Proportional))
                    .strong(),
            );
            ui.add_space(6.0);

            let (b1, b2, b3, b4) = match report_opt {
                Some(r) => (
                    (r.raw_buttons[0] & 0x01) != 0,
                    (r.raw_buttons[0] & 0x02) != 0,
                    (r.raw_buttons[0] & 0x04) != 0,
                    (r.raw_buttons[0] & 0x08) != 0,
                ),
                None => (false, false, false, false),
            };

            ui.horizontal(|ui| {
                render_bit_box(ui, "ACTION 1", "0001 (1)", b1);
                render_bit_box(ui, "ACTION 2", "0010 (2)", b2);
                render_bit_box(ui, "ACTION 3", "0100 (4)", b3);
                render_bit_box(ui, "ACTION 4", "1000 (8)", b4);
            });

            ui.add_space(6.0);
            if let Some(r) = report_opt {
                ui.label(
                    RichText::new(format!(
                        "Raw Bytes: [{:02X} {:02X} {:02X} {:02X}] | Nibble: {:04b}",
                        r.raw_buttons[0],
                        r.raw_buttons[1],
                        r.raw_buttons[2],
                        r.raw_buttons[3],
                        r.action_buttons_nibble()
                    ))
                    .color(theme::COLOR_TEXT_SECONDARY)
                    .font(FontId::new(10.0, FontFamily::Monospace)),
                );
            }
        });
    }

    fn render_encoders_card(&self, ui: &mut Ui, report_opt: Option<&InputReport>) {
        card_frame(false).show(ui, |ui| {
            ui.set_width(400.0);
            ui.label(
                RichText::new("ROTARY ENCODER COUNTERS")
                    .color(theme::COLOR_PRIMARY)
                    .font(FontId::new(13.0, FontFamily::Proportional))
                    .strong(),
            );
            ui.add_space(6.0);

            let (encs, aux) = match report_opt {
                Some(r) => (r.encoders, r.aux_encoder),
                None => ([0, 0, 0, 0], 0),
            };

            ui.horizontal(|ui| {
                for i in 0..4 {
                    sunken_frame().show(ui, |ui| {
                        ui.set_width(70.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new(format!("ENC {}", i + 1))
                                    .color(theme::COLOR_TEXT_SECONDARY)
                                    .font(FontId::new(10.0, FontFamily::Proportional))
                                    .strong(),
                            );
                            ui.label(
                                RichText::new(format!("{}", encs[i]))
                                    .color(theme::COLOR_PRIMARY)
                                    .font(FontId::new(15.0, FontFamily::Monospace))
                                    .strong(),
                            );
                        });
                    });
                }

                sunken_frame().show(ui, |ui| {
                    ui.set_width(70.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("AUX")
                                .color(theme::COLOR_TEXT_SECONDARY)
                                .font(FontId::new(10.0, FontFamily::Proportional))
                                .strong(),
                        );
                        ui.label(
                            RichText::new(format!("{}", aux))
                                .color(theme::COLOR_ACCENT_PURPLE)
                                .font(FontId::new(15.0, FontFamily::Monospace))
                                .strong(),
                        );
                    });
                });
            });
        });
    }

    fn render_device_telemetry_card(&self, ui: &mut Ui, dev_info_opt: Option<&DeviceInfo>) {
        card_frame(false).show(ui, |ui| {
            ui.set_width(400.0);
            ui.label(
                RichText::new("USB CONTROLLER TELEMETRY")
                    .color(theme::COLOR_PRIMARY)
                    .font(FontId::new(13.0, FontFamily::Proportional))
                    .strong(),
            );
            ui.add_space(6.0);

            if let Some(info) = dev_info_opt {
                ui.label(format!("Product      : {}", info.product));
                ui.label(format!("Manufacturer : {}", info.manufacturer));
                ui.label(format!("Serial Number: {}", info.serial_number));
                ui.label(format!("Firmware     : {}", info.firmware_version_string()));
                ui.label(format!(
                    "USB Location : Bus {:03} Device {:03} (06f8:e054)",
                    info.bus_number, info.address
                ));
                ui.label(format!(
                    "Bulk Endpoints: Max IN = {} B, Max OUT = {} B",
                    info.rx_max_transfer_size, info.tx_max_transfer_size
                ));
            } else {
                ui.label(
                    RichText::new("Device not connected (searching on USB bus...)")
                        .color(theme::COLOR_WARNING)
                        .italics(),
                );
            }
        });
    }

    fn render_event_log_card(&self, ui: &mut Ui) {
        card_frame(true).show(ui, |ui| {
            ui.set_width(480.0);
            ui.label(
                RichText::new("HARDWARE EVENT STREAM LOG")
                    .color(theme::COLOR_PRIMARY)
                    .font(FontId::new(14.0, FontFamily::Proportional))
                    .strong(),
            );
            ui.add_space(4.0);

            sunken_frame().show(ui, |ui| {
                ui.set_min_height(380.0);
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        if self.event_log.is_empty() {
                            ui.label(
                                RichText::new("No hardware events received yet. Rotate dials or press buttons on the controller.")
                                    .color(theme::COLOR_TEXT_MUTED)
                                    .font(FontId::new(11.0, FontFamily::Proportional))
                                    .italics(),
                            );
                        } else {
                            for (t, msg) in &self.event_log {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!("[{:>6.2}s]", t))
                                            .color(theme::COLOR_TEXT_MUTED)
                                            .font(FontId::new(10.0, FontFamily::Monospace)),
                                    );
                                    let color = if msg.contains("PRESSED") {
                                        theme::COLOR_PRIMARY
                                    } else if msg.contains("ENCODER") {
                                        theme::COLOR_ACCENT_LAVENDER
                                    } else {
                                        theme::COLOR_TEXT_PRIMARY
                                    };
                                    ui.label(
                                        RichText::new(msg)
                                            .color(color)
                                            .font(FontId::new(10.0, FontFamily::Monospace)),
                                    );
                                });
                            }
                        }
                    });
            });
        });
    }
}

fn render_bit_box(ui: &mut Ui, label: &str, bit_str: &str, active: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(85.0, 52.0), Sense::hover());
    let (fill, stroke, text_color) = if active {
        (
            theme::COLOR_PRIMARY,
            Stroke::new(1.5_f32, theme::COLOR_PRIMARY_HOVER),
            Color32::WHITE,
        )
    } else {
        (
            theme::COLOR_BG_SUNKEN,
            Stroke::new(1.0_f32, theme::COLOR_BORDER_SUBTLE),
            theme::COLOR_TEXT_MUTED,
        )
    };

    ui.painter().rect_filled(rect, Rounding::same(6.0), fill);
    ui.painter().rect_stroke(rect, Rounding::same(6.0), stroke);

    ui.painter().text(
        Pos2::new(rect.center().x, rect.min.y + 16.0),
        Align2::CENTER_CENTER,
        label,
        FontId::new(10.0, FontFamily::Proportional),
        text_color,
    );

    ui.painter().text(
        Pos2::new(rect.center().x, rect.min.y + 34.0),
        Align2::CENTER_CENTER,
        bit_str,
        FontId::new(9.0, FontFamily::Monospace),
        if active { Color32::WHITE } else { theme::COLOR_TEXT_SECONDARY },
    );
}
