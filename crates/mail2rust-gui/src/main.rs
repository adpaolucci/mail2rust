mod app;
mod state;
mod views;

use app::Mail2RustApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1380.0, 850.0])
            .with_min_inner_size([960.0, 600.0])
            .with_title("Mail2Rust Studio - 100% Pure Rust Email Engine"),
        ..Default::default()
    };

    eframe::run_native(
        "Mail2Rust Studio",
        native_options,
        Box::new(|cc| {
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = egui::Color32::from_rgb(15, 23, 42);
            visuals.window_fill = egui::Color32::from_rgb(15, 23, 42);
            visuals.window_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(51, 65, 85));
            cc.egui_ctx.set_visuals(visuals);

            Ok(Box::new(Mail2RustApp::default()))
        }),
    )
}
