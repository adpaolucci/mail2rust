use crate::state::{ActiveTab, AppState};
use eframe::egui::{self, Color32, RichText, ScrollArea, Stroke, Vec2};

pub fn render_message_preview(ui: &mut egui::Ui, state: &mut AppState) {
    let msg = match state.get_selected_message().cloned() {
        Some(m) => m,
        None => {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new("Select a message from the list to view its properties and content.")
                        .color(Color32::from_rgb(120, 120, 120))
                        .size(12.0),
                );
            });
            return;
        }
    };

    ui.vertical(|ui| {
        // Classic Win32 Message Header Panel
        egui::Frame::none()
            .fill(Color32::from_rgb(244, 243, 238))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(172, 168, 153)))
            .inner_margin(egui::Margin::symmetric(6.0, 5.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Subject:").size(12.0).strong().color(Color32::from_rgb(80, 80, 80)));
                    ui.label(RichText::new(&msg.subject).size(13.0).strong().color(Color32::BLACK));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(msg.date.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                                .size(11.0)
                                .color(Color32::from_rgb(80, 80, 80)),
                        );
                    });
                });

                ui.add_space(2.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("From:").size(12.0).strong().color(Color32::from_rgb(80, 80, 80)));
                    ui.label(RichText::new(format!("{} <{}>", msg.from.name, msg.from.email)).size(12.0).color(Color32::BLACK));
                });

                if !msg.to.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("To:").size(12.0).strong().color(Color32::from_rgb(80, 80, 80)));
                        let to_str = msg
                            .to
                            .iter()
                            .map(|r| format!("{} <{}>", r.name, r.email))
                            .collect::<Vec<_>>()
                            .join("; ");
                        ui.label(RichText::new(to_str).size(12.0).color(Color32::BLACK));
                    });
                }

                if !msg.attachments.is_empty() {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Attachments:").size(12.0).strong().color(Color32::from_rgb(80, 80, 80)));
                        for att in &msg.attachments {
                            let size_kb = att.size_bytes as f64 / 1024.0;
                            ui.label(
                                RichText::new(format!("📎 {} ({:.1} KB)", att.filename, size_kb))
                                    .size(11.0)
                                    .color(Color32::from_rgb(0, 51, 153)),
                            );
                        }
                    });
                }
            });

        ui.add_space(3.0);

        // Win32 Property Sheet Tabs
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);

            if property_tab_button(ui, "General (Body)", state.active_tab == ActiveTab::Preview) {
                state.active_tab = ActiveTab::Preview;
            }
            if property_tab_button(ui, "RFC 5322 Headers", state.active_tab == ActiveTab::Headers) {
                state.active_tab = ActiveTab::Headers;
            }
            if property_tab_button(ui, "MAPI Properties", state.active_tab == ActiveTab::Mapi) {
                state.active_tab = ActiveTab::Mapi;
            }
            if property_tab_button(ui, "Forensics & Hashes", state.active_tab == ActiveTab::Forensics) {
                state.active_tab = ActiveTab::Forensics;
            }
        });

        // Sunken White Tab Content Area
        egui::Frame::none()
            .fill(Color32::WHITE)
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(127, 157, 185))) // #7F9DB9
            .inner_margin(egui::Margin::symmetric(6.0, 6.0))
            .show(ui, |ui| {
                ScrollArea::vertical().show(ui, |ui| match state.active_tab {
                    ActiveTab::Preview => {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(&msg.body_text)
                                .color(Color32::BLACK)
                                .size(12.0),
                        );
                    }

                    ActiveTab::Headers => {
                        ui.horizontal(|ui| {
                            if ui.small_button("📋 Copy All Headers").clicked() {
                                let dump = format!(
                                    "Message-ID: {}\nDate: {}\nFrom: {} <{}>\nSubject: {}\nSource-Format: {}\nConversation-ID: {}",
                                    msg.message_id,
                                    msg.date.to_rfc2822(),
                                    msg.from.name,
                                    msg.from.email,
                                    msg.subject,
                                    msg.source_format,
                                    msg.conversation_id
                                );
                                ui.output_mut(|o| o.copied_text = dump);
                            }
                        });
                        ui.add_space(4.0);

                        egui::Grid::new("headers_grid")
                            .striped(true)
                            .spacing([12.0, 4.0])
                            .show(ui, |ui| {
                                ui.label(RichText::new("Header Field").strong().color(Color32::BLACK));
                                ui.label(RichText::new("Header Value").strong().color(Color32::BLACK));
                                ui.end_row();

                                ui.label("Message-ID");
                                ui.monospace(RichText::new(&msg.message_id).size(11.0));
                                ui.end_row();

                                ui.label("Date");
                                ui.label(RichText::new(msg.date.to_rfc2822()).size(11.0));
                                ui.end_row();

                                ui.label("From");
                                ui.label(RichText::new(format!("{} <{}>", msg.from.name, msg.from.email)).size(11.0));
                                ui.end_row();

                                ui.label("Subject");
                                ui.label(RichText::new(&msg.subject).size(11.0));
                                ui.end_row();

                                ui.label("Source Format");
                                ui.label(RichText::new(&msg.source_format).size(11.0));
                                ui.end_row();

                                ui.label("Conversation-ID");
                                ui.monospace(RichText::new(&msg.conversation_id).size(11.0));
                                ui.end_row();
                            });
                    }

                    ActiveTab::Mapi => {
                        ui.label(
                            RichText::new("Outlook [MS-OXPROPS] MAPI DASL Mapping (Pure-Rust Engine, Zero COM):")
                                .size(11.0)
                                .color(Color32::from_rgb(100, 100, 100)),
                        );
                        ui.add_space(4.0);

                        egui::Grid::new("mapi_grid")
                            .striped(true)
                            .spacing([12.0, 4.0])
                            .show(ui, |ui| {
                                ui.label(RichText::new("Tag (Hex)").strong().color(Color32::BLACK));
                                ui.label(RichText::new("Canonical Name").strong().color(Color32::BLACK));
                                ui.label(RichText::new("Type").strong().color(Color32::BLACK));
                                ui.label(RichText::new("Value").strong().color(Color32::BLACK));
                                ui.end_row();

                                ui.monospace("0x0037001F");
                                ui.label("PR_SUBJECT");
                                ui.label("PT_UNICODE");
                                ui.label(&msg.subject);
                                ui.end_row();

                                ui.monospace("0x0E070003");
                                ui.label("PR_MESSAGE_FLAGS");
                                ui.label("PT_LONG");
                                let flag_val = if msg.flags.read { "0x0001 (MSGFLAG_READ)" } else { "0x0000" };
                                ui.label(flag_val);
                                ui.end_row();

                                ui.monospace("0x30070040");
                                ui.label("PR_CREATION_TIME");
                                ui.label("PT_SYSTIME");
                                ui.label(msg.date.format("%Y-%m-%d %H:%M:%S UTC").to_string());
                                ui.end_row();

                                ui.monospace("0x30130102");
                                ui.label("PR_CONVERSATION_ID");
                                ui.label("PT_BINARY");
                                ui.monospace(&msg.conversation_id);
                                ui.end_row();
                            });
                    }

                    ActiveTab::Forensics => {
                        ui.label(RichText::new("Chain-of-Custody & Integrity Verification").size(12.0).strong().color(Color32::BLACK));
                        ui.add_space(4.0);

                        egui::Grid::new("forensic_grid")
                            .striped(true)
                            .spacing([12.0, 6.0])
                            .show(ui, |ui| {
                                ui.label(RichText::new("Property").strong().color(Color32::BLACK));
                                ui.label(RichText::new("Value").strong().color(Color32::BLACK));
                                ui.label("");
                                ui.end_row();

                                ui.label("SHA-256 Hash:");
                                ui.monospace(RichText::new(&msg.sha256).size(11.0).color(Color32::from_rgb(0, 100, 0)));
                                if ui.small_button("Copy").clicked() {
                                    ui.output_mut(|o| o.copied_text = msg.sha256.clone());
                                }
                                ui.end_row();

                                ui.label("MD5 Checksum:");
                                ui.monospace(RichText::new(&msg.md5).size(11.0));
                                if ui.small_button("Copy").clicked() {
                                    ui.output_mut(|o| o.copied_text = msg.md5.clone());
                                }
                                ui.end_row();

                                ui.label("Bates Number:");
                                let bates = msg.bates_number.as_deref().unwrap_or("Unassigned");
                                ui.label(RichText::new(bates).strong().color(Color32::from_rgb(180, 100, 0)));
                                ui.label("");
                                ui.end_row();

                                ui.label("Legal Hold:");
                                let hold_text = if msg.legal_hold { "ACTIVE (Tamper-Evident)" } else { "Inactive" };
                                ui.label(RichText::new(hold_text).strong().color(if msg.legal_hold { Color32::from_rgb(180, 0, 0) } else { Color32::from_rgb(100, 100, 100) }));
                                ui.label("");
                                ui.end_row();
                            });
                    }
                });
            });
    });
}

fn property_tab_button(ui: &mut egui::Ui, title: &str, is_active: bool) -> bool {
    let (bg, border, text_color) = if is_active {
        (Color32::WHITE, Color32::from_rgb(127, 157, 185), Color32::BLACK)
    } else {
        (
            Color32::from_rgb(236, 233, 216),
            Color32::from_rgb(172, 168, 153),
            Color32::from_rgb(80, 80, 80),
        )
    };

    let btn = egui::Button::new(RichText::new(title).size(11.0).color(text_color).strong())
        .fill(bg)
        .stroke(Stroke::new(1.0_f32, border))
        .rounding(egui::Rounding {
            nw: 2.0_f32,
            ne: 2.0_f32,
            sw: 0.0_f32,
            se: 0.0_f32,
        })
        .min_size(Vec2::new(0.0, 20.0));

    ui.add(btn).clicked()
}
