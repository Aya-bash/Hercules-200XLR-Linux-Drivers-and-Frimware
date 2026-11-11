use egui::{
    vec2, Align, Color32, FontFamily, FontId, Layout, Pos2, Rect, RichText, Rounding,
    Sense, Stroke, Ui,
};

use crate::bridge::{DeviceBridge, GuiCommand};
use crate::theme::{self, card_frame, sunken_frame};
use hsm200_driver::{Button, HardwareEvent, InputReport, LedMode, MicSource, MixBus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixMode {
    Dual,
    Creator,
    Audience,
}

#[derive(Debug, Clone)]
pub struct ChannelStripState {
    pub name: String,
    pub subtitle: String,
    pub creator_vol: f32,
    pub audience_vol: f32,
    pub creator_muted: bool,
    pub audience_muted: bool,
    pub linked: bool,
    pub vu_peak: f32,
}

impl ChannelStripState {
    pub fn new(name: &str, subtitle: &str, initial_vol: f32) -> Self {
        Self {
            name: name.to_string(),
            subtitle: subtitle.to_string(),
            creator_vol: initial_vol,
            audience_vol: initial_vol,
            creator_muted: false,
            audience_muted: false,
            linked: true,
            vu_peak: (initial_vol * 0.85).clamp(10.0, 95.0),
        }
    }
}

pub struct MixerView {
    pub mix_mode: MixMode,
    pub active_page: u8, // 1: Ch 1-4, 2: Ch 5-8
    pub channels: [ChannelStripState; 8],
    pub headphone_vol: f32,
    pub headphone_muted: bool,
    pub phantom_power: bool,
    pub mic_source: MicSource,
}

impl Default for MixerView {
    fn default() -> Self {
        Self {
            mix_mode: MixMode::Dual,
            active_page: 1,
            channels: [
                ChannelStripState::new("CH 1: MIC / LINE", "XLR Combo Input", 80.0),
                ChannelStripState::new("CH 2: GAME AUDIO", "Application Bus", 75.0),
                ChannelStripState::new("CH 3: CHAT / COMM", "Discord / Voice", 70.0),
                ChannelStripState::new("CH 4: MUSIC / MEDIA", "Spotify / Browser", 65.0),
                ChannelStripState::new("CH 5: SFX / SOUNDS", "Soundboard / Alerts", 60.0),
                ChannelStripState::new("CH 6: AUX / SYSTEM", "Desktop System Audio", 70.0),
                ChannelStripState::new("CH 7: STREAM BUS", "OBS / Broadcast Output", 80.0),
                ChannelStripState::new("CH 8: EXTRA / VOD", "Secondary Mix Track", 50.0),
            ],
            headphone_vol: 75.0,
            headphone_muted: false,
            phantom_power: false,
            mic_source: MicSource::Xlr,
        }
    }
}

impl MixerView {
    /// Handles physical hardware events (encoder rotations, button presses, aux encoder).
    pub fn handle_hardware_event(
        &mut self,
        event: &HardwareEvent,
        bridge: &DeviceBridge,
    ) -> Option<String> {
        match *event {
            HardwareEvent::EncoderTurned { encoder, delta, .. } => {
                if !(1..=4).contains(&encoder) {
                    return None;
                }
                let enc_idx = (encoder - 1) as usize;
                let ch_idx = if self.active_page == 1 {
                    enc_idx
                } else {
                    enc_idx + 4
                };

                let step = (delta as f32) * 1.5;
                let ch = &mut self.channels[ch_idx];

                if ch.linked {
                    ch.creator_vol = (ch.creator_vol + step).clamp(0.0, 100.0);
                    ch.audience_vol = (ch.audience_vol + step).clamp(0.0, 100.0);
                    ch.vu_peak = (ch.creator_vol * 0.9).clamp(5.0, 100.0);

                    bridge.send(GuiCommand::SetChannelVolume {
                        channel: ch_idx as u8,
                        bus: MixBus::Creator,
                        volume_pct: ch.creator_vol,
                    });
                    bridge.send(GuiCommand::SetChannelVolume {
                        channel: ch_idx as u8,
                        bus: MixBus::Audience,
                        volume_pct: ch.audience_vol,
                    });

                    Some(format!(
                        "🎛️ Knob {encoder} -> {} (Linked): {:.0}%",
                        ch.name, ch.creator_vol
                    ))
                } else {
                    match self.mix_mode {
                        MixMode::Creator | MixMode::Dual => {
                            ch.creator_vol = (ch.creator_vol + step).clamp(0.0, 100.0);
                            ch.vu_peak = (ch.creator_vol * 0.9).clamp(5.0, 100.0);
                            bridge.send(GuiCommand::SetChannelVolume {
                                channel: ch_idx as u8,
                                bus: MixBus::Creator,
                                volume_pct: ch.creator_vol,
                            });
                            Some(format!(
                                "🎛️ Knob {encoder} -> {} (Creator): {:.0}%",
                                ch.name, ch.creator_vol
                            ))
                        }
                        MixMode::Audience => {
                            ch.audience_vol = (ch.audience_vol + step).clamp(0.0, 100.0);
                            ch.vu_peak = (ch.audience_vol * 0.9).clamp(5.0, 100.0);
                            bridge.send(GuiCommand::SetChannelVolume {
                                channel: ch_idx as u8,
                                bus: MixBus::Audience,
                                volume_pct: ch.audience_vol,
                            });
                            Some(format!(
                                "🎛️ Knob {encoder} -> {} (Audience): {:.0}%",
                                ch.name, ch.audience_vol
                            ))
                        }
                    }
                }
            }
            HardwareEvent::ButtonPressed(button) => match button {
                Button::Encoder1Push
                | Button::Encoder2Push
                | Button::Encoder3Push
                | Button::Encoder4Push => {
                    let enc_idx = match button {
                        Button::Encoder1Push => 0,
                        Button::Encoder2Push => 1,
                        Button::Encoder3Push => 2,
                        Button::Encoder4Push => 3,
                        _ => 0,
                    };
                    let ch_idx = if self.active_page == 1 {
                        enc_idx
                    } else {
                        enc_idx + 4
                    };
                    let ch = &mut self.channels[ch_idx];
                    ch.creator_muted = !ch.creator_muted;

                    bridge.send(GuiCommand::SetChannelMute {
                        channel: ch_idx as u8,
                        bus: MixBus::Creator,
                        muted: ch.creator_muted,
                    });

                    let led_mode = if ch.creator_muted {
                        LedMode::Blink
                    } else {
                        LedMode::Solid
                    };
                    bridge.send(GuiCommand::SetButtonLed {
                        button,
                        mode: led_mode,
                    });

                    Some(format!(
                        "🔘 Knob {} Pressed -> {} Creator Mute: {}",
                        enc_idx + 1,
                        ch.name,
                        if ch.creator_muted { "MUTED" } else { "UNMUTED" }
                    ))
                }
                Button::Action1 | Button::Action2 | Button::Action3 | Button::Action4 => {
                    let act_idx = match button {
                        Button::Action1 => 0,
                        Button::Action2 => 1,
                        Button::Action3 => 2,
                        Button::Action4 => 3,
                        _ => 0,
                    };
                    let ch_idx = if self.active_page == 1 {
                        act_idx
                    } else {
                        act_idx + 4
                    };
                    let ch = &mut self.channels[ch_idx];
                    ch.audience_muted = !ch.audience_muted;

                    bridge.send(GuiCommand::SetChannelMute {
                        channel: ch_idx as u8,
                        bus: MixBus::Audience,
                        muted: ch.audience_muted,
                    });

                    let led_mode = if ch.audience_muted {
                        LedMode::Blink
                    } else {
                        LedMode::Solid
                    };
                    bridge.send(GuiCommand::SetButtonLed {
                        button,
                        mode: led_mode,
                    });

                    Some(format!(
                        "⚡ Action {} -> {} Audience Mute: {}",
                        act_idx + 1,
                        ch.name,
                        if ch.audience_muted { "MUTED" } else { "UNMUTED" }
                    ))
                }
                Button::ControlA => {
                    // Page Switch Button
                    self.active_page = if self.active_page == 1 { 2 } else { 1 };
                    bridge.send(GuiCommand::SetScreenPage(self.active_page));
                    Some(format!("📄 Hardware Page Switched -> Page {}", self.active_page))
                }
                Button::ControlF => {
                    // Mix Mode Toggle
                    self.mix_mode = match self.mix_mode {
                        MixMode::Dual => MixMode::Creator,
                        MixMode::Creator => MixMode::Audience,
                        MixMode::Audience => MixMode::Dual,
                    };
                    Some(format!("🔀 Mix Mode: {:?}", self.mix_mode))
                }
                _ => None,
            },
            HardwareEvent::AuxEncoderChanged { old_value, new_value } => {
                let delta = (new_value as i16) - (old_value as i16);
                let step = (delta as f32) * 1.5;
                self.headphone_vol = (self.headphone_vol + step).clamp(0.0, 100.0);
                bridge.send(GuiCommand::SetHeadphoneVolume(self.headphone_vol));
                Some(format!("🎧 Headphone Volume: {:.0}%", self.headphone_vol))
            }
            _ => None,
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, _report_opt: Option<&InputReport>, bridge: &DeviceBridge) {
        ui.vertical(|ui| {
            // Top Controls Bar
            ui.horizontal(|ui| {
                theme::section_header(
                    ui,
                    "AUDIO MIXER CONSOLE",
                    "Dual-bus 8-channel audio matrix (Creator & Audience mixes) bound to hardware encoders",
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // View Mode Switcher
                    ui.selectable_value(&mut self.mix_mode, MixMode::Audience, "AUDIENCE MIX");
                    ui.selectable_value(&mut self.mix_mode, MixMode::Creator, "CREATOR MIX");
                    ui.selectable_value(&mut self.mix_mode, MixMode::Dual, "DUAL BUS");
                    ui.label(
                        RichText::new("VIEW MODE:")
                            .color(theme::COLOR_TEXT_SECONDARY)
                            .font(FontId::new(11.0, FontFamily::Proportional)),
                    );

                    ui.add_space(16.0);

                    // Page Switcher Tabs
                    let p2_active = self.active_page == 2;
                    let p1_active = self.active_page == 1;

                    if ui
                        .selectable_label(p2_active, "📄 PAGE 2 (CH 5 - 8)")
                        .clicked()
                    {
                        self.active_page = 2;
                        bridge.send(GuiCommand::SetScreenPage(2));
                    }
                    if ui
                        .selectable_label(p1_active, "📄 PAGE 1 (CH 1 - 4)")
                        .clicked()
                    {
                        self.active_page = 1;
                        bridge.send(GuiCommand::SetScreenPage(1));
                    }

                    ui.label(
                        RichText::new("ACTIVE PAGE:")
                            .color(theme::COLOR_PRIMARY)
                            .font(FontId::new(11.0, FontFamily::Proportional))
                            .strong(),
                    );
                });
            });

            ui.add_space(10.0);

            // Channel Strips Container
            ui.horizontal_top(|ui| {
                let base_ch = if self.active_page == 1 { 0 } else { 4 };

                for slot in 0..4 {
                    let ch_idx = base_ch + slot;
                    self.render_channel_strip(ui, ch_idx, slot + 1, bridge);
                    ui.add_space(8.0);
                }

                // Master & Hardware Inputs Section
                self.render_master_section(ui, bridge);
            });
        });
    }

    fn render_channel_strip(
        &mut self,
        ui: &mut Ui,
        ch_idx: usize,
        knob_num: usize,
        bridge: &DeviceBridge,
    ) {
        let ch = &self.channels[ch_idx];
        let is_linked = ch.linked;
        let creator_vol = ch.creator_vol;
        let audience_vol = ch.audience_vol;
        let creator_muted = ch.creator_muted;
        let audience_muted = ch.audience_muted;
        let title = ch.name.clone();
        let subtitle = ch.subtitle.clone();
        let vu_peak = ch.vu_peak;

        card_frame(false).show(ui, |ui| {
            ui.set_width(185.0);
            ui.set_min_height(490.0);

            ui.vertical_centered(|ui| {
                // Header badge with channel title & physical knob indicator
                sunken_frame().show(ui, |ui| {
                    ui.set_width(170.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(&title)
                                .color(theme::COLOR_PRIMARY)
                                .font(FontId::new(13.0, FontFamily::Proportional))
                                .strong(),
                        );
                        ui.label(
                            RichText::new(&subtitle)
                                .color(theme::COLOR_TEXT_MUTED)
                                .font(FontId::new(9.0, FontFamily::Proportional)),
                        );
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(format!("🎛️ KNOB {knob_num}"))
                                .color(theme::COLOR_ACCENT_LAVENDER)
                                .font(FontId::new(10.0, FontFamily::Monospace))
                                .strong(),
                        );
                    });
                });

                ui.add_space(6.0);

                // Link / Split Bus Toggle
                let link_btn_text = if is_linked { "🔗 LINKED BUSES" } else { "⚡ SPLIT BUSES" };
                let link_btn_color = if is_linked {
                    theme::COLOR_PRIMARY
                } else {
                    theme::COLOR_ACCENT_PURPLE
                };

                let link_btn = egui::Button::new(
                    RichText::new(link_btn_text)
                        .color(link_btn_color)
                        .font(FontId::new(10.0, FontFamily::Proportional))
                        .strong(),
                )
                .fill(theme::COLOR_BG_SUNKEN)
                .stroke(Stroke::new(1.0_f32, theme::COLOR_BORDER_SUBTLE))
                .rounding(Rounding::same(4.0));

                if ui.add(link_btn).clicked() {
                    self.channels[ch_idx].linked = !is_linked;
                }

                ui.add_space(6.0);

                // Fader Well Tracks Area
                sunken_frame().show(ui, |ui| {
                    ui.set_min_height(290.0);
                    ui.horizontal(|ui| {
                        match self.mix_mode {
                            MixMode::Dual => {
                                // Creator Fader (Neon Pink)
                                let mut c_vol = creator_vol;
                                if render_interactive_fader(
                                    ui,
                                    "CREATOR",
                                    &mut c_vol,
                                    theme::COLOR_PRIMARY,
                                    creator_muted,
                                ) {
                                    self.channels[ch_idx].creator_vol = c_vol;
                                    if is_linked {
                                        self.channels[ch_idx].audience_vol = c_vol;
                                        bridge.send(GuiCommand::SetChannelVolume {
                                            channel: ch_idx as u8,
                                            bus: MixBus::Audience,
                                            volume_pct: c_vol,
                                        });
                                    }
                                    bridge.send(GuiCommand::SetChannelVolume {
                                        channel: ch_idx as u8,
                                        bus: MixBus::Creator,
                                        volume_pct: c_vol,
                                    });
                                }

                                // Audience Fader (Electric Purple)
                                let mut a_vol = audience_vol;
                                if render_interactive_fader(
                                    ui,
                                    "AUDIENCE",
                                    &mut a_vol,
                                    theme::COLOR_ACCENT_PURPLE,
                                    audience_muted,
                                ) {
                                    self.channels[ch_idx].audience_vol = a_vol;
                                    if is_linked {
                                        self.channels[ch_idx].creator_vol = a_vol;
                                        bridge.send(GuiCommand::SetChannelVolume {
                                            channel: ch_idx as u8,
                                            bus: MixBus::Creator,
                                            volume_pct: a_vol,
                                        });
                                    }
                                    bridge.send(GuiCommand::SetChannelVolume {
                                        channel: ch_idx as u8,
                                        bus: MixBus::Audience,
                                        volume_pct: a_vol,
                                    });
                                }
                            }
                            MixMode::Creator => {
                                let mut c_vol = creator_vol;
                                if render_interactive_fader(
                                    ui,
                                    "CREATOR BUS",
                                    &mut c_vol,
                                    theme::COLOR_PRIMARY,
                                    creator_muted,
                                ) {
                                    self.channels[ch_idx].creator_vol = c_vol;
                                    if is_linked {
                                        self.channels[ch_idx].audience_vol = c_vol;
                                        bridge.send(GuiCommand::SetChannelVolume {
                                            channel: ch_idx as u8,
                                            bus: MixBus::Audience,
                                            volume_pct: c_vol,
                                        });
                                    }
                                    bridge.send(GuiCommand::SetChannelVolume {
                                        channel: ch_idx as u8,
                                        bus: MixBus::Creator,
                                        volume_pct: c_vol,
                                    });
                                }
                            }
                            MixMode::Audience => {
                                let mut a_vol = audience_vol;
                                if render_interactive_fader(
                                    ui,
                                    "AUDIENCE BUS",
                                    &mut a_vol,
                                    theme::COLOR_ACCENT_PURPLE,
                                    audience_muted,
                                ) {
                                    self.channels[ch_idx].audience_vol = a_vol;
                                    if is_linked {
                                        self.channels[ch_idx].creator_vol = a_vol;
                                        bridge.send(GuiCommand::SetChannelVolume {
                                            channel: ch_idx as u8,
                                            bus: MixBus::Creator,
                                            volume_pct: a_vol,
                                        });
                                    }
                                    bridge.send(GuiCommand::SetChannelVolume {
                                        channel: ch_idx as u8,
                                        bus: MixBus::Audience,
                                        volume_pct: a_vol,
                                    });
                                }
                            }
                        }
                    });
                });

                ui.add_space(6.0);

                // VU Meter Peak Indicator Bar
                render_horizontal_vu(ui, vu_peak, creator_muted || audience_muted);

                ui.add_space(6.0);

                // Dual Mute Action Buttons
                ui.horizontal(|ui| {
                    // Creator Mute Button
                    let c_text = if creator_muted { "🔇 MUTED" } else { "MUTE C" };
                    let c_fill = if creator_muted {
                        theme::COLOR_PRIMARY
                    } else {
                        theme::COLOR_BG_SUNKEN
                    };
                    let btn_c = egui::Button::new(
                        RichText::new(c_text)
                            .color(if creator_muted { Color32::WHITE } else { theme::COLOR_TEXT_PRIMARY })
                            .font(FontId::new(10.0, FontFamily::Proportional))
                            .strong(),
                    )
                    .fill(c_fill)
                    .stroke(Stroke::new(1.0_f32, theme::COLOR_BORDER_SUBTLE))
                    .rounding(Rounding::same(4.0));

                    if ui.add(btn_c).clicked() {
                        self.channels[ch_idx].creator_muted = !creator_muted;
                        bridge.send(GuiCommand::SetChannelMute {
                            channel: ch_idx as u8,
                            bus: MixBus::Creator,
                            muted: !creator_muted,
                        });
                    }

                    // Audience Mute Button
                    let a_text = if audience_muted { "🔇 MUTED" } else { "MUTE A" };
                    let a_fill = if audience_muted {
                        theme::COLOR_ACCENT_PURPLE
                    } else {
                        theme::COLOR_BG_SUNKEN
                    };
                    let btn_a = egui::Button::new(
                        RichText::new(a_text)
                            .color(if audience_muted { Color32::WHITE } else { theme::COLOR_TEXT_PRIMARY })
                            .font(FontId::new(10.0, FontFamily::Proportional))
                            .strong(),
                    )
                    .fill(a_fill)
                    .stroke(Stroke::new(1.0_f32, theme::COLOR_BORDER_SUBTLE))
                    .rounding(Rounding::same(4.0));

                    if ui.add(btn_a).clicked() {
                        self.channels[ch_idx].audience_muted = !audience_muted;
                        bridge.send(GuiCommand::SetChannelMute {
                            channel: ch_idx as u8,
                            bus: MixBus::Audience,
                            muted: !audience_muted,
                        });
                    }
                });
            });
        });
    }

    fn render_master_section(&mut self, ui: &mut Ui, bridge: &DeviceBridge) {
        card_frame(true).show(ui, |ui| {
            ui.set_width(210.0);
            ui.set_min_height(490.0);

            ui.vertical_centered(|ui| {
                // Master Header
                sunken_frame().show(ui, |ui| {
                    ui.set_width(190.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("MASTER & HARDWARE")
                                .color(theme::COLOR_PRIMARY)
                                .font(FontId::new(13.0, FontFamily::Proportional))
                                .strong(),
                        );
                        ui.label(
                            RichText::new("Monitor bus & XLR preamp")
                                .color(theme::COLOR_TEXT_MUTED)
                                .font(FontId::new(9.0, FontFamily::Proportional)),
                        );
                    });
                });

                ui.add_space(8.0);

                // Headphone / Monitor Volume Fader (bound to physical side aux encoder)
                sunken_frame().show(ui, |ui| {
                    ui.set_min_height(200.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("🎧 HEADPHONES")
                                .color(theme::COLOR_ACCENT_LAVENDER)
                                .font(FontId::new(11.0, FontFamily::Proportional))
                                .strong(),
                        );
                        ui.label(
                            RichText::new("(SIDE AUX KNOB)")
                                .color(theme::COLOR_TEXT_MUTED)
                                .font(FontId::new(8.0, FontFamily::Monospace)),
                        );
                        ui.add_space(4.0);

                        let mut hp_vol = self.headphone_vol;
                        if render_interactive_fader(
                            ui,
                            "MONITOR",
                            &mut hp_vol,
                            theme::COLOR_PRIMARY,
                            self.headphone_muted,
                        ) {
                            self.headphone_vol = hp_vol;
                            bridge.send(GuiCommand::SetHeadphoneVolume(hp_vol));
                        }
                    });
                });

                ui.add_space(6.0);

                // Headphone Mute
                let hp_mute_text = if self.headphone_muted {
                    "🔇 HEADPHONES MUTED"
                } else {
                    "MUTE HEADPHONES"
                };
                let hp_mute_fill = if self.headphone_muted {
                    theme::COLOR_PRIMARY
                } else {
                    theme::COLOR_BG_SUNKEN
                };
                let btn_hp = egui::Button::new(
                    RichText::new(hp_mute_text)
                        .color(if self.headphone_muted { Color32::WHITE } else { theme::COLOR_TEXT_PRIMARY })
                        .font(FontId::new(10.0, FontFamily::Proportional))
                        .strong(),
                )
                .fill(hp_mute_fill)
                .stroke(Stroke::new(1.0_f32, theme::COLOR_BORDER_SUBTLE))
                .rounding(Rounding::same(4.0));

                if ui.add(btn_hp).clicked() {
                    self.headphone_muted = !self.headphone_muted;
                }

                ui.add_space(10.0);

                // Hardware Microphone Preamp & Phantom Power Controls
                card_frame(false).show(ui, |ui| {
                    ui.set_width(190.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("HARDWARE MIC INPUT")
                                .color(theme::COLOR_PRIMARY)
                                .font(FontId::new(11.0, FontFamily::Proportional))
                                .strong(),
                        );
                        ui.add_space(4.0);

                        // Source Selector
                        ui.horizontal(|ui| {
                            ui.label("SOURCE:");
                            let xlr_active = self.mic_source == MicSource::Xlr;
                            if ui.selectable_label(xlr_active, "XLR").clicked() {
                                self.mic_source = MicSource::Xlr;
                                bridge.send(GuiCommand::SetMicSource(MicSource::Xlr));
                            }
                            let hs_active = self.mic_source == MicSource::Headset;
                            if ui.selectable_label(hs_active, "HEADSET").clicked() {
                                self.mic_source = MicSource::Headset;
                                bridge.send(GuiCommand::SetMicSource(MicSource::Headset));
                            }
                        });

                        ui.add_space(6.0);

                        // Phantom Power +48V Toggle Button
                        let p48 = self.phantom_power;
                        let p48_btn = egui::Button::new(
                            RichText::new(if p48 { "⚡ +48V PHANTOM: ON" } else { "+48V PHANTOM: OFF" })
                                .color(if p48 { Color32::WHITE } else { theme::COLOR_TEXT_SECONDARY })
                                .font(FontId::new(10.0, FontFamily::Proportional))
                                .strong(),
                        )
                        .fill(if p48 { Color32::from_rgb(0xD9, 0x00, 0x40) } else { theme::COLOR_BG_SUNKEN })
                        .stroke(Stroke::new(1.0_f32, if p48 { Color32::RED } else { theme::COLOR_BORDER_SUBTLE }))
                        .rounding(Rounding::same(4.0));

                        if ui.add(p48_btn).clicked() {
                            self.phantom_power = !p48;
                            bridge.send(GuiCommand::SetPhantomPower(!p48));
                        }
                    });
                });
            });
        });
    }
}

