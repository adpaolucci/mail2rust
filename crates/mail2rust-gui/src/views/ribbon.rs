use crate::state::{ActiveModal, AppState};
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};

pub fn render_ribbon(ctx: &egui::Context, state: &mut AppState) {
    egui::TopBottomPanel::top("top_menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("📂  Open Mailbox Archive...").clicked() {
                    ui.close_menu();
                }
                if ui.button("💾  Direct PST Export...").clicked() {
                    state.active_modal = ActiveModal::DirectPst;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("❌  Exit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            ui.menu_button("Convert", |ui| {
                if ui.button("⚡  Direct PST Generation (Zero Outlook)").clicked() {
                    state.active_modal = ActiveModal::DirectPst;
                    ui.close_menu();
                }
                if ui.button("✂  Split Mailbox by Size / Year...").clicked() {
                    state.active_modal = ActiveModal::AutoSplit;
                    ui.close_menu();
                }
                if ui.button("🔍  Pre-Flight Dry-Run Scan...").clicked() {
                    state.active_modal = ActiveModal::DryRun;
                    ui.close_menu();
                }
            });

            ui.menu_button("Forensics", |ui| {
                if ui.button("⚖  Bates Stamping & Merkle Hash...").clicked() {
                    state.active_modal = ActiveModal::Forensics;
                    ui.close_menu();
                }
                if ui.button("🏷  Query & Date Filter Rules...").clicked() {
                    state.active_modal = ActiveModal::FilterCriteria;
                    ui.close_menu();
                }
            });

            ui.menu_button("Tools", |ui| {
                if ui.button("🛡  MS-PST Container Validator").clicked() {
                    state.active_modal = ActiveModal::ValidatePst;
                    ui.close_menu();
                }
            });

            ui.menu_button("Help", |ui| {
                if ui.button("ℹ  About Mail2Rust Studio").clicked() {
                    state.active_modal = ActiveModal::About;
                    ui.close_menu();
                }
            });
        });
    });

    egui::TopBottomPanel::top("ribbon_toolbar")
        .frame(egui::Frame::none().fill(Color32::from_rgb(24, 28, 36)).inner_margin(8.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);

                if ribbon_button(ui, "⚡ Direct PST", Color32::from_rgb(37, 99, 235)) {
                    state.active_modal = ActiveModal::DirectPst;
                }

                if ribbon_button(ui, "✂ Auto-Split", Color32::from_rgb(124, 58, 237)) {
                    state.active_modal = ActiveModal::AutoSplit;
                }

                if ribbon_button(ui, "🏷 Filter Rules", Color32::from_rgb(79, 70, 229)) {
                    state.active_modal = ActiveModal::FilterCriteria;
                }

                if ribbon_button(ui, "⚖ Forensics & Bates", Color32::from_rgb(13, 148, 136)) {
                    state.active_modal = ActiveModal::Forensics;
                }

                if ribbon_button(ui, "🔍 Dry-Run Scan", Color32::from_rgb(217, 119, 6)) {
                    state.active_modal = ActiveModal::DryRun;
                }

                if ribbon_button(ui, "🛡 Validate PST", Color32::from_rgb(16, 185, 129)) {
                    state.active_modal = ActiveModal::ValidatePst;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("100% Pure Rust • Zero Outlook Dependency")
                            .color(Color32::from_rgb(148, 163, 184))
                            .small(),
                    );
                });
            });
        });
}

fn ribbon_button(ui: &mut egui::Ui, text: &str, bg: Color32) -> bool {
    let btn = egui::Button::new(RichText::new(text).color(Color32::WHITE).strong())
        .fill(bg)
        .stroke(Stroke::new(1.0_f32, bg))
        .rounding(4.0_f32)
        .min_size(Vec2::new(110.0, 32.0));
    ui.add(btn).clicked()
}
