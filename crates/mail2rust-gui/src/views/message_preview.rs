use crate::state::{ActiveTab, AppState};
use eframe::egui::{self, Color32, RichText, ScrollArea, Stroke};

pub fn render_message_preview(ui: &mut egui::Ui, state: &mut AppState) {
    let msg = match state.get_selected_message().cloned() {
        Some(m) => m,
        None => {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new(
                        "Select an email from the list to preview content, MAPI properties, and forensic hashes.",
                    )
                    .color(Color32::from_rgb(100, 116, 139))
                    .size(13.0),
                );
            });
            return;
        }
    };

    ui.vertical(|ui| {
        // Email Header Card
        egui::Frame::none()
            .fill(Color32::from_rgb(15, 23, 42))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(30, 41, 59)))
            .inner_margin(8.0)
            .rounding(4.0_f32)
            .show(ui, |ui| {
                ui.heading(RichText::new(&msg.subject).size(18.0).color(Color32::WHITE).strong());
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("From:").color(Color32::from_rgb(148, 163, 184)).strong());
                    ui.label(RichText::new(format!("{} <{}>", msg.from.name, msg.from.email)).color(Color32::WHITE));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(msg.date.format("%a, %b %d, %Y at %H:%M:%S UTC").to_string())
                                .color(Color32::from_rgb(148, 163, 184))
                                .size(11.0),
                        );
                    });
                });

                if !msg.to.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("To:").color(Color32::from_rgb(148, 163, 184)).strong());
                        let to_str = msg
                            .to
                            .iter()
                            .map(|r| format!("{} <{}>", r.name, r.email))
                            .collect::<Vec<_>>()
                            .join(", ");
                        ui.label(RichText::new(to_str).color(Color32::from_rgb(226, 232, 240)));
                    });
                }

                if !msg.cc.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("CC:").color(Color32::from_rgb(148, 163, 184)).strong());
                        let cc_str = msg
                            .cc
                            .iter()
                            .map(|r| format!("{} <{}>", r.name, r.email))
                            .collect::<Vec<_>>()
                            .join(", ");
                        ui.label(RichText::new(cc_str).color(Color32::from_rgb(203, 213, 225)));
                    });
                }

                // Attachments Chips
                if !msg.attachments.is_empty() {
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new("📎 Attachments:")
                                .color(Color32::from_rgb(148, 163, 184))
                                .strong(),
                        );
                        for att in &msg.attachments {
                            let size_kb = att.size_bytes as f64 / 1024.0;
                            let chip = egui::Button::new(
                                RichText::new(format!("📄 {} ({:.1} KB)", att.filename, size_kb))
                                    .color(Color32::from_rgb(96, 165, 250))
                                    .size(11.0),
                            )
                            .fill(Color32::from_rgb(30, 41, 59))
                            .rounding(4.0_f32);
                            ui.add(chip);
                        }
                    });
                }
            });

        ui.add_space(6.0);

        // Tab Navigation
        ui.horizontal(|ui| {
            if ui
                .selectable_label(state.active_tab == ActiveTab::Preview, "📝 Body Preview")
                .clicked()
            {
                state.active_tab = ActiveTab::Preview;
            }
            if ui
                .selectable_label(state.active_tab == ActiveTab::Headers, "📋 RFC 5322 Headers")
                .clicked()
            {
                state.active_tab = ActiveTab::Headers;
            }
            if ui
                .selectable_label(state.active_tab == ActiveTab::Mapi, "🛡 MAPI DASL Inspector")
                .clicked()
            {
                state.active_tab = ActiveTab::Mapi;
            }
            if ui
                .selectable_label(state.active_tab == ActiveTab::Forensics, "⚖ Forensics & Hashes")
                .clicked()
            {
                state.active_tab = ActiveTab::Forensics;
            }
        });

        ui.separator();
        ui.add_space(4.0);

        // Tab Content
        ScrollArea::vertical().show(ui, |ui| match state.active_tab {
            ActiveTab::Preview => {
                egui::Frame::none()
                    .fill(Color32::from_rgb(15, 23, 42))
                    .inner_margin(8.0)
                    .rounding(4.0_f32)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(&msg.body_text)
                                .color(Color32::from_rgb(241, 245, 249))
                                .size(13.0),
                        );
                    });
            }

            ActiveTab::Headers => {
                egui::Grid::new("headers_grid")
                    .striped(true)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Header").strong().color(Color32::from_rgb(96, 165, 250)));
                        ui.label(RichText::new("Value").strong().color(Color32::from_rgb(96, 165, 250)));
                        ui.end_row();

                        ui.label("Message-ID");
                        ui.label(&msg.message_id);
                        ui.end_row();

                        ui.label("Date");
                        ui.label(msg.date.to_rfc2822());
                        ui.end_row();

                        ui.label("From");
                        ui.label(format!("{} <{}>", msg.from.name, msg.from.email));
                        ui.end_row();

                        ui.label("Subject");
                        ui.label(&msg.subject);
                        ui.end_row();

                        ui.label("Source Format");
                        ui.label(&msg.source_format);
                        ui.end_row();

                        ui.label("Conversation-ID");
                        ui.label(&msg.conversation_id);
                        ui.end_row();
                    });
            }

            ActiveTab::Mapi => {
                ui.label(
                    RichText::new("Native Outlook [MS-OXPROPS] MAPI DASL Tag Mapping (Zero Outlook COM)")
                        .color(Color32::from_rgb(148, 163, 184))
                        .small(),
                );
                ui.add_space(4.0);

                egui::Grid::new("mapi_grid")
                    .striped(true)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("MAPI Tag (Hex)")
                                .strong()
                                .color(Color32::from_rgb(96, 165, 250)),
                        );
                        ui.label(
                            RichText::new("Canonical Property Name")
                                .strong()
                                .color(Color32::from_rgb(96, 165, 250)),
                        );
                        ui.label(RichText::new("Type").strong().color(Color32::from_rgb(96, 165, 250)));
                        ui.label(RichText::new("Value").strong().color(Color32::from_rgb(96, 165, 250)));
                        ui.end_row();

                        ui.label("0x0037001F");
                        ui.label("PR_SUBJECT");
                        ui.label("PT_UNICODE");
                        ui.label(&msg.subject);
                        ui.end_row();

                        ui.label("0x0E070003");
                        ui.label("PR_MESSAGE_FLAGS");
                        ui.label("PT_LONG");
                        let flag_val = if msg.flags.read {
                            "0x0001 (MSGFLAG_READ)"
                        } else {
                            "0x0000"
                        };
                        ui.label(flag_val);
                        ui.end_row();

                        ui.label("0x30070040");
                        ui.label("PR_CREATION_TIME");
                        ui.label("PT_SYSTIME");
                        ui.label(msg.date.format("%Y-%m-%d %H:%M:%S UTC").to_string());
                        ui.end_row();

                        ui.label("0x30130102");
                        ui.label("PR_CONVERSATION_ID");
                        ui.label("PT_BINARY");
                        ui.label(&msg.conversation_id);
                        ui.end_row();
                    });
            }

            ActiveTab::Forensics => {
                egui::Frame::none()
                    .fill(Color32::from_rgb(15, 23, 42))
                    .inner_margin(8.0)
                    .rounding(4.0_f32)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("⚖ Forensic Chain-of-Custody Verification")
                                .size(14.0)
                                .strong()
                                .color(Color32::WHITE),
                        );
                        ui.add_space(6.0);

                        ui.label(RichText::new("SHA-256 Digest:").color(Color32::from_rgb(148, 163, 184)));
                        ui.monospace(RichText::new(&msg.sha256).color(Color32::from_rgb(52, 211, 153)));
                        ui.add_space(4.0);

                        ui.label(RichText::new("MD5 Digest:").color(Color32::from_rgb(148, 163, 184)));
                        ui.monospace(RichText::new(&msg.md5).color(Color32::from_rgb(203, 213, 225)));
                        ui.add_space(4.0);

                        ui.label(RichText::new("Bates Number:").color(Color32::from_rgb(148, 163, 184)));
                        let bates = msg.bates_number.as_deref().unwrap_or("Unassigned");
                        ui.label(RichText::new(bates).color(Color32::from_rgb(251, 191, 36)).strong());
                        ui.add_space(4.0);

                        ui.label(RichText::new("Legal Hold Status:").color(Color32::from_rgb(148, 163, 184)));
                        let hold_text = if msg.legal_hold {
                            "🔒 ACTIVE (Tamper-Evident)"
                        } else {
                            "Inactive"
                        };
                        ui.label(
                            RichText::new(hold_text)
                                .color(if msg.legal_hold {
                                    Color32::from_rgb(239, 68, 68)
                                } else {
                                    Color32::from_rgb(148, 163, 184)
                                })
                                .strong(),
                        );
                    });
            }
        });
    });
}