/// Renders an interactive vertical fader with sunken track depth, ticks, and draggable thumb.
fn render_interactive_fader(
    ui: &mut Ui,
    label: &str,
    value: &mut f32,
    accent_color: Color32,
    is_muted: bool,
) -> bool {
    let height = 210.0;
    let width = 34.0;
    let mut changed = false;

    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(label)
                .color(if is_muted { theme::COLOR_TEXT_MUTED } else { accent_color })
                .font(FontId::new(9.0, FontFamily::Proportional))
                .strong(),
        );

        let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::drag());

        if response.dragged() {
            let drag_delta = -response.drag_delta().y;
            let val_step = (drag_delta / height) * 100.0;
            *value = (*value + val_step).clamp(0.0, 100.0);
            changed = true;
        }

        // Sunken Track Background
        ui.painter()
            .rect_filled(rect, Rounding::same(4.0), theme::COLOR_BG_CARD);
        ui.painter().rect_stroke(
            rect,
            Rounding::same(4.0),
            Stroke::new(1.0_f32, theme::COLOR_BORDER_SUBTLE),
        );

        // Ticks Marks
        let steps = 10;
        for i in 1..steps {
            let y = rect.min.y + (rect.height() * (i as f32) / (steps as f32));
            ui.painter().line_segment(
                [Pos2::new(rect.min.x + 2.0, y), Pos2::new(rect.min.x + 6.0, y)],
                Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(0xFF, 0xFF, 0xFF, 0x18)),
            );
        }

        // Active Level Fill
        let effective_val = if is_muted { 0.0 } else { *value };
        let filled_h = (rect.height() * (effective_val / 100.0)).clamp(0.0, rect.height());
        let filled_rect = Rect::from_min_max(
            Pos2::new(rect.min.x + 3.0, rect.max.y - filled_h - 2.0),
            Pos2::new(rect.max.x - 3.0, rect.max.y - 2.0),
        );

        if filled_h > 1.0 {
            ui.painter()
                .rect_filled(filled_rect, Rounding::same(3.0), accent_color);
        }

        // Fader Thumb Handle
        let thumb_y = rect.max.y - (rect.height() * (*value / 100.0));
        let thumb_rect = Rect::from_center_size(
            Pos2::new(rect.center().x, thumb_y),
            vec2(width + 6.0, 10.0),
        );
        let thumb_fill = if response.hovered() || response.dragged() {
            Color32::WHITE
        } else {
            theme::COLOR_BG_RAISED
        };
        ui.painter().rect_filled(thumb_rect, Rounding::same(3.0), thumb_fill);
        ui.painter().rect_stroke(
            thumb_rect,
            Rounding::same(3.0),
            Stroke::new(1.2_f32, if is_muted { Color32::GRAY } else { accent_color }),
        );

        // Value text readout
        let db = hsm200_driver::MixerState::percent_to_db(*value);
        ui.add_space(2.0);
        ui.label(
            RichText::new(format!("{:.0}%", *value))
                .color(if is_muted { theme::COLOR_TEXT_MUTED } else { Color32::WHITE })
                .font(FontId::new(11.0, FontFamily::Monospace))
                .strong(),
        );
        ui.label(
            RichText::new(format!("{:.1} dB", db))
                .color(theme::COLOR_TEXT_MUTED)
                .font(FontId::new(9.0, FontFamily::Monospace)),
        );
    });

    changed
}

/// Renders a horizontal mini VU level indicator bar.
fn render_horizontal_vu(ui: &mut Ui, peak_pct: f32, muted: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(160.0, 6.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, Rounding::same(2.0), theme::COLOR_BG_CARD);

    if !muted {
        let fill_w = (rect.width() * (peak_pct / 100.0)).clamp(0.0, rect.width());
        let fill_rect = Rect::from_min_size(rect.min, vec2(fill_w, rect.height()));
        ui.painter()
            .rect_filled(fill_rect, Rounding::same(2.0), theme::COLOR_PRIMARY);
    }
}
