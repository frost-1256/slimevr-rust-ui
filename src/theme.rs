use std::collections::HashMap;

use eframe::egui::{self, Color32, Stroke, Vec2};
use egui_material3::theme::{self as m3, ContrastLevel, MaterialThemeContext, ThemeMode};

pub const ACCENT: Color32 = Color32::from_rgb(64, 220, 180);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(38, 120, 104);
pub const PURPLE: Color32 = Color32::from_rgb(200, 194, 255);
pub const BG: Color32 = Color32::from_rgb(16, 18, 24);
pub const SURFACE: Color32 = Color32::from_rgb(24, 27, 35);
pub const SURFACE_ALT: Color32 = Color32::from_rgb(31, 36, 47);
pub const BORDER: Color32 = Color32::from_rgb(76, 85, 102);
pub const TEXT: Color32 = Color32::from_rgb(226, 232, 240);
pub const MUTED: Color32 = Color32::from_rgb(154, 163, 180);
pub const OK: Color32 = Color32::from_rgb(94, 226, 154);
pub const WARN: Color32 = Color32::from_rgb(255, 209, 102);
pub const ERROR: Color32 = Color32::from_rgb(255, 138, 128);

pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    install_material_theme();

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = SURFACE;
    visuals.extreme_bg_color = Color32::from_rgb(11, 13, 18);
    visuals.faint_bg_color = SURFACE_ALT;
    visuals.selection.bg_fill = ACCENT_DIM;
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.noninteractive.bg_fill = SURFACE;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.inactive.bg_fill = SURFACE_ALT;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(42, 49, 63);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT_DIM);
    visuals.widgets.active.bg_fill = ACCENT_DIM;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.hyperlink_color = ACCENT;
    ctx.set_visuals(visuals);

    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = Vec2::new(10.0, 10.0);
        style.spacing.button_padding = Vec2::new(14.0, 9.0);
        style.spacing.interact_size.y = 34.0;
        style.spacing.slider_width = 200.0;
        style.spacing.indent = 20.0;
        style.spacing.scroll.bar_width = 10.0;
        style.spacing.menu_margin = egui::Margin::same(8);
        style.visuals.striped = false;

        // Material 3 type scale, tuned for CJK: body 16 (was 14), headings 23.
        style.text_styles = [
            (egui::TextStyle::Heading, egui::FontId::proportional(23.0)),
            (egui::TextStyle::Body, egui::FontId::proportional(16.0)),
            (egui::TextStyle::Button, egui::FontId::proportional(16.0)),
            (egui::TextStyle::Small, egui::FontId::proportional(14.0)),
            (egui::TextStyle::Monospace, egui::FontId::monospace(15.0)),
        ]
        .into();
    });
}

pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0_f32, SURFACE_ALT))
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin::same(16))
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

/// A Material 3 button that respects egui's disabled scope: `ui.add_enabled`
/// turns the button gray instead of leaving it looking clickable.
pub struct Button(egui_material3::MaterialButton<'static>);

impl Button {
    pub fn text_color(mut self, color: Color32) -> Self {
        self.0 = self.0.text_color(color);
        self
    }
}

impl egui::Widget for Button {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let enabled = ui.is_enabled();
        ui.add(self.0.enabled(enabled))
    }
}

/// A filled/prominent action.
pub fn primary_button(text: impl Into<egui::WidgetText>) -> Button {
    Button(egui_material3::MaterialButton::filled(text))
}

/// A low-emphasis action that still needs a visible boundary.
pub fn outlined_button(text: impl Into<egui::WidgetText>) -> Button {
    Button(egui_material3::MaterialButton::outlined(text))
}

/// A borderless action for dense rows.
pub fn text_button(text: impl Into<egui::WidgetText>) -> Button {
    Button(egui_material3::MaterialButton::text(text))
}

