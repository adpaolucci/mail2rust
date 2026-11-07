mod app;
mod state;
mod views;

use app::Mail2RustApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 780.0])
            .with_min_inner_size([880.0, 520.0])
            .with_title("Mail2Rust Management Console - Universal Email & Mailbox Studio"),
        ..Default::default()
    };

    eframe::run_native(
        "Mail2Rust Management Console",
        native_options,
        Box::new(|cc| {
            let mut visuals = egui::Visuals::light();

            // Classic Win32 / MMC / Windows 2000 & XP Neutral Palette
            let bg_face = egui::Color32::from_rgb(238, 238, 238);
            let border_color = egui::Color32::from_rgb(160, 160, 160);
            let highlight_blue = egui::Color32::from_rgb(49, 106, 197);

            visuals.panel_fill = bg_face;
            visuals.window_fill = bg_face;
            visuals.window_stroke = egui::Stroke::new(1.0_f32, border_color);
            visuals.window_rounding = egui::Rounding::same(2.0_f32);

            visuals.widgets.noninteractive.bg_fill = bg_face;
            visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::BLACK);
            visuals.widgets.noninteractive.rounding = egui::Rounding::same(1.0_f32);

            visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(244, 244, 244);
            visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::BLACK);
            visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(172, 172, 172));
            visuals.widgets.inactive.rounding = egui::Rounding::same(2.0_f32);

            visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(229, 241, 251);
            visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::BLACK);
            visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(0, 120, 215));
            visuals.widgets.hovered.rounding = egui::Rounding::same(2.0_f32);

            visuals.widgets.active.bg_fill = egui::Color32::from_rgb(204, 228, 247);
            visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::BLACK);
            visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(0, 84, 153));
            visuals.widgets.active.rounding = egui::Rounding::same(2.0_f32);

            visuals.selection.bg_fill = highlight_blue;
            visuals.selection.stroke = egui::Stroke::new(1.0_f32, highlight_blue);

            let mut style = (*cc.egui_ctx.style()).clone();
            style.spacing.item_spacing = egui::vec2(4.0, 3.0);
            style.spacing.button_padding = egui::vec2(6.0, 3.0);
            style.spacing.window_margin = egui::Margin::same(6.0);
            style.visuals = visuals;

            cc.egui_ctx.set_style(style);

            Ok(Box::new(Mail2RustApp::default()))
        }),
    )
}
