use crate::state::AppState;
use eframe::egui::{self, Color32, RichText, ScrollArea, Stroke};

pub fn render_message_list(ui: &mut egui::Ui, state: &mut AppState) {
    ui.vertical(|ui| {
        // Search header
        ui.horizontal(|ui| {
            ui.label(RichText::new("🔍").size(14.0));
            ui.add(
                egui::TextEdit::singleline(&mut state.search_query)
                    .hint_text("Search emails by subject, sender, or content...")
                    .desired_width(f32::INFINITY),
            );
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        let filtered = state.filtered_messages().into_iter().cloned().collect::<Vec<_>>();

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "Showing {} messages in {}",
                    filtered.len(),
                    state.active_folder
                ))
                .color(Color32::from_rgb(148, 163, 184))
                .size(11.0),
            );
        });

        ui.add_space(4.0);

        let mut newly_selected_id = None;

        ScrollArea::vertical().show(ui, |ui| {
            for msg in filtered {
                let is_selected = state.selected_message_id.as_deref() == Some(&msg.id);
                let bg_color = if is_selected {
                    Color32::from_rgb(30, 58, 138) // Dark blue
                } else if !msg.flags.read {
                    Color32::from_rgb(30, 41, 59)
                } else {
                    Color32::from_rgb(15, 23, 42)
                };

                let border_stroke = if is_selected {
                    Stroke::new(1.0_f32, Color32::from_rgb(96, 165, 250))
                } else {
                    Stroke::new(1.0_f32, Color32::from_rgb(51, 65, 85))
                };

                let frame = egui::Frame::none()
                    .fill(bg_color)
                    .stroke(border_stroke)
                    .inner_margin(6.0)
                    .rounding(4.0_f32);

                let resp = frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    // Top row: Flags + Sender + Date
                    ui.horizontal(|ui| {
                        let status_icon = if !msg.flags.read { "✉" } else { "📭" };
                        ui.label(RichText::new(status_icon).size(12.0));

                        if msg.flags.flagged {
                            ui.label(RichText::new("⭐").size(11.0));
                        }

                        if !msg.attachments.is_empty() {
                            ui.label(RichText::new("📎").size(11.0));
                        }

                        if msg.legal_hold {
                            ui.label(RichText::new("🔒").size(11.0));
                        }

                        let sender_style = if !msg.flags.read {
                            RichText::new(&msg.from.name).color(Color32::WHITE).strong()
                        } else {
                            RichText::new(&msg.from.name).color(Color32::from_rgb(203, 213, 225))
                        };
                        ui.label(sender_style);

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let date_str = msg.date.format("%b %d, %H:%M").to_string();
                            ui.label(
                                RichText::new(date_str)
                                    .color(Color32::from_rgb(148, 163, 184))
                                    .size(11.0),
                            );
                        });
                    });

                    ui.add_space(2.0);

                    // Subject line
                    let subj_text = if msg.subject.is_empty() {
                        "(No Subject)"
                    } else {
                        &msg.subject
                    };
                    let subj_style = if !msg.flags.read {
                        RichText::new(subj_text).color(Color32::WHITE).strong().size(13.0)
                    } else {
                        RichText::new(subj_text)
                            .color(Color32::from_rgb(226, 232, 240))
                            .size(13.0)
                    };
                    ui.label(subj_style);

                    ui.add_space(2.0);

                    // Snippet preview
                    let snippet: String = msg.body_text.chars().take(80).collect();
                    ui.label(
                        RichText::new(format!("{}...", snippet))
                            .color(Color32::from_rgb(148, 163, 184))
                            .size(11.0),
                    );
                });

                if resp.response.interact(egui::Sense::click()).clicked() {
                    newly_selected_id = Some(msg.id.clone());
                }

                ui.add_space(4.0);
            }

            if let Some(id) = newly_selected_id {
                state.selected_message_id = Some(id);
            }
        });
    });
}
