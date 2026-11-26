use chrono::{TimeZone, Utc};
use mail2rust_core::{
    Attachment, DedupStrategy, EmailAddress, EmailMessage, FormatSniffer, Importance, MailboxMerger,
    MergeFolderStrategy, MergeOptions, MergeSourceConfig, MessageFlags,
};
use mail2rust_eml::{EmlExporter, EmlLayout};
use mail2rust_filter::{FolderFilter, SplitConfig, VolumeSplitter};
use mail2rust_forensics::ForensicEngine;
use mail2rust_mbox::MboxFileExporter;
use mail2rust_pst::{PstEncryption, PstReader, PstValidator, PstWriter};
use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Importance,
    Flags,
    Subject,
    From,
    Date,
    Size,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone)]
pub struct GuiFolder {
    pub name: String,
    pub message_count: usize,
    pub unread_count: usize,
    pub selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Preview,
    Headers,
    Mapi,
    Forensics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveModal {
    None,
    OpenMailbox,
    ImapConnect,
    ImapSync,
    MergeMailboxes,
    DirectPst,
    AutoSplit,
    FilterCriteria,
    Forensics,
    DryRun,
    ValidatePst,
    ConfirmOutlookMutate,
    About,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportScope {
    #[default]
    All,
    Filtered,
    Selected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportFormat {
    #[default]
    Pst,
    Mbox,
    Eml,
}

impl std::fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportFormat::Pst => f.write_str("Microsoft Outlook (.pst)"),
            ExportFormat::Mbox => f.write_str("Standard MBOX (.mbox)"),
            ExportFormat::Eml => f.write_str("EML Tree Directory"),
        }
    }
}

pub struct AppState {
    pub folders: Vec<GuiFolder>,
    pub active_folder: String,
    pub messages: Vec<EmailMessage>,
    pub selected_message_id: Option<String>,
    pub selected_message_ids: HashSet<String>,
    pub search_query: String,
    pub active_tab: ActiveTab,
    pub active_modal: ActiveModal,
    pub dark_mode: bool,

    pub sort_column: SortColumn,
    pub sort_direction: SortDirection,
    pub show_tree_pane: bool,
    pub show_preview_pane: bool,
    pub status_message: String,

    // Open Mailbox
    pub open_mailbox_path: String,
    pub open_mailbox_status: Option<String>,

    // IMAP Connect
    pub imap_host: String,
    pub imap_port: String,
    pub imap_username: String,
    pub imap_password: String,
    pub imap_auth_method: String,
    pub imap_oauth_token: String,
    pub imap_use_tls: bool,
    pub imap_insecure_certs: bool,
    pub imap_folder_filter: String,
    pub imap_max_per_folder: String,
    pub imap_status: Option<String>,

    // Server-to-Server IMAP Migration
    pub imap_sync_src_host: String,
    pub imap_sync_src_port: String,
    pub imap_sync_src_user: String,
    pub imap_sync_src_pass: String,
    pub imap_sync_src_auth_method: String,
    pub imap_sync_src_oauth_token: String,
    pub imap_sync_src_tls: bool,
    pub imap_sync_dst_host: String,
    pub imap_sync_dst_port: String,
    pub imap_sync_dst_user: String,
    pub imap_sync_dst_pass: String,
    pub imap_sync_dst_auth_method: String,
    pub imap_sync_dst_oauth_token: String,
    pub imap_sync_dst_tls: bool,
    pub imap_sync_insecure: bool,
    pub imap_sync_folders: String,
    pub imap_sync_folder_prefix: String,
    pub imap_sync_preserve_flags: bool,
    pub imap_sync_preserve_dates: bool,
    pub imap_sync_dry_run: bool,
    pub imap_sync_status: Option<String>,

    // Multi-Format Merger
    pub merge_source_paths: Vec<String>,
    pub merge_source_input: String,
    pub merge_dest_path: String,
    pub merge_folder_strategy: String,
    pub merge_dedup_strategy: String,
    pub merge_export_format: ExportFormat,
    pub merge_sort_by_date: bool,
    pub merge_status: Option<String>,
    pub merge_report_text: Option<String>,

    // Export & Auto-Split
    pub pst_dest_path: String,
    pub pst_max_gb: usize,
    pub pst_cluster_threads: bool,
    pub pst_encryption: PstEncryption,
    pub pst_password: String,
    pub pst_root: String,
    pub pst_status_message: Option<String>,
    pub export_scope: ExportScope,
    pub export_format: ExportFormat,

    // Filter Criteria
    pub filter_since: String,
    pub filter_until: String,
    pub filter_include_folder: String,
    pub filter_exclude_folder: String,
    pub filter_include_deleted: bool,

    // Forensics
    pub bates_prefix: String,
    pub bates_start: usize,
    pub case_number: String,
    pub examiner: String,
    pub manifest_csv_path: String,
    pub forensics_result: Option<String>,

    // PST Validator
    pub validate_pst_path: String,
    pub validate_pst_password: String,
    pub validation_report: Option<String>,

    // Outlook & Office Diagnostics (guap-gui-iced)
    pub show_outlook_tools: bool,
    pub outlook_tool_idx: usize,
    pub outlook_source_path: String,
    pub outlook_status: Option<String>,
    pub outlook_grid: guap_gui_iced::IcedDataGrid,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            folders: Vec::new(),
            active_folder: "All Mail".to_string(),
            messages: Vec::new(),
            selected_message_id: None,
            selected_message_ids: HashSet::new(),
            search_query: String::new(),
            active_tab: ActiveTab::Preview,
            active_modal: ActiveModal::None,
            dark_mode: false,

            sort_column: SortColumn::Date,
            sort_direction: SortDirection::Descending,
            show_tree_pane: true,
            show_preview_pane: true,
            status_message: "Ready - open a mailbox archive, connect to IMAP, or load sample data.".to_string(),

            open_mailbox_path: String::new(),
            open_mailbox_status: None,

