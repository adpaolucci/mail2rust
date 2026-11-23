use crate::message::Message;
use crate::state::{ActiveModal, ActiveTab, AppState, SortColumn, SortDirection, format_size};
use iced::widget::{button, checkbox, column, container, row, rule, scrollable, space, text};
use iced::{Alignment, Element, Length};
use mail2rust_core::Importance;

pub fn view_workbench(state: &AppState) -> Element<'_, Message> {
    let mut panes = row![].spacing(6).padding(6).height(Length::Fill);

    if state.show_tree_pane {
        panes = panes.push(view_folder_tree(state));
    }

    panes = panes.push(view_message_list(state));

    if state.show_preview_pane {
        panes = panes.push(view_message_preview(state));
    }

    panes.into()
}

fn view_folder_tree(state: &AppState) -> Element<'_, Message> {
    let header = row![
        text("Console Tree").size(13),
        space().width(Length::Fill),
        button(text("All").size(11))
            .on_press(Message::SelectAllFolders)
            .style(button::secondary),
        button(text("None").size(11))
            .on_press(Message::SelectNoFolders)
            .style(button::secondary),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    let all_active = state.active_folder == "All Mail";
    let mut tree_items = column![
        button(
            row![
                text("All Messages").size(12),
                space().width(Length::Fill),
                text(format!("({})", state.messages.len())).size(11),
            ]
            .align_y(Alignment::Center),
        )
        .on_press(Message::SelectFolder("All Mail".to_string()))
        .width(Length::Fill)
        .style(if all_active { button::primary } else { button::secondary }),
    ]
    .spacing(4);

    if state.folders.is_empty() {
        tree_items = tree_items.push(text("(No archive loaded)").size(11));
    } else {
        for (idx, folder) in state.folders.iter().enumerate() {
            let is_active = state.active_folder == folder.name;
            let count_str = if folder.unread_count > 0 {
                format!("({}/{})", folder.unread_count, folder.message_count)
            } else {
                format!("({})", folder.message_count)
            };
            let folder_row = row![
                checkbox(folder.selected).on_toggle(move |_| Message::ToggleFolderChecked(idx)),
                button(
                    row![
                        text(&folder.name).size(12),
                        space().width(Length::Fill),
                        text(count_str).size(11),
                    ]
                    .align_y(Alignment::Center),
                )
                .on_press(Message::SelectFolder(folder.name.clone()))
                .width(Length::Fill)
                .style(if is_active { button::primary } else { button::secondary }),
            ]
            .spacing(4)
            .align_y(Alignment::Center);
            tree_items = tree_items.push(folder_row);
        }
    }

    container(column![header, rule::horizontal(1), scrollable(tree_items).height(Length::Fill),].spacing(6))
        .width(Length::Fixed(245.0))
        .height(Length::Fill)
        .padding(8)
        .style(container::bordered_box)
        .into()
}

fn sort_header<'a>(title: &'a str, col: SortColumn, width: Length, state: &AppState) -> Element<'a, Message> {
    let suffix = if state.sort_column == col {
        match state.sort_direction {
            SortDirection::Ascending => " ^",
            SortDirection::Descending => " v",
        }
    } else {
        ""
    };
    button(text(format!("{title}{suffix}")).size(11))
        .on_press(Message::ToggleSort(col))
        .width(width)
        .style(button::secondary)
        .into()
}

