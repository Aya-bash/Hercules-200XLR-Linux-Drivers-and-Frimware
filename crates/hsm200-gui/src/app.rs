use egui::{
    vec2, Align, Color32, FontFamily, FontId, Layout, RichText, Rounding, Sense, Stroke,
};

use crate::audio::AudioBridge;
use crate::bridge::{DeviceBridge, DeviceEvent};
use crate::theme::{self, card_frame};
use crate::views::deck::DeckView;
use crate::views::display::DisplayView;
use crate::views::dsp::DspView;
use crate::views::lighting::LightingView;
use crate::views::monitor::MonitorView;
use hsm200_driver::{DeviceInfo, InputReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTab {
    Deck,
    Dsp,
    Lighting,
    Display,
    Monitor,
}

pub struct StreamControlApp {
    bridge: DeviceBridge,
    active_tab: AppTab,
    connected_info: Option<DeviceInfo>,
    latest_report: Option<InputReport>,
    toast_message: Option<(String, Color32)>,
    audio: AudioBridge,
    deck_view: DeckView,
    dsp_view: DspView,
    lighting_view: LightingView,
    display_view: DisplayView,
    monitor_view: MonitorView,
}

impl StreamControlApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply_theme(&cc.egui_ctx);

        let bridge = DeviceBridge::spawn();

        Self {
            bridge,
            active_tab: AppTab::Deck,
            connected_info: None,
            latest_report: None,
            toast_message: None,
            audio: AudioBridge::spawn(),
            deck_view: DeckView::default(),
            dsp_view: DspView::default(),
            lighting_view: LightingView::default(),
            display_view: DisplayView::default(),
            monitor_view: MonitorView::default(),
        }
    }

    fn process_events(&mut self) {
        for ev in self.bridge.drain_events() {
            match ev {
                DeviceEvent::Connected(info) => {
                    self.toast_message = Some((
                        format!("Connected: {} (FW: {})", info.product, info.firmware_version_string()),
                        theme::COLOR_SUCCESS,
                    ));
                    self.connected_info = Some(info);
                    self.deck_view.reset_leds();
                    self.bridge.send(crate::bridge::GuiCommand::SetScreenPage(self.deck_view.page as u8 + 1));
                }
                DeviceEvent::Disconnected(reason) => {
                    self.toast_message = Some((
                        format!("Disconnected: {reason}"),
                        theme::COLOR_WARNING,
                    ));
                    self.connected_info = None;
                }
                DeviceEvent::InputUpdate { report, events } => {
                    for hw_ev in &events {
                        if let Some(msg) = self.deck_view.handle_event(hw_ev, &self.audio, &self.bridge) {
                            self.toast_message = Some((msg, theme::COLOR_PRIMARY));
                            self.display_view.active_page = self.deck_view.page as u8 + 1;
                        }
                    }
                    for hw_ev in events {
                        self.monitor_view.push_event(hw_ev);
                    }
                    self.latest_report = Some(report);
                }
                DeviceEvent::CommandSuccess(msg) => {
                    self.toast_message = Some((msg, theme::COLOR_PRIMARY));
                }
                DeviceEvent::CommandError(err) => {
                    self.toast_message = Some((format!("Error: {err}"), theme::COLOR_ERROR));
                }
            }
        }
    }
}

