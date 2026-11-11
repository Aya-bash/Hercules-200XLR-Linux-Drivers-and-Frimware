//! The "Deck" tab: live view of the 4 knobs, their assignments, and running audio apps.

use std::f32::consts::PI;

use egui::{
    epaint::PathShape, vec2, Align2, Color32, FontFamily, FontId, Pos2, RichText, Rounding, Sense,
    Stroke, Ui,
};

use crate::audio::{AudioBridge, AudioSnapshot, MediaCmd, Target};
use crate::bridge::{DeviceBridge, GuiCommand};
use crate::config::{ButtonAction, Config};
use crate::theme::{self, card_frame, sunken_frame};
use hsm200_driver::{Button, HardwareEvent, InputReport, LedMode};

const PUSH_BUTTONS: [Button; 4] = [
    Button::Encoder1Push,
    Button::Encoder2Push,
    Button::Encoder3Push,
    Button::Encoder4Push,
];

pub struct DeckView {
    pub config: Config,
    /// 0 = LCD page 1, 1 = LCD page 2.
    pub page: usize,
    /// Knob currently being edited (0..3 on the active page).
    editing: Option<usize>,
    last_led: [Option<LedMode>; 4],
}

impl Default for DeckView {
    fn default() -> Self {
        Self {
            config: Config::load(),
            page: 0,
            editing: None,
            last_led: [None; 4],
        }
    }
}

impl DeckView {
    pub fn reset_leds(&mut self) {
        self.last_led = [None; 4];
    }

    fn knob_idx(&self, slot: usize) -> usize {
        self.page * 4 + slot
    }

    // ---------------------------------------------------------------- hardware input

    pub fn handle_event(
        &mut self,
        ev: &HardwareEvent,
        audio: &AudioBridge,
        bridge: &DeviceBridge,
    ) -> Option<String> {
        match ev {
            HardwareEvent::EncoderTurned { encoder, delta, .. } => {
                let slot = (*encoder as usize).checked_sub(1).filter(|s| *s < 4)?;
                let b = self.config.knobs[self.knob_idx(slot)].clone();
                if b.target == Target::None {
                    return Some(format!("Knob {encoder} is not assigned"));
                }
                let Some(level) = audio.snapshot().level(&b.target) else {
                    return Some(format!("Knob {encoder}: {} is not running", b.target.label()));
                };
                let new = (level.volume + f32::from(*delta) * b.step_pct / 100.0).clamp(0.0, 1.0);
                audio.set_volume(&b.target, new);
                Some(format!("{}  {:.0}%", b.target.label(), new * 100.0))
            }
            HardwareEvent::ButtonPressed(btn) => {
                let action = match btn {
                    Button::Encoder1Push
                    | Button::Encoder2Push
                    | Button::Encoder3Push
                    | Button::Encoder4Push => {
                        let slot = PUSH_BUTTONS.iter().position(|b| b == btn)?;
                        self.config.knobs[self.knob_idx(slot)].push.clone()
                    }
                    Button::Action1 => self.config.actions[0].clone(),
                    Button::Action2 => self.config.actions[1].clone(),
                    Button::Action3 => self.config.actions[2].clone(),
                    Button::Action4 => self.config.actions[3].clone(),
                    Button::ControlA => self.config.controls[0].clone(),
                    Button::ControlB => self.config.controls[1].clone(),
                    Button::ControlC => self.config.controls[2].clone(),
                    Button::ControlD => self.config.controls[3].clone(),
                    Button::ControlE => self.config.controls[4].clone(),
                    Button::ControlF => self.config.controls[5].clone(),
                };
                self.run_action(&action, audio, bridge)
            }
            _ => None,
        }
    }

