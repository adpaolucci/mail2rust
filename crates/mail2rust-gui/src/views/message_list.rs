use crate::state::{AppState, SortColumn, SortDirection};
use eframe::egui::{self, Color32, RichText, ScrollArea, Stroke, Vec2};
use mail2rust_core::Importance;

pub fn render_message_list(ui: &mut egui::Ui, state: &mut AppState) {
    ui.vertical(|ui| {
        let filtered = state.filtered_messages().into_iter().cloned().collect::<Vec<_>>();
        let selected_count = state.selected_message_ids.len();

        // Subheader showing current folder item count and selection
        egui::Frame::none()
            .fill(Color32::from_rgb(235, 235, 235))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(180, 180, 180)))
            .inner_margin(egui::Margin::symmetric(4.0, 3.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "Folder: {}  ({} messages displayed | {} selected)",
                            state.active_folder,
                            filtered.len(),
                            selected_count
                        ))
                        .size(11.0)
                        .strong()
                        .color(Color32::BLACK),
                    );
                });
            });

        ui.add_space(2.0);

        // Win32 ListView Column Headers
        let mut toggle_all_selection = false;
        egui::Frame::none()
            .fill(Color32::from_rgb(236, 233, 216))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(172, 168, 153)))
            .inner_margin(egui::Margin::symmetric(2.0, 1.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);

                    // Checkbox column header (toggles select all/none)
                    let check_header_label = if !filtered.is_empty() && filtered.iter().all(|m| state.is_message_selected(&m.id)) {
                        "[X]"
                    } else if filtered.iter().any(|m| state.is_message_selected(&m.id)) {
                        "[-]"
                    } else {
                        "[ ]"
                    };

                    let btn_all = egui::Button::new(RichText::new(check_header_label).size(10.0).strong().color(Color32::BLACK))
                        .fill(Color32::from_rgb(238, 235, 222))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(172, 168, 153)))
                        .rounding(1.0_f32)
                        .min_size(Vec2::new(24.0, 20.0));
                    if ui.add(btn_all).clicked() {
                        toggle_all_selection = true;
                    }

                    if list_header_button(ui, "!", 24.0, state.sort_column == SortColumn::Importance, state.sort_direction) {
                        state.toggle_sort(SortColumn::Importance);
                    }
                    if list_header_button(ui, "F", 28.0, state.sort_column == SortColumn::Flags, state.sort_direction) {
                        state.toggle_sort(SortColumn::Flags);
                    }
                    if list_header_button(ui, "From", 130.0, state.sort_column == SortColumn::From, state.sort_direction) {
                        state.toggle_sort(SortColumn::From);
                    }
                    if list_header_button(ui, "Subject", 180.0, state.sort_column == SortColumn::Subject, state.sort_direction) {
                        state.toggle_sort(SortColumn::Subject);
                    }
                    if list_header_button(ui, "Received", 110.0, state.sort_column == SortColumn::Date, state.sort_direction) {
                        state.toggle_sort(SortColumn::Date);
                    }
                    if list_header_button(ui, "Size", 60.0, state.sort_column == SortColumn::Size, state.sort_direction) {
                        state.toggle_sort(SortColumn::Size);
                    }
                });
            });

        if toggle_all_selection {
            let all_selected = !filtered.is_empty() && filtered.iter().all(|m| state.is_message_selected(&m.id));
            if all_selected {
                state.clear_selection();
            } else {
                state.select_all_filtered();
            }
        }

        ui.add_space(1.0);

        // Sunken White Report ListView Body
        let mut newly_selected_id = None;
        let mut toggle_id = None;

        egui::Frame::none()
            .fill(Color32::WHITE)
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(127, 157, 185))) // #7F9DB9
            .inner_margin(egui::Margin::symmetric(1.0, 1.0))
            .show(ui, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    if filtered.is_empty() {
                        ui.add_space(40.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new("No mailbox archive or messages loaded.").size(13.0).strong().color(Color32::from_rgb(90, 90, 90)));
                            ui.add_space(6.0);
                            ui.label(RichText::new("Open an archive (PST, OST, MBOX, Maildir, EML) or connect to an IMAP server to begin.").size(11.0).color(Color32::from_rgb(120, 120, 120)));
                            ui.add_space(14.0);
                            ui.horizontal(|ui| {
                                ui.add_space((ui.available_width() - 320.0).max(0.0) / 2.0);
                                if ui.button(RichText::new("Open Archive...  (Ctrl+O)").size(11.0)).clicked() {
                                    state.active_modal = crate::state::ActiveModal::OpenMailbox;
                                }
                                if ui.button(RichText::new("Connect to IMAP...  (Ctrl+M)").size(11.0)).clicked() {
                                    state.active_modal = crate::state::ActiveModal::ImapConnect;
                                }
                                if ui.button(RichText::new("Load Sample Data").size(11.0)).clicked() {
                                    state.load_sample_data();
                                }
                            });
                        });
                    }

                    for (idx, msg) in filtered.iter().enumerate() {
                        let is_active_preview = state.selected_message_id.as_deref() == Some(&msg.id);
                        let is_checked = state.is_message_selected(&msg.id);

                        let bg_color = if is_active_preview {
                            Color32::from_rgb(49, 106, 197) // Classic Win32 selection blue #316AC5
                        } else if is_checked {
                            Color32::from_rgb(228, 238, 250) // Checked light blue tint
                        } else if idx % 2 == 1 {
                            Color32::from_rgb(248, 248, 248) // Subtle zebra striping
                        } else {
                            Color32::WHITE
                        };

                        let text_color = if is_active_preview {
                            Color32::WHITE
                        } else if !msg.flags.read {
                            Color32::BLACK
                        } else {
                            Color32::from_rgb(50, 50, 50)
                        };

                        let row_frame = egui::Frame::none()
                            .fill(bg_color)
                            .inner_margin(egui::Margin::symmetric(2.0, 2.0));

                        let resp = row_frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

                                // Checkbox button
                                let check_glyph = if is_checked { "[X]" } else { "[ ]" };
                                let check_resp = ui.add_sized(
                                    [22.0, 18.0],
                                    egui::Button::new(
                                        RichText::new(check_glyph)
                                            .size(9.0)
                                            .strong()
                                            .color(if is_active_preview { Color32::WHITE } else { Color32::BLACK }),
                                    )
                                    .fill(if is_active_preview { Color32::from_rgb(30, 80, 160) } else { Color32::from_rgb(240, 240, 240) })
                                    .rounding(1.0_f32),
                                );
                                if check_resp.clicked() {
                                    toggle_id = Some(msg.id.clone());
                                }

                                // Importance
                                let imp_str = match msg.importance {
                                    Importance::High => "!",
                                    Importance::Low => "v",
                                    Importance::Normal => " ",
                                };
                                ui.add_sized([20.0, 18.0], egui::Label::new(RichText::new(imp_str).size(11.0).strong().color(if msg.importance == Importance::High { Color32::RED } else { text_color })));

                                // Flags (unread, attachment, flagged)
                                let flag_str = format!(
                                    "{}{}{}",
                                    if !msg.flags.read { "U" } else { " " },
                                    if !msg.attachments.is_empty() { "@" } else { " " },
                                    if msg.flags.flagged { "*" } else { " " }
                                );
                                ui.add_sized([26.0, 18.0], egui::Label::new(RichText::new(flag_str).size(10.0).color(text_color)));

                                // Sender Name / Email
                                let from_text = if !msg.flags.read && !is_active_preview {
                                    RichText::new(&msg.from.name).size(12.0).strong().color(text_color)
                                } else {
                                    RichText::new(&msg.from.name).size(12.0).color(text_color)
                                };
                                ui.add_sized([126.0, 18.0], egui::Label::new(from_text).truncate());

                                // Subject
                                let subj_str = if msg.subject.is_empty() { "(No Subject)" } else { &msg.subject };
                                let subj_text = if !msg.flags.read && !is_active_preview {
                                    RichText::new(subj_str).size(12.0).strong().color(text_color)
                                } else {
                                    RichText::new(subj_str).size(12.0).color(text_color)
                                };
                                ui.add_sized([176.0, 18.0], egui::Label::new(subj_text).truncate());

                                // Date
                                let date_str = msg.date.format("%Y-%m-%d %H:%M").to_string();
                                ui.add_sized([106.0, 18.0], egui::Label::new(RichText::new(date_str).size(11.0).color(text_color)));

                                // Size
                                let size_str = format_size(msg.size_bytes);
                                ui.add_sized([56.0, 18.0], egui::Label::new(RichText::new(size_str).size(11.0).color(text_color)));
                            });
                        });

                        if resp.response.interact(egui::Sense::click()).clicked() {
                            newly_selected_id = Some(msg.id.clone());
                        }
                    }
                });
            });

        if let Some(id) = toggle_id {
            state.toggle_message_selection(&id);
        }
        if let Some(id) = newly_selected_id {
            state.selected_message_id = Some(id);
        }
    });
}

fn list_header_button(ui: &mut egui::Ui, title: &str, width: f32, is_sorted: bool, dir: SortDirection) -> bool {
    let glyph = if is_sorted {
        match dir {
            SortDirection::Ascending => " ^",
            SortDirection::Descending => " v",
        }
    } else {
        ""
    };
    let label = format!("{}{}", title, glyph);

    let btn = egui::Button::new(RichText::new(label).size(11.0).color(Color32::BLACK).strong())
        .fill(Color32::from_rgb(238, 235, 222))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(172, 168, 153)))
        .rounding(1.0_f32)
        .min_size(Vec2::new(width, 20.0));

    ui.add(btn).clicked()
}

fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
