use eframe::egui::{self, Color32, Stroke, Vec2};

pub const ACCENT: Color32 = Color32::from_rgb(64, 220, 180);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(38, 120, 104);
pub const PURPLE: Color32 = Color32::from_rgb(142, 132, 255);
pub const BG: Color32 = Color32::from_rgb(16, 18, 24);
pub const SURFACE: Color32 = Color32::from_rgb(24, 27, 35);
pub const SURFACE_ALT: Color32 = Color32::from_rgb(31, 36, 47);
pub const BORDER: Color32 = Color32::from_rgb(43, 49, 64);
pub const TEXT: Color32 = Color32::from_rgb(226, 232, 240);
pub const MUTED: Color32 = Color32::from_rgb(139, 149, 167);
pub const OK: Color32 = Color32::from_rgb(74, 222, 128);
pub const WARN: Color32 = Color32::from_rgb(250, 204, 21);
pub const ERROR: Color32 = Color32::from_rgb(248, 113, 113);

pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = SURFACE;
    visuals.extreme_bg_color = Color32::from_rgb(11, 13, 18);
    visuals.faint_bg_color = SURFACE_ALT;
    visuals.selection.bg_fill = ACCENT_DIM;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.noninteractive.bg_fill = SURFACE;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.bg_fill = SURFACE_ALT;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(40, 47, 61);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_DIM);
    visuals.widgets.active.bg_fill = ACCENT_DIM;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    ctx.set_theme(egui::Theme::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, visuals);

    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = Vec2::new(10.0, 8.0);
        style.spacing.button_padding = Vec2::new(12.0, 7.0);
        style.spacing.interact_size.y = 30.0;
    });
}

pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(14))
        .show(ui, add)
        .inner
}

pub fn battery_color(battery: u8) -> Color32 {
    match battery {
        60..=100 => OK,
        25..=59 => WARN,
        _ => ERROR,
    }
}

/// Japanese-capable fallback font. The default egui fonts cover Latin and
/// emoji only, so every Japanese label would otherwise render as tofu.
const JP_FONT: &[u8] = include_bytes!("../assets/fonts/ipaexg.ttf");

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "ipaexg".to_owned(),
        egui::FontData::from_static(JP_FONT).into(),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("ipaexg".to_owned());
    }
    ctx.set_fonts(fonts);
}