    pub fn run_action(
        &mut self,
        action: &ButtonAction,
        audio: &AudioBridge,
        bridge: &DeviceBridge,
    ) -> Option<String> {
        match action {
            ButtonAction::None => None,
            ButtonAction::NextPage => {
                self.page = 1 - self.page;
                self.editing = None;
                bridge.send(GuiCommand::SetScreenPage(self.page as u8 + 1));
                Some(format!("Page {}", self.page + 1))
            }
            ButtonAction::MuteKnob(n) => {
                let t = self.config.knobs[self.knob_idx((*n as usize).min(3))].target.clone();
                self.toggle_mute(&t, audio)
            }
            ButtonAction::MuteTarget(t) => self.toggle_mute(t, audio),
            ButtonAction::PlayPause => {
                audio.media(MediaCmd::PlayPause);
                Some("Play / Pause".into())
            }
            ButtonAction::NextTrack => {
                audio.media(MediaCmd::Next);
                Some("Next track".into())
            }
            ButtonAction::PrevTrack => {
                audio.media(MediaCmd::Previous);
                Some("Previous track".into())
            }
            ButtonAction::Command(c) => {
                if !c.is_empty() {
                    audio.run_command(c.clone());
                }
                Some(format!("Ran: {c}"))
            }
        }
    }

    fn toggle_mute(&self, t: &Target, audio: &AudioBridge) -> Option<String> {
        match audio.toggle_mute(t) {
            Some(true) => Some(format!("{} muted", t.label())),
            Some(false) => Some(format!("{} unmuted", t.label())),
            None => Some(format!("{} is not running", t.label())),
        }
    }

    /// Keeps the 4 knob-push LEDs in sync with what each knob controls:
    /// solid = live, blinking = muted, off = unassigned or app not running.
    pub fn sync_leds(&mut self, audio: &AudioBridge, bridge: &DeviceBridge) {
        let snap = audio.snapshot();
        for slot in 0..4 {
            let t = &self.config.knobs[self.knob_idx(slot)].target;
            let mode = match snap.level(t) {
                None => LedMode::Off,
                Some(l) if l.muted => LedMode::Blink,
                Some(_) => LedMode::Solid,
            };
            if self.last_led[slot] != Some(mode) {
                self.last_led[slot] = Some(mode);
                bridge.send(GuiCommand::SetButtonLed {
                    button: PUSH_BUTTONS[slot],
                    mode,
                });
            }
        }
    }