            imap_host: "imap.gmail.com".to_string(),
            imap_port: "993".to_string(),
            imap_username: String::new(),
            imap_password: String::new(),
            imap_auth_method: "password".to_string(),
            imap_oauth_token: String::new(),
            imap_use_tls: true,
            imap_insecure_certs: false,
            imap_folder_filter: String::new(),
            imap_max_per_folder: String::new(),
            imap_status: None,

            imap_sync_src_host: "imap.gmail.com".to_string(),
            imap_sync_src_port: "993".to_string(),
            imap_sync_src_user: String::new(),
            imap_sync_src_pass: String::new(),
            imap_sync_src_auth_method: "password".to_string(),
            imap_sync_src_oauth_token: String::new(),
            imap_sync_src_tls: true,
            imap_sync_dst_host: "outlook.office365.com".to_string(),
            imap_sync_dst_port: "993".to_string(),
            imap_sync_dst_user: String::new(),
            imap_sync_dst_pass: String::new(),
            imap_sync_dst_auth_method: "password".to_string(),
            imap_sync_dst_oauth_token: String::new(),
            imap_sync_dst_tls: true,
            imap_sync_insecure: false,
            imap_sync_folders: String::new(),
            imap_sync_folder_prefix: String::new(),
            imap_sync_preserve_flags: true,
            imap_sync_preserve_dates: true,
            imap_sync_dry_run: false,
            imap_sync_status: None,

            merge_source_paths: Vec::new(),
            merge_source_input: String::new(),
            merge_dest_path: "merged_archive.pst".to_string(),
            merge_folder_strategy: "combine".to_string(),
            merge_dedup_strategy: "message-id".to_string(),
            merge_export_format: ExportFormat::Pst,
            merge_sort_by_date: true,
            merge_status: None,
            merge_report_text: None,

            pst_dest_path: String::new(),
            pst_max_gb: 20,
            pst_cluster_threads: true,
            pst_encryption: PstEncryption::Permute,
            pst_password: String::new(),
            pst_root: "Imported (Mail2Rust)".to_string(),
            pst_status_message: None,
            export_scope: ExportScope::All,
            export_format: ExportFormat::Pst,

            filter_since: String::new(),
            filter_until: String::new(),
            filter_include_folder: String::new(),
            filter_exclude_folder: "*Trash*;*Spam*".to_string(),
            filter_include_deleted: false,

            bates_prefix: "CASE".to_string(),
            bates_start: 1000,
            case_number: String::new(),
            examiner: String::new(),
            manifest_csv_path: String::new(),
            forensics_result: None,

            validate_pst_path: String::new(),
            validate_pst_password: String::new(),
            validation_report: None,

            show_outlook_tools: false,
            outlook_tool_idx: 0,
            outlook_source_path: String::new(),
            outlook_status: None,
            outlook_grid: guap_gui_iced::IcedDataGrid::new(Vec::new()),
        }
    }
}

impl AppState {
    pub fn load_sample_data(&mut self) {
        let (folders, messages) = generate_seed_data();
        self.selected_message_id = messages.first().map(|m| m.id.clone());
        self.folders = folders;
        self.messages = messages;
        self.active_folder = "Inbox".to_string();
        self.status_message = format!("Loaded {} sample evaluation messages.", self.messages.len());
    }

    pub fn clear_data(&mut self) {
        self.folders.clear();
        self.messages.clear();
        self.selected_message_id = None;
        self.selected_message_ids.clear();
        self.active_folder = "All Mail".to_string();
        self.status_message = "Cleared loaded messages.".to_string();
    }

    pub fn get_selected_message(&self) -> Option<&EmailMessage> {
        let id = self.selected_message_id.as_ref()?;
        self.messages.iter().find(|m| &m.id == id)
    }

