use egui::{
    vec2, Align, Align2, Color32, FontFamily, FontId, Layout, Pos2, Rect, RichText, Rounding,
    Sense, Stroke, Ui,
};

use crate::bridge::DeviceBridge;
use crate::theme::{self, card_frame, sunken_frame};
use hsm200_driver::InputReport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixMode {
    Dual,
    Creator,
    Audience,
}

pub struct MixerView {
    pub mix_mode: MixMode,
    pub linked: [bool; 4],
    #[allow(dead_code)]
    pub simulated_vu: [f32; 8],
}

impl Default for MixerView {
    fn default() -> Self {
        Self {
            mix_mode: MixMode::Dual,
            linked: [false; 4],
            simulated_vu: [0.0; 8],
        }
    }
}

impl MixerView {
    pub fn ui(&mut self, ui: &mut Ui, report_opt: Option<&InputReport>, bridge: &DeviceBridge) {
        ui.vertical(|ui| {
            // Top Controls Bar
            ui.horizontal(|ui| {
                theme::section_header(
                    ui,
                    "AUDIO MIXER CONSOLE",
                    "Dual-bus 4-channel audio matrix (Creator & Audience mixes)",
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.selectable_value(&mut self.mix_mode, MixMode::Audience, "AUDIENCE MIX");
                    ui.selectable_value(&mut self.mix_mode, MixMode::Creator, "CREATOR MIX");
                    ui.selectable_value(&mut self.mix_mode, MixMode::Dual, "DUAL BUS");
                    ui.label(
                        RichText::new("VIEW MODE:")
                            .color(theme::COLOR_TEXT_SECONDARY)
                            .font(FontId::new(12.0, FontFamily::Proportional)),
                    );
                });
            });

            ui.add_space(12.0);

            let (creator_faders, audience_faders, headphone_vol) = match report_opt {
                Some(r) => (
                    [
                        r.creator_faders[0] / 100.0,
                        r.creator_faders[1] / 100.0,
                        r.creator_faders[2] / 100.0,
                        r.creator_faders[3] / 100.0,
                    ],
                    [
                        r.audience_faders[0] / 100.0,
                        r.audience_faders[1] / 100.0,
                        r.audience_faders[2] / 100.0,
                        r.audience_faders[3] / 100.0,
                    ],
                    r.headphone_volume / 100.0,
                ),
                None => ([75.0, 65.0, 50.0, 80.0], [70.0, 60.0, 45.0, 75.0], 72.0),
            };

            // Main Channel Strips Container
            ui.horizontal_top(|ui| {
                let channel_names = [
                    ("CH 1", "XLR MIC / LINE"),
                    ("CH 2", "GAME / MUSIC"),
                    ("CH 3", "VOICE / CHAT"),
                    ("CH 4", "SYSTEM / SFX"),
                ];

                for ch in 0..4 {
                    self.render_channel_strip(
                        ui,
                        ch,
                        channel_names[ch].0,
                        channel_names[ch].1,
                        creator_faders[ch],
                        audience_faders[ch],
                    );
                    ui.add_space(10.0);
                }

                // Master / Headphone Output Strip
                self.render_master_strip(ui, headphone_vol, bridge);
            });
        });
    }

    fn render_channel_strip(
        &mut self,
        ui: &mut Ui,
        ch_idx: usize,
        title: &str,
        subtitle: &str,
        creator_val: f32,
        audience_val: f32,
    ) {
        let is_linked = self.linked[ch_idx];

        card_frame(false).show(ui, |ui| {
            ui.set_width(175.0);
            ui.set_min_height(480.0);

            ui.vertical_centered(|ui| {
                // Header badge
                let (badge_rect, _) = ui.allocate_exact_size(vec2(150.0, 26.0), Sense::hover());
                ui.painter().rect_filled(
                    badge_rect,
                    Rounding::same(6.0),
                    theme::COLOR_BG_SUNKEN,
                );
                ui.painter().text(
                    badge_rect.center(),
                    Align2::CENTER_CENTER,
                    title,
                    FontId::new(14.0, FontFamily::Proportional),
                    theme::COLOR_PRIMARY,
                );

                ui.label(
                    RichText::new(subtitle)
                        .color(theme::COLOR_TEXT_MUTED)
                        .font(FontId::new(10.0, FontFamily::Proportional)),
                );

                ui.add_space(8.0);

                // Link toggle
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("LINK BUSES")
                            .color(theme::COLOR_TEXT_SECONDARY)
                            .font(FontId::new(10.0, FontFamily::Proportional)),
                    );
                    let link_text = if is_linked { "LINKED" } else { "SPLIT" };
                    let link_color = if is_linked {
                        theme::COLOR_PRIMARY
                    } else {
                        theme::COLOR_TEXT_MUTED
                    };
                    if ui
                        .button(RichText::new(link_text).color(link_color).strong())
                        .clicked()
                    {
                        self.linked[ch_idx] = !self.linked[ch_idx];
                    }
                });

                ui.add_space(8.0);

                // Fader Tracks Area with Height/Well effect
                sunken_frame().show(ui, |ui| {
                    ui.set_min_height(310.0);
                    ui.horizontal(|ui| {
                        match self.mix_mode {
                            MixMode::Dual => {
                                render_vertical_meter(
                                    ui,
                                    "CREATOR",
                                    creator_val,
                                    theme::COLOR_PRIMARY,
                                );
                                render_vertical_meter(
                                    ui,
                                    "AUDIENCE",
                                    audience_val,
                                    theme::COLOR_ACCENT_PURPLE,
                                );
                            }
                            MixMode::Creator => {
                                render_vertical_meter(
                                    ui,
                                    "CREATOR FADER",
                                    creator_val,
                                    theme::COLOR_PRIMARY,
                                );
                            }
                            MixMode::Audience => {
                                render_vertical_meter(
                                    ui,
                                    "AUDIENCE FADER",
                                    audience_val,
                                    theme::COLOR_ACCENT_PURPLE,
                                );
                            }
                        }
                    });
                });

                ui.add_space(8.0);

                // Quick Mute Buttons
                ui.horizontal(|ui| {
                    let btn_cr = egui::Button::new(
                        RichText::new("MUTE C")
                            .color(theme::COLOR_TEXT_PRIMARY)
                            .font(FontId::new(11.0, FontFamily::Proportional)),
                    )
                    .fill(theme::COLOR_BG_SUNKEN)
                    .stroke(Stroke::new(1.0_f32, theme::COLOR_BORDER_SUBTLE))
                    .rounding(Rounding::same(6.0));

                    let btn_aud = egui::Button::new(
                        RichText::new("MUTE A")
                            .color(theme::COLOR_TEXT_PRIMARY)
                            .font(FontId::new(11.0, FontFamily::Proportional)),
                    )
                    .fill(theme::COLOR_BG_SUNKEN)
                    .stroke(Stroke::new(1.0_f32, theme::COLOR_BORDER_SUBTLE))
                    .rounding(Rounding::same(6.0));

                    ui.add(btn_cr);
                    ui.add(btn_aud);
                });
            });
        });
    }

    fn render_master_strip(&mut self, ui: &mut Ui, headphone_vol: f32, _bridge: &DeviceBridge) {
        card_frame(true).show(ui, |ui| {
            ui.set_width(170.0);
            ui.set_min_height(480.0);

            ui.vertical_centered(|ui| {
                // Header badge
                let (badge_rect, _) = ui.allocate_exact_size(vec2(145.0, 26.0), Sense::hover());
                ui.painter()
                    .rect_filled(badge_rect, Rounding::same(6.0), theme::COLOR_PLUM_DEEP);
                ui.painter().text(
                    badge_rect.center(),
                    Align2::CENTER_CENTER,
                    "MONITOR",
                    FontId::new(14.0, FontFamily::Proportional),
                    theme::COLOR_PRIMARY,
                );

                ui.label(
                    RichText::new("HEADPHONES & AUX")
                        .color(theme::COLOR_TEXT_SECONDARY)
                        .font(FontId::new(10.0, FontFamily::Proportional)),
                );

                ui.add_space(8.0);

                ui.label(
                    RichText::new("DIAL ENCODER")
                        .color(theme::COLOR_TEXT_MUTED)
                        .font(FontId::new(10.0, FontFamily::Proportional)),
                );

                ui.add_space(8.0);

                sunken_frame().show(ui, |ui| {
                    ui.set_min_height(310.0);
                    render_vertical_meter(
                        ui,
                        "HEADPHONE",
                        headphone_vol,
                        theme::COLOR_PRIMARY,
                    );
                });

                ui.add_space(8.0);

                ui.label(
                    RichText::new(format!("{:.1}%", headphone_vol))
                        .color(theme::COLOR_PRIMARY)
                        .font(FontId::new(18.0, FontFamily::Proportional))
                        .strong(),
                );
            });
        });
    }
}

