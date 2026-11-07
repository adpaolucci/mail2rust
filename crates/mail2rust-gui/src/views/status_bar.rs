use crate::state::AppState;
use eframe::egui::{self, Color32, RichText, Stroke};

pub fn render_status_bar(ctx: &egui::Context, state: &AppState) {
    egui::TopBottomPanel::bottom("status_bar")
        .frame(
            egui::Frame::none()
                .fill(Color32::from_rgb(236, 233, 216)) // Win2k/XP face
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(172, 168, 153)))
                .inner_margin(egui::Margin::symmetric(2.0, 2.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);

                // Segment 1: Ready state
                sunken_status_panel(ui, 100.0, |ui| {
                    ui.label(RichText::new("Ready").size(11.0).color(Color32::BLACK));
                });

                // Segment 2: Folder details
                let folder_info = format!("Folder: {}", state.active_folder);
                sunken_status_panel(ui, 140.0, |ui| {
                    ui.label(RichText::new(folder_info).size(11.0).color(Color32::BLACK));
                });

                // Segment 3: Total count
                let msg_info = format!("{} message(s)", state.messages.len());
                sunken_status_panel(ui, 110.0, |ui| {
                    ui.label(RichText::new(msg_info).size(11.0).color(Color32::BLACK));
                });

                // Segment 4: Selected message details
                let sel_info = if let Some(m) = state.get_selected_message() {
                    let kb = m.size_bytes as f64 / 1024.0;
                    format!("Selected: {:.1} KB ({})", kb, m.source_format)
                } else {
                    "None selected".to_string()
                };
                sunken_status_panel(ui, 180.0, |ui| {
                    ui.label(RichText::new(sel_info).size(11.0).color(Color32::BLACK));
                });

                // Segment 5: Engine & Architecture (flexible right)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    sunken_status_panel(ui, 240.0, |ui| {
                        ui.label(
                            RichText::new("100% Pure Rust • Offline • Zero Outlook COM")
                                .size(11.0)
                                .color(Color32::from_rgb(80, 80, 80)),
                        );
                    });
                });
            });
        });
}

fn sunken_status_panel<R>(ui: &mut egui::Ui, width: f32, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let frame = egui::Frame::none()
        .fill(Color32::from_rgb(244, 243, 238))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(172, 168, 153)))
        .inner_margin(egui::Margin::symmetric(4.0, 1.0));

    ui.allocate_ui_with_layout(
        egui::vec2(width, 18.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| frame.show(ui, add_contents).inner,
    )
    .inner
}
