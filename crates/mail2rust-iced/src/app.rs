use crate::LaunchArgs;
use crate::message::Message;
use crate::state::{ActiveModal, AppState, ExportFormat};
use crate::views;
use iced::widget::{column, container, mouse_area, row, rule, space, text};
use iced::{Alignment, Color, Element, Length, Task, Theme};
use std::path::Path;

pub struct Mail2RustApp {
    pub state: AppState,
}

fn mail_file_dialog(title: &str) -> rfd::FileDialog {
    rfd::FileDialog::new()
        .set_title(title)
        .add_filter(
            "All Supported Mail Files",
            &[
                "pst", "ost", "mbox", "mbx", "eml", "emlx", "msg", "olm", "zip", "gz", "bz2", "xz", "tar", "tgz",
            ],
        )
        .add_filter("Outlook PST / OST", &["pst", "ost"])
        .add_filter("MBOX Mailbox", &["mbox", "mbx"])
        .add_filter("Email Messages", &["eml", "emlx", "msg"])
        .add_filter("All Files", &["*"])
}

fn copy_to_clipboard(text_val: &str) -> bool {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        return cb.set_text(text_val.to_string()).is_ok();
    }
    false
}

impl Mail2RustApp {
    pub fn new(launch: LaunchArgs) -> (Self, Task<Message>) {
        let mut state = AppState::default();
        if let Some(p) = launch.path {
            state.open_mailbox_path = p.to_string_lossy().to_string();
            state.execute_open_mailbox();
        } else if launch.sample {
            state.load_sample_data();
        }
        (Self { state }, Task::none())
    }

    pub fn title(&self) -> String {
        format!(
            "Mail2Rust Studio (Iced v{}) — {}",
            env!("CARGO_PKG_VERSION"),
            self.state.active_folder
        )
    }

