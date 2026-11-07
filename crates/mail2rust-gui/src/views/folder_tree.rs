use crate::state::AppState;
use eframe::egui::{self, Color32, RichText, ScrollArea};

pub fn render_folder_tree(ui: &mut egui::Ui, state: &mut AppState) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("📁 Folders").size(16.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("All").clicked() {
                    for f in &mut state.folders {
                        f.selected = true;
                    }
                }
            });
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        ScrollArea::vertical().show(ui, |ui| {
            let mut newly_selected_folder = None;

            for folder in &mut state.folders {
                let is_active = state.active_folder == folder.name;
                let bg_color = if is_active {
                    Color32::from_rgb(30, 41, 59)
                } else {
                    Color32::TRANSPARENT
                };

                let frame = egui::Frame::none().fill(bg_color).inner_margin(4.0).rounding(4.0_f32);
                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut folder.selected, "");

                        let icon = if folder.name.contains("Inbox") {
                            "📥"
                        } else if folder.name.contains("Sent") {
                            "📤"
                        } else if folder.name.contains("Deleted") || folder.name.contains("Trash") {
                            "🗑"
                        } else if folder.name.contains("Archive") {
                            "📦"
                        } else {
                            "📁"
                        };

                        let text_color = if is_active {
                            Color32::from_rgb(96, 165, 250)
                        } else {
                            Color32::from_rgb(226, 232, 240)
                        };

                        let label_resp = ui.selectable_label(
                            is_active,
                            RichText::new(format!("{} {}", icon, folder.name))
                                .color(text_color)
                                .size(13.0),
                        );

                        if label_resp.clicked() {
                            newly_selected_folder = Some(folder.name.clone());
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if folder.unread_count > 0 {
                                let badge = RichText::new(format!("{}", folder.unread_count))
                                    .color(Color32::WHITE)
                                    .size(11.0)
                                    .strong();
                                ui.label(badge);
                            } else {
                                ui.label(
                                    RichText::new(format!("{}", folder.message_count))
                                        .color(Color32::from_rgb(148, 163, 184))
                                        .size(11.0),
                                );
                            }
                        });
                    });
                });
                ui.add_space(2.0);
            }

            if let Some(f) = newly_selected_folder {
                state.active_folder = f;
            }
        });
    });
}
