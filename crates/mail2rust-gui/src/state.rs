use chrono::{TimeZone, Utc};
use mail2rust_core::{Attachment, EmailAddress, EmailMessage, Importance, MessageFlags};

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
#[allow(dead_code)]
pub struct GuiFolder {
    pub id: String,
    pub name: String,
    pub path: Vec<String>,
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

pub struct AppState {
    pub folders: Vec<GuiFolder>,
    pub active_folder: String,
    pub messages: Vec<EmailMessage>,
    pub selected_message_id: Option<String>,
    pub selected_message_ids: std::collections::HashSet<String>,
    pub search_query: String,
    pub active_tab: ActiveTab,
    pub active_modal: ActiveModal,

    // Win32 Management Console Layout & Sorting
    pub sort_column: SortColumn,
    pub sort_direction: SortDirection,
    pub show_tree_pane: bool,
    pub show_preview_pane: bool,

    // Open Mailbox States
    pub open_mailbox_path: String,
    pub open_mailbox_status: Option<String>,

    // IMAP Connection States
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

    // Server-to-Server IMAP Migration States
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

    // Multi-Format Merge States
    pub merge_source_paths: Vec<String>,
    pub merge_source_input: String,
    pub merge_dest_path: String,
    pub merge_folder_strategy: String,
    pub merge_dedup_strategy: String,
    pub merge_export_format: ExportFormat,
    pub merge_sort_by_date: bool,
    pub merge_status: Option<String>,
    pub merge_report_text: Option<String>,

    // Modal Form States
    #[allow(dead_code)]
    pub pst_source_path: String,
    pub pst_dest_path: String,
    pub pst_max_gb: usize,
    pub pst_cluster_threads: bool,
    pub pst_encryption: mail2rust_pst::PstEncryption,
    pub pst_password: String,
    pub pst_root: String,
    pub pst_status_message: Option<String>,
    pub export_scope: ExportScope,
    pub export_format: ExportFormat,

    // Filter Form States
    pub filter_since: String,
    pub filter_until: String,
    pub filter_include_folder: String,
    pub filter_exclude_folder: String,
    pub filter_include_deleted: bool,

    // Forensics Form States
    pub bates_prefix: String,
    pub bates_start: usize,
    pub case_number: String,
    pub examiner: String,
    pub manifest_csv_path: String,
    pub forensics_result: Option<String>,

    // PST Validator States
    pub validate_pst_path: String,
    pub validate_pst_password: String,
    pub validation_report: Option<String>,

    // Dry Run States
    #[allow(dead_code)]
    pub dry_run_report: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            folders: Vec::new(),
            active_folder: "All Mail".to_string(),
            messages: Vec::new(),
            selected_message_id: None,
            selected_message_ids: std::collections::HashSet::new(),
            search_query: String::new(),
            active_tab: ActiveTab::Preview,
            active_modal: ActiveModal::None,

            sort_column: SortColumn::Date,
            sort_direction: SortDirection::Descending,
            show_tree_pane: true,
            show_preview_pane: true,

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

            pst_source_path: String::new(),
            pst_dest_path: String::new(),
            pst_max_gb: 20,
            pst_cluster_threads: true,
            pst_encryption: mail2rust_pst::PstEncryption::Permute,
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

            dry_run_report: None,
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
    }

