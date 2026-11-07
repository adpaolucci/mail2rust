use crate::state::AppState;
use eframe::egui::{self, Color32, RichText};

pub fn render_status_bar(ctx: &egui::Context, state: &AppState) {
    egui::TopBottomPanel::bottom("status_bar")
        .frame(
            egui::Frame::none()
                .fill(Color32::from_rgb(15, 23, 42))
                .inner_margin(6.0),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("● Engine Ready")
                        .color(Color32::from_rgb(34, 197, 94))
                        .size(11.0),
                );

                ui.separator();

                ui.label(
                    RichText::new(format!("Folder: {}", state.active_folder))
                        .color(Color32::from_rgb(203, 213, 225))
                        .size(11.0),
                );

                ui.separator();

                ui.label(
                    RichText::new(format!("Total Messages: {}", state.messages.len()))
                        .color(Color32::from_rgb(148, 163, 184))
                        .size(11.0),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("Mail2Rust Studio v3.2.0 • 100% Pure Rust Architecture")
                            .color(Color32::from_rgb(100, 116, 139))
                            .size(11.0),
                    );
                });
            });
        });
}