fn view_message_list(state: &AppState) -> Element<'_, Message> {
    let filtered = state.filtered_messages();
    let list_header = row![
        text(format!(
            "Folder: {} ({} displayed | {} selected)",
            state.active_folder,
            filtered.len(),
            state.selected_message_ids.len()
        ))
        .size(12),
    ];

    let col_headers = row![
        space().width(Length::Fixed(26.0)),
        sort_header("!", SortColumn::Importance, Length::Fixed(34.0), state),
        sort_header("F", SortColumn::Flags, Length::Fixed(44.0), state),
        sort_header("From", SortColumn::From, Length::FillPortion(2), state),
        sort_header("Subject", SortColumn::Subject, Length::FillPortion(3), state),
        sort_header("Received", SortColumn::Date, Length::Fixed(122.0), state),
        sort_header("Size", SortColumn::Size, Length::Fixed(68.0), state),
    ]
    .spacing(3)
    .align_y(Alignment::Center);

    let body: Element<'_, Message> = if filtered.is_empty() {
        container(
            column![
                text("No mailbox archive or messages loaded.").size(14),
                text("Open an archive (PST, OST, MBOX, Maildir, EML) or connect to IMAP to begin.").size(12),
                row![
                    button(text("Open Archive...").size(12))
                        .on_press(Message::OpenModal(ActiveModal::OpenMailbox))
                        .style(button::primary),
                    button(text("Connect to IMAP...").size(12))
                        .on_press(Message::OpenModal(ActiveModal::ImapConnect))
                        .style(button::secondary),
                    button(text("Load Sample Data").size(12))
                        .on_press(Message::LoadSampleData)
                        .style(button::secondary),
                ]
                .spacing(8),
            ]
            .spacing(10)
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    } else {
        let mut rows_col = column![].spacing(3);
        for msg in filtered.into_iter().take(2000) {
            let is_active = state.selected_message_id.as_deref() == Some(msg.id.as_str());
            let is_checked = state.is_message_selected(&msg.id);
            let msg_id_chk = msg.id.clone();
            let msg_id_sel = msg.id.clone();

            let imp = match msg.importance {
                Importance::High => "!",
                Importance::Low => "v",
                Importance::Normal => " ",
            };
            let flags = format!(
                "{}{}{}",
                if !msg.flags.read { "U" } else { "-" },
                if !msg.attachments.is_empty() { "@" } else { "-" },
                if msg.flags.flagged { "*" } else { "-" },
            );
            let from = if msg.from.name.is_empty() {
                msg.from.email.clone()
            } else {
                msg.from.name.clone()
            };
            let subj = if msg.subject.is_empty() {
                "(No Subject)"
            } else {
                msg.subject.as_str()
            };
            let date_str = msg.date.format("%Y-%m-%d %H:%M").to_string();
            let size_str = format_size(msg.size_bytes);

            let msg_btn = button(
                row![
                    text(imp).size(11).width(Length::Fixed(24.0)),
                    text(flags).size(11).width(Length::Fixed(38.0)),
                    text(from).size(12).width(Length::FillPortion(2)),
                    text(subj).size(12).width(Length::FillPortion(3)),
                    text(date_str).size(11).width(Length::Fixed(116.0)),
                    text(size_str).size(11).width(Length::Fixed(64.0)),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .on_press(Message::SelectEmail(msg_id_sel))
            .width(Length::Fill)
            .style(if is_active { button::primary } else { button::secondary });

            rows_col = rows_col.push(
                row![
                    checkbox(is_checked).on_toggle(move |_| Message::ToggleMessageChecked(msg_id_chk.clone())),
                    msg_btn,
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            );
        }
        scrollable(rows_col).height(Length::Fill).into()
    };

    container(column![list_header, col_headers, rule::horizontal(1), body].spacing(5))
        .width(Length::FillPortion(3))
        .height(Length::Fill)
        .padding(8)
        .style(container::bordered_box)
        .into()
}

fn view_message_preview(state: &AppState) -> Element<'_, Message> {
    let Some(msg) = state.get_selected_message() else {
        return container(
            text("Select a message from the list to inspect its body, headers, MAPI tags, and hashes.").size(12),
        )
        .width(Length::FillPortion(2))
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .padding(8)
        .style(container::bordered_box)
        .into();
    };

    let to_str = msg
        .to
        .iter()
        .map(|r| format!("{} <{}>", r.name, r.email))
        .collect::<Vec<_>>()
        .join("; ");

    let mut header_block = column![
        text(format!("Subject: {}", msg.subject)).size(13),
        text(format!("From: {} <{}>", msg.from.name, msg.from.email)).size(12),
        text(format!("Date: {}", msg.date.format("%Y-%m-%d %H:%M:%S UTC"))).size(11),
    ]
    .spacing(2);
    if !to_str.is_empty() {
        header_block = header_block.push(text(format!("To: {to_str}")).size(11));
    }
    if !msg.attachments.is_empty() {
        let names = msg
            .attachments
            .iter()
            .map(|a| format!("{} ({})", a.filename, format_size(a.size_bytes)))
            .collect::<Vec<_>>()
            .join(", ");
        header_block = header_block.push(text(format!("Attachments: {names}")).size(11));
    }

    let tab_bar = row![
        tab_btn("Body", ActiveTab::Preview, state.active_tab),
        tab_btn("RFC 5322 Headers", ActiveTab::Headers, state.active_tab),
        tab_btn("MAPI", ActiveTab::Mapi, state.active_tab),
        tab_btn("Forensics", ActiveTab::Forensics, state.active_tab),
    ]
    .spacing(4);

    let tab_content: Element<'_, Message> = match state.active_tab {
        ActiveTab::Preview => column![text(&msg.body_text).size(12)].into(),
        ActiveTab::Headers => {
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
            column![
                button(text("Copy Headers").size(11))
                    .on_press(Message::CopyText(dump.clone()))
                    .style(button::secondary),
                text(dump).size(11),
            ]
            .spacing(6)
            .into()
        }
        ActiveTab::Mapi => {
            let mapi = format!(
                "0x0037001F PR_SUBJECT (PT_UNICODE): {}\n\
                 0x0E070003 PR_MESSAGE_FLAGS (PT_LONG): {}\n\
                 0x30070040 PR_CREATION_TIME (PT_SYSTIME): {}\n\
                 0x30130102 PR_CONVERSATION_ID (PT_BINARY): {}",
                msg.subject,
                if msg.flags.read {
                    "0x0001 (MSGFLAG_READ)"
                } else {
                    "0x0000"
                },
                msg.date.format("%Y-%m-%d %H:%M:%S UTC"),
                msg.conversation_id
            );
            column![
                text("Outlook [MS-OXPROPS] MAPI Properties (Pure Rust):").size(11),
                text(mapi).size(11),
            ]
            .spacing(6)
            .into()
        }
        ActiveTab::Forensics => {
            let bates = msg.bates_number.as_deref().unwrap_or("Unassigned");
            let hold = if msg.legal_hold {
                "ACTIVE (Tamper-Evident)"
            } else {
                "Inactive"
            };
            column![
                text(format!("SHA-256: {}", msg.sha256)).size(11),
                button(text("Copy SHA-256").size(11))
                    .on_press(Message::CopyText(msg.sha256.clone()))
                    .style(button::secondary),
                text(format!("MD5: {}", msg.md5)).size(11),
                text(format!("Bates Number: {bates}")).size(11),
                text(format!("Legal Hold: {hold}")).size(11),
            ]
            .spacing(5)
            .into()
        }
    };

    container(
        column![
            header_block,
            rule::horizontal(1),
            tab_bar,
            rule::horizontal(1),
            scrollable(tab_content).height(Length::Fill),
        ]
        .spacing(6),
    )
    .width(Length::FillPortion(2))
    .height(Length::Fill)
    .padding(8)
    .style(container::bordered_box)
    .into()
}

fn tab_btn<'a>(label: &'a str, target: ActiveTab, active: ActiveTab) -> Element<'a, Message> {
    button(text(label).size(11))
        .on_press(Message::SetActiveTab(target))
        .style(if target == active {
            button::primary
        } else {
            button::secondary
        })
        .into()
}