    pub fn clear_data(&mut self) {
        self.folders.clear();
        self.messages.clear();
        self.selected_message_id = None;
        self.selected_message_ids.clear();
        self.active_folder = "All Mail".to_string();
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
        if self.selected_message_ids.contains(id) {
            self.selected_message_ids.remove(id);
        } else {
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
            if self.selected_message_ids.contains(&id) {
                self.selected_message_ids.remove(&id);
            } else {
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
        let mut map: std::collections::BTreeMap<String, (usize, usize)> = std::collections::BTreeMap::new();
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
                id: format!("fld_{}", name.to_lowercase().replace(' ', "_")),
                name: name.clone(),
                path: vec![name],
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

    pub fn filtered_messages(&self) -> Vec<&EmailMessage> {
        let q = self.search_query.to_lowercase();
        let mut list: Vec<&EmailMessage> = self
            .messages
            .iter()
            .filter(|m| {
                // Folder match
                let folder_name = m.folder_path.last().map(|s| s.as_str()).unwrap_or("Inbox");
                if self.active_folder != "All Mail" && folder_name != self.active_folder {
                    return false;
                }

                // Deleted messages filter
                if m.flags.deleted && !self.filter_include_deleted {
                    return false;
                }

                // Since date filter
                if !self.filter_since.is_empty() {
                    if let Ok(since_dt) = mail2rust_filter::parse_cli_datetime(&self.filter_since, false) {
                        if m.date < since_dt {
                            return false;
                        }
                    }
                }

                // Until date filter
                if !self.filter_until.is_empty() {
                    if let Ok(until_dt) = mail2rust_filter::parse_cli_datetime(&self.filter_until, true) {
                        if m.date > until_dt {
                            return false;
                        }
                    }
                }

                // Search query match
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

        // Apply sorting based on column and direction
        list.sort_by(|a, b| {
            let cmp = match self.sort_column {
                SortColumn::Importance => {
                    let rank = |imp: &Importance| match imp {
                        Importance::High => 2,
                        Importance::Normal => 1,
                        Importance::Low => 0,
                    };
                    rank(&a.importance).cmp(&rank(&b.importance))
                }
                SortColumn::Flags => {
                    let a_read = a.flags.read as u8;
                    let b_read = b.flags.read as u8;
                    a_read.cmp(&b_read)
                }
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
}

fn generate_seed_data() -> (Vec<GuiFolder>, Vec<EmailMessage>) {
    let folders = vec![
        GuiFolder {
            id: "fld_inbox".to_string(),
            name: "Inbox".to_string(),
            path: vec!["Inbox".to_string()],
            message_count: 5,
            unread_count: 2,
            selected: true,
        },
        GuiFolder {
            id: "fld_sent".to_string(),
            name: "Sent Items".to_string(),
            path: vec!["Sent Items".to_string()],
            message_count: 2,
            unread_count: 0,
            selected: true,
        },
        GuiFolder {
            id: "fld_projects".to_string(),
            name: "Project Titan".to_string(),
            path: vec!["Projects".to_string(), "Project Titan".to_string()],
            message_count: 3,
            unread_count: 1,
            selected: true,
        },
        GuiFolder {
            id: "fld_archive".to_string(),
            name: "Archive 2024".to_string(),
            path: vec!["Archive".to_string(), "2024".to_string()],
            message_count: 4,
            unread_count: 0,
            selected: false,
        },
        GuiFolder {
            id: "fld_trash".to_string(),
            name: "Deleted Items".to_string(),
            path: vec!["Deleted Items".to_string()],
            message_count: 1,
            unread_count: 0,
            selected: false,
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
            to: vec![EmailAddress::new("Anthony Paolucci", "adpaolucci@gmail.com")],
            cc: vec![EmailAddress::new("Executive Committee", "exec@titan.corp")],
            bcc: vec![],
            date: Utc.with_ymd_and_hms(2026, 11, 6, 14, 30, 0).unwrap(),
            size_bytes: 42_500,
            body_text: "Anthony,\n\nThe PST generation validation on Linux and Windows confirmed 100% specification compliance with MS-PST. CRC-32 checksums and B-Tree root pointers align with Outlook 2024/M365 requirements.\n\nAll attachments and folder hierarchies have been preserved.\n\nBest regards,\nElena".to_string(),
            body_html: None,
            raw_mime: b"From: Dr. Elena Vance <elena.vance@titan.corp>\r\nSubject: Project Titan Launch\r\n\r\nBody".to_vec(),
            attachments: vec![
                Attachment::new("Project_Titan_Migration_Signoff.pdf".to_string(), "application/pdf".to_string(), vec![0u8; 15200], None),
                Attachment::new("PST_BTree_Validation_Report.xlsx".to_string(), "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(), vec![0u8; 26100], None),
            ],
            mapi_properties: vec![],
            flags: MessageFlags { read: false, flagged: true, replied: false, forwarded: false, deleted: false, draft: false },
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
            to: vec![EmailAddress::new("Anthony Paolucci", "adpaolucci@gmail.com")],
            cc: vec![],
            bcc: vec![],
            date: Utc.with_ymd_and_hms(2026, 11, 6, 15, 45, 0).unwrap(),
            size_bytes: 18_400,
            body_text: "Team,\n\nThe new pure-Rust PST writer creates valid NDB headers (!BDN, 0x4E44424E), BREF pointers, and compressible encryption tables. We verified that Outlook opens generated archives without errors.\n\nReady for v3.2.0 production deployment.".to_string(),
            body_html: None,
            raw_mime: b"From: Marcus Brody <m.brody@titan.corp>\r\nSubject: Re: Pure-Rust MS-PST\r\n\r\nContent".to_vec(),
            attachments: vec![],
            mapi_properties: vec![],
            flags: MessageFlags { read: true, flagged: false, replied: true, forwarded: false, deleted: false, draft: false },
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
            body_text: "Cryptographic hash verification completed across 14,800 migrated items.\n\nMerkle Root: 7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069\nBates Range: CASE-2026-0001000 through CASE-2026-0015800\n\nNo discrepancies found.".to_string(),
            body_html: None,
            raw_mime: b"From: Sarah Connor <sconnor@cyber.gov>\r\nSubject: Forensic Audit\r\n\r\nReport".to_vec(),
            attachments: vec![
                Attachment::new("Chain_of_Custody_Audit.csv".to_string(), "text/csv".to_string(), vec![0u8; 82000], None),
            ],
            mapi_properties: vec![],
            flags: MessageFlags { read: false, flagged: true, replied: false, forwarded: false, deleted: false, draft: false },
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
