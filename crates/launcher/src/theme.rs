use eframe::egui::{self, Color32, Stroke, Style, Visuals};

pub const COLOR_BG: Color32 = Color32::from_rgb(16, 14, 18);
pub const COLOR_PANEL: Color32 = Color32::from_rgb(24, 22, 28);
pub const COLOR_SURFACE: Color32 = Color32::from_rgb(34, 30, 38);
pub const COLOR_CRIMSON: Color32 = Color32::from_rgb(186, 24, 38);
pub const COLOR_CRIMSON_HOVER: Color32 = Color32::from_rgb(215, 36, 52);
pub const COLOR_CRIMSON_ACTIVE: Color32 = Color32::from_rgb(148, 16, 28);
pub const COLOR_BORDER: Color32 = Color32::from_rgb(64, 36, 44);
pub const COLOR_TEXT_PRIMARY: Color32 = Color32::from_rgb(245, 240, 245);
pub const COLOR_TEXT_MUTED: Color32 = Color32::from_rgb(160, 148, 160);
pub const COLOR_GOLD: Color32 = Color32::from_rgb(214, 172, 80);

pub fn apply_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    visuals.override_text_color = Some(COLOR_TEXT_PRIMARY);
    visuals.panel_fill = COLOR_BG;
    visuals.window_fill = COLOR_PANEL;
    visuals.faint_bg_color = COLOR_PANEL;
    visuals.extreme_bg_color = Color32::from_rgb(12, 10, 14);

    visuals.widgets.noninteractive.bg_fill = COLOR_PANEL;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, COLOR_BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, COLOR_TEXT_MUTED);

    visuals.widgets.inactive.bg_fill = COLOR_SURFACE;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, COLOR_BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, COLOR_TEXT_PRIMARY);

    visuals.widgets.hovered.bg_fill = COLOR_CRIMSON_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, COLOR_CRIMSON_HOVER);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);

    visuals.widgets.active.bg_fill = COLOR_CRIMSON_ACTIVE;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, COLOR_CRIMSON);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);

    visuals.selection.bg_fill = COLOR_CRIMSON;
    visuals.selection.stroke = Stroke::new(1.0_f32, COLOR_CRIMSON_HOVER);

    let mut style = Style {
        visuals,
        ..Style::default()
    };
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(16.0, 8.0);
    ctx.set_style(style);
}
