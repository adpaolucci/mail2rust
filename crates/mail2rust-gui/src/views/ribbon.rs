use crate::state::{ActiveModal, AppState, ExportScope};
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};

pub fn render_ribbon(ctx: &egui::Context, state: &mut AppState) {
    // 1. Classic Win32 Menu Bar
    egui::TopBottomPanel::top("top_menu_bar")
        .frame(
            egui::Frame::none()
                .fill(Color32::from_rgb(240, 240, 240))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(200, 200, 200)))
                .inner_margin(egui::Margin::symmetric(4.0, 2.0)),
        )
        .show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open Mailbox Archive...  (Ctrl+O)").clicked() {
                        state.active_modal = ActiveModal::OpenMailbox;
                        ui.close_menu();
                    }
                    if ui.button("Connect to IMAP Mailbox...  (Ctrl+M)").clicked() {
                        state.active_modal = ActiveModal::ImapConnect;
                        ui.close_menu();
                    }
                    if ui.button("Export Messages (PST/MBOX/EML)...  (Ctrl+E)").clicked() {
                        state.export_scope = ExportScope::All;
                        state.active_modal = ActiveModal::DirectPst;
                        ui.close_menu();
                    }
                    if ui.button("Export Selected Items Only...").clicked() {
                        state.export_scope = ExportScope::Selected;
                        state.active_modal = ActiveModal::DirectPst;
                        ui.close_menu();
                    }
                    if ui.button("Split Mailbox into Volumes...").clicked() {
                        state.active_modal = ActiveModal::AutoSplit;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit  (Alt+F4)").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("Action", |ui| {
                    if ui.button("Select All Filtered  (Ctrl+A)").clicked() {
                        state.select_all_filtered();
                        ui.close_menu();
                    }
                    if ui.button("Invert Selection").clicked() {
                        state.invert_selection_filtered();
                        ui.close_menu();
                    }
                    if ui.button("Clear Selection  (Ctrl+D)").clicked() {
                        state.clear_selection();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Export Selected Messages...").clicked() {
                        state.export_scope = ExportScope::Selected;
                        state.active_modal = ActiveModal::DirectPst;
                        ui.close_menu();
                    }
                    if ui.button("Export Filtered Messages...").clicked() {
                        state.export_scope = ExportScope::Filtered;
                        state.active_modal = ActiveModal::DirectPst;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Connect to IMAP Server").clicked() {
                        state.active_modal = ActiveModal::ImapConnect;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Multi-Volume Auto-Split").clicked() {
                        state.active_modal = ActiveModal::AutoSplit;
                        ui.close_menu();
                    }
                    if ui.button("Forensic Bates Stamping & Audit").clicked() {
                        state.active_modal = ActiveModal::Forensics;
                        ui.close_menu();
                    }
                    if ui.button("Validate PST Container Integrity").clicked() {
                        state.active_modal = ActiveModal::ValidatePst;
                        ui.close_menu();
                    }
                    if ui.button("Pre-Flight Dry-Run Scan").clicked() {
                        state.active_modal = ActiveModal::DryRun;
                        ui.close_menu();
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui
                        .checkbox(&mut state.show_tree_pane, "Show Console Tree (Folders)")
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    if ui
                        .checkbox(&mut state.show_preview_pane, "Show Message Preview Pane")
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Refresh (F5)").clicked() {
                        state.rebuild_folders_from_messages();
                        ui.close_menu();
                    }
                    if ui.button("Clear Search Query").clicked() {
                        state.search_query.clear();
                        ui.close_menu();
                    }
                });

                ui.menu_button("Tools", |ui| {
                    if ui.button("Query & Date Filter Criteria...").clicked() {
                        state.active_modal = ActiveModal::FilterCriteria;
                        ui.close_menu();
                    }
                    if ui.button("Forensic Manifest & Legal Hold...").clicked() {
                        state.active_modal = ActiveModal::Forensics;
                        ui.close_menu();
                    }
                    if ui.button("MS-PST Container Validator...").clicked() {
                        state.active_modal = ActiveModal::ValidatePst;
                        ui.close_menu();
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("Load Sample / Evaluation Data...").clicked() {
                        state.load_sample_data();
                        ui.close_menu();
                    }
                    if ui.button("Clear Loaded Messages").clicked() {
                        state.clear_data();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("About Mail2Rust Studio...").clicked() {
                        state.active_modal = ActiveModal::About;
                        ui.close_menu();
                    }
                });
            });
        });

    // 2. Classic Win32 Utility Toolbar (High Density, Minimalist)
    egui::TopBottomPanel::top("classic_toolbar")
        .frame(
            egui::Frame::none()
                .fill(Color32::from_rgb(236, 233, 216)) // Classic Win2k/XP face
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(172, 168, 153)))
                .inner_margin(egui::Margin::symmetric(4.0, 3.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);

                if win32_tool_button(ui, "Open...") {
                    state.active_modal = ActiveModal::OpenMailbox;
                }
                if win32_tool_button(ui, "IMAP...") {
                    state.active_modal = ActiveModal::ImapConnect;
                }
                if win32_tool_button(ui, "Export...") {
                    state.active_modal = ActiveModal::DirectPst;
                }

                toolbar_separator(ui);

                if win32_tool_button(ui, "Select All") {
                    state.select_all_filtered();
                }
                if win32_tool_button(ui, "Clear Sel") {
                    state.clear_selection();
                }

                toolbar_separator(ui);

                if win32_tool_button(ui, "Split") {
                    state.active_modal = ActiveModal::AutoSplit;
                }
                if win32_tool_button(ui, "Filter") {
                    state.active_modal = ActiveModal::FilterCriteria;
                }
                if win32_tool_button(ui, "Forensics") {
                    state.active_modal = ActiveModal::Forensics;
                }
                if win32_tool_button(ui, "Validate") {
                    state.active_modal = ActiveModal::ValidatePst;
                }
                if win32_tool_button(ui, "Dry-Run") {
                    state.active_modal = ActiveModal::DryRun;
                }

                toolbar_separator(ui);

                if win32_tool_button(ui, "Refresh") {
                    state.rebuild_folders_from_messages();
                }

                // Right aligned search box in classic toolbar style
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !state.search_query.is_empty() && ui.small_button("X").clicked() {
                        state.search_query.clear();
                    }

                    ui.add(
                        egui::TextEdit::singleline(&mut state.search_query)
                            .hint_text("Search emails...")
                            .desired_width(180.0),
                    );

                    ui.label(RichText::new("Find:").size(12.0));
                });
            });
        });
}

fn toolbar_separator(ui: &mut egui::Ui) {
    ui.add_space(2.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(2.0, 18.0), egui::Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.y_range(),
        Stroke::new(1.0_f32, Color32::from_rgb(172, 168, 153)),
    );
    ui.add_space(2.0);
}

fn win32_tool_button(ui: &mut egui::Ui, text: &str) -> bool {
    let btn = egui::Button::new(RichText::new(text).size(12.0).color(Color32::BLACK))
        .fill(Color32::from_rgb(244, 243, 238))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(172, 168, 153)))
        .rounding(2.0_f32)
        .min_size(Vec2::new(0.0, 22.0));
    ui.add(btn).clicked()
}