    // ---------------------------------------------------------------- UI

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        audio: &AudioBridge,
        bridge: &DeviceBridge,
        report: Option<&InputReport>,
    ) {
        let snap = audio.snapshot();
        let before = self.config.clone();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal(|ui| {
                theme::section_header(
                    ui,
                    "DECK",
                    "Turn a knob to change what it controls. Click a knob below to reassign it.",
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    for (p, label) in [(1usize, "PAGE 2"), (0usize, "PAGE 1")] {
                        let active = self.page == p;
                        let btn = egui::Button::new(
                            RichText::new(label)
                                .strong()
                                .color(if active { Color32::WHITE } else { theme::COLOR_TEXT_SECONDARY }),
                        )
                        .fill(if active { theme::COLOR_PRIMARY } else { theme::COLOR_BG_CARD })
                        .rounding(Rounding::same(6.0));
                        if ui.add(btn).clicked() && !active {
                            self.page = p;
                            self.editing = None;
                            bridge.send(GuiCommand::SetScreenPage(p as u8 + 1));
                        }
                    }
                });
            });
            ui.add_space(8.0);

            // The four knobs of the active page.
            ui.horizontal_top(|ui| {
                for slot in 0..4 {
                    self.knob_card(ui, slot, &snap);
                    ui.add_space(6.0);
                }
            });

            if let Some(slot) = self.editing {
                ui.add_space(8.0);
                self.assign_panel(ui, slot, &snap);
            }

            ui.add_space(12.0);
            ui.horizontal_top(|ui| {
                self.apps_panel(ui, audio, &snap);
                ui.add_space(10.0);
                self.side_panel(ui, audio, &snap, report);
            });
        });

        if self.config != before {
            self.config.save();
            self.reset_leds();
        }
    }

    fn knob_card(&mut self, ui: &mut Ui, slot: usize, snap: &AudioSnapshot) {
        let idx = self.knob_idx(slot);
        let target = self.config.knobs[idx].target.clone();
        let level = snap.level(&target);
        let selected = self.editing == Some(slot);

        card_frame(selected).show(ui, |ui| {
            ui.set_width(190.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(format!("KNOB {}", slot + 1))
                        .color(theme::COLOR_ACCENT_LAVENDER)
                        .font(FontId::new(10.0, FontFamily::Monospace))
                        .strong(),
                );
                let resp = draw_dial(ui, 130.0, &target, level);
                ui.label(
                    RichText::new(target.label())
                        .color(theme::COLOR_TEXT_PRIMARY)
                        .font(FontId::new(14.0, FontFamily::Proportional))
                        .strong(),
                );
                let status = match (&target, level) {
                    (Target::None, _) => "Click to assign".to_string(),
                    (_, None) => "Not running".to_string(),
                    (_, Some(l)) if l.muted => "MUTED".to_string(),
                    (_, Some(_)) => format!("Push: {}", self.config.knobs[idx].push.label()),
                };
                ui.label(
                    RichText::new(status)
                        .color(match level {
                            Some(l) if l.muted => theme::COLOR_PRIMARY,
                            _ => theme::COLOR_TEXT_MUTED,
                        })
                        .font(FontId::new(10.0, FontFamily::Proportional)),
                );
                if resp.clicked() {
                    self.editing = if selected { None } else { Some(slot) };
                }
            });
        });
    }

    fn assign_panel(&mut self, ui: &mut Ui, slot: usize, snap: &AudioSnapshot) {
        let idx = self.knob_idx(slot);
        card_frame(true).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(format!("ASSIGN KNOB {} (page {})", slot + 1, self.page + 1))
                    .color(theme::COLOR_PRIMARY)
                    .strong(),
            );
            ui.add_space(6.0);

            // Target kind
            ui.horizontal(|ui| {
                ui.label("Controls:");
                let t = &mut self.config.knobs[idx].target;
                if ui.selectable_label(*t == Target::None, "Nothing").clicked() {
                    *t = Target::None;
                }
                if ui.selectable_label(*t == Target::System, "System volume").clicked() {
                    *t = Target::System;
                }
                if ui.selectable_label(*t == Target::Mic, "Microphone").clicked() {
                    *t = Target::Mic;
                }
                let is_app = matches!(t, Target::App(_));
                if ui.selectable_label(is_app, "An app").clicked() && !is_app {
                    *t = Target::App(String::new());
                }
            });

            // App picker
            if let Target::App(name) = &mut self.config.knobs[idx].target {
                ui.horizontal(|ui| {
                    ui.label("App name:");
                    ui.add(
                        egui::TextEdit::singleline(name)
                            .hint_text("e.g. Spotify, Discord, firefox")
                            .desired_width(200.0),
                    );
                    egui::ComboBox::from_id_salt("running_apps")
                        .selected_text("Pick running app")
                        .show_ui(ui, |ui| {
                            for app in &snap.apps {
                                if ui.selectable_label(false, &app.name).clicked() {
                                    *name = app.name.clone();
                                }
                            }
                            if snap.apps.is_empty() {
                                ui.label("No app is playing audio right now");
                            }
                        });
                });
                ui.label(
                    RichText::new("Matches any app whose name contains this text. Apps appear in the list once they play sound.")
                        .color(theme::COLOR_TEXT_MUTED)
                        .font(FontId::new(10.0, FontFamily::Proportional)),
                );
            }

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Sensitivity:");
                ui.add(
                    egui::Slider::new(&mut self.config.knobs[idx].step_pct, 0.5..=10.0)
                        .suffix("% per click"),
                );
            });

            ui.horizontal(|ui| {
                ui.label("When pushed:");
                action_picker(ui, format!("push_{idx}"), &mut self.config.knobs[idx].push);
            });
        });
    }

    fn apps_panel(&mut self, ui: &mut Ui, audio: &AudioBridge, snap: &AudioSnapshot) {
        card_frame(false).show(ui, |ui| {
            ui.set_width(520.0);
            ui.label(
                RichText::new("RUNNING APPS")
                    .color(theme::COLOR_PRIMARY)
                    .font(FontId::new(13.0, FontFamily::Proportional))
                    .strong(),
            );
            ui.label(
                RichText::new("Anything playing sound shows up here. Drag to adjust, or send it to a knob.")
                    .color(theme::COLOR_TEXT_MUTED)
                    .font(FontId::new(10.0, FontFamily::Proportional)),
            );
            ui.add_space(6.0);

            if snap.apps.is_empty() {
                ui.label(RichText::new("Nothing is playing audio.").italics().color(theme::COLOR_TEXT_MUTED));
            }

            // Collapse duplicate names (e.g. browsers with many streams).
            let mut seen: Vec<&str> = Vec::new();
            for app in &snap.apps {
                if seen.contains(&app.name.as_str()) {
                    continue;
                }
                seen.push(&app.name);
                let target = Target::App(app.name.clone());
                let level = snap.level(&target).unwrap_or_default();

                sunken_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.set_width(490.0);
                        ui.label(RichText::new(&app.name).strong().color(theme::COLOR_TEXT_PRIMARY));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            for k in (0..4).rev() {
                                if ui.small_button(format!("→K{}", k + 1)).on_hover_text("Assign to this knob (current page)").clicked() {
                                    let i = self.knob_idx(k);
                                    self.config.knobs[i].target = target.clone();
                                }
                            }
                            let mute_label = if level.muted { "🔇" } else { "🔊" };
                            if ui.button(mute_label).clicked() {
                                audio.set_mute(&target, !level.muted);
                            }
                            let mut v = level.volume * 100.0;
                            if ui
                                .add(egui::Slider::new(&mut v, 0.0..=100.0).suffix("%").show_value(true))
                                .changed()
                            {
                                audio.set_volume(&target, v / 100.0);
                            }
                        });
                    });
                });
                ui.add_space(3.0);
            }
        });
    }

    fn side_panel(
        &mut self,
        ui: &mut Ui,
        audio: &AudioBridge,
        snap: &AudioSnapshot,
        report: Option<&InputReport>,
    ) {
        ui.vertical(|ui| {
            card_frame(false).show(ui, |ui| {
                ui.set_width(300.0);
                ui.label(RichText::new("DEVICES").color(theme::COLOR_PRIMARY).strong());
                ui.add_space(4.0);
                for (label, target) in [("System", Target::System), ("Mic", Target::Mic)] {
                    let Some(l) = snap.level(&target) else { continue };
                    ui.horizontal(|ui| {
                        ui.label(format!("{label:<7}"));
                        let mut v = l.volume * 100.0;
                        if ui.add(egui::Slider::new(&mut v, 0.0..=100.0).suffix("%")).changed() {
                            audio.set_volume(&target, v / 100.0);
                        }
                        if ui.button(if l.muted { "🔇" } else { "🔊" }).clicked() {
                            audio.set_mute(&target, !l.muted);
                        }
                    });
                }
                if let Some(r) = report {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!(
                            "Headphone knob (hardware): {:.0}%",
                            r.headphone_volume / 100.0
                        ))
                        .color(theme::COLOR_TEXT_SECONDARY)
                        .font(FontId::new(11.0, FontFamily::Proportional)),
                    );
                }
            });

            ui.add_space(10.0);
            card_frame(false).show(ui, |ui| {
                ui.set_width(300.0);
                ui.label(RichText::new("BUTTONS").color(theme::COLOR_PRIMARY).strong());
                ui.add_space(4.0);
                for i in 0..4 {
                    ui.horizontal(|ui| {
                        ui.label(format!("Action {}", i + 1));
                        action_picker(ui, format!("act_{i}"), &mut self.config.actions[i]);
                    });
                }
                ui.separator();
                for (i, name) in ["A", "B", "C", "D", "E", "F"].iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("Button {name}"));
                        action_picker(ui, format!("ctl_{i}"), &mut self.config.controls[i]);
                    });
                }
            });
        });
    }
}

