use egui::{
    epaint::Shadow, vec2, Color32, FontFamily, FontId, Frame, Margin, Rounding, Stroke, Style,
    Visuals,
};

/// Primary branding color: Electric Neon Pink / Magenta (#FF0080).
pub const COLOR_PRIMARY: Color32 = Color32::from_rgb(0xFF, 0x00, 0x80);

/// Hovered neon pink.
pub const COLOR_PRIMARY_HOVER: Color32 = Color32::from_rgb(0xFF, 0x33, 0x99);

/// Active / clicked neon pink.
pub const COLOR_PRIMARY_ACTIVE: Color32 = Color32::from_rgb(0xD9, 0x00, 0x6C);

/// Electric Purple / Violet accent (#9D00FF).
pub const COLOR_ACCENT_PURPLE: Color32 = Color32::from_rgb(0x9D, 0x00, 0xFF);

/// Soft neon lavender (#C77DFF).
pub const COLOR_ACCENT_LAVENDER: Color32 = Color32::from_rgb(0xC7, 0x7D, 0xFF);

/// Deep Plum / Wine (#3D1038).
pub const COLOR_PLUM_DEEP: Color32 = Color32::from_rgb(0x3D, 0x10, 0x38);

/// Abyssal canvas background (#08070D).
pub const COLOR_BG_CANVAS: Color32 = Color32::from_rgb(0x08, 0x07, 0x0D);

/// Level 0 base panel background (#0E0D17).
pub const COLOR_BG_PANEL: Color32 = Color32::from_rgb(0x0E, 0x0D, 0x17);

/// Level 1 sunken well / groove background (#131120).
pub const COLOR_BG_SUNKEN: Color32 = Color32::from_rgb(0x13, 0x11, 0x20);

/// Level 2 elevated card background (#1A172A).
pub const COLOR_BG_CARD: Color32 = Color32::from_rgb(0x1A, 0x17, 0x2A);

/// Level 3 floating / raised surface (#241F3B).
pub const COLOR_BG_RAISED: Color32 = Color32::from_rgb(0x24, 0x1F, 0x3B);

/// Level 4 modal / popup surface (#2E274D).
#[allow(dead_code)]
pub const COLOR_BG_MODAL: Color32 = Color32::from_rgb(0x2E, 0x27, 0x4D);

/// Border stroke subtle (#2B2544).
pub const COLOR_BORDER_SUBTLE: Color32 = Color32::from_rgb(0x2B, 0x25, 0x44);

/// Border stroke focused / highlighted (#FF0080).
#[allow(dead_code)]
pub const COLOR_BORDER_HIGHLIGHT: Color32 = COLOR_PRIMARY;

/// Text primary (White with soft pink-lavender tint).
pub const COLOR_TEXT_PRIMARY: Color32 = Color32::from_rgb(0xF4, 0xEE, 0xFA);

/// Text secondary (Subtle muted lavender-gray).
pub const COLOR_TEXT_SECONDARY: Color32 = Color32::from_rgb(0x9E, 0x95, 0xB6);

/// Text muted (#6A6284).
pub const COLOR_TEXT_MUTED: Color32 = Color32::from_rgb(0x6A, 0x62, 0x84);

/// Success green.
pub const COLOR_SUCCESS: Color32 = Color32::from_rgb(0x00, 0xE6, 0x99);

/// Warning amber.
pub const COLOR_WARNING: Color32 = Color32::from_rgb(0xFF, 0xB8, 0x00);

/// Error red.
pub const COLOR_ERROR: Color32 = Color32::from_rgb(0xFF, 0x33, 0x4B);

