use crate::message::Message;
use crate::state::{ActiveModal, AppState};
use iced::widget::{button, column, container, row, text, text_input};
use iced::{Alignment, Element, Length};

pub fn view_toolbar(state: &AppState) -> Element<'_, Message> {
    let row1 = row![
        button(text("Open File...").size(12))
            .on_press(Message::QuickOpenFile)
            .style(button::primary),
        button(text("Open Folder...").size(12))
            .on_press(Message::QuickOpenFolder)
            .style(button::primary),
        button(text("Wizard...").size(12))
            .on_press(Message::OpenModal(ActiveModal::OpenMailbox))
            .style(button::secondary),
        button(text("IMAP...").size(12))
            .on_press(Message::OpenModal(ActiveModal::ImapConnect))
            .style(button::secondary),
        button(text("Sync IMAP...").size(12))
            .on_press(Message::OpenModal(ActiveModal::ImapSync))
            .style(button::secondary),
        button(text("Merge...").size(12))
            .on_press(Message::OpenModal(ActiveModal::MergeMailboxes))
            .style(button::secondary),
        button(text("Export...").size(12))
            .on_press(Message::OpenModal(ActiveModal::DirectPst))
            .style(button::success),
        button(text("Auto-Split").size(12))
            .on_press(Message::OpenModal(ActiveModal::AutoSplit))
            .style(button::secondary),
        button(text("Filter").size(12))
            .on_press(Message::OpenModal(ActiveModal::FilterCriteria))
            .style(button::secondary),
        button(text("Forensics").size(12))
            .on_press(Message::OpenModal(ActiveModal::Forensics))
            .style(button::secondary),
        button(text("Validate PST").size(12))
            .on_press(Message::OpenModal(ActiveModal::ValidatePst))
            .style(button::secondary),
        button(text("Dry-Run").size(12))
            .on_press(Message::OpenModal(ActiveModal::DryRun))
            .style(button::secondary),
        button(text("Outlook / Office").size(12))
            .on_press(Message::ToggleOutlookToolsView)
            .style(if state.show_outlook_tools {
                button::primary
            } else {
                button::secondary
            }),
    ]
    .spacing(5)
    .align_y(Alignment::Center);

    let row2 = row![
        button(text("Sample Data").size(12))
            .on_press(Message::LoadSampleData)
            .style(button::secondary),
        button(text("Clear").size(12))
            .on_press(Message::ClearData)
            .style(button::secondary),
        button(text("Select All").size(12))
            .on_press(Message::SelectAllVisible)
            .style(button::secondary),
        button(text("Invert Sel").size(12))
            .on_press(Message::InvertSelection)
            .style(button::secondary),
        button(text("Clear Sel").size(12))
            .on_press(Message::ClearSelection)
            .style(button::secondary),
        button(text(if state.show_tree_pane { "Hide Tree" } else { "Show Tree" }).size(12))
            .on_press(Message::ToggleTreePane)
            .style(button::secondary),
        button(
            text(if state.show_preview_pane {
                "Hide Preview"
            } else {
                "Show Preview"
            })
            .size(12),
        )
        .on_press(Message::TogglePreviewPane)
        .style(button::secondary),
        button(text(if state.dark_mode { "Light" } else { "Dark" }).size(12))
            .on_press(Message::ToggleDarkMode)
            .style(button::secondary),
        button(text("Refresh").size(12))
            .on_press(Message::RefreshWorkspace)
            .style(button::secondary),
        button(text("About").size(12))
            .on_press(Message::OpenModal(ActiveModal::About))
            .style(button::secondary),
        text("Find:").size(12),
        text_input("Search subject, sender, body, ID...", &state.search_query)
            .on_input(Message::SearchChanged)
            .size(12)
            .width(Length::Fill),
        button(text("X").size(12))
            .on_press(Message::ClearSearch)
            .style(button::secondary),
    ]
    .spacing(5)
    .align_y(Alignment::Center);

    container(column![row1, row2].spacing(5))
        .padding([6, 8])
        .width(Length::Fill)
        .into()
}