/// Drop-down for choosing a button action (with a text box for custom commands).
fn action_picker(ui: &mut Ui, id: String, action: &mut ButtonAction) {
    let options = [
        ButtonAction::None,
        ButtonAction::NextPage,
        ButtonAction::MuteKnob(0),
        ButtonAction::MuteKnob(1),
        ButtonAction::MuteKnob(2),
        ButtonAction::MuteKnob(3),
        ButtonAction::MuteTarget(Target::System),
        ButtonAction::MuteTarget(Target::Mic),
        ButtonAction::PlayPause,
        ButtonAction::NextTrack,
        ButtonAction::PrevTrack,
    ];
    egui::ComboBox::from_id_salt(id)
        .selected_text(action.label())
        .width(170.0)
        .show_ui(ui, |ui| {
            for o in options {
                let label = o.label();
                ui.selectable_value(action, o, label);
            }
            let is_cmd = matches!(action, ButtonAction::Command(_));
            if ui.selectable_label(is_cmd, "Run command…").clicked() && !is_cmd {
                *action = ButtonAction::Command(String::new());
            }
        });
    if let ButtonAction::Command(c) = action {
        ui.add(egui::TextEdit::singleline(c).hint_text("shell command").desired_width(160.0));
    }
}

/// Circular volume dial: 270° arc, fills pink with the level, grey when muted.
fn draw_dial(ui: &mut Ui, size: f32, target: &Target, level: Option<crate::audio::Level>) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let painter = ui.painter();
    let c = rect.center();
    let r = size * 0.42;
    let start = 0.75 * PI;
    let sweep = 1.5 * PI;

    let arc = |t0: f32, t1: f32| -> Vec<Pos2> {
        let n = 48;
        (0..=n)
            .map(|i| {
                let a = start + sweep * (t0 + (t1 - t0) * i as f32 / n as f32);
                c + vec2(a.cos(), a.sin()) * r
            })
            .collect()
    };

    painter.circle_filled(c, r + 10.0, theme::COLOR_BG_SUNKEN);
    painter.add(PathShape::line(arc(0.0, 1.0), Stroke::new(9.0_f32, theme::COLOR_BG_RAISED)));

    let (value, color) = match level {
        Some(l) if l.muted => (l.volume, theme::COLOR_TEXT_MUTED),
        Some(l) => (l.volume, theme::COLOR_PRIMARY),
        None => (0.0, theme::COLOR_TEXT_MUTED),
    };
    if value > 0.005 {
        painter.add(PathShape::line(arc(0.0, value.clamp(0.0, 1.0)), Stroke::new(9.0_f32, color)));
    }

    // Pointer
    let a = start + sweep * value.clamp(0.0, 1.0);
    painter.line_segment(
        [c + vec2(a.cos(), a.sin()) * (r - 20.0), c + vec2(a.cos(), a.sin()) * (r - 5.0)],
        Stroke::new(3.0_f32, Color32::WHITE),
    );

    let text = match (target, level) {
        (Target::None, _) => "—".to_string(),
        (_, None) => "OFF".to_string(),
        (_, Some(l)) if l.muted => "MUTE".to_string(),
        (_, Some(l)) => format!("{:.0}", l.volume * 100.0),
    };
    painter.text(
        c,
        Align2::CENTER_CENTER,
        text,
        FontId::new(26.0, FontFamily::Proportional),
        if level.is_some() { Color32::WHITE } else { theme::COLOR_TEXT_MUTED },
    );
    if resp.hovered() {
        painter.circle_stroke(c, r + 10.0, Stroke::new(1.5_f32, theme::COLOR_PRIMARY));
    }
    resp
}