/// Applies the dark Cyberpunk / Studio elevated theme to egui.
pub fn apply_theme(ctx: &egui::Context) {
    let mut style = Style::default();
    let mut visuals = Visuals::dark();

    visuals.override_text_color = Some(COLOR_TEXT_PRIMARY);
    visuals.panel_fill = COLOR_BG_CANVAS;
    visuals.window_fill = COLOR_BG_PANEL;
    visuals.extreme_bg_color = COLOR_BG_SUNKEN;
    visuals.faint_bg_color = COLOR_BG_CARD;

    // Widgets inactive
    visuals.widgets.inactive.bg_fill = COLOR_BG_CARD;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, COLOR_BORDER_SUBTLE);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, COLOR_TEXT_SECONDARY);
    visuals.widgets.inactive.rounding = Rounding::same(6.0);

    // Widgets hovered
    visuals.widgets.hovered.bg_fill = COLOR_BG_RAISED;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.2_f32, COLOR_PRIMARY);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.2_f32, COLOR_TEXT_PRIMARY);
    visuals.widgets.hovered.rounding = Rounding::same(6.0);

    // Widgets active / pressed
    visuals.widgets.active.bg_fill = COLOR_PLUM_DEEP;
    visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, COLOR_PRIMARY_ACTIVE);
    visuals.widgets.active.fg_stroke = Stroke::new(1.5_f32, Color32::WHITE);
    visuals.widgets.active.rounding = Rounding::same(6.0);

    // Widgets open
    visuals.widgets.open.bg_fill = COLOR_BG_RAISED;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, COLOR_ACCENT_PURPLE);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, COLOR_TEXT_PRIMARY);

    // Selection
    visuals.selection.bg_fill = Color32::from_rgba_premultiplied(0xFF, 0x00, 0x80, 0x40);
    visuals.selection.stroke = Stroke::new(1.0_f32, COLOR_PRIMARY);

    // Sliders & Handles
    visuals.slider_trailing_fill = true;

    // Window shadow with soft purple glow
    visuals.window_shadow = Shadow {
        offset: vec2(0.0, 8.0),
        blur: 24.0,
        spread: 0.0,
        color: Color32::from_rgba_premultiplied(0x3D, 0x10, 0x38, 0x80),
    };

    style.visuals = visuals;
    style.spacing.item_spacing = vec2(8.0, 8.0);
    style.spacing.button_padding = vec2(10.0, 6.0);

    ctx.set_style(style);
}

/// Helper to render an elevated card with subtle drop shadow and borders.
pub fn card_frame(highlighted: bool) -> Frame {
    let stroke = if highlighted {
        Stroke::new(1.5_f32, COLOR_PRIMARY)
    } else {
        Stroke::new(1.0_f32, COLOR_BORDER_SUBTLE)
    };

    let shadow_color = if highlighted {
        Color32::from_rgba_premultiplied(0xFF, 0x00, 0x80, 0x30)
    } else {
        Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0x90)
    };

    Frame::group(&Style::default())
        .fill(COLOR_BG_CARD)
        .stroke(stroke)
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::same(12.0))
        .shadow(Shadow {
            offset: vec2(0.0, 4.0),
            blur: 16.0,
            spread: 0.0,
            color: shadow_color,
        })
}

/// Helper for sunken grooves / well tracks (e.g. for faders or meters).
pub fn sunken_frame() -> Frame {
    Frame::group(&Style::default())
        .fill(COLOR_BG_SUNKEN)
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0x1B, 0x17, 0x2A)))
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::same(8.0))
}

/// Renders a neon glowing section header.
pub fn section_header(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(4.0, 24.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, Rounding::same(2.0), COLOR_PRIMARY);

        ui.vertical(|ui| {
            ui.heading(
                egui::RichText::new(title)
                    .color(COLOR_TEXT_PRIMARY)
                    .font(FontId::new(16.0, FontFamily::Proportional))
                    .strong(),
            );
            if !subtitle.is_empty() {
                ui.label(
                    egui::RichText::new(subtitle)
                        .color(COLOR_TEXT_SECONDARY)
                        .font(FontId::new(11.0, FontFamily::Proportional)),
                );
            }
        });
    });
}