impl eframe::App for StreamControlApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_events();
        if self.connected_info.is_some() {
            self.deck_view.sync_leds(&self.audio, &self.bridge);
        }

        // 50 Hz request repaint for smooth VU meters & faders
        ctx.request_repaint();

        // Top Header Navigation Bar
        egui::TopBottomPanel::top("top_header")
            .frame(
                egui::Frame::group(&ctx.style())
                    .fill(theme::COLOR_BG_PANEL)
                    .stroke(Stroke::new(1.0_f32, theme::COLOR_BORDER_SUBTLE))
                    .inner_margin(egui::Margin::symmetric(16.0, 10.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // App Logo with glowing accent
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                        ui.painter()
                            .circle_filled(rect.center(), 7.0, theme::COLOR_PRIMARY);

                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new("HERCULES STREAM 200 XLR")
                                    .color(theme::COLOR_PRIMARY)
                                    .font(FontId::new(16.0, FontFamily::Proportional))
                                    .strong(),
                            );
                            ui.label(
                                RichText::new("STUDIO LINUX DRIVER & CONTROLLER")
                                    .color(theme::COLOR_TEXT_SECONDARY)
                                    .font(FontId::new(9.0, FontFamily::Proportional)),
                            );
                        });
                    });

                    ui.add_space(24.0);

                    // Navigation Tabs
                    let tabs = [
                        (AppTab::Deck, "🎛️ DECK"),
                        (AppTab::Dsp, "🎙️ VOCAL DSP"),
                        (AppTab::Lighting, "💡 LIGHTING & VU"),
                        (AppTab::Display, "🖥️ LCD STUDIO"),
                        (AppTab::Monitor, "📊 MONITOR"),
                    ];

                    for (tab, label) in tabs {
                        let is_active = self.active_tab == tab;
                        let (fill, stroke, text_color) = if is_active {
                            (
                                theme::COLOR_PLUM_DEEP,
                                Stroke::new(1.5_f32, theme::COLOR_PRIMARY),
                                theme::COLOR_PRIMARY,
                            )
                        } else {
                            (
                                theme::COLOR_BG_CARD,
                                Stroke::new(1.0_f32, theme::COLOR_BORDER_SUBTLE),
                                theme::COLOR_TEXT_SECONDARY,
                            )
                        };

                        let btn = egui::Button::new(
                            RichText::new(label)
                                .color(text_color)
                                .font(FontId::new(12.0, FontFamily::Proportional))
                                .strong(),
                        )
                        .fill(fill)
                        .stroke(stroke)
                        .rounding(Rounding::same(6.0));

                        if ui.add(btn).clicked() {
                            self.active_tab = tab;
                        }
                        ui.add_space(4.0);
                    }

                    // Right Side: Hardware Connection Status Pill
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let (dot_color, status_text) = if let Some(info) = &self.connected_info {
                            (
                                theme::COLOR_SUCCESS,
                                format!("ONLINE (FW {})", info.firmware_version_string()),
                            )
                        } else {
                            (theme::COLOR_WARNING, "SCANNING USB...".to_string())
                        };

                        card_frame(false).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (dot_rect, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                                ui.painter().circle_filled(dot_rect.center(), 4.5, dot_color);

                                ui.label(
                                    RichText::new(status_text)
                                        .color(theme::COLOR_TEXT_PRIMARY)
                                        .font(FontId::new(11.0, FontFamily::Proportional))
                                        .strong(),
                                );
                            });
                        });
                    });
                });
            });

        // Bottom Status / Toast Bar
        egui::TopBottomPanel::bottom("bottom_status")
            .frame(
                egui::Frame::group(&ctx.style())
                    .fill(theme::COLOR_BG_PANEL)
                    .stroke(Stroke::new(1.0_f32, theme::COLOR_BORDER_SUBTLE))
                    .inner_margin(egui::Margin::symmetric(14.0, 6.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some((msg, color)) = &self.toast_message {
                        ui.label(
                            RichText::new(msg)
                                .color(*color)
                                .font(FontId::new(11.0, FontFamily::Proportional))
                                .strong(),
                        );
                    } else {
                        ui.label(
                            RichText::new("Ready | Real-time USB EP0 & Bulk polling at 50 Hz")
                                .color(theme::COLOR_TEXT_MUTED)
                                .font(FontId::new(10.0, FontFamily::Proportional)),
                        );
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new("USB ID 06f8:e054 | AK7735EQ DSP | 480x272 LCD")
                                .color(theme::COLOR_TEXT_MUTED)
                                .font(FontId::new(10.0, FontFamily::Monospace)),
                        );
                    });
                });
            });

        // Main Center Content Area
        egui::CentralPanel::default()
            .frame(
                egui::Frame::group(&ctx.style())
                    .fill(theme::COLOR_BG_CANVAS)
                    .inner_margin(egui::Margin::same(16.0)),
            )
            .show(ctx, |ui| match self.active_tab {
                AppTab::Deck => {
                    self.deck_view.ui(ui, &self.audio, &self.bridge, self.latest_report.as_ref());
                }
                AppTab::Dsp => {
                    self.dsp_view.ui(ui, &self.bridge);
                }
                AppTab::Lighting => {
                    self.lighting_view.ui(ui, &self.bridge);
                }
                AppTab::Display => {
                    self.display_view.ui(ui, &self.bridge);
                }
                AppTab::Monitor => {
                    self.monitor_view.ui(
                        ui,
                        self.latest_report.as_ref(),
                        self.connected_info.as_ref(),
                    );
                }
            });
    }
}
