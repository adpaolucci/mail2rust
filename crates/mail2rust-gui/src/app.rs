use crate::state::AppState;
use crate::views::{
    folder_tree::render_folder_tree, message_list::render_message_list, message_preview::render_message_preview,
    modals::render_modals, ribbon::render_ribbon, status_bar::render_status_bar,
};
use eframe::egui::{self, Color32};

#[derive(Default)]
pub struct Mail2RustApp {
    pub state: AppState,
}

impl eframe::App for Mail2RustApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Render top ribbon toolbar & menu
        render_ribbon(ctx, &mut self.state);

        // Render bottom status bar
        render_status_bar(ctx, &self.state);

        // Left Pane: Folder Tree
        egui::SidePanel::left("left_folder_tree")
            .resizable(true)
            .default_width(220.0)
            .width_range(160.0..=360.0)
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(15, 23, 42))
                    .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(30, 41, 59)))
                    .inner_margin(8.0),
            )
            .show(ctx, |ui| {
                render_folder_tree(ui, &mut self.state);
            });

        // Middle Pane: Message List
        egui::SidePanel::left("middle_message_list")
            .resizable(true)
            .default_width(340.0)
            .width_range(260.0..=500.0)
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(15, 23, 42))
                    .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(30, 41, 59)))
                    .inner_margin(8.0),
            )
            .show(ctx, |ui| {
                render_message_list(ui, &mut self.state);
            });

        // Right Pane: Message Preview
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(10, 15, 26))
                    .inner_margin(12.0),
            )
            .show(ctx, |ui| {
                render_message_preview(ui, &mut self.state);
            });

        // Render any active modals / dialogs on top
        render_modals(ctx, &mut self.state);
    }
}
