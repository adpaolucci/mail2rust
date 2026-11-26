use crate::message::Message;
use crate::state::{ActiveModal, AppState, ExportFormat, ExportScope};
use iced::widget::{
    button, checkbox, column, container, mouse_area, pick_list, row, rule, scrollable, space, text, text_input,
};
use iced::{Alignment, Element, Length};
use mail2rust_pst::PstEncryption;

const EXPORT_FORMATS: &[ExportFormat] = &[ExportFormat::Pst, ExportFormat::Mbox, ExportFormat::Eml];

pub fn view_modal(state: &AppState) -> Element<'_, Message> {
    let inner: Element<'_, Message> = match state.active_modal {
        ActiveModal::None => column![].into(),
        ActiveModal::OpenMailbox => view_open_mailbox(state),
        ActiveModal::ImapConnect => view_imap_connect(state),
        ActiveModal::ImapSync => view_imap_sync(state),
        ActiveModal::MergeMailboxes => view_merge_mailboxes(state),
        ActiveModal::DirectPst => view_direct_export(state),
        ActiveModal::AutoSplit => view_auto_split(state),
        ActiveModal::FilterCriteria => view_filter_criteria(state),
        ActiveModal::Forensics => view_forensics(state),
        ActiveModal::DryRun => view_dry_run(state),
        ActiveModal::ValidatePst => view_validate_pst(state),
        ActiveModal::ConfirmOutlookMutate => view_confirm_outlook_mutate(state),
        ActiveModal::About => view_about(),
    };

    mouse_area(
        container(scrollable(inner).height(Length::Shrink))
            .padding(18)
            .width(Length::Fixed(680.0))
            .max_height(720.0)
            .style(container::rounded_box),
    )
    .on_press(Message::OpenModal(state.active_modal))
    .into()
}

fn toggle_btn<'a>(label: &'a str, active: bool, msg: Message) -> Element<'a, Message> {
    button(text(label).size(11))
        .on_press(msg)
        .style(if active { button::primary } else { button::secondary })
        .into()
}

fn view_open_mailbox(state: &AppState) -> Element<'_, Message> {
    let mut col = column![
        text("Open Mailbox Archive / Directory").size(17),
        rule::horizontal(1),
        text("Ingest MBOX, Maildir, Apple Mail EMLX, Google Takeout .zip, PST/OST, EML, OLM, or MSG.").size(12),
        row![
            text("Path:").size(12).width(Length::Fixed(60.0)),
            text_input("Path to archive or folder", &state.open_mailbox_path)
                .on_input(Message::OpenMailboxPathChanged)
                .size(12)
                .width(Length::Fill),
            button(text("File...").size(12))
                .on_press(Message::BrowseOpenMailboxFile)
                .style(button::secondary),
            button(text("Folder...").size(12))
                .on_press(Message::BrowseOpenMailboxFolder)
                .style(button::secondary),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    ]
    .spacing(8);

    if let Some(st) = &state.open_mailbox_status {
        col = col.push(text(st).size(11));
    }

    col.push(
        row![
            space().width(Length::Fill),
            button(text("Open / Ingest").size(12))
                .on_press(Message::ExecuteOpenMailbox)
                .style(button::primary),
            button(text("Close").size(12))
                .on_press(Message::CloseModal)
                .style(button::secondary),
        ]
        .spacing(8),
    )
    .into()
}

fn preset_btn<'a>(label: &'a str, host: &'a str, port: &'a str, insecure: bool) -> Element<'a, Message> {
    button(text(label).size(11))
        .on_press(Message::ApplyImapPreset {
            host: host.to_string(),
            port: port.to_string(),
            insecure,
        })
        .style(button::secondary)
        .into()
}