    pub fn toggle_sort(&mut self, col: SortColumn) {
        if self.sort_column == col {
            self.sort_direction = match self.sort_direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => SortDirection::Ascending,
            };
        } else {
            self.sort_column = col;
            self.sort_direction = SortDirection::Ascending;
        }
    }

    pub fn toggle_message_selection(&mut self, id: &str) {
        if !self.selected_message_ids.remove(id) {
            self.selected_message_ids.insert(id.to_string());
        }
    }

    pub fn is_message_selected(&self, id: &str) -> bool {
        self.selected_message_ids.contains(id)
    }

    pub fn select_all_filtered(&mut self) {
        let ids: Vec<String> = self.filtered_messages().into_iter().map(|m| m.id.clone()).collect();
        for id in ids {
            self.selected_message_ids.insert(id);
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_message_ids.clear();
    }

    pub fn invert_selection_filtered(&mut self) {
        let ids: Vec<String> = self.filtered_messages().into_iter().map(|m| m.id.clone()).collect();
        for id in ids {
            if !self.selected_message_ids.remove(&id) {
                self.selected_message_ids.insert(id);
            }
        }
    }

    pub fn get_messages_for_export(&self) -> Vec<EmailMessage> {
        match self.export_scope {
            ExportScope::All => self.messages.clone(),
            ExportScope::Filtered => self.filtered_messages().into_iter().cloned().collect(),
            ExportScope::Selected => self
                .messages
                .iter()
                .filter(|m| self.selected_message_ids.contains(&m.id))
                .cloned()
                .collect(),
        }
    }

    pub fn rebuild_folders_from_messages(&mut self) {
        let mut map: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for m in &self.messages {
            let fname = m.folder_path.last().cloned().unwrap_or_else(|| "Inbox".to_string());
            let entry = map.entry(fname).or_insert((0, 0));
            entry.0 += 1;
            if !m.flags.read {
                entry.1 += 1;
            }
        }

        if map.is_empty() {
            map.insert("Inbox".to_string(), (0, 0));
        }

        self.folders = map
            .into_iter()
            .map(|(name, (count, unread))| GuiFolder {
                name,
                message_count: count,
                unread_count: unread,
                selected: true,
            })
            .collect();

        if let Some(first_folder) = self.folders.first().map(|f| f.name.clone()) {
            self.active_folder = first_folder;
        }
        self.selected_message_id = self.messages.first().map(|m| m.id.clone());
    }

    pub fn ingest_from_path(&mut self, p: &Path) -> Result<usize, Box<dyn std::error::Error>> {
        let msgs = ingest_any_mailbox(p)?;
        let count = msgs.len();
        self.messages = msgs;
        self.selected_message_ids.clear();
        self.rebuild_folders_from_messages();
        self.open_mailbox_path = p.to_string_lossy().to_string();
        let msg = format!(
            "Loaded {count} message(s) across {} folder(s) from: {}",
            self.folders.len(),
            p.display()
        );
        self.open_mailbox_status = Some(msg.clone());
        self.status_message = msg;
        Ok(count)
    }

    pub fn execute_open_mailbox(&mut self) {
        let path = self.open_mailbox_path.trim().to_string();
        if path.is_empty() {
            let err = "Error: Please specify a valid file or folder path.".to_string();
            self.open_mailbox_status = Some(err.clone());
            self.status_message = err;
            return;
        }
        if let Err(e) = self.ingest_from_path(Path::new(&path)) {
            let err = format!("Error: {e}");
            self.open_mailbox_status = Some(err.clone());
            self.status_message = err;
        }
    }

    pub fn execute_imap_connect(&mut self) {
        let host = self.imap_host.trim().to_string();
        let port = self.imap_port.trim().parse::<u16>().unwrap_or(993);
        let user = self.imap_username.trim().to_string();
        let pass = self.imap_password.clone();
        let auth_method: mail2rust_imap::ImapAuthMethod = self
            .imap_auth_method
            .parse()
            .unwrap_or(mail2rust_imap::ImapAuthMethod::Password);

        let cred_missing = if auth_method == mail2rust_imap::ImapAuthMethod::Password {
            pass.is_empty()
        } else {
            self.imap_oauth_token.trim().is_empty()
        };

        if host.is_empty() || user.is_empty() || cred_missing {
            self.imap_status = Some("Error: Please provide host, username, and password/token.".to_string());
            return;
        }

        let folder_filter = parse_csv_folders(&self.imap_folder_filter);
        let max_messages_per_folder = self.imap_max_per_folder.trim().parse::<usize>().ok();
        let oauth_token = if auth_method != mail2rust_imap::ImapAuthMethod::Password {
            Some(self.imap_oauth_token.trim().to_string())
        } else {
            None
        };

        let config = mail2rust_imap::ImapConfig {
            host: host.clone(),
            port,
            username: user.clone(),
            password: pass,
            use_tls: self.imap_use_tls,
            insecure_certs: self.imap_insecure_certs,
            folder_filter,
            max_messages_per_folder,
            since_date: None,
            auth_method,
            oauth_token,
        };

        let mut client = mail2rust_imap::ImapClient::new(config);
        match client.connect() {
            Ok(()) => {
                let mut checkpoint =
                    mail2rust_sync::CheckpointManager::new_checkpoint(&format!("imap://{host}/{user}"), "iced_studio");
                let mut ingested = Vec::new();
                match client.sync_all(&mut checkpoint, |_prog, msg| ingested.push(msg)) {
                    Ok(count) => {
                        self.messages = ingested;
                        self.selected_message_ids.clear();
                        self.rebuild_folders_from_messages();
                        let msg = format!(
                            "Successfully ingested {count} message(s) across {} folder(s).",
                            self.folders.len()
                        );
                        self.imap_status = Some(msg.clone());
                        self.status_message = msg;
                    }
                    Err(e) => {
                        self.imap_status = Some(format!("Error syncing mailboxes: {e}"));
                    }
                }
            }
            Err(e) => {
                self.imap_status = Some(mail2rust_imap::diagnose_auth_failure(&host, &e.to_string()));
            }
        }
    }

    pub fn execute_imap_sync(&mut self) {
        let src_host = self.imap_sync_src_host.trim().to_string();
        let src_user = self.imap_sync_src_user.trim().to_string();
        let src_pass = self.imap_sync_src_pass.clone();
        let dst_host = self.imap_sync_dst_host.trim().to_string();
        let dst_user = self.imap_sync_dst_user.trim().to_string();
        let dst_pass = self.imap_sync_dst_pass.clone();

        let src_auth: mail2rust_imap::ImapAuthMethod = self
            .imap_sync_src_auth_method
            .parse()
            .unwrap_or(mail2rust_imap::ImapAuthMethod::Password);
        let dst_auth: mail2rust_imap::ImapAuthMethod = self
            .imap_sync_dst_auth_method
            .parse()
            .unwrap_or(mail2rust_imap::ImapAuthMethod::Password);

        let src_ok = if src_auth == mail2rust_imap::ImapAuthMethod::Password {
            !src_pass.is_empty()
        } else {
            !self.imap_sync_src_oauth_token.trim().is_empty()
        };
        let dst_ok = if dst_auth == mail2rust_imap::ImapAuthMethod::Password {
            !dst_pass.is_empty()
        } else {
            !self.imap_sync_dst_oauth_token.trim().is_empty()
        };

        if src_host.is_empty() || src_user.is_empty() || !src_ok {
            self.imap_sync_status = Some("Error: Source host, username, and password/token are required.".to_string());
            return;
        }
        if dst_host.is_empty() || dst_user.is_empty() || !dst_ok {
            self.imap_sync_status =
                Some("Error: Destination host, username, and password/token are required.".to_string());
            return;
        }

        let folder_filter = parse_csv_folders(&self.imap_sync_folders);
        let folder_prefix =
            (!self.imap_sync_folder_prefix.trim().is_empty()).then(|| self.imap_sync_folder_prefix.trim().to_string());
        let src_oauth = (src_auth != mail2rust_imap::ImapAuthMethod::Password)
            .then(|| self.imap_sync_src_oauth_token.trim().to_string());
        let dst_oauth = (dst_auth != mail2rust_imap::ImapAuthMethod::Password)
            .then(|| self.imap_sync_dst_oauth_token.trim().to_string());

        let config = mail2rust_imap::ImapMigrationConfig {
            source: mail2rust_imap::ImapConfig {
                host: src_host.clone(),
                port: self.imap_sync_src_port.trim().parse::<u16>().unwrap_or(993),
                username: src_user,
                password: src_pass,
                use_tls: self.imap_sync_src_tls,
                insecure_certs: self.imap_sync_insecure,
                folder_filter: folder_filter.clone(),
                max_messages_per_folder: None,
                since_date: None,
                auth_method: src_auth,
                oauth_token: src_oauth,
            },
            destination: mail2rust_imap::ImapConfig {
                host: dst_host,
                port: self.imap_sync_dst_port.trim().parse::<u16>().unwrap_or(993),
                username: dst_user,
                password: dst_pass,
                use_tls: self.imap_sync_dst_tls,
                insecure_certs: self.imap_sync_insecure,
                folder_filter: None,
                max_messages_per_folder: None,
                since_date: None,
                auth_method: dst_auth,
                oauth_token: dst_oauth,
            },
            folder_filter,
            folder_prefix,
            folder_mapping: None,
            preserve_flags: self.imap_sync_preserve_flags,
            preserve_dates: self.imap_sync_preserve_dates,
            create_missing_folders: true,
            dry_run: self.imap_sync_dry_run,
        };

        match mail2rust_imap::ImapMigrator::migrate(&config, |_prog| {}) {
            Ok(summary) => {
                let mb = summary.total_bytes_transferred as f64 / (1024.0 * 1024.0);
                let msg = format!(
                    "Migration Succeeded: Transferred {} message(s) across {} folder(s) ({mb:.2} MB in {:.1}s).",
                    summary.total_messages_migrated, summary.total_folders_migrated, summary.duration_seconds
                );
                self.imap_sync_status = Some(msg.clone());
                self.status_message = msg;
            }
            Err(e) => {
                let diag = mail2rust_imap::diagnose_auth_failure(&src_host, &e.to_string());
                self.imap_sync_status = Some(format!("Error during server-to-server migration: {diag}"));
            }
        }
    }

    pub fn execute_merge_to_workbench(&mut self) {
        if self.merge_source_paths.is_empty() {
            self.merge_status = Some("Error: Please add at least one source mailbox.".to_string());
            return;
        }
        let configs = build_merge_configs(&self.merge_source_paths);
        let options = build_merge_options(
            &self.merge_folder_strategy,
            &self.merge_dedup_strategy,
            self.merge_sort_by_date,
        );
        match MailboxMerger::merge_sources(&configs, &options, ingest_any_mailbox) {
            Ok((merged_msgs, report)) => {
                let count = merged_msgs.len();
                self.messages = merged_msgs;
                self.selected_message_ids.clear();
                self.rebuild_folders_from_messages();
                self.merge_report_text = Some(report.format_summary());
                let msg = format!("Merged {} sources into workbench ({count} messages).", configs.len());
                self.merge_status = Some(msg.clone());
                self.status_message = msg;
            }
            Err(e) => {
                self.merge_status = Some(format!("Error merging sources: {e}"));
            }
        }
    }

    pub fn execute_merge_to_file(&mut self) {
        if self.merge_source_paths.is_empty() {
            self.merge_status = Some("Error: Please add at least one source mailbox.".to_string());
            return;
        }
        if self.merge_dest_path.trim().is_empty() {
            self.merge_status = Some("Error: Please specify a destination target path.".to_string());
            return;
        }
        let configs = build_merge_configs(&self.merge_source_paths);
        let options = build_merge_options(
            &self.merge_folder_strategy,
            &self.merge_dedup_strategy,
            self.merge_sort_by_date,
        );
        match MailboxMerger::merge_sources(&configs, &options, ingest_any_mailbox) {
            Ok((merged_msgs, report)) => {
                let dest = Path::new(self.merge_dest_path.trim());
                match export_messages_to_target(
                    &merged_msgs,
                    dest,
                    self.merge_export_format,
                    "Inbox",
                    PstEncryption::Permute,
                    None,
                ) {
                    Ok(summary) => {
                        self.merge_report_text = Some(report.format_summary());
                        self.merge_status = Some(summary.clone());
                        self.status_message = summary;
                    }
                    Err(e) => {
                        self.merge_status = Some(format!("Error exporting merged output: {e}"));
                    }
                }
            }
            Err(e) => {
                self.merge_status = Some(format!("Error merging sources: {e}"));
            }
        }
    }

    pub fn execute_export(&mut self) {
        let items = self.get_messages_for_export();
        let dest = self.pst_dest_path.trim().to_string();
        if items.is_empty() {
            self.pst_status_message = Some("Error: No messages match the selected export scope.".to_string());
            return;
        }
        if dest.is_empty() {
            self.pst_status_message = Some("Error: Destination path cannot be empty.".to_string());
            return;
        }
        let pwd = (!self.pst_password.trim().is_empty()).then(|| self.pst_password.trim().to_string());
        match export_messages_to_target(
            &items,
            Path::new(&dest),
            self.export_format,
            &self.pst_root,
            self.pst_encryption,
            pwd,
        ) {
            Ok(summary) => {
                self.pst_status_message = Some(summary.clone());
                self.status_message = summary;
            }
            Err(e) => {
                self.pst_status_message = Some(format!("Error: {e}"));
            }
        }
    }

    pub fn execute_auto_split(&mut self) {
        let max_bytes = self.pst_max_gb.max(1).saturating_mul(1024 * 1024 * 1024);
        let trimmed_dest = self.pst_dest_path.trim();
        let target_ext = match self.export_format {
            ExportFormat::Pst => "pst",
            ExportFormat::Mbox => "mbox",
            ExportFormat::Eml => "eml",
        };
        let (parent_dir, file_prefix) = if trimmed_dest.is_empty() {
            (None, "Archive".to_string())
        } else {
            let p = Path::new(trimmed_dest);
            let dir = p
                .parent()
                .filter(|d| !d.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let stem = p
                .file_stem()
                .and_then(|s| s.to_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("Archive")
                .to_string();
            (Some(dir.to_path_buf()), stem)
        };
        let config = SplitConfig {
            max_volume_bytes: max_bytes,
            cluster_threads: self.pst_cluster_threads,
            file_prefix,
            target_ext: target_ext.to_string(),
        };
        let volumes = VolumeSplitter::split(&self.messages, &config);
        let msg = if let Some(dir) = parent_dir {
            let pwd = (!self.pst_password.trim().is_empty()).then(|| self.pst_password.trim().to_string());
            let mut written = 0usize;
            for vol in &volumes {
                let vol_path = dir.join(&vol.file_name);
                if let Err(e) = export_messages_to_target(
                    &vol.messages,
                    &vol_path,
                    self.export_format,
                    &self.pst_root,
                    self.pst_encryption,
                    pwd.clone(),
                ) {
                    let err = format!("Error writing split volume '{}': {e}", vol_path.display());
                    self.pst_status_message = Some(err.clone());
                    self.status_message = err;
                    return;
                }
                written += 1;
            }
            format!(
                "Success: Partitioned {} messages into {} volume(s) and wrote {} file(s) to '{}' (limit {} GB/volume).",
                self.messages.len(),
                volumes.len(),
                written,
                dir.display(),
                self.pst_max_gb
            )
        } else {
            format!(
                "Success: Partitioned {} messages into {} volume(s) (limit {} GB/volume; set Target path to write files).",
                self.messages.len(),
                volumes.len(),
                self.pst_max_gb
            )
        };
        self.pst_status_message = Some(msg.clone());
        self.status_message = msg;
    }

    pub fn execute_forensics_stamping(&mut self) {
        let range = ForensicEngine::apply_bates_stamping(&mut self.messages, &self.bates_prefix, self.bates_start, 7);
        let root_hash = ForensicEngine::compute_root_hash(&self.messages);
        let mut msg = format!(
            "Success: Applied Bates numbering ({} through {}). Merkle root SHA-256: {root_hash}.",
            range.start, range.end
        );
        let csv_path = self.manifest_csv_path.trim();
        if !csv_path.is_empty() {
            let p = Path::new(csv_path);
            if let Some(parent) = p.parent()
                && !parent.as_os_str().is_empty()
            {
                let _ = std::fs::create_dir_all(parent);
            }
            match ForensicEngine::export_manifest_csv(&self.messages, p) {
                Ok(()) => msg.push_str(&format!("\nExported manifest CSV to {}", p.display())),
                Err(e) => msg = format!("Applied Bates numbering, but CSV export failed: {e}"),
            }
        }
        self.forensics_result = Some(msg.clone());
        self.status_message = msg;
    }

    pub fn execute_validate_pst(&mut self) {
        let path_str = self.validate_pst_path.trim();
        let path = Path::new(path_str);
        if path_str.is_empty() || !path.exists() {
            self.validation_report = Some("Error: Specified PST file does not exist.".to_string());
            return;
        }
        let Ok(file) = File::open(path) else {
            self.validation_report = Some("Error: Failed to open PST file.".to_string());
            return;
        };
        let Ok(report) = PstValidator::validate(file) else {
            self.validation_report = Some("Error: Failed to validate PST structure.".to_string());
            return;
        };

        let mut pwd_info = String::new();
        if let Ok(f2) = File::open(path)
            && let Ok(mut r) = PstReader::open(f2)
        {
            if r.is_password_protected() {
                pwd_info.push_str("\nPassword Protected: YES");
                if !self.validate_pst_password.is_empty() {
                    if r.verify_password(&self.validate_pst_password) {
                        pwd_info.push_str(" (MATCH / VERIFIED)");
                    } else {
                        pwd_info.push_str(" (MISMATCH / INCORRECT)");
                    }
                } else {
                    pwd_info.push_str(" (No test password supplied)");
                }
            } else {
                pwd_info.push_str("\nPassword Protected: NO");
            }
        }

        self.validation_report = Some(format!(
            "Status: {}\nFormat: {}\nMagic: {}\nEncryption: {}\nDeclared CRC: 0x{:08X}\nCalculated CRC: 0x{:08X}\nCRC Match: {}{pwd_info}",
            if report.is_valid {
                "VALID COMPLIANT PST"
            } else {
                "INVALID PST"
            },
            report.format,
            report.magic,
            report.encryption_method,
            report.declared_crc,
            report.calculated_crc,
            report.crc_match,
        ));
    }

    pub fn compute_dry_run_summary(&self) -> String {
        let total_count = self.messages.len();
        let total_bytes: usize = self.messages.iter().map(|m| m.size_bytes).sum();
        let read_count = self.messages.iter().filter(|m| m.flags.read).count();
        let flagged_count = self.messages.iter().filter(|m| m.flags.flagged).count();
        let large_attachments: usize = self
            .messages
            .iter()
            .flat_map(|m| m.attachments.iter())
            .filter(|a| a.size_bytes >= 20 * 1024 * 1024)
            .count();

        format!(
            "Total Messages: {total_count}\n\
             Total Payload Size: {total_bytes} bytes ({:.2} MB)\n\
             Read / Unread: {read_count} read / {} unread\n\
             Flagged Items: {flagged_count}\n\
             Large Attachments (>= 20 MB): {large_attachments}",
            total_bytes as f64 / (1024.0 * 1024.0),
            total_count.saturating_sub(read_count),
        )
    }

    pub fn filtered_messages(&self) -> Vec<&EmailMessage> {
        let q = self.search_query.to_lowercase();
        let folder_filter = FolderFilter::new(
            parse_folder_patterns(&self.filter_include_folder),
            parse_folder_patterns(&self.filter_exclude_folder),
        );
        let mut list: Vec<&EmailMessage> = self
            .messages
            .iter()
            .filter(|m| {
                let folder_name = m.folder_path.last().map(|s| s.as_str()).unwrap_or("Inbox");
                if self.active_folder == "All Mail" {
                    if let Some(folder) = self.folders.iter().find(|f| f.name == folder_name)
                        && !folder.selected
                    {
                        return false;
                    }
                } else if folder_name != self.active_folder {
                    return false;
                }
                if !folder_filter.matches(&m.folder_path) {
                    return false;
                }
                if m.flags.deleted && !self.filter_include_deleted {
                    return false;
                }
                if !self.filter_since.is_empty()
                    && let Ok(since_dt) = mail2rust_filter::parse_cli_datetime(&self.filter_since, false)
                    && m.date < since_dt
                {
                    return false;
                }
                if !self.filter_until.is_empty()
                    && let Ok(until_dt) = mail2rust_filter::parse_cli_datetime(&self.filter_until, true)
                    && m.date > until_dt
                {
                    return false;
                }
                if !q.is_empty() {
                    let in_subject = m.subject.to_lowercase().contains(&q);
                    let in_sender = m.from.name.to_lowercase().contains(&q) || m.from.email.to_lowercase().contains(&q);
                    let in_body = m.body_text.to_lowercase().contains(&q);
                    let in_id = m.id.to_lowercase().contains(&q) || m.message_id.to_lowercase().contains(&q);
                    if !in_subject && !in_sender && !in_body && !in_id {
                        return false;
                    }
                }
                true
            })
            .collect();

        list.sort_by(|a, b| {
            let cmp = match self.sort_column {
                SortColumn::Importance => {
                    let rank = |imp: &Importance| match imp {
                        Importance::High => 2u8,
                        Importance::Normal => 1,
                        Importance::Low => 0,
                    };
                    rank(&a.importance).cmp(&rank(&b.importance))
                }
                SortColumn::Flags => (a.flags.read as u8).cmp(&(b.flags.read as u8)),
                SortColumn::Subject => a.subject.to_lowercase().cmp(&b.subject.to_lowercase()),
                SortColumn::From => {
                    let a_from = format!("{} {}", a.from.name, a.from.email).to_lowercase();
                    let b_from = format!("{} {}", b.from.name, b.from.email).to_lowercase();
                    a_from.cmp(&b_from)
                }
                SortColumn::Date => a.date.cmp(&b.date),
                SortColumn::Size => a.size_bytes.cmp(&b.size_bytes),
            };
            match self.sort_direction {
                SortDirection::Ascending => cmp,
                SortDirection::Descending => cmp.reverse(),
            }
        });

        list
    }

    pub fn load_outlook_providers(&mut self) {
        use std::sync::Arc;
        let src = self.outlook_source_path.trim().to_string();
        let res: Result<Vec<Arc<dyn guap_table::TableProvider>>, String> = match self.outlook_tool_idx {
            0 => mail2rust_outlook::Nk2TableProvider::from_path(Path::new(&src))
                .map(|p| vec![Arc::new(p) as Arc<dyn guap_table::TableProvider>])
                .map_err(|e| e.to_string()),
            1 => mail2rust_outlook::AttachTableProvider::from_source(
                Path::new(&src),
                &mail2rust_outlook::AttachmentFilter {
                    include_inline: true,
                    ..Default::default()
                },
            )
            .map(|p| vec![Arc::new(p) as Arc<dyn guap_table::TableProvider>])
            .map_err(|e| e.to_string()),
            2 => mail2rust_outlook::StatsTableProvider::from_source(
                Path::new(&src),
                &mail2rust_outlook::StatsOptions::default(),
            )
            .map(|p| vec![Arc::new(p) as Arc<dyn guap_table::TableProvider>])
            .map_err(|e| e.to_string()),
            3 => mail2rust_outlook::ContactsTableProvider::from_source(Path::new(&src))
                .map(|p| vec![Arc::new(p) as Arc<dyn guap_table::TableProvider>])
                .map_err(|e| e.to_string()),
            _ => {
                let hives: Vec<std::path::PathBuf> = src
                    .split(';')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(std::path::PathBuf::from)
                    .collect();
                mail2rust_outlook::OfficeAddinsTableProvider::scan_from_hives(&hives, None)
                    .map(|p| vec![Arc::new(p) as Arc<dyn guap_table::TableProvider>])
                    .map_err(|e| e.to_string())
            }
        };

        match res {
            Ok(providers) => {
                self.outlook_grid = guap_gui_iced::IcedDataGrid::new(providers).with_dark_mode(self.dark_mode);
                let msg = format!("Outlook/Office scan loaded: {}", self.outlook_grid.status_message());
                self.outlook_status = Some(msg.clone());
                self.status_message = msg;
            }
            Err(err) => {
                let msg = format!("Outlook/Office error: {err}");
                self.outlook_status = Some(msg.clone());
                self.status_message = msg;
            }
        }
    }

    pub fn execute_confirmed_outlook_mutate(&mut self) {
        let src = self.outlook_source_path.trim().to_string();
        if self.outlook_tool_idx == 0 && !src.is_empty() {
            let p = Path::new(&src);
            match mail2rust_outlook::Nk2Archive::from_file(p).and_then(|mut arc| {
                let removed = arc.dedup();
                arc.save_atomic(p, true)?;
                Ok(removed)
            }) {
                Ok(removed) => {
                    self.load_outlook_providers();
                    let msg = format!("NK2 deduplicated {removed} duplicate(s) and saved atomically.");
                    self.outlook_status = Some(msg.clone());
                    self.status_message = msg;
                }
                Err(e) => {
                    let msg = format!("NK2 edit failed: {e}");
                    self.outlook_status = Some(msg.clone());
                    self.status_message = msg;
                }
            }
        } else if self.outlook_tool_idx == 4 {
            #[cfg(windows)]
            {
                let prog_id = self
                    .outlook_grid
                    .selected_row()
                    .and_then(|r| r.cells.get(2))
                    .map(|c| c.to_string())
                    .unwrap_or_default();
                let enabled = self
                    .outlook_grid
                    .selected_row()
                    .and_then(|r| r.cells.get(8))
                    .map(|c| c.to_string().eq_ignore_ascii_case("true"))
                    .unwrap_or(true);
                if prog_id.is_empty() {
                    self.outlook_status = Some("Select an Office Add-in row to toggle.".to_string());
                    return;
                }
                let backend = mail2rust_outlook::LiveWindowsRegistry;
                match mail2rust_outlook::toggle_office_addin(&backend, &prog_id, None, !enabled, false) {
                    Ok((reg_path, old_v, new_v)) => {
                        self.load_outlook_providers();
                        let msg = format!("Toggled {prog_id} ({reg_path}): LoadBehavior {old_v} -> {new_v}");
                        self.outlook_status = Some(msg.clone());
                        self.status_message = msg;
                    }
                    Err(e) => {
                        let msg = format!("Office add-in toggle failed: {e}");
                        self.outlook_status = Some(msg.clone());
                        self.status_message = msg;
                    }
                }
            }
            #[cfg(not(windows))]
            {
                self.outlook_status = Some("Live Registry toggle requires Windows.".to_string());
            }
        }
    }
}

fn parse_folder_patterns(raw: &str) -> Vec<String> {
    raw.split([';', ','])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_csv_folders(raw: &str) -> Option<Vec<String>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(
            trimmed
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        )
    }
}

fn ingest_any_mailbox(p: &Path) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>> {
    let fmt = FormatSniffer::sniff_path(p);
    if fmt == mail2rust_core::MailboxFormat::Pst || fmt == mail2rust_core::MailboxFormat::Ost {
        return PstReader::read_path(p).map_err(|e| Box::new(e) as Box<dyn std::error::Error>);
    }
    mail2rust_core::ingest_path(p)
}

fn build_merge_configs(paths: &[String]) -> Vec<MergeSourceConfig> {
    paths.iter().map(MergeSourceConfig::new).collect()
}

fn build_merge_options(folder_strategy: &str, dedup_strategy: &str, sort_by_date: bool) -> MergeOptions {
    let folder_strategy: MergeFolderStrategy = folder_strategy.parse().unwrap_or(MergeFolderStrategy::Combine);
    let dedup_strategy = match dedup_strategy {
        "sha256" => DedupStrategy::Sha256,
        "content" => DedupStrategy::NormalizedContent,
        "none" => DedupStrategy::None,
        _ => DedupStrategy::MessageId,
    };
    MergeOptions {
        folder_strategy,
        dedup_strategy,
        sort_by_date,
        dry_run: false,
    }
}

fn export_messages_to_target(
    messages: &[EmailMessage],
    dest: &Path,
    format: ExportFormat,
    pst_root: &str,
    encryption: PstEncryption,
    password: Option<String>,
) -> Result<String, String> {
    if let Some(parent) = dest.parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = std::fs::create_dir_all(parent);
    }

    match format {
        ExportFormat::Pst => {
            let file = File::create(dest).map_err(|e| format!("Failed creating PST file: {e}"))?;
            let mut writer =
                PstWriter::with_encryption(file, encryption, password).map_err(|e| format!("PST init failed: {e}"))?;
            let root_name = if pst_root.trim().is_empty() {
                "Imported (Mail2Rust)"
            } else {
                pst_root.trim()
            };
            let root_nid = writer.add_folder(root_name, None).unwrap_or(0x8000);

            for msg in messages {
                let target_nid = writer
                    .ensure_folder_path(&msg.folder_path, Some(root_name))
                    .unwrap_or(root_nid);
                let _ = writer.add_message(msg, target_nid);
            }

            writer.finalize().map_err(|e| format!("PST finalize failed: {e}"))?;
            if let Ok(vfile) = File::open(dest)
                && let Ok(report) = PstValidator::validate(vfile)
            {
                return Ok(format!(
                    "Success: Exported {} message(s) to valid [MS-PST] container '{}' (CRC-32: 0x{:08X}).",
                    messages.len(),
                    dest.display(),
                    report.calculated_crc
                ));
            }
            Ok(format!(
                "Success: Exported {} message(s) to PST '{}'.",
                messages.len(),
                dest.display()
            ))
        }
        ExportFormat::Mbox => {
            let count = MboxFileExporter::export_to_file_options(messages, dest, false, false)
                .map_err(|e| format!("MBOX export failed: {e}"))?;
            Ok(format!(
                "Success: Exported {count} message(s) to MBOX '{}'.",
                dest.display()
            ))
        }
        ExportFormat::Eml => {
            let mut exporter = EmlExporter::new(dest, EmlLayout::Folder);
            let mut written = 0usize;
            for msg in messages {
                exporter
                    .export_message(msg)
                    .map_err(|e| format!("EML export failed after {written} message(s): {e}"))?;
                written += 1;
            }
            Ok(format!(
                "Success: Exported {written} message(s) to EML directory '{}'.",
                dest.display()
            ))
        }
    }
}