    pub fn theme(&self) -> Theme {
        if self.state.dark_mode {
            Theme::Dark
        } else {
            Theme::Light
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectFolder(name) => {
                self.state.active_folder = name;
            }
            Message::ToggleFolderChecked(idx) => {
                if let Some(f) = self.state.folders.get_mut(idx) {
                    f.selected = !f.selected;
                }
            }
            Message::SelectAllFolders => {
                for f in &mut self.state.folders {
                    f.selected = true;
                }
            }
            Message::SelectNoFolders => {
                for f in &mut self.state.folders {
                    f.selected = false;
                }
            }
            Message::SelectEmail(id) => {
                self.state.selected_message_id = Some(id);
            }
            Message::ToggleMessageChecked(id) => {
                self.state.toggle_message_selection(&id);
            }
            Message::SelectAllVisible => {
                self.state.select_all_filtered();
            }
            Message::ClearSelection => {
                self.state.clear_selection();
            }
            Message::InvertSelection => {
                self.state.invert_selection_filtered();
            }
            Message::ToggleSort(col) => {
                self.state.toggle_sort(col);
            }
            Message::SearchChanged(q) => {
                self.state.search_query = q;
            }
            Message::ClearSearch => {
                self.state.search_query.clear();
            }
            Message::SetActiveTab(tab) => {
                self.state.active_tab = tab;
            }
            Message::ToggleTreePane => {
                self.state.show_tree_pane = !self.state.show_tree_pane;
            }
            Message::TogglePreviewPane => {
                self.state.show_preview_pane = !self.state.show_preview_pane;
            }
            Message::ToggleDarkMode => {
                self.state.dark_mode = !self.state.dark_mode;
            }
            Message::RefreshWorkspace => {
                self.state.rebuild_folders_from_messages();
            }
            Message::LoadSampleData => {
                self.state.load_sample_data();
            }
            Message::ClearData => {
                self.state.clear_data();
            }
            Message::CopyText(s) => {
                if copy_to_clipboard(&s) {
                    self.state.status_message = "Copied text to clipboard.".to_string();
                }
            }
            Message::QuickOpenFile => {
                if let Some(picked) = mail_file_dialog("Open Mailbox File").pick_file() {
                    let _ = self.state.ingest_from_path(&picked);
                }
            }
            Message::QuickOpenFolder => {
                if let Some(picked) = rfd::FileDialog::new()
                    .set_title("Open Mailbox Directory (Thunderbird Profile, Maildir, EML Tree)")
                    .pick_folder()
                {
                    let _ = self.state.ingest_from_path(&picked);
                }
            }
            Message::OpenModal(modal) => {
                self.state.active_modal = modal;
            }
            Message::CloseModal => {
                self.state.active_modal = ActiveModal::None;
            }

            // Open Mailbox modal
            Message::OpenMailboxPathChanged(p) => {
                self.state.open_mailbox_path = p;
            }
            Message::BrowseOpenMailboxFile => {
                if let Some(picked) = mail_file_dialog("Select Mailbox Archive File").pick_file() {
                    self.state.open_mailbox_path = picked.to_string_lossy().to_string();
                }
            }
            Message::BrowseOpenMailboxFolder => {
                if let Some(picked) = rfd::FileDialog::new()
                    .set_title("Select Mailbox Directory (Thunderbird Profile, Maildir, EML Tree)")
                    .pick_folder()
                {
                    self.state.open_mailbox_path = picked.to_string_lossy().to_string();
                }
            }
            Message::ExecuteOpenMailbox => {
                self.state.execute_open_mailbox();
            }

            // IMAP Connect
            Message::ApplyImapPreset { host, port, insecure } => {
                self.state.imap_host = host;
                self.state.imap_port = port;
                self.state.imap_use_tls = true;
                self.state.imap_insecure_certs = insecure;
            }
            Message::ImapHostChanged(v) => self.state.imap_host = v,
            Message::ImapPortChanged(v) => self.state.imap_port = v,
            Message::ImapAuthMethodChanged(v) => self.state.imap_auth_method = v,
            Message::ImapUserChanged(v) => self.state.imap_username = v,
            Message::ImapPassChanged(v) => self.state.imap_password = v,
            Message::ImapTokenChanged(v) => self.state.imap_oauth_token = v,
            Message::ImapTlsToggled(v) => self.state.imap_use_tls = v,
            Message::ImapInsecureToggled(v) => self.state.imap_insecure_certs = v,
            Message::ImapFoldersChanged(v) => self.state.imap_folder_filter = v,
            Message::ImapMaxChanged(v) => self.state.imap_max_per_folder = v,
            Message::ExecuteImapConnect => self.state.execute_imap_connect(),

            // IMAP Sync
            Message::ImapSyncSrcHostChanged(v) => self.state.imap_sync_src_host = v,
            Message::ImapSyncSrcPortChanged(v) => self.state.imap_sync_src_port = v,
            Message::ImapSyncSrcUserChanged(v) => self.state.imap_sync_src_user = v,
            Message::ImapSyncSrcPassChanged(v) => self.state.imap_sync_src_pass = v,
            Message::ImapSyncSrcAuthMethodChanged(v) => self.state.imap_sync_src_auth_method = v,
            Message::ImapSyncSrcTokenChanged(v) => self.state.imap_sync_src_oauth_token = v,
            Message::ImapSyncSrcTlsToggled(v) => self.state.imap_sync_src_tls = v,
            Message::ImapSyncDstHostChanged(v) => self.state.imap_sync_dst_host = v,
            Message::ImapSyncDstPortChanged(v) => self.state.imap_sync_dst_port = v,
            Message::ImapSyncDstUserChanged(v) => self.state.imap_sync_dst_user = v,
            Message::ImapSyncDstPassChanged(v) => self.state.imap_sync_dst_pass = v,
            Message::ImapSyncDstAuthMethodChanged(v) => self.state.imap_sync_dst_auth_method = v,
            Message::ImapSyncDstTokenChanged(v) => self.state.imap_sync_dst_oauth_token = v,
            Message::ImapSyncDstTlsToggled(v) => self.state.imap_sync_dst_tls = v,
            Message::ImapSyncInsecureToggled(v) => self.state.imap_sync_insecure = v,
            Message::ImapSyncFoldersChanged(v) => self.state.imap_sync_folders = v,
            Message::ImapSyncPrefixChanged(v) => self.state.imap_sync_folder_prefix = v,
            Message::ImapSyncFlagsToggled(v) => self.state.imap_sync_preserve_flags = v,
            Message::ImapSyncDatesToggled(v) => self.state.imap_sync_preserve_dates = v,
            Message::ImapSyncDryRunToggled(v) => self.state.imap_sync_dry_run = v,
            Message::ExecuteImapSync => self.state.execute_imap_sync(),

            // Merge Mailboxes
            Message::MergeInputChanged(v) => self.state.merge_source_input = v,
            Message::MergeAddInputPath => {
                let t = self.state.merge_source_input.trim().to_string();
                if !t.is_empty() && !self.state.merge_source_paths.contains(&t) {
                    self.state.merge_source_paths.push(t);
                    self.state.merge_source_input.clear();
                }
            }
            Message::MergeBrowseAddFile => {
                if let Some(files) = mail_file_dialog("Select Source Mailbox File(s)").pick_files() {
                    for picked in files {
                        let s = picked.to_string_lossy().to_string();
                        if !self.state.merge_source_paths.contains(&s) {
                            self.state.merge_source_paths.push(s);
                        }
                    }
                }
            }
            Message::MergeBrowseAddFolder => {
                if let Some(picked) = rfd::FileDialog::new()
                    .set_title("Select Source Mailbox Directory")
                    .pick_folder()
                {
                    let s = picked.to_string_lossy().to_string();
                    if !self.state.merge_source_paths.contains(&s) {
                        self.state.merge_source_paths.push(s);
                    }
                }
            }
            Message::MergeAddSamples => {
                for c in [
                    "samples/takeout/sample_google_takeout.zip",
                    "samples/maildir",
                    "samples/emlx/sample_apple_mail.emlx",
                    "samples/contacts/sample_contacts.vcf",
                    "samples/calendar/sample_calendar.ics",
                    "samples/mbox/sample_mailbox.mbox",
                ] {
                    if Path::new(c).exists() && !self.state.merge_source_paths.iter().any(|x| x == c) {
                        self.state.merge_source_paths.push(c.to_string());
                    }
                }
            }
            Message::MergeClearSources => {
                self.state.merge_source_paths.clear();
            }
            Message::MergeRemoveSource(idx) => {
                if idx < self.state.merge_source_paths.len() {
                    self.state.merge_source_paths.remove(idx);
                }
            }
            Message::MergeFolderStrategyChanged(v) => self.state.merge_folder_strategy = v,
            Message::MergeDedupStrategyChanged(v) => self.state.merge_dedup_strategy = v,
            Message::MergeFormatChanged(v) => self.state.merge_export_format = v,
            Message::MergeSortToggled(v) => self.state.merge_sort_by_date = v,
            Message::MergeDestChanged(v) => self.state.merge_dest_path = v,
            Message::MergeBrowseDest => {
                let picked = match self.state.merge_export_format {
                    ExportFormat::Pst => rfd::FileDialog::new()
                        .set_title("Select Merged PST Target")
                        .add_filter("Outlook PST", &["pst"])
                        .set_file_name("merged_archive.pst")
                        .save_file(),
                    ExportFormat::Mbox => rfd::FileDialog::new()
                        .set_title("Select Merged MBOX Target")
                        .add_filter("MBOX", &["mbox"])
                        .set_file_name("merged_archive.mbox")
                        .save_file(),
                    ExportFormat::Eml => rfd::FileDialog::new()
                        .set_title("Select Merged EML Directory")
                        .pick_folder(),
                };
                if let Some(p) = picked {
                    self.state.merge_dest_path = p.to_string_lossy().to_string();
                }
            }
            Message::ExecuteMergeToWorkbench => self.state.execute_merge_to_workbench(),
            Message::ExecuteMergeToFile => self.state.execute_merge_to_file(),

            // Export Modal
            Message::ExportScopeChanged(v) => self.state.export_scope = v,
            Message::ExportFormatChanged(v) => self.state.export_format = v,
            Message::ExportDestChanged(v) => self.state.pst_dest_path = v,
            Message::ExportBrowseDest => {
                let picked = match self.state.export_format {
                    ExportFormat::Pst => rfd::FileDialog::new()
                        .set_title("Select Destination Outlook PST File")
                        .add_filter("Outlook PST", &["pst"])
                        .set_file_name("Export.pst")
                        .save_file(),
                    ExportFormat::Mbox => rfd::FileDialog::new()
                        .set_title("Select Destination MBOX File")
                        .add_filter("MBOX", &["mbox"])
                        .set_file_name("Export.mbox")
                        .save_file(),
                    ExportFormat::Eml => rfd::FileDialog::new()
                        .set_title("Select Destination EML Directory")
                        .pick_folder(),
                };
                if let Some(p) = picked {
                    self.state.pst_dest_path = p.to_string_lossy().to_string();
                }
            }
            Message::ExportRootChanged(v) => self.state.pst_root = v,
            Message::ExportMaxGbChanged(v) => {
                if let Ok(gb) = v.trim().parse::<usize>() {
                    self.state.pst_max_gb = gb.clamp(1, 50);
                }
            }
            Message::ExportClusterToggled(v) => self.state.pst_cluster_threads = v,
            Message::ExportEncryptionChanged(v) => self.state.pst_encryption = v,
            Message::ExportPasswordChanged(v) => self.state.pst_password = v,
            Message::ExecuteExport => self.state.execute_export(),

            // Auto-Split Modal
            Message::AutoSplitSelectGb(gb) => self.state.pst_max_gb = gb,
            Message::ExecuteAutoSplit => self.state.execute_auto_split(),

            // Filters Modal
            Message::FilterSinceChanged(v) => self.state.filter_since = v,
            Message::FilterUntilChanged(v) => self.state.filter_until = v,
            Message::FilterIncludeGlobChanged(v) => self.state.filter_include_folder = v,
            Message::FilterExcludeGlobChanged(v) => self.state.filter_exclude_folder = v,
            Message::FilterIncludeDeletedToggled(v) => self.state.filter_include_deleted = v,
            Message::ResetFilters => {
                self.state.filter_since.clear();
                self.state.filter_until.clear();
                self.state.filter_include_folder.clear();
                self.state.filter_exclude_folder = "*Trash*;*Spam*".to_string();
                self.state.filter_include_deleted = false;
            }

            // Forensics Modal
            Message::CaseNumberChanged(v) => self.state.case_number = v,
            Message::ExaminerChanged(v) => self.state.examiner = v,
            Message::BatesPrefixChanged(v) => self.state.bates_prefix = v,
            Message::BatesStartChanged(v) => {
                if let Ok(n) = v.trim().parse::<usize>() {
                    self.state.bates_start = n;
                }
            }
            Message::ManifestCsvChanged(v) => self.state.manifest_csv_path = v,
            Message::BrowseManifestCsv => {
                if let Some(p) = rfd::FileDialog::new()
                    .set_title("Select Destination Manifest CSV")
                    .add_filter("CSV Spreadsheet", &["csv"])
                    .set_file_name("forensic_manifest.csv")
                    .save_file()
                {
                    self.state.manifest_csv_path = p.to_string_lossy().to_string();
                }
            }
            Message::ExecuteForensicsStamping => self.state.execute_forensics_stamping(),

            // PST Validator Modal
            Message::ValidatePstPathChanged(v) => self.state.validate_pst_path = v,
            Message::BrowseValidatePstFile => {
                if let Some(p) = rfd::FileDialog::new()
                    .set_title("Select PST or OST File to Validate")
                    .add_filter("Outlook Storage Files", &["pst", "ost"])
                    .add_filter("All Files", &["*"])
                    .pick_file()
                {
                    self.state.validate_pst_path = p.to_string_lossy().to_string();
                }
            }
            Message::ValidatePstPasswordChanged(v) => self.state.validate_pst_password = v,
            Message::ExecuteValidatePst => self.state.execute_validate_pst(),
        }

        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let toolbar = views::toolbar::view_toolbar(&self.state);
        let workbench = views::workbench::view_workbench(&self.state);

        let filtered_count = self.state.filtered_messages().len();
        let status_bar = container(
            row![
                text(&self.state.status_message).size(11).width(Length::Fill),
                space().width(Length::Fixed(8.0)),
                text(format!(
                    "Folder: {}  |  Total: {} (Shown: {}, Checked: {})  |  Pure Rust (Iced 0.14)",
                    self.state.active_folder,
                    self.state.messages.len(),
                    filtered_count,
                    self.state.selected_message_ids.len()
                ))
                .size(11),
            ]
            .spacing(12)
            .height(Length::Shrink)
            .align_y(Alignment::Center),
        )
        .padding([4, 10])
        .width(Length::Fill)
        .height(Length::Shrink)
        .style(container::bordered_box);

        let main_layout = column![toolbar, rule::horizontal(1), workbench, rule::horizontal(1), status_bar]
            .width(Length::Fill)
            .height(Length::Fill);

        if self.state.active_modal == ActiveModal::None {
            main_layout.into()
        } else {
            let modal_card = views::modals::view_modal(&self.state);
            let backdrop = mouse_area(
                container(modal_card)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .style(|_| container::Style {
                        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()),
                        ..Default::default()
                    }),
            )
            .on_press(Message::CloseModal);

            iced::widget::stack![main_layout, backdrop].into()
        }
    }
}