fn view_imap_connect(state: &AppState) -> Element<'_, Message> {
    let mut col = column![
        text("Connect to IMAP Mailbox (Live Ingestion)").size(17),
        rule::horizontal(1),
        row![
            text("Presets:").size(12),
            preset_btn("Gmail", "imap.gmail.com", "993", false),
            preset_btn("Outlook", "outlook.office365.com", "993", false),
            preset_btn("Yahoo", "imap.mail.yahoo.com", "993", false),
            preset_btn("Proton Bridge", "127.0.0.1", "1143", true),
            preset_btn("iCloud", "imap.mail.me.com", "993", false),
            preset_btn("Fastmail", "imap.fastmail.com", "993", false),
        ]
        .spacing(5)
        .align_y(Alignment::Center),
        row![
            text("Host:").size(12).width(Length::Fixed(65.0)),
            text_input("imap.gmail.com", &state.imap_host)
                .on_input(Message::ImapHostChanged)
                .size(12)
                .width(Length::Fill),
            text("Port:").size(12),
            text_input("993", &state.imap_port)
                .on_input(Message::ImapPortChanged)
                .size(12)
                .width(Length::Fixed(70.0)),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        row![
            checkbox(state.imap_use_tls)
                .label("Use SSL/TLS")
                .on_toggle(Message::ImapTlsToggled),
            checkbox(state.imap_insecure_certs)
                .label("Accept Untrusted / Self-Signed Certs")
                .on_toggle(Message::ImapInsecureToggled),
        ]
        .spacing(12),
        row![
            text("Auth:").size(12).width(Length::Fixed(65.0)),
            toggle_btn(
                "Password",
                state.imap_auth_method == "password",
                Message::ImapAuthMethodChanged("password".to_string()),
            ),
            toggle_btn(
                "XOAUTH2",
                state.imap_auth_method == "xoauth2",
                Message::ImapAuthMethodChanged("xoauth2".to_string()),
            ),
            toggle_btn(
                "OAUTHBEARER",
                state.imap_auth_method == "oauthbearer",
                Message::ImapAuthMethodChanged("oauthbearer".to_string()),
            ),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        row![
            text("Username:").size(12).width(Length::Fixed(65.0)),
            text_input("user@example.com", &state.imap_username)
                .on_input(Message::ImapUserChanged)
                .size(12)
                .width(Length::Fill),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    ]
    .spacing(8);

    if state.imap_auth_method == "password" {
        col = col.push(
            row![
                text("Password:").size(12).width(Length::Fixed(65.0)),
                text_input("Account or 16-char App Password", &state.imap_password)
                    .on_input(Message::ImapPassChanged)
                    .secure(true)
                    .size(12)
                    .width(Length::Fill),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        );
    } else {
        col = col.push(
            row![
                text("OAuth Token:").size(12).width(Length::Fixed(75.0)),
                text_input("Bearer access token", &state.imap_oauth_token)
                    .on_input(Message::ImapTokenChanged)
                    .secure(true)
                    .size(12)
                    .width(Length::Fill),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        );
    }

    col = col.push(
        row![
            text("Folders:").size(12).width(Length::Fixed(65.0)),
            text_input("INBOX, Sent (optional)", &state.imap_folder_filter)
                .on_input(Message::ImapFoldersChanged)
                .size(12)
                .width(Length::Fill),
            text("Max:").size(12),
            text_input("Unlimited", &state.imap_max_per_folder)
                .on_input(Message::ImapMaxChanged)
                .size(12)
                .width(Length::Fixed(90.0)),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    );

    if let Some(st) = &state.imap_status {
        col = col.push(text(st).size(11));
    }

    col.push(
        row![
            space().width(Length::Fill),
            button(text("Connect & Ingest").size(12))
                .on_press(Message::ExecuteImapConnect)
                .style(button::primary),
            button(text("Close").size(12))
                .on_press(Message::CloseModal)
                .style(button::secondary),
        ]
        .spacing(8),
    )
    .into()
}

fn view_imap_sync(state: &AppState) -> Element<'_, Message> {
    let src_cred: Element<'_, Message> = if state.imap_sync_src_auth_method == "password" {
        text_input("Source Password", &state.imap_sync_src_pass)
            .on_input(Message::ImapSyncSrcPassChanged)
            .secure(true)
            .size(12)
            .width(Length::Fill)
            .into()
    } else {
        text_input("Source OAuth Token", &state.imap_sync_src_oauth_token)
            .on_input(Message::ImapSyncSrcTokenChanged)
            .secure(true)
            .size(12)
            .width(Length::Fill)
            .into()
    };

    let dst_cred: Element<'_, Message> = if state.imap_sync_dst_auth_method == "password" {
        text_input("Dest Password", &state.imap_sync_dst_pass)
            .on_input(Message::ImapSyncDstPassChanged)
            .secure(true)
            .size(12)
            .width(Length::Fill)
            .into()
    } else {
        text_input("Dest OAuth Token", &state.imap_sync_dst_oauth_token)
            .on_input(Message::ImapSyncDstTokenChanged)
            .secure(true)
            .size(12)
            .width(Length::Fill)
            .into()
    };

    let mut col = column![
        text("IMAP Server-to-Server Account Migration (Zero Local Disk)").size(17),
        rule::horizontal(1),
        text("Source Account").size(13),
        row![
            text_input("Source Host", &state.imap_sync_src_host)
                .on_input(Message::ImapSyncSrcHostChanged)
                .size(12)
                .width(Length::Fill),
            text_input("993", &state.imap_sync_src_port)
                .on_input(Message::ImapSyncSrcPortChanged)
                .size(12)
                .width(Length::Fixed(65.0)),
            text_input("Source User", &state.imap_sync_src_user)
                .on_input(Message::ImapSyncSrcUserChanged)
                .size(12)
                .width(Length::Fill),
            src_cred,
        ]
        .spacing(5),
        row![
            text("Src Auth:").size(11),
            toggle_btn(
                "Password",
                state.imap_sync_src_auth_method == "password",
                Message::ImapSyncSrcAuthMethodChanged("password".to_string()),
            ),
            toggle_btn(
                "XOAUTH2",
                state.imap_sync_src_auth_method == "xoauth2",
                Message::ImapSyncSrcAuthMethodChanged("xoauth2".to_string()),
            ),
            toggle_btn(
                "OAUTHBEARER",
                state.imap_sync_src_auth_method == "oauthbearer",
                Message::ImapSyncSrcAuthMethodChanged("oauthbearer".to_string()),
            ),
            checkbox(state.imap_sync_src_tls)
                .label("Source TLS")
                .on_toggle(Message::ImapSyncSrcTlsToggled),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        text("Destination Account").size(13),
        row![
            text_input("Dest Host", &state.imap_sync_dst_host)
                .on_input(Message::ImapSyncDstHostChanged)
                .size(12)
                .width(Length::Fill),
            text_input("993", &state.imap_sync_dst_port)
                .on_input(Message::ImapSyncDstPortChanged)
                .size(12)
                .width(Length::Fixed(65.0)),
            text_input("Dest User", &state.imap_sync_dst_user)
                .on_input(Message::ImapSyncDstUserChanged)
                .size(12)
                .width(Length::Fill),
            dst_cred,
        ]
        .spacing(5),
        row![
            text("Dst Auth:").size(11),
            toggle_btn(
                "Password",
                state.imap_sync_dst_auth_method == "password",
                Message::ImapSyncDstAuthMethodChanged("password".to_string()),
            ),
            toggle_btn(
                "XOAUTH2",
                state.imap_sync_dst_auth_method == "xoauth2",
                Message::ImapSyncDstAuthMethodChanged("xoauth2".to_string()),
            ),
            toggle_btn(
                "OAUTHBEARER",
                state.imap_sync_dst_auth_method == "oauthbearer",
                Message::ImapSyncDstAuthMethodChanged("oauthbearer".to_string()),
            ),
            checkbox(state.imap_sync_dst_tls)
                .label("Dest TLS")
                .on_toggle(Message::ImapSyncDstTlsToggled),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        row![
            text_input("Folder Filter (optional)", &state.imap_sync_folders)
                .on_input(Message::ImapSyncFoldersChanged)
                .size(12)
                .width(Length::Fill),
            text_input("Target Prefix (optional)", &state.imap_sync_folder_prefix)
                .on_input(Message::ImapSyncPrefixChanged)
                .size(12)
                .width(Length::Fill),
        ]
        .spacing(5),
        row![
            checkbox(state.imap_sync_preserve_flags)
                .label("Preserve Flags")
                .on_toggle(Message::ImapSyncFlagsToggled),
            checkbox(state.imap_sync_preserve_dates)
                .label("Preserve INTERNALDATE")
                .on_toggle(Message::ImapSyncDatesToggled),
            checkbox(state.imap_sync_insecure)
                .label("Self-Signed TLS")
                .on_toggle(Message::ImapSyncInsecureToggled),
            checkbox(state.imap_sync_dry_run)
                .label("Dry Run")
                .on_toggle(Message::ImapSyncDryRunToggled),
        ]
        .spacing(12),
    ]
    .spacing(8);

    if let Some(st) = &state.imap_sync_status {
        col = col.push(text(st).size(11));
    }

    col.push(
        row![
            space().width(Length::Fill),
            button(text("Start Server-to-Server Transfer").size(12))
                .on_press(Message::ExecuteImapSync)
                .style(button::primary),
            button(text("Close").size(12))
                .on_press(Message::CloseModal)
                .style(button::secondary),
        ]
        .spacing(8),
    )
    .into()
}

fn view_merge_mailboxes(state: &AppState) -> Element<'_, Message> {
    let mut sources_list = column![].spacing(4);
    if state.merge_source_paths.is_empty() {
        sources_list = sources_list.push(text("No source mailboxes added yet.").size(11));
    } else {
        for (idx, p) in state.merge_source_paths.iter().enumerate() {
            sources_list = sources_list.push(
                row![
                    text(p).size(11).width(Length::Fill),
                    button(text("X").size(10))
                        .on_press(Message::MergeRemoveSource(idx))
                        .style(button::danger),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            );
        }
    }

    let mut col = column![
        text("Merge Mailboxes (Multi-Format Consolidation)").size(17),
        rule::horizontal(1),
        row![
            text_input("Source path to add...", &state.merge_source_input)
                .on_input(Message::MergeInputChanged)
                .size(12)
                .width(Length::Fill),
            button(text("Add").size(11))
                .on_press(Message::MergeAddInputPath)
                .style(button::secondary),
            button(text("File...").size(11))
                .on_press(Message::MergeBrowseAddFile)
                .style(button::secondary),
            button(text("Folder...").size(11))
                .on_press(Message::MergeBrowseAddFolder)
                .style(button::secondary),
            button(text("Samples").size(11))
                .on_press(Message::MergeAddSamples)
                .style(button::secondary),
            button(text("Clear").size(11))
                .on_press(Message::MergeClearSources)
                .style(button::secondary),
        ]
        .spacing(5),
        container(scrollable(sources_list).height(Length::Fixed(100.0)))
            .padding(6)
            .width(Length::Fill)
            .style(container::bordered_box),
        row![
            text("Folders:").size(11),
            toggle_btn(
                "Combine",
                state.merge_folder_strategy == "combine",
                Message::MergeFolderStrategyChanged("combine".to_string()),
            ),
            toggle_btn(
                "Prefix",
                state.merge_folder_strategy == "prefix",
                Message::MergeFolderStrategyChanged("prefix".to_string()),
            ),
            toggle_btn(
                "Flatten",
                state.merge_folder_strategy == "flatten",
                Message::MergeFolderStrategyChanged("flatten".to_string()),
            ),
        ]
        .spacing(5)
        .align_y(Alignment::Center),
        row![
            text("Dedup:").size(11),
            toggle_btn(
                "Message-ID",
                state.merge_dedup_strategy == "message-id",
                Message::MergeDedupStrategyChanged("message-id".to_string()),
            ),
            toggle_btn(
                "SHA-256",
                state.merge_dedup_strategy == "sha256",
                Message::MergeDedupStrategyChanged("sha256".to_string()),
            ),
            toggle_btn(
                "Content",
                state.merge_dedup_strategy == "content",
                Message::MergeDedupStrategyChanged("content".to_string()),
            ),
            toggle_btn(
                "None",
                state.merge_dedup_strategy == "none",
                Message::MergeDedupStrategyChanged("none".to_string()),
            ),
        ]
        .spacing(5)
        .align_y(Alignment::Center),
        row![
            text("Format:").size(11),
            pick_list(
                EXPORT_FORMATS,
                Some(state.merge_export_format),
                Message::MergeFormatChanged
            )
            .text_size(11),
            checkbox(state.merge_sort_by_date)
                .label("Sort Chronologically (UTC)")
                .on_toggle(Message::MergeSortToggled),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        row![
            text("Target:").size(12).width(Length::Fixed(60.0)),
            text_input("merged_archive.pst", &state.merge_dest_path)
                .on_input(Message::MergeDestChanged)
                .size(12)
                .width(Length::Fill),
            button(text("Browse...").size(12))
                .on_press(Message::MergeBrowseDest)
                .style(button::secondary),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    ]
    .spacing(7);

    if let Some(st) = &state.merge_status {
        col = col.push(text(st).size(11));
    }
    if let Some(rep) = &state.merge_report_text {
        col = col.push(text(rep).size(10));
    }

    col.push(
        row![
            space().width(Length::Fill),
            button(text("Merge & Load Workbench").size(12))
                .on_press(Message::ExecuteMergeToWorkbench)
                .style(button::secondary),
            button(text("Merge & Export File").size(12))
                .on_press(Message::ExecuteMergeToFile)
                .style(button::primary),
            button(text("Close").size(12))
                .on_press(Message::CloseModal)
                .style(button::secondary),
        ]
        .spacing(8),
    )
    .into()
}

fn view_direct_export(state: &AppState) -> Element<'_, Message> {
    let mut col = column![
        text("Export Mailbox Archive / Container").size(17),
        rule::horizontal(1),
        row![
            text("Scope:").size(12).width(Length::Fixed(70.0)),
            toggle_btn(
                "All Messages",
                state.export_scope == ExportScope::All,
                Message::ExportScopeChanged(ExportScope::All),
            ),
            toggle_btn(
                "Current Filter",
                state.export_scope == ExportScope::Filtered,
                Message::ExportScopeChanged(ExportScope::Filtered),
            ),
            toggle_btn(
                "Checked Items",
                state.export_scope == ExportScope::Selected,
                Message::ExportScopeChanged(ExportScope::Selected),
            ),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        row![
            text("Format:").size(12).width(Length::Fixed(70.0)),
            pick_list(EXPORT_FORMATS, Some(state.export_format), Message::ExportFormatChanged).text_size(12),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        row![
            text("Target:").size(12).width(Length::Fixed(70.0)),
            text_input("Destination file or directory", &state.pst_dest_path)
                .on_input(Message::ExportDestChanged)
                .size(12)
                .width(Length::Fill),
            button(text("Browse...").size(12))
                .on_press(Message::ExportBrowseDest)
                .style(button::secondary),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    ]
    .spacing(8);

    if state.export_format == ExportFormat::Pst {
        col = col
            .push(
                row![
                    text("PST Root:").size(12).width(Length::Fixed(70.0)),
                    text_input("Imported (Mail2Rust)", &state.pst_root)
                        .on_input(Message::ExportRootChanged)
                        .size(12)
                        .width(Length::Fill),
                    text("Max GB:").size(12),
                    text_input("20", &state.pst_max_gb.to_string())
                        .on_input(Message::ExportMaxGbChanged)
                        .size(12)
                        .width(Length::Fixed(60.0)),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .push(
                checkbox(state.pst_cluster_threads)
                    .label("Preserve conversation threads across split volumes")
                    .on_toggle(Message::ExportClusterToggled),
            )
            .push(
                row![
                    text("Encrypt:").size(12).width(Length::Fixed(70.0)),
                    toggle_btn(
                        "Permute",
                        state.pst_encryption == PstEncryption::Permute,
                        Message::ExportEncryptionChanged(PstEncryption::Permute),
                    ),
                    toggle_btn(
                        "Cyclic (High)",
                        state.pst_encryption == PstEncryption::Cyclic,
                        Message::ExportEncryptionChanged(PstEncryption::Cyclic),
                    ),
                    toggle_btn(
                        "None",
                        state.pst_encryption == PstEncryption::None,
                        Message::ExportEncryptionChanged(PstEncryption::None),
                    ),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .push(
                row![
                    text("Password:").size(12).width(Length::Fixed(70.0)),
                    text_input("(Optional PST password)", &state.pst_password)
                        .on_input(Message::ExportPasswordChanged)
                        .secure(true)
                        .size(12)
                        .width(Length::Fill),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            );
    }

    if let Some(st) = &state.pst_status_message {
        col = col.push(text(st).size(11));
    }

    col.push(
        row![
            space().width(Length::Fill),
            button(text("Execute Export").size(12))
                .on_press(Message::ExecuteExport)
                .style(button::primary),
            button(text("Close").size(12))
                .on_press(Message::CloseModal)
                .style(button::secondary),
        ]
        .spacing(8),
    )
    .into()
}

fn view_auto_split(state: &AppState) -> Element<'_, Message> {
    let mut col = column![
        text("Auto-Split Mailbox by Capacity").size(17),
        rule::horizontal(1),
        text("Partition oversized mail archives into multi-volume containers without severing reply chains.").size(12),
        row![
            toggle_btn(
                "15 GB Safe Ceiling",
                state.pst_max_gb == 15,
                Message::AutoSplitSelectGb(15),
            ),
            toggle_btn("20 GB Default", state.pst_max_gb == 20, Message::AutoSplitSelectGb(20),),
            toggle_btn(
                "50 GB Max Unicode",
                state.pst_max_gb == 50,
                Message::AutoSplitSelectGb(50),
            ),
        ]
        .spacing(6),
        checkbox(state.pst_cluster_threads)
            .label("Maintain Thread Integrity (keep conversations together)")
            .on_toggle(Message::ExportClusterToggled),
    ]
    .spacing(8);

    if let Some(st) = &state.pst_status_message {
        col = col.push(text(st).size(11));
    }

    col.push(
        row![
            space().width(Length::Fill),
            button(text("Compute Split Plan").size(12))
                .on_press(Message::ExecuteAutoSplit)
                .style(button::primary),
            button(text("Close").size(12))
                .on_press(Message::CloseModal)
                .style(button::secondary),
        ]
        .spacing(8),
    )
    .into()
}

fn view_filter_criteria(state: &AppState) -> Element<'_, Message> {
    column![
        text("Query & Date Filter Criteria").size(17),
        rule::horizontal(1),
        row![
            text("Since Date:").size(12).width(Length::Fixed(90.0)),
            text_input("YYYY-MM-DD", &state.filter_since)
                .on_input(Message::FilterSinceChanged)
                .size(12)
                .width(Length::Fill),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        row![
            text("Until Date:").size(12).width(Length::Fixed(90.0)),
            text_input("YYYY-MM-DD", &state.filter_until)
                .on_input(Message::FilterUntilChanged)
                .size(12)
                .width(Length::Fill),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        row![
            text("Include Glob:").size(12).width(Length::Fixed(90.0)),
            text_input("Projects/*", &state.filter_include_folder)
                .on_input(Message::FilterIncludeGlobChanged)
                .size(12)
                .width(Length::Fill),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        row![
            text("Exclude Glob:").size(12).width(Length::Fixed(90.0)),
            text_input("*Trash*;*Spam*", &state.filter_exclude_folder)
                .on_input(Message::FilterExcludeGlobChanged)
                .size(12)
                .width(Length::Fill),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        checkbox(state.filter_include_deleted)
            .label("Include deleted / expunged items")
            .on_toggle(Message::FilterIncludeDeletedToggled),
        row![
            space().width(Length::Fill),
            button(text("Reset").size(12))
                .on_press(Message::ResetFilters)
                .style(button::secondary),
            button(text("Apply & Close").size(12))
                .on_press(Message::CloseModal)
                .style(button::primary),
        ]
        .spacing(8),
    ]
    .spacing(8)
    .into()
}

fn view_forensics(state: &AppState) -> Element<'_, Message> {
    let mut col = column![
        text("Forensic Bates Stamping & Audit Manifest").size(17),
        rule::horizontal(1),
        row![
            text("Case Number:").size(12).width(Length::Fixed(100.0)),
            text_input("CASE-2026", &state.case_number)
                .on_input(Message::CaseNumberChanged)
                .size(12)
                .width(Length::Fill),
            text("Examiner:").size(12),
            text_input("Examiner Name", &state.examiner)
                .on_input(Message::ExaminerChanged)
                .size(12)
                .width(Length::Fill),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        row![
            text("Bates Prefix:").size(12).width(Length::Fixed(100.0)),
            text_input("CASE", &state.bates_prefix)
                .on_input(Message::BatesPrefixChanged)
                .size(12)
                .width(Length::Fill),
            text("Start #:").size(12),
            text_input("1000", &state.bates_start.to_string())
                .on_input(Message::BatesStartChanged)
                .size(12)
                .width(Length::Fixed(100.0)),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        row![
            text("Manifest CSV:").size(12).width(Length::Fixed(100.0)),
            text_input("Optional output manifest CSV path", &state.manifest_csv_path)
                .on_input(Message::ManifestCsvChanged)
                .size(12)
                .width(Length::Fill),
            button(text("Browse...").size(12))
                .on_press(Message::BrowseManifestCsv)
                .style(button::secondary),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    ]
    .spacing(8);

    if let Some(res) = &state.forensics_result {
        col = col.push(text(res).size(11));
    }

    col.push(
        row![
            space().width(Length::Fill),
            button(text("Execute Stamping").size(12))
                .on_press(Message::ExecuteForensicsStamping)
                .style(button::primary),
            button(text("Close").size(12))
                .on_press(Message::CloseModal)
                .style(button::secondary),
        ]
        .spacing(8),
    )
    .into()
}

fn view_dry_run(state: &AppState) -> Element<'_, Message> {
    column![
        text("Pre-Flight Mailbox Inventory").size(17),
        rule::horizontal(1),
        text(state.compute_dry_run_summary()).size(12),
        row![
            space().width(Length::Fill),
            button(text("Close").size(12))
                .on_press(Message::CloseModal)
                .style(button::primary),
        ],
    ]
    .spacing(10)
    .into()
}

fn view_validate_pst(state: &AppState) -> Element<'_, Message> {
    let mut col = column![
        text("MS-PST Container Validator").size(17),
        rule::horizontal(1),
        row![
            text("PST / OST File:").size(12).width(Length::Fixed(95.0)),
            text_input("Path to .pst or .ost file", &state.validate_pst_path)
                .on_input(Message::ValidatePstPathChanged)
                .size(12)
                .width(Length::Fill),
            button(text("Browse...").size(12))
                .on_press(Message::BrowseValidatePstFile)
                .style(button::secondary),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        row![
            text("Password:").size(12).width(Length::Fixed(95.0)),
            text_input("(Optional password check)", &state.validate_pst_password)
                .on_input(Message::ValidatePstPasswordChanged)
                .secure(true)
                .size(12)
                .width(Length::Fill),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    ]
    .spacing(8);

    if let Some(rep) = &state.validation_report {
        col = col.push(
            container(text(rep).size(11))
                .padding(8)
                .width(Length::Fill)
                .style(container::bordered_box),
        );
    }

    col.push(
        row![
            space().width(Length::Fill),
            button(text("Validate").size(12))
                .on_press(Message::ExecuteValidatePst)
                .style(button::primary),
            button(text("Close").size(12))
                .on_press(Message::CloseModal)
                .style(button::secondary),
        ]
        .spacing(8),
    )
    .into()
}

fn view_confirm_outlook_mutate(state: &AppState) -> Element<'_, Message> {
    let summary = if state.outlook_tool_idx == 0 {
        format!(
            "Deduplicate and atomically save '{}' (creating a .bak backup)?",
            state.outlook_source_path
        )
    } else if state.outlook_tool_idx == 4 {
        let prog_id = state
            .outlook_grid
            .selected_row()
            .and_then(|r| r.cells.get(2))
            .map(|c| c.to_string())
            .unwrap_or_else(|| "(none selected)".to_string());
        format!("Toggle LoadBehavior for Office add-in '{prog_id}' in live Windows Registry?")
    } else {
        "Select NK2Edit (dedup & save) or OfficeIns (toggle selected add-in) to perform a mutating action.".to_string()
    };

    column![
        text("Confirm Mutating Outlook / Office Action").size(17),
        rule::horizontal(1),
        text(summary).size(13),
        row![
            space().width(Length::Fill),
            button(text("Confirm & Execute").size(12))
                .on_press(Message::ConfirmOutlookMutate)
                .style(button::danger),
            button(text("Cancel").size(12))
                .on_press(Message::CloseModal)
                .style(button::secondary),
        ]
        .spacing(8),
    ]
    .spacing(10)
    .into()
}

fn view_about() -> Element<'static, Message> {
    column![
        text(format!("Mail2Rust Studio v{} (Iced 0.14)", env!("CARGO_PKG_VERSION"))).size(17),
        rule::horizontal(1),
        text("Universal High-Performance Email & [MS-PST] Engine in 100% Pure Rust").size(13),
        text("- Zero Microsoft Outlook or Windows COM runtime dependencies").size(12),
        text("- Native [MS-PST] 64-bit Unicode container writer, reader & CRC-32 validator").size(12),
        text("- Pure-Rust MBOX, Maildir, EML, EMLX, MSG, OLM, Babyl, IMAP & JMAP engines").size(12),
        text("- Forensic Bates stamping, SHA-256 Merkle verification, and thread clustering").size(12),
        text("- License: Apache License, Version 2.0").size(12),
        row![
            space().width(Length::Fill),
            button(text("OK").size(12))
                .on_press(Message::CloseModal)
                .style(button::primary),
        ],
    ]
    .spacing(8)
    .into()
}