pub fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn generate_seed_data() -> (Vec<GuiFolder>, Vec<EmailMessage>) {
    let folders = vec![
        GuiFolder {
            name: "Inbox".to_string(),
            message_count: 2,
            unread_count: 1,
            selected: true,
        },
        GuiFolder {
            name: "Project Titan".to_string(),
            message_count: 1,
            unread_count: 1,
            selected: true,
        },
    ];

    let messages = vec![
        EmailMessage {
            id: "msg_001".to_string(),
            message_id: "<titan-launch-2026@corp.local>".to_string(),
            conversation_id: "conv_titan_01".to_string(),
            folder_path: vec!["Inbox".to_string()],
            subject: "Project Titan Launch Schedule & Migration Signoff".to_string(),
            from: EmailAddress::new("Dr. Elena Vance", "elena.vance@titan.corp"),
            to: vec![EmailAddress::new("Alex Wright", "alex.wright@corp.local")],
            cc: vec![EmailAddress::new("Executive Committee", "exec@titan.corp")],
            bcc: vec![],
            date: Utc.with_ymd_and_hms(2026, 11, 6, 14, 30, 0).unwrap(),
            size_bytes: 42_500,
            body_text: "Team,\n\nThe PST generation validation confirmed 100% specification compliance with MS-PST."
                .to_string(),
            body_html: None,
            raw_mime: b"From: Dr. Elena Vance <elena.vance@titan.corp>\r\nSubject: Project Titan Launch\r\n\r\nBody"
                .to_vec(),
            attachments: vec![Attachment::new(
                "Project_Titan_Migration_Signoff.pdf".to_string(),
                "application/pdf".to_string(),
                vec![0u8; 15200],
                None,
            )],
            mapi_properties: vec![],
            flags: MessageFlags {
                read: false,
                flagged: true,
                replied: false,
                forwarded: false,
                deleted: false,
                draft: false,
            },
            importance: Importance::High,
            sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
            md5: "d41d8cd98f00b204e9800998ecf8427e".to_string(),
            bates_number: Some("CASE-2026-0001001".to_string()),
            legal_hold: true,
            source_format: "RFC 5322 EML".to_string(),
        },
        EmailMessage {
            id: "msg_002".to_string(),
            message_id: "<pst-zero-outlook@mail2rust.local>".to_string(),
            conversation_id: "conv_titan_01".to_string(),
            folder_path: vec!["Inbox".to_string()],
            subject: "Re: Pure-Rust MS-PST Binary Engine Architecture Verified".to_string(),
            from: EmailAddress::new("Marcus Brody", "m.brody@titan.corp"),
            to: vec![EmailAddress::new("Alex Wright", "alex.wright@corp.local")],
            cc: vec![],
            bcc: vec![],
            date: Utc.with_ymd_and_hms(2026, 11, 6, 15, 45, 0).unwrap(),
            size_bytes: 18_400,
            body_text:
                "Team,\n\nThe pure-Rust PST writer creates valid NDB headers and compressible encryption tables."
                    .to_string(),
            body_html: None,
            raw_mime: b"From: Marcus Brody <m.brody@titan.corp>\r\nSubject: Re: Pure-Rust MS-PST\r\n\r\nContent"
                .to_vec(),
            attachments: vec![],
            mapi_properties: vec![],
            flags: MessageFlags {
                read: true,
                flagged: false,
                replied: true,
                forwarded: false,
                deleted: false,
                draft: false,
            },
            importance: Importance::Normal,
            sha256: "8729837498237498237498237498237498237498237498237498237498237498".to_string(),
            md5: "98237498237498237498237498237498".to_string(),
            bates_number: Some("CASE-2026-0001002".to_string()),
            legal_hold: false,
            source_format: "Standard MBOX".to_string(),
        },
        EmailMessage {
            id: "msg_003".to_string(),
            message_id: "<audit-ediscovery-2026@forensics.org>".to_string(),
            conversation_id: "conv_forensic_09".to_string(),
            folder_path: vec!["Projects".to_string(), "Project Titan".to_string()],
            subject: "Forensic Chain-of-Custody SHA-256 Merkle Manifest Export".to_string(),
            from: EmailAddress::new("Sarah Connor", "sconnor@cyber.gov"),
            to: vec![EmailAddress::new("Auditor Vance", "vance@forensics.org")],
            cc: vec![],
            bcc: vec![],
            date: Utc.with_ymd_and_hms(2026, 11, 7, 2, 10, 0).unwrap(),
            size_bytes: 88_900,
            body_text: "Cryptographic hash verification completed with zero discrepancies.".to_string(),
            body_html: None,
            raw_mime: b"From: Sarah Connor <sconnor@cyber.gov>\r\nSubject: Forensic Audit\r\n\r\nReport".to_vec(),
            attachments: vec![],
            mapi_properties: vec![],
            flags: MessageFlags {
                read: false,
                flagged: true,
                replied: false,
                forwarded: false,
                deleted: false,
                draft: false,
            },
            importance: Importance::High,
            sha256: "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069".to_string(),
            md5: "3b1657ff1fc53b92dc18148a1d65dfc2".to_string(),
            bates_number: Some("CASE-2026-0001003".to_string()),
            legal_hold: true,
            source_format: "Google Takeout (MBOX)".to_string(),
        },
    ];

    (folders, messages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_data_populates_folders_and_filters_messages() {
        let mut st = AppState::default();
        assert!(st.messages.is_empty());
        st.load_sample_data();
        assert_eq!(st.messages.len(), 3);
        assert_eq!(st.active_folder, "Inbox");
        assert_eq!(st.filtered_messages().len(), 2);

        st.active_folder = "All Mail".to_string();
        st.search_query = "merkle".to_string();
        assert_eq!(st.filtered_messages().len(), 1);
    }

    #[test]
    fn bates_stamping_and_auto_split_update_state() {
        let mut st = AppState::default();
        st.load_sample_data();
        st.bates_prefix = "TEST".to_string();
        st.bates_start = 500;
        st.execute_forensics_stamping();
        assert_eq!(st.messages[0].bates_number.as_deref(), Some("TEST-0000500"));
        assert!(st.forensics_result.as_deref().unwrap_or("").contains("TEST-0000502"));

        st.execute_auto_split();
        assert!(
            st.pst_status_message
                .as_deref()
                .unwrap_or("")
                .contains("Partitioned 3 messages")
        );
    }

    #[test]
    fn folder_wildcard_filter_and_checkbox_selection_work() {
        let mut st = AppState::default();
        st.load_sample_data();
        st.active_folder = "All Mail".to_string();
        assert_eq!(st.filtered_messages().len(), 3);

        // Exclude Project Titan via glob
        st.filter_exclude_folder = "*Titan*".to_string();
        assert_eq!(st.filtered_messages().len(), 2);

        // Reset glob and test include glob
        st.filter_exclude_folder.clear();
        st.filter_include_folder = "Projects/*".to_string();
        assert_eq!(st.filtered_messages().len(), 1);

        // Reset include glob and uncheck Inbox folder in Console Tree
        st.filter_include_folder.clear();
        if let Some(inbox) = st.folders.iter_mut().find(|f| f.name == "Inbox") {
            inbox.selected = false;
        }
        assert_eq!(st.filtered_messages().len(), 1);
    }

    #[test]
    fn outlook_iced_grid_loads_nk2_and_office_hives() {
        let dir = tempfile::tempdir().unwrap();
        let fx = mail2rust_outlook::fixtures::generate_all_outlook_fixtures(dir.path()).unwrap();

        let mut st = AppState {
            outlook_tool_idx: 0,
            outlook_source_path: fx.nk2_path.to_string_lossy().to_string(),
            ..Default::default()
        };
        st.load_outlook_providers();
        assert_eq!(st.outlook_grid.total_rows(), 3);

        st.outlook_tool_idx = 4;
        st.outlook_source_path = format!("{};{}", fx.ntuser_dat_path.display(), fx.software_hive_path.display());
        st.load_outlook_providers();
        assert!(st.outlook_grid.total_rows() >= 2);
    }
}
