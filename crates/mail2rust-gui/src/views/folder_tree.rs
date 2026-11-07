use crate::state::AppState;
use eframe::egui::{self, Color32, RichText, ScrollArea, Stroke};

pub fn render_folder_tree(ui: &mut egui::Ui, state: &mut AppState) {
    ui.vertical(|ui| {
        // Classic Win32 Tree Header
        egui::Frame::none()
            .fill(Color32::from_rgb(235, 235, 235))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(180, 180, 180)))
            .inner_margin(egui::Margin::symmetric(4.0, 3.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Console Tree").size(11.0).strong().color(Color32::BLACK));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("All").clicked() {
                            for f in &mut state.folders {
                                f.selected = true;
                            }
                        }
                        if ui.small_button("None").clicked() {
                            for f in &mut state.folders {
                                f.selected = false;
                            }
                        }
                    });
                });
            });

        ui.add_space(2.0);

        // Sunken White TreeView Control
        egui::Frame::none()
            .fill(Color32::WHITE)
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(127, 157, 185))) // #7F9DB9 Classic Windows edit/tree border
            .inner_margin(egui::Margin::symmetric(4.0, 4.0))
            .show(ui, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    // Root Node
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("[-] 🖥").size(12.0));
                        ui.label(RichText::new("Local Mailbox Store").size(12.0).strong().color(Color32::BLACK));
                    });

                    ui.add_space(2.0);

                    let mut newly_selected_folder = None;

                    // "All Mail" virtual folder node
                    {
                        let is_active = state.active_folder == "All Mail";
                        let bg = if is_active { Color32::from_rgb(49, 106, 197) } else { Color32::TRANSPARENT };
                        let fg = if is_active { Color32::WHITE } else { Color32::BLACK };

                        let row = egui::Frame::none().fill(bg).inner_margin(egui::Margin::symmetric(4.0, 2.0));
                        row.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("   📁").size(11.0).color(fg));
                                if ui.selectable_label(is_active, RichText::new("All Messages").size(12.0).color(fg)).clicked() {
                                    newly_selected_folder = Some("All Mail".to_string());
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new(format!("({})", state.messages.len())).size(11.0).color(if is_active { Color32::WHITE } else { Color32::from_rgb(120, 120, 120) }));
                                });
                            });
                        });
                    }

                    // Folder Nodes
                    if state.folders.is_empty() {
                        ui.horizontal(|ui| {
                            ui.add_space(20.0);
                            ui.label(RichText::new("(No archive loaded)").italics().size(11.0).color(Color32::from_rgb(130, 130, 130)));
                        });
                    }

                    for folder in &mut state.folders {
                        let is_active = state.active_folder == folder.name;
                        let bg = if is_active {
                            Color32::from_rgb(49, 106, 197) // Classic Win32 highlight
                        } else {
                            Color32::TRANSPARENT
                        };
                        let fg = if is_active { Color32::WHITE } else { Color32::BLACK };

                        let row_frame = egui::Frame::none().fill(bg).inner_margin(egui::Margin::symmetric(2.0, 1.0));
                        row_frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
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
                                    "🗀"
                                };

                                let name_text = if folder.unread_count > 0 && !is_active {
                                    RichText::new(format!("{} {}", icon, folder.name)).size(12.0).strong().color(fg)
                                } else {
                                    RichText::new(format!("{} {}", icon, folder.name)).size(12.0).color(fg)
                                };

                                if ui.selectable_label(is_active, name_text).clicked() {
                                    newly_selected_folder = Some(folder.name.clone());
                                }

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let count_str = if folder.unread_count > 0 {
                                        format!("({} / {})", folder.unread_count, folder.message_count)
                                    } else {
                                        format!("({})", folder.message_count)
                                    };
                                    let count_fg = if is_active {
                                        Color32::WHITE
                                    } else if folder.unread_count > 0 {
                                        Color32::from_rgb(0, 51, 153)
                                    } else {
                                        Color32::from_rgb(120, 120, 120)
                                    };
                                    ui.label(RichText::new(count_str).size(11.0).color(count_fg));
                                });
                            });
                        });
                    }

                    if let Some(f) = newly_selected_folder {
                        state.active_folder = f;
                    }
                });
            });
    });
}
