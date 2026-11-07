use crate::state::AppState;
use crate::views::{
    folder_tree::render_folder_tree, message_list::render_message_list, message_preview::render_message_preview,
    modals::render_modals, ribbon::render_ribbon, status_bar::render_status_bar,
};
use eframe::egui::{self, Color32, Stroke};

#[derive(Default)]
pub struct Mail2RustApp {
    pub state: AppState,
}

impl eframe::App for Mail2RustApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let win_face = Color32::from_rgb(236, 233, 216);
        let win_border = Color32::from_rgb(172, 168, 153);

        // 1. Classic Win32 Menu Bar and Utility Toolbar
        render_ribbon(ctx, &mut self.state);

        // 2. Classic Win32 Multi-Segment Sunken Status Bar
        render_status_bar(ctx, &self.state);

        // 3. Left Pane: Console Tree (Folders)
        if self.state.show_tree_pane {
            egui::SidePanel::left("left_folder_tree")
                .resizable(true)
                .default_width(220.0)
                .width_range(160.0..=360.0)
                .frame(
                    egui::Frame::none()
                        .fill(win_face)
                        .stroke(Stroke::new(1.0_f32, win_border))
                        .inner_margin(egui::Margin::same(3.0)),
                )
                .show(ctx, |ui| {
                    render_folder_tree(ui, &mut self.state);
                });
        }

        // 4. Center / Right Panes (Message List & Details Property Sheet)
        if self.state.show_preview_pane {
            egui::SidePanel::left("middle_message_list")
                .resizable(true)
                .default_width(500.0)
                .width_range(320.0..=750.0)
                .frame(
                    egui::Frame::none()
                        .fill(win_face)
                        .stroke(Stroke::new(1.0_f32, win_border))
                        .inner_margin(egui::Margin::same(3.0)),
                )
                .show(ctx, |ui| {
                    render_message_list(ui, &mut self.state);
                });

            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(win_face).inner_margin(egui::Margin::same(3.0)))
                .show(ctx, |ui| {
                    render_message_preview(ui, &mut self.state);
                });
        } else {
            // Full-width Message List when preview pane is hidden
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(win_face).inner_margin(egui::Margin::same(3.0)))
                .show(ctx, |ui| {
                    render_message_list(ui, &mut self.state);
                });
        }

        // 5. Render active dialogs / wizards
        render_modals(ctx, &mut self.state);
    }
}