fn render_vertical_meter(ui: &mut Ui, label: &str, value_pct: f32, accent_color: Color32) {
    let height = 240.0;
    let width = 36.0;

    ui.vertical_centered(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());

        // Background track with sunken depth
        ui.painter()
            .rect_filled(rect, Rounding::same(4.0), theme::COLOR_BG_CARD);
        ui.painter()
            .rect_stroke(rect, Rounding::same(4.0), Stroke::new(1.0_f32, theme::COLOR_BORDER_SUBTLE));

        // Ticks / dB marks
        let steps = 10;
        for i in 1..steps {
            let y = rect.min.y + (rect.height() * (i as f32) / (steps as f32));
            ui.painter().line_segment(
                [Pos2::new(rect.min.x + 2.0, y), Pos2::new(rect.min.x + 7.0, y)],
                Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(0xFF, 0xFF, 0xFF, 0x20)),
            );
        }

        // Active fill bar
        let filled_h = (rect.height() * (value_pct / 100.0)).clamp(0.0, rect.height());
        let filled_rect = Rect::from_min_max(
            Pos2::new(rect.min.x + 3.0, rect.max.y - filled_h - 3.0),
            Pos2::new(rect.max.x - 3.0, rect.max.y - 3.0),
        );

        if filled_h > 1.0 {
            ui.painter()
                .rect_filled(filled_rect, Rounding::same(3.0), accent_color);
        }

        // Fader thumb handle
        let thumb_y = rect.max.y - filled_h;
        let thumb_rect = Rect::from_center_size(Pos2::new(rect.center().x, thumb_y), vec2(width + 8.0, 10.0));
        ui.painter().rect_filled(
            thumb_rect,
            Rounding::same(3.0),
            theme::COLOR_BG_RAISED,
        );
        ui.painter().rect_stroke(
            thumb_rect,
            Rounding::same(3.0),
            Stroke::new(1.2_f32, theme::COLOR_PRIMARY),
        );
        // Center stripe on thumb
        ui.painter().line_segment(
            [
                Pos2::new(thumb_rect.min.x + 4.0, thumb_rect.center().y),
                Pos2::new(thumb_rect.max.x - 4.0, thumb_rect.center().y),
            ],
            Stroke::new(1.5_f32, theme::COLOR_PRIMARY),
        );

        ui.add_space(4.0);
        ui.label(
            RichText::new(format!("{:.0}%", value_pct))
                .color(theme::COLOR_TEXT_PRIMARY)
                .font(FontId::new(11.0, FontFamily::Proportional))
                .strong(),
        );

        ui.label(
            RichText::new(label)
                .color(theme::COLOR_TEXT_MUTED)
                .font(FontId::new(8.0, FontFamily::Proportional)),
        );
    });
}