/// Material 3 switch; use for "on/off" settings rather than multi-select.
pub fn switch(ui: &mut egui::Ui, value: &mut bool, label: &str) -> egui::Response {
    ui.add(egui_material3::MaterialSwitch::new(value).text(label))
}

/// Material 3 checkbox, for lists of independent items.
pub fn checkbox(ui: &mut egui::Ui, value: &mut bool, label: &str) -> egui::Response {
    ui.add(egui_material3::MaterialCheckbox::new(value, label))
}

/// Material 3 snackbar-style toast background.
pub const TOAST_INFO: Color32 = Color32::from_rgb(45, 53, 68);

/// Japanese text font (Noto Sans JP, instanced at 400 and subset) and the
/// Material Symbols icon font. Both are embedded so the UI looks the same on a
/// clean Windows install with no CJK fonts.
const JP_FONT: &[u8] = include_bytes!("../assets/fonts/NotoSansJP-Regular.ttf");
const ICON_FONT: &[u8] = include_bytes!("../assets/fonts/MaterialSymbolsOutlined.ttf");

fn install_fonts(ctx: &egui::Context) {
    m3::setup_local_fonts_from_bytes("NotoSansJP", JP_FONT);
    m3::setup_local_fonts_from_bytes("MaterialSymbolsOutlined", ICON_FONT);
    m3::load_fonts(ctx);
}

/// Point the Material 3 components at the SlimeVR palette instead of the
/// library's green default, and pin the dark scheme.
fn install_material_theme() {
    let mut theme = MaterialThemeContext {
        theme_mode: ThemeMode::Dark,
        contrast_level: ContrastLevel::Normal,
        ..Default::default()
    };

    let mut colors: HashMap<String, Color32> = HashMap::new();
    let mut set = |name: &str, color: Color32| {
        colors.insert(name.to_string(), color);
    };
    set("primary", ACCENT);
    set("onPrimary", Color32::from_rgb(0, 33, 26));
    set("primaryContainer", Color32::from_rgb(14, 77, 64));
    set("onPrimaryContainer", Color32::from_rgb(160, 242, 221));
    set("surfaceTint", ACCENT);
    set("secondary", PURPLE);
    set("onSecondary", Color32::from_rgb(28, 23, 62));
    set("secondaryContainer", Color32::from_rgb(52, 45, 99));
    set("onSecondaryContainer", Color32::from_rgb(228, 223, 255));
    set("tertiary", Color32::from_rgb(255, 180, 168));
    set("onTertiary", Color32::from_rgb(70, 12, 4));
    set("tertiaryContainer", Color32::from_rgb(104, 33, 20));
    set("onTertiaryContainer", Color32::from_rgb(255, 218, 211));
    set("error", Color32::from_rgb(255, 180, 171));
    set("onError", Color32::from_rgb(105, 0, 5));
    set("errorContainer", Color32::from_rgb(147, 0, 10));
    set("onErrorContainer", Color32::from_rgb(255, 218, 214));
    set("background", BG);
    set("onBackground", TEXT);
    set("surface", SURFACE);
    set("onSurface", TEXT);
    set("surfaceVariant", SURFACE_ALT);
    set("onSurfaceVariant", MUTED);
    set("outline", BORDER);
    set("outlineVariant", SURFACE_ALT);
    set("inverseSurface", TEXT);
    set("inverseOnSurface", BG);
    set("inversePrimary", ACCENT_DIM);
    set("surfaceDim", BG);
    set("surfaceBright", SURFACE_ALT);
    set("surfaceContainerLowest", Color32::from_rgb(11, 13, 18));
    set("surfaceContainerLow", Color32::from_rgb(21, 24, 31));
    set("surfaceContainer", SURFACE);
    set("surfaceContainerHigh", SURFACE_ALT);
    set("surfaceContainerHighest", Color32::from_rgb(37, 43, 55));
    theme.selected_colors = colors;

    m3::update_global_theme(theme);
}
