use egui::{
    vec2, Color32, FontFamily, FontId, Pos2, Rect, RichText, Rounding, Sense, Stroke, Ui,
};

use crate::bridge::{DeviceBridge, GuiCommand};
use crate::theme::{self, card_frame, sunken_frame};
use hsm200_driver::{DspPreset, EqFilterType, MicDspConfig, MicSource};

pub struct DspView {
    pub config: MicDspConfig,
    pub active_preset: Option<DspPreset>,
    pub auto_sync: bool,
    pub phantom: bool,
    pub mic_source: MicSource,
}

impl Default for DspView {
    fn default() -> Self {
        Self {
            config: MicDspConfig::from_preset(DspPreset::Default),
            active_preset: Some(DspPreset::Default),
            auto_sync: true,
            phantom: false,
            mic_source: MicSource::Xlr,
        }
    }
}

impl DspView {
    pub fn ui(&mut self, ui: &mut Ui, bridge: &DeviceBridge) {
        ui.vertical(|ui| {
            // Header Bar
            ui.horizontal(|ui| {
                theme::section_header(
                    ui,
                    "AK7735EQ VOCAL DSP PROCESSOR",
                    "Hardware studio DSP: analog preamp, noise gate, 3-band parametric EQ, compressor & limiter",
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let sync_btn = egui::Button::new(
                        RichText::new("⚡ SYNC DSP TO HARDWARE")
                            .color(Color32::WHITE)
                            .font(FontId::new(12.0, FontFamily::Proportional))
                            .strong(),
                    )
                    .fill(theme::COLOR_PRIMARY)
                    .rounding(Rounding::same(6.0));

                    if ui.add(sync_btn).clicked() {
                        bridge.send(GuiCommand::SyncDsp(self.config.clone()));
                    }

                    ui.checkbox(&mut self.auto_sync, "LIVE SYNC");
                });
            });

            ui.add_space(8.0);
            card_frame(false).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("MIC INPUT").color(theme::COLOR_PRIMARY).strong());
                    if ui.selectable_label(self.mic_source == MicSource::Xlr, "XLR").clicked() {
                        self.mic_source = MicSource::Xlr;
                        bridge.send(GuiCommand::SetMicSource(MicSource::Xlr));
                    }
                    if ui.selectable_label(self.mic_source == MicSource::Headset, "Headset").clicked() {
                        self.mic_source = MicSource::Headset;
                        bridge.send(GuiCommand::SetMicSource(MicSource::Headset));
                    }
                    ui.separator();
                    if ui.checkbox(&mut self.phantom, "+48V phantom power").changed() {
                        bridge.send(GuiCommand::SetPhantomPower(self.phantom));
                    }
                    ui.label(
                        RichText::new("Only enable for condenser mics that need it.")
                            .color(theme::COLOR_TEXT_MUTED),
                    );
                });
            });

            ui.add_space(10.0);

            // Preset Selector Bar with elevated cards
            self.render_presets_bar(ui, bridge);

            ui.add_space(12.0);

            // Grid of DSP Processing Modules
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    // Left Column: Preamp + Noise Gate
                    ui.vertical(|ui| {
                        self.render_preamp_module(ui, bridge);
                        ui.add_space(10.0);
                        self.render_gate_module(ui, bridge);
                    });

                    ui.add_space(12.0);

                    // Center Column: 3-Band Parametric EQ
                    ui.vertical(|ui| {
                        self.render_eq_module(ui, bridge);
                    });

                    ui.add_space(12.0);

                    // Right Column: Compressor + Peak Limiter
                    ui.vertical(|ui| {
                        self.render_compressor_module(ui, bridge);
                        ui.add_space(10.0);
                        self.render_limiter_module(ui, bridge);
                    });
                });
            });
        });
    }

    fn render_presets_bar(&mut self, ui: &mut Ui, bridge: &DeviceBridge) {
        let presets = [
            (DspPreset::Default, "DEFAULT (BALANCED)"),
            (DspPreset::FxDisabled, "BYPASS / FLAT"),
            (DspPreset::NaturalBright, "NATURAL BRIGHT"),
            (DspPreset::NaturalDark, "NATURAL DARK"),
            (DspPreset::RadioBright, "RADIO BRIGHT"),
            (DspPreset::RadioDark, "RADIO DARK"),
            (DspPreset::Hxd700Close, "HXD700 CLOSE"),
            (DspPreset::Hxd700Table, "HXD700 TABLE"),
        ];

        ui.horizontal_wrapped(|ui| {
            for (preset, label) in presets {
                let is_active = self.active_preset == Some(preset);
                let btn = egui::Button::new(
                    RichText::new(label)
                        .color(if is_active { Color32::WHITE } else { theme::COLOR_TEXT_SECONDARY })
                        .font(FontId::new(11.0, FontFamily::Proportional))
                        .strong(),
                )
                .fill(if is_active { theme::COLOR_PRIMARY } else { theme::COLOR_BG_CARD })
                .stroke(Stroke::new(
                    1.0_f32,
                    if is_active { theme::COLOR_PRIMARY } else { theme::COLOR_BORDER_SUBTLE },
                ))
                .rounding(Rounding::same(6.0));

                if ui.add(btn).clicked() {
                    self.active_preset = Some(preset);
                    self.config = MicDspConfig::from_preset(preset);
                    self.maybe_sync(bridge);
                }
            }
        });
    }

    fn render_preamp_module(&mut self, ui: &mut Ui, bridge: &DeviceBridge) {
        card_frame(false).show(ui, |ui| {
            ui.set_width(280.0);
            ui.label(
                RichText::new("ANALOG PREAMP")
                    .color(theme::COLOR_PRIMARY)
                    .font(FontId::new(13.0, FontFamily::Proportional))
                    .strong(),
            );
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("MIC GAIN:")
                        .color(theme::COLOR_TEXT_SECONDARY)
                        .font(FontId::new(11.0, FontFamily::Proportional)),
                );
                let mut gain = self.config.preamp_gain_db;
                let slider = egui::Slider::new(&mut gain, 0.0..=65.0)
                    .suffix(" dB")
                    .step_by(0.5);
                if ui.add(slider).changed() {
                    self.config.preamp_gain_db = gain;
                    self.active_preset = None;
                    self.maybe_sync(bridge);
                }
            });

            // Gain meter indicator bar
            sunken_frame().show(ui, |ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(250.0, 8.0), Sense::hover());
                ui.painter()
                    .rect_filled(rect, Rounding::same(2.0), theme::COLOR_BG_CARD);
                let fill_w =
                    (rect.width() * (self.config.preamp_gain_db / 65.0)).clamp(0.0, rect.width());
                let fill_rect = Rect::from_min_size(rect.min, vec2(fill_w, rect.height()));
                ui.painter()
                    .rect_filled(fill_rect, Rounding::same(2.0), theme::COLOR_PRIMARY);
            });
        });
    }

    fn render_gate_module(&mut self, ui: &mut Ui, bridge: &DeviceBridge) {
        card_frame(false).show(ui, |ui| {
            ui.set_width(280.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("NOISE GATE")
                        .color(theme::COLOR_PRIMARY)
                        .font(FontId::new(13.0, FontFamily::Proportional))
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut en = self.config.noise_gate.enabled;
                    if ui.checkbox(&mut en, "ENABLE").changed() {
                        self.config.noise_gate.enabled = en;
                        self.active_preset = None;
                        self.maybe_sync(bridge);
                    }
                });
            });

            ui.add_space(6.0);

            let mut thresh = self.config.noise_gate.threshold_db;
            ui.horizontal(|ui| {
                ui.label("THRESHOLD:");
                if ui
                    .add(egui::Slider::new(&mut thresh, -80.0..=-10.0).suffix(" dB"))
                    .changed()
                {
                    self.config.noise_gate.threshold_db = thresh;
                    self.active_preset = None;
                    self.maybe_sync(bridge);
                }
            });

            let mut atten = self.config.noise_gate.attenuation_percent;
            ui.horizontal(|ui| {
                ui.label("ATTENUATION:");
                if ui
                    .add(egui::Slider::new(&mut atten, 0..=100).suffix(" %"))
                    .changed()
                {
                    self.config.noise_gate.attenuation_percent = atten;
                    self.active_preset = None;
                    self.maybe_sync(bridge);
                }
            });

            let mut attack = self.config.noise_gate.attack_ms;
            ui.horizontal(|ui| {
                ui.label("ATTACK:");
                if ui
                    .add(egui::Slider::new(&mut attack, 1..=100).suffix(" ms"))
                    .changed()
                {
                    self.config.noise_gate.attack_ms = attack;
                    self.active_preset = None;
                    self.maybe_sync(bridge);
                }
            });

            let mut release = self.config.noise_gate.release_ms;
            ui.horizontal(|ui| {
                ui.label("RELEASE:");
                if ui
                    .add(egui::Slider::new(&mut release, 10..=1000).suffix(" ms"))
                    .changed()
                {
                    self.config.noise_gate.release_ms = release;
                    self.active_preset = None;
                    self.maybe_sync(bridge);
                }
            });
        });
    }

    fn render_eq_module(&mut self, ui: &mut Ui, bridge: &DeviceBridge) {
        card_frame(true).show(ui, |ui| {
            ui.set_width(340.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("3-BAND PARAMETRIC EQUALIZER")
                        .color(theme::COLOR_PRIMARY)
                        .font(FontId::new(14.0, FontFamily::Proportional))
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut en = self.config.eq_enabled;
                    if ui.checkbox(&mut en, "ENABLE").changed() {
                        self.config.eq_enabled = en;
                        self.active_preset = None;
                        self.maybe_sync(bridge);
                    }
                });
            });

            ui.add_space(8.0);

            // Curve Visualizer Canvas
            sunken_frame().show(ui, |ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(310.0, 110.0), Sense::hover());
                ui.painter()
                    .rect_filled(rect, Rounding::same(6.0), theme::COLOR_BG_CARD);

                // Zero line
                let zero_y = rect.center().y;
                ui.painter().line_segment(
                    [Pos2::new(rect.min.x, zero_y), Pos2::new(rect.max.x, zero_y)],
                    Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(0xFF, 0xFF, 0xFF, 0x20)),
                );

                // Draw curved response representation
                let bands = &self.config.eq_bands;
                let b0_gain = if self.config.eq_enabled { bands[0].gain_db } else { 0.0 };
                let b1_gain = if self.config.eq_enabled { bands[1].gain_db } else { 0.0 };
                let b2_gain = if self.config.eq_enabled { bands[2].gain_db } else { 0.0 };

                let p0 = Pos2::new(rect.min.x, zero_y - (b0_gain * 2.5));
                let p1 = Pos2::new(rect.min.x + 60.0, zero_y - (b0_gain * 2.5));
                let p2 = Pos2::new(rect.center().x, zero_y - (b1_gain * 2.5));
                let p3 = Pos2::new(rect.max.x - 60.0, zero_y - (b2_gain * 2.5));
                let p4 = Pos2::new(rect.max.x, zero_y - (b2_gain * 2.5));

                let points = vec![p0, p1, p2, p3, p4];
                for i in 0..points.len() - 1 {
                    ui.painter().line_segment(
                        [points[i], points[i + 1]],
                        Stroke::new(2.5_f32, theme::COLOR_PRIMARY),
                    );
                }

                // Control point handles
                for p in [p1, p2, p3] {
                    ui.painter().circle_filled(p, 4.0, Color32::WHITE);
                    ui.painter()
                        .circle_stroke(p, 4.0, Stroke::new(1.5_f32, theme::COLOR_PRIMARY));
                }
            });

            ui.add_space(10.0);

            // 3 Bands: Low, Mid, High
            let band_labels = ["BAND 1 (LOW)", "BAND 2 (MID)", "BAND 3 (HIGH)"];
            let mut eq_changed = false;
            for (idx, band) in self.config.eq_bands.iter_mut().enumerate() {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(band_labels[idx])
                                .color(theme::COLOR_ACCENT_LAVENDER)
                                .font(FontId::new(11.0, FontFamily::Proportional))
                                .strong(),
                        );
                        egui::ComboBox::from_id_salt(format!("eq_filter_{idx}"))
                            .selected_text(filter_type_name(band.filter_type))
                            .show_ui(ui, |ui| {
                                for ft in [
                                    EqFilterType::Bypass,
                                    EqFilterType::HighPass,
                                    EqFilterType::LowPass,
                                    EqFilterType::Peaking,
                                    EqFilterType::LowShelf,
                                    EqFilterType::HighShelf,
                                ] {
                                    if ui
                                        .selectable_value(
                                            &mut band.filter_type,
                                            ft,
                                            filter_type_name(ft),
                                        )
                                        .changed()
                                    {
                                        eq_changed = true;
                                    }
                                }
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("GAIN:");
                        if ui
                            .add(egui::Slider::new(&mut band.gain_db, -15.0..=15.0).suffix(" dB"))
                            .changed()
                        {
                            eq_changed = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("FREQ:");
                        let mut freq = band.frequency_hz as u32;
                        if ui
                            .add(egui::Slider::new(&mut freq, 20..=20000).suffix(" Hz"))
                            .changed()
                        {
                            band.frequency_hz = freq as u16;
                            eq_changed = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Q:");
                        if ui
                            .add(egui::Slider::new(&mut band.q_factor, 0.1..=5.0).step_by(0.1))
                            .changed()
                        {
                            eq_changed = true;
                        }
                    });
                });
                ui.add_space(4.0);
            }

            if eq_changed {
                self.active_preset = None;
                self.maybe_sync(bridge);
            }
        });
    }

    fn render_compressor_module(&mut self, ui: &mut Ui, bridge: &DeviceBridge) {
        card_frame(false).show(ui, |ui| {
            ui.set_width(280.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("COMPRESSOR")
                        .color(theme::COLOR_PRIMARY)
                        .font(FontId::new(13.0, FontFamily::Proportional))
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut en = self.config.compressor.enabled;
                    if ui.checkbox(&mut en, "ENABLE").changed() {
                        self.config.compressor.enabled = en;
                        self.active_preset = None;
                        self.maybe_sync(bridge);
                    }
                });
            });

            ui.add_space(6.0);
            let mut thresh = self.config.compressor.threshold_db;
            ui.horizontal(|ui| {
                ui.label("THRESH:");
                if ui
                    .add(egui::Slider::new(&mut thresh, -40.0..=0.0).suffix(" dB"))
                    .changed()
                {
                    self.config.compressor.threshold_db = thresh;
                    self.active_preset = None;
                    self.maybe_sync(bridge);
                }
            });

            let mut ratio = self.config.compressor.ratio;
            ui.horizontal(|ui| {
                ui.label("RATIO:");
                if ui
                    .add(egui::Slider::new(&mut ratio, 1..=20).suffix(":1"))
                    .changed()
                {
                    self.config.compressor.ratio = ratio;
                    self.active_preset = None;
                    self.maybe_sync(bridge);
                }
            });

            let mut makeup = self.config.compressor.makeup_gain_db;
            ui.horizontal(|ui| {
                ui.label("MAKEUP:");
                if ui
                    .add(egui::Slider::new(&mut makeup, 0.0..=24.0).suffix(" dB"))
                    .changed()
                {
                    self.config.compressor.makeup_gain_db = makeup;
                    self.active_preset = None;
                    self.maybe_sync(bridge);
                }
            });

            let mut attack = self.config.compressor.attack_ms;
            ui.horizontal(|ui| {
                ui.label("ATTACK:");
                if ui
                    .add(egui::Slider::new(&mut attack, 1..=100).suffix(" ms"))
                    .changed()
                {
                    self.config.compressor.attack_ms = attack;
                    self.active_preset = None;
                    self.maybe_sync(bridge);
                }
            });

            let mut release = self.config.compressor.release_ms;
            ui.horizontal(|ui| {
                ui.label("RELEASE:");
                if ui
                    .add(egui::Slider::new(&mut release, 10..=1000).suffix(" ms"))
                    .changed()
                {
                    self.config.compressor.release_ms = release;
                    self.active_preset = None;
                    self.maybe_sync(bridge);
                }
            });
        });
    }

    fn render_limiter_module(&mut self, ui: &mut Ui, bridge: &DeviceBridge) {
        card_frame(false).show(ui, |ui| {
            ui.set_width(280.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("PEAK LIMITER")
                        .color(theme::COLOR_PRIMARY)
                        .font(FontId::new(13.0, FontFamily::Proportional))
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut en = self.config.limiter.enabled;
                    if ui.checkbox(&mut en, "ENABLE").changed() {
                        self.config.limiter.enabled = en;
                        self.active_preset = None;
                        self.maybe_sync(bridge);
                    }
                });
            });

            ui.add_space(6.0);
            let mut thresh = self.config.limiter.threshold_db;
            ui.horizontal(|ui| {
                ui.label("CEILING:");
                if ui
                    .add(egui::Slider::new(&mut thresh, -24.0..=0.0).suffix(" dB"))
                    .changed()
                {
                    self.config.limiter.threshold_db = thresh;
                    self.active_preset = None;
                    self.maybe_sync(bridge);
                }
            });

            let mut attack = self.config.limiter.attack_ms;
            ui.horizontal(|ui| {
                ui.label("ATTACK:");
                if ui
                    .add(egui::Slider::new(&mut attack, 1..=50).suffix(" ms"))
                    .changed()
                {
                    self.config.limiter.attack_ms = attack;
                    self.active_preset = None;
                    self.maybe_sync(bridge);
                }
            });

            let mut release = self.config.limiter.release_ms;
            ui.horizontal(|ui| {
                ui.label("RELEASE:");
                if ui
                    .add(egui::Slider::new(&mut release, 10..=500).suffix(" ms"))
                    .changed()
                {
                    self.config.limiter.release_ms = release;
                    self.active_preset = None;
                    self.maybe_sync(bridge);
                }
            });
        });
    }

    fn maybe_sync(&self, bridge: &DeviceBridge) {
        if self.auto_sync {
            bridge.send(GuiCommand::SyncDsp(self.config.clone()));
        }
    }
}

fn filter_type_name(ft: EqFilterType) -> &'static str {
    match ft {
        EqFilterType::Bypass => "Bypass",
        EqFilterType::HighPass => "High-Pass",
        EqFilterType::LowPass => "Low-Pass",
        EqFilterType::Peaking => "Peaking",
        EqFilterType::LowShelf => "Low Shelf",
        EqFilterType::HighShelf => "High Shelf",
    }
}
