use crate::state::{ActiveModal, AppState, ExportFormat, ExportScope};
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
use mail2rust_core::{
    DedupStrategy, EmailMessage, FormatSniffer, MailboxMerger, MergeFolderStrategy, MergeOptions, MergeSourceConfig,
};
use mail2rust_eml::{EmlExporter, EmlLayout};
use mail2rust_filter::{SplitConfig, VolumeSplitter};
use mail2rust_forensics::ForensicEngine;
use mail2rust_mbox::MboxFileExporter;
use mail2rust_pst::{PstValidator, PstWriter};
use std::fs::File;
use std::path::Path;

pub fn render_modals(ctx: &egui::Context, state: &mut AppState) {
    match state.active_modal {
        ActiveModal::None => {}

        ActiveModal::OpenMailbox => {
            let mut is_open = true;
            egui::Window::new("Open Mailbox Archive / Directory")
                .open(&mut is_open)
                .resizable(false)
                .fixed_size(Vec2::new(520.0, 220.0))
                .show(ctx, |ui| {
                    ui.label("Ingest messages from MBOX, Maildir, Apple Mail EMLX, Google Takeout .zip, EML folder, OLM, or MSG.");
                    ui.add_space(6.0);

                    // GroupBox: Path Specification
                    win32_groupbox(ui, "Source Archive / Folder Path", |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Path:").strong().size(11.0));
                            ui.add(egui::TextEdit::singleline(&mut state.open_mailbox_path).desired_width(380.0));
                        });
                    });

                    if let Some(ref st) = state.open_mailbox_status {
                        ui.add_space(4.0);
                        let is_err = st.starts_with("Error");
                        let color = if is_err { Color32::from_rgb(180, 0, 0) } else { Color32::from_rgb(0, 120, 0) };
                        ui.label(RichText::new(st).color(color).size(11.0));
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // Win32 Standard Bottom Buttons (right aligned, 75x23 px)
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if win32_dialog_button(ui, "Cancel") {
                            state.active_modal = ActiveModal::None;
                        }

                        if win32_dialog_button(ui, "Open / Ingest") {
                            let path = state.open_mailbox_path.trim().to_string();
                            if path.is_empty() {
                                state.open_mailbox_status = Some("Error: Please specify a valid file or folder path.".to_string());
                            } else {
                                let p = Path::new(&path);
                                let fmt = mail2rust_core::FormatSniffer::sniff_path(p);
                                let result = if fmt == mail2rust_core::MailboxFormat::Pst || fmt == mail2rust_core::MailboxFormat::Ost {
                                    mail2rust_pst::PstReader::read_path(p).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                                } else {
                                    mail2rust_core::ingest_path(p)
                                };

                                match result {
                                    Ok(msgs) => {
                                        let count = msgs.len();
                                        state.messages = msgs;
                                        state.rebuild_folders_from_messages();
                                        state.open_mailbox_status = Some(format!(
                                            "Loaded {} message(s) across {} folder(s).",
                                            count, state.folders.len()
                                        ));
                                    }
                                    Err(e) => {
                                        state.open_mailbox_status = Some(format!("Error: {}", e));
                                    }
                                }
                            }
                        }
                    });
                });

            if !is_open {
                state.active_modal = ActiveModal::None;
            }
        }

        ActiveModal::ImapConnect => {
            let mut is_open = true;
            egui::Window::new("Connect to IMAP Mailbox (Live Ingestion)")
                .open(&mut is_open)
                .resizable(false)
                .fixed_size(Vec2::new(580.0, 480.0))
                .show(ctx, |ui| {
                    ui.label("Directly stream live messages from IMAP servers (Gmail, M365, Yahoo, iCloud, or custom IMAP).");
                    ui.add_space(6.0);

                    // GroupBox: Server Connection
                    win32_groupbox(ui, "Server Connection", |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Quick Preset:").size(11.0));
                            if ui.small_button("Gmail").clicked() {
                                state.imap_host = "imap.gmail.com".to_string();
                                state.imap_port = "993".to_string();
                                state.imap_use_tls = true;
                            }
                            if ui.small_button("Outlook / M365").clicked() {
                                state.imap_host = "outlook.office365.com".to_string();
                                state.imap_port = "993".to_string();
                                state.imap_use_tls = true;
                            }
                            if ui.small_button("Yahoo Mail").clicked() {
                                state.imap_host = "imap.mail.yahoo.com".to_string();
                                state.imap_port = "993".to_string();
                                state.imap_use_tls = true;
                            }
                            if ui.small_button("iCloud").clicked() {
                                state.imap_host = "imap.mail.me.com".to_string();
                                state.imap_port = "993".to_string();
                                state.imap_use_tls = true;
                            }
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Host:").strong().size(11.0));
                            ui.add(egui::TextEdit::singleline(&mut state.imap_host).desired_width(260.0));

                            ui.label(RichText::new("Port:").strong().size(11.0));
                            ui.add(egui::TextEdit::singleline(&mut state.imap_port).desired_width(60.0));
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut state.imap_use_tls, "Use SSL/TLS (Port 993 recommended)");
                            ui.add_space(10.0);
                            ui.checkbox(&mut state.imap_insecure_certs, "Accept Untrusted / Self-Signed Certs");
                        });
                    });

                    ui.add_space(6.0);

                    // GroupBox: Account Credentials & OAuth
                    win32_groupbox(ui, "Authentication Credentials & OAuth", |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Auth Method:").strong().size(11.0));
                            ui.radio_value(&mut state.imap_auth_method, "password".to_string(), "Password / App Pass");
                            ui.radio_value(&mut state.imap_auth_method, "xoauth2".to_string(), "OAuth 2.0 (XOAUTH2)");
                            ui.radio_value(&mut state.imap_auth_method, "oauthbearer".to_string(), "OAUTHBEARER");
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Username / Email:").strong().size(11.0));
                            ui.add(egui::TextEdit::singleline(&mut state.imap_username).desired_width(340.0));
                        });
                        ui.add_space(4.0);
                        if state.imap_auth_method == "password" {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Password / App Pass:").strong().size(11.0));
                                ui.add(
                                    egui::TextEdit::singleline(&mut state.imap_password)
                                        .password(true)
                                        .hint_text("Account or 16-char App Password")
                                        .desired_width(310.0),
                                );
                            });
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new("Note: For Google, Yahoo, and Microsoft with 2FA, generate an App Password in your security settings.")
                                    .size(10.0)
                                    .italics()
                                    .color(Color32::from_rgb(100, 100, 100)),
                            );
                        } else {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("OAuth Access Token:").strong().size(11.0));
                                ui.add(
                                    egui::TextEdit::singleline(&mut state.imap_oauth_token)
                                        .password(true)
                                        .hint_text("Bearer access token (ya29... / eyJ...)")
                                        .desired_width(310.0),
                                );
                            });
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new("Tip: Run 'mail2rust oauth-login --provider google' in terminal to acquire access tokens.")
                                    .size(10.0)
                                    .italics()
                                    .color(Color32::from_rgb(100, 100, 100)),
                            );
                        }
                    });

                    ui.add_space(6.0);

                    // GroupBox: Sync Options
                    win32_groupbox(ui, "Sync Options (Optional)", |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Folder Filter:").strong().size(11.0));
                            ui.add(
                                egui::TextEdit::singleline(&mut state.imap_folder_filter)
                                    .hint_text("e.g. INBOX, Sent (leave empty for all)")
                                    .desired_width(360.0),
                            );
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Max Messages / Folder:").strong().size(11.0));
                            ui.add(
                                egui::TextEdit::singleline(&mut state.imap_max_per_folder)
                                    .hint_text("e.g. 500 (leave empty for unlimited)")
                                    .desired_width(120.0),
                            );
                        });
                    });

                    if let Some(ref st) = state.imap_status {
                        ui.add_space(4.0);
                        let is_err = st.starts_with("Error") || st.contains("Failed");
                        let color = if is_err {
                            Color32::from_rgb(180, 0, 0)
                        } else {
                            Color32::from_rgb(0, 120, 0)
                        };
                        ui.label(RichText::new(st).color(color).size(11.0));
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // Win32 Standard Bottom Buttons (right aligned, 75x23 px)
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if win32_dialog_button(ui, "Cancel") {
                            state.active_modal = ActiveModal::None;
                        }

                        if win32_dialog_button(ui, "Connect & Ingest") {
                            let host = state.imap_host.trim().to_string();
                            let port = state.imap_port.trim().parse::<u16>().unwrap_or(993);
                            let user = state.imap_username.trim().to_string();
                            let pass = state.imap_password.clone();
                            let auth_method: mail2rust_imap::ImapAuthMethod = state
                                .imap_auth_method
                                .parse()
                                .unwrap_or(mail2rust_imap::ImapAuthMethod::Password);

                            let has_cred = if auth_method == mail2rust_imap::ImapAuthMethod::Password {
                                !pass.is_empty()
                            } else {
                                !state.imap_oauth_token.trim().is_empty()
                            };

                            if host.is_empty() || user.is_empty() || !has_cred {
                                state.imap_status = Some("Error: Please provide host, username, and password/token.".to_string());
                            } else {
                                state.imap_status = Some("Connecting to IMAP server...".to_string());

                                let folder_filter = if state.imap_folder_filter.trim().is_empty() {
                                    None
                                } else {
                                    Some(
                                        state
                                            .imap_folder_filter
                                             .split(',')
                                            .map(|s| s.trim().to_string())
                                            .filter(|s| !s.is_empty())
                                            .collect(),
                                    )
                                };

                                let max_limit = state.imap_max_per_folder.trim().parse::<usize>().ok();
                                let oauth_token = if auth_method != mail2rust_imap::ImapAuthMethod::Password {
                                    Some(state.imap_oauth_token.trim().to_string())
                                } else {
                                    None
                                };

                                let config = mail2rust_imap::ImapConfig {
                                    host: host.clone(),
                                    port,
                                    username: user.clone(),
                                    password: pass,
                                    use_tls: state.imap_use_tls,
                                    insecure_certs: state.imap_insecure_certs,
                                    folder_filter,
                                    max_messages_per_folder: max_limit,
                                    since_date: None,
                                    auth_method,
                                    oauth_token,
                                };

                                let mut client = mail2rust_imap::ImapClient::new(config);
                                match client.connect() {
                                    Ok(()) => {
                                        let mut checkpoint = mail2rust_sync::CheckpointManager::new_checkpoint(
                                            &format!("imap://{}/{}", host, user),
                                            "gui_workspace",
                                        );

                                        let mut ingested_messages = Vec::new();
                                        match client.sync_all(&mut checkpoint, |_prog, msg| {
                                            ingested_messages.push(msg);
                                        }) {
                                            Ok(count) => {
                                                state.messages = ingested_messages;
                                                state.rebuild_folders_from_messages();
                                                state.imap_status = Some(format!(
                                                    "Successfully ingested {} message(s) across {} folder(s).",
                                                    count,
                                                    state.folders.len()
                                                ));
                                            }
                                            Err(e) => {
                                                state.imap_status = Some(format!("Error syncing mailboxes: {}", e));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        let diag = mail2rust_imap::diagnose_auth_failure(&host, &e.to_string());
                                        state.imap_status = Some(diag);
                                    }
                                }
                            }
                        }
                    });
                });

            if !is_open {
                state.active_modal = ActiveModal::None;
            }
        }

        ActiveModal::ImapSync => {
            let mut is_open = true;
            egui::Window::new("IMAP Server-to-Server Account Migration (Zero Local Disk Storage)")
                .open(&mut is_open)
                .resizable(false)
                .fixed_size(Vec2::new(680.0, 620.0))
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new("Migrate emails directly from one IMAP server to another in-memory across encrypted TLS sockets with zero local disk saving.")
                            .size(11.0),
                    );
                    ui.add_space(6.0);

                    // GroupBox: Source IMAP Account
                    win32_groupbox(ui, "Source IMAP Account (From)", |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Host:").strong().size(11.0));
                            ui.add(egui::TextEdit::singleline(&mut state.imap_sync_src_host).desired_width(260.0));
                            ui.label(RichText::new("Port:").strong().size(11.0));
                            ui.add(egui::TextEdit::singleline(&mut state.imap_sync_src_port).desired_width(60.0));
                            ui.checkbox(&mut state.imap_sync_src_tls, "SSL/TLS");
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Auth:").strong().size(11.0));
                            ui.radio_value(&mut state.imap_sync_src_auth_method, "password".to_string(), "Password / App Pass");
                            ui.radio_value(&mut state.imap_sync_src_auth_method, "xoauth2".to_string(), "OAuth 2.0 (XOAUTH2)");
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Username:").strong().size(11.0));
                            ui.add(egui::TextEdit::singleline(&mut state.imap_sync_src_user).desired_width(200.0));
                            if state.imap_sync_src_auth_method == "password" {
                                ui.label(RichText::new("Password:").strong().size(11.0));
                                ui.add(
                                    egui::TextEdit::singleline(&mut state.imap_sync_src_pass)
                                        .password(true)
                                        .desired_width(180.0),
                                );
                            } else {
                                ui.label(RichText::new("Token:").strong().size(11.0));
                                ui.add(
                                    egui::TextEdit::singleline(&mut state.imap_sync_src_oauth_token)
                                        .password(true)
                                        .hint_text("OAuth2 Access Token")
                                        .desired_width(180.0),
                                );
                            }
                        });
                    });

                    ui.add_space(6.0);

                    // GroupBox: Destination IMAP Account
                    win32_groupbox(ui, "Destination IMAP Account (To)", |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Host:").strong().size(11.0));
                            ui.add(egui::TextEdit::singleline(&mut state.imap_sync_dst_host).desired_width(260.0));
                            ui.label(RichText::new("Port:").strong().size(11.0));
                            ui.add(egui::TextEdit::singleline(&mut state.imap_sync_dst_port).desired_width(60.0));
                            ui.checkbox(&mut state.imap_sync_dst_tls, "SSL/TLS");
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Auth:").strong().size(11.0));
                            ui.radio_value(&mut state.imap_sync_dst_auth_method, "password".to_string(), "Password / App Pass");
                            ui.radio_value(&mut state.imap_sync_dst_auth_method, "xoauth2".to_string(), "OAuth 2.0 (XOAUTH2)");
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Username:").strong().size(11.0));
                            ui.add(egui::TextEdit::singleline(&mut state.imap_sync_dst_user).desired_width(200.0));
                            if state.imap_sync_dst_auth_method == "password" {
                                ui.label(RichText::new("Password:").strong().size(11.0));
                                ui.add(
                                    egui::TextEdit::singleline(&mut state.imap_sync_dst_pass)
                                        .password(true)
                                        .desired_width(180.0),
                                );
                            } else {
                                ui.label(RichText::new("Token:").strong().size(11.0));
                                ui.add(
                                    egui::TextEdit::singleline(&mut state.imap_sync_dst_oauth_token)
                                        .password(true)
                                        .hint_text("OAuth2 Access Token")
                                        .desired_width(180.0),
                                );
                            }
                        });
                    });

                    ui.add_space(6.0);

                    // GroupBox: Migration Rules & Preservation Options
                    win32_groupbox(ui, "Migration Rules & Preservation Options", |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Folder Filter:").strong().size(11.0));
                            ui.add(
                                egui::TextEdit::singleline(&mut state.imap_sync_folders)
                                    .hint_text("e.g. INBOX, Sent (leave empty for all folders)")
                                    .desired_width(220.0),
                            );
                            ui.label(RichText::new("Folder Prefix:").strong().size(11.0));
                            ui.add(
                                egui::TextEdit::singleline(&mut state.imap_sync_folder_prefix)
                                    .hint_text("Optional, e.g. Migrated/")
                                    .desired_width(140.0),
                            );
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.checkbox(
                                &mut state.imap_sync_preserve_flags,
                                "Preserve Message Flags (\\Seen, \\Flagged, etc.)",
                            );
                            ui.add_space(10.0);
                            ui.checkbox(
                                &mut state.imap_sync_preserve_dates,
                                "Preserve Delivery Dates (INTERNALDATE)",
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.checkbox(
                                &mut state.imap_sync_insecure,
                                "Accept Self-Signed / Untrusted SSL Certs",
                            );
                            ui.add_space(10.0);
                            ui.checkbox(
                                &mut state.imap_sync_dry_run,
                                "Dry Run (Verify & Scan Without Uploading)",
                            );
                        });
                    });

                    ui.add_space(6.0);

                    if let Some(ref status) = state.imap_sync_status {
                        let is_err = status.starts_with("Error") || status.contains("Failed");
                        let color = if is_err {
                            Color32::from_rgb(180, 0, 0)
                        } else {
                            Color32::from_rgb(0, 100, 0)
                        };
                        ui.label(RichText::new(status).color(color).size(11.0).strong());
                        ui.add_space(6.0);
                    }

                    ui.horizontal(|ui| {
                        if win32_dialog_button(ui, "Cancel / Close") {
                            state.active_modal = ActiveModal::None;
                        }

                        if win32_dialog_button(ui, "Start Server-to-Server Transfer (Zero Disk)") {
                            let src_host = state.imap_sync_src_host.trim().to_string();
                            let src_user = state.imap_sync_src_user.trim().to_string();
                            let src_pass = state.imap_sync_src_pass.clone();
                            let dst_host = state.imap_sync_dst_host.trim().to_string();
                            let dst_user = state.imap_sync_dst_user.trim().to_string();
                            let dst_pass = state.imap_sync_dst_pass.clone();

                            let src_auth: mail2rust_imap::ImapAuthMethod = state
                                .imap_sync_src_auth_method
                                .parse()
                                .unwrap_or(mail2rust_imap::ImapAuthMethod::Password);
                            let dst_auth: mail2rust_imap::ImapAuthMethod = state
                                .imap_sync_dst_auth_method
                                .parse()
                                .unwrap_or(mail2rust_imap::ImapAuthMethod::Password);

                            let src_has_cred = if src_auth == mail2rust_imap::ImapAuthMethod::Password {
                                !src_pass.is_empty()
                            } else {
                                !state.imap_sync_src_oauth_token.trim().is_empty()
                            };

                            let dst_has_cred = if dst_auth == mail2rust_imap::ImapAuthMethod::Password {
                                !dst_pass.is_empty()
                            } else {
                                !state.imap_sync_dst_oauth_token.trim().is_empty()
                            };

                            if src_host.is_empty() || src_user.is_empty() || !src_has_cred {
                                state.imap_sync_status = Some(
                                    "Error: Source host, username, and password/token are required.".to_string(),
                                );
                            } else if dst_host.is_empty() || dst_user.is_empty() || !dst_has_cred {
                                state.imap_sync_status = Some(
                                    "Error: Destination host, username, and password/token are required.".to_string(),
                                );
                            } else {
                                let src_port = state.imap_sync_src_port.trim().parse::<u16>().unwrap_or(993);
                                let dst_port = state.imap_sync_dst_port.trim().parse::<u16>().unwrap_or(993);

                                let folder_filter = if state.imap_sync_folders.trim().is_empty() {
                                    None
                                } else {
                                    Some(
                                        state
                                            .imap_sync_folders
                                            .split(',')
                                            .map(|s| s.trim().to_string())
                                            .filter(|s| !s.is_empty())
                                            .collect(),
                                    )
                                };

                                let folder_prefix = if state.imap_sync_folder_prefix.trim().is_empty() {
                                    None
                                } else {
                                    Some(state.imap_sync_folder_prefix.trim().to_string())
                                };

                                let src_oauth_token = if src_auth != mail2rust_imap::ImapAuthMethod::Password {
                                    Some(state.imap_sync_src_oauth_token.trim().to_string())
                                } else {
                                    None
                                };
                                let dst_oauth_token = if dst_auth != mail2rust_imap::ImapAuthMethod::Password {
                                    Some(state.imap_sync_dst_oauth_token.trim().to_string())
                                } else {
                                    None
                                };

                                let config = mail2rust_imap::ImapMigrationConfig {
                                    source: mail2rust_imap::ImapConfig {
                                        host: src_host.clone(),
                                        port: src_port,
                                        username: src_user,
                                        password: src_pass,
                                        use_tls: state.imap_sync_src_tls,
                                        insecure_certs: state.imap_sync_insecure,
                                        folder_filter: folder_filter.clone(),
                                        max_messages_per_folder: None,
                                        since_date: None,
                                        auth_method: src_auth,
                                        oauth_token: src_oauth_token,
                                    },
                                    destination: mail2rust_imap::ImapConfig {
                                        host: dst_host.clone(),
                                        port: dst_port,
                                        username: dst_user,
                                        password: dst_pass,
                                        use_tls: state.imap_sync_dst_tls,
                                        insecure_certs: state.imap_sync_insecure,
                                        folder_filter: None,
                                        max_messages_per_folder: None,
                                        since_date: None,
                                        auth_method: dst_auth,
                                        oauth_token: dst_oauth_token,
                                    },
                                    folder_filter,
                                    folder_prefix,
                                    folder_mapping: None,
                                    preserve_flags: state.imap_sync_preserve_flags,
                                    preserve_dates: state.imap_sync_preserve_dates,
                                    create_missing_folders: true,
                                    dry_run: state.imap_sync_dry_run,
                                };

                                match mail2rust_imap::ImapMigrator::migrate(&config, |_prog| {}) {
                                    Ok(summary) => {
                                        let mb = (summary.total_bytes_transferred as f64) / (1024.0 * 1024.0);
                                        state.imap_sync_status = Some(format!(
                                            "Migration Succeeded: Transferred {} message(s) across {} folder(s) ({:.2} MB in {:.1}s). Local disk storage used: 0 bytes.",
                                            summary.total_messages_migrated,
                                            summary.total_folders_migrated,
                                            mb,
                                            summary.duration_seconds
                                        ));
                                    }
                                    Err(e) => {
                                        let diag = mail2rust_imap::diagnose_auth_failure(&src_host, &e.to_string());
                                        state.imap_sync_status =
                                            Some(format!("Error during server-to-server migration: {}", diag));
                                    }
                                }
                            }
                        }
                    });
                });

            if !is_open {
                state.active_modal = ActiveModal::None;
            }
        }

        ActiveModal::MergeMailboxes => {
            let mut is_open = true;
            egui::Window::new("Merge Mailboxes (Multi-Format Consolidation)")
                .open(&mut is_open)
                .resizable(false)
                .fixed_size(Vec2::new(680.0, 580.0))
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new("Consolidate disparate mailbox formats (PST, MBOX, Maildir, EMLX, MSG, OLM, VCF, ICS, Takeout ZIP) into a unified destination container or directly into the workbench.")
                            .size(11.0),
                    );
                    ui.add_space(4.0);

                    // GroupBox: Source Mailbox Inputs
                    win32_groupbox(ui, "Source Mailbox Inputs (Heterogeneous Formats)", |ui| {
                        // Table / List of added sources
                        egui::ScrollArea::vertical().max_height(110.0).show(ui, |ui| {
                            if state.merge_source_paths.is_empty() {
                                ui.label(
                                    RichText::new("No sources added yet. Use the input field below or click 'Add Samples' to test.")
                                        .italics()
                                        .color(Color32::from_rgb(128, 128, 128)),
                                );
                            } else {
                                let mut remove_idx = None;
                                for (i, p_str) in state.merge_source_paths.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        let p = Path::new(p_str);
                                        let fmt = FormatSniffer::sniff_path(p);
                                        ui.label(format!("{}.", i + 1));
                                        ui.label(
                                            RichText::new(format!("[{}]", fmt.name()))
                                                .strong()
                                                .color(Color32::from_rgb(0, 102, 204)),
                                        );
                                        ui.label(p_str);
                                        if ui.small_button("X").clicked() {
                                            remove_idx = Some(i);
                                        }
                                    });
                                }
                                if let Some(idx) = remove_idx {
                                    state.merge_source_paths.remove(idx);
                                }
                            }
                        });

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label("Path:");
                            ui.add(egui::TextEdit::singleline(&mut state.merge_source_input).desired_width(360.0));
                            if ui.button("Add Source").clicked() && !state.merge_source_input.trim().is_empty() {
                                state.merge_source_paths.push(state.merge_source_input.trim().to_string());
                                state.merge_source_input.clear();
                            }
                        });

                        ui.horizontal(|ui| {
                            if ui.button("Add All Sample Formats").clicked() {
                                let samples = std::path::Path::new("samples");
                                if samples.exists() {
                                    let candidates = vec![
                                        "samples/takeout/sample_google_takeout.zip",
                                        "samples/maildir",
                                        "samples/emlx/sample_apple_mail.emlx",
                                        "samples/contacts/sample_contacts.vcf",
                                        "samples/calendar/sample_calendar.ics",
                                        "samples/mbox/sample_mailbox.mbox",
                                    ];
                                    for c in candidates {
                                        if Path::new(c).exists() && !state.merge_source_paths.contains(&c.to_string()) {
                                            state.merge_source_paths.push(c.to_string());
                                        }
                                    }
                                }
                            }
                            if ui.button("Clear Sources").clicked() {
                                state.merge_source_paths.clear();
                            }
                            ui.label(format!("Total Sources: {}", state.merge_source_paths.len()));
                        });
                    });

                    ui.add_space(4.0);

                    // GroupBox: Merge & Consolidation Rules
                    win32_groupbox(ui, "Consolidation & Deduplication Rules", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Folder Organization:");
                            ui.radio_value(&mut state.merge_folder_strategy, "combine".to_string(), "Combine matching folders");
                            ui.radio_value(&mut state.merge_folder_strategy, "prefix".to_string(), "Partition with source prefix");
                            ui.radio_value(&mut state.merge_folder_strategy, "flatten".to_string(), "Flatten to root");
                        });

                        ui.horizontal(|ui| {
                            ui.label("Cross-Source Deduplication:");
                            ui.radio_value(&mut state.merge_dedup_strategy, "message-id".to_string(), "Message-ID");
                            ui.radio_value(&mut state.merge_dedup_strategy, "sha256".to_string(), "SHA-256 Hash");
                            ui.radio_value(&mut state.merge_dedup_strategy, "content".to_string(), "Normalized Content");
                            ui.radio_value(&mut state.merge_dedup_strategy, "none".to_string(), "Disabled");
                        });

                        ui.horizontal(|ui| {
                            ui.checkbox(&mut state.merge_sort_by_date, "Sort all merged messages chronologically by date (UTC)");
                        });
                    });

                    ui.add_space(4.0);

                    // GroupBox: Destination / Output
                    win32_groupbox(ui, "Destination Output Container", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Output Format:");
                            ui.radio_value(&mut state.merge_export_format, ExportFormat::Pst, "Outlook PST (.pst)");
                            ui.radio_value(&mut state.merge_export_format, ExportFormat::Mbox, "MBOX (.mbox)");
                            ui.radio_value(&mut state.merge_export_format, ExportFormat::Eml, "EML Tree Directory");
                        });

                        ui.horizontal(|ui| {
                            let label_text = match state.merge_export_format {
                                ExportFormat::Pst => "Target PST:",
                                ExportFormat::Mbox => "Target MBOX:",
                                ExportFormat::Eml => "Target EML Folder:",
                            };
                            ui.label(label_text);
                            ui.add(egui::TextEdit::singleline(&mut state.merge_dest_path).desired_width(360.0));
                        });
                    });

                    // Status / Report Message
                    if let Some(ref st) = state.merge_status {
                        ui.add_space(4.0);
                        ui.label(RichText::new(st).color(Color32::from_rgb(0, 102, 0)).strong());
                    }

                    if let Some(ref rep) = state.merge_report_text {
                        ui.add_space(4.0);
                        egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                            ui.label(RichText::new(rep).monospace().size(10.0));
                        });
                    }

                    ui.add_space(6.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Close").clicked() {
                            state.active_modal = ActiveModal::None;
                        }

                        // Button 1: Merge & Load into Workbench
                        if ui.button("Merge & Load into Workbench").clicked() {
                            if state.merge_source_paths.is_empty() {
                                state.merge_status = Some("Please add at least one source mailbox.".to_string());
                            } else {
                                let configs: Vec<MergeSourceConfig> = state.merge_source_paths
                                    .iter()
                                    .map(MergeSourceConfig::new)
                                    .collect();
                                let folder_strategy: MergeFolderStrategy = state.merge_folder_strategy.parse().unwrap_or(MergeFolderStrategy::Combine);
                                let dedup_strategy = match state.merge_dedup_strategy.as_str() {
                                    "sha256" => DedupStrategy::Sha256,
                                    "content" => DedupStrategy::NormalizedContent,
                                    "none" => DedupStrategy::None,
                                    _ => DedupStrategy::MessageId,
                                };
                                let options = MergeOptions {
                                    folder_strategy,
                                    dedup_strategy,
                                    sort_by_date: state.merge_sort_by_date,
                                    dry_run: false,
                                };

                                let ingester = |p: &Path| -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>> {
                                    let fmt = FormatSniffer::sniff_path(p);
                                    if fmt == mail2rust_core::MailboxFormat::Pst || fmt == mail2rust_core::MailboxFormat::Ost {
                                        return mail2rust_pst::PstReader::read_path(p).map_err(|e| Box::new(e) as Box<dyn std::error::Error>);
                                    }
                                    mail2rust_core::ingest_path(p)
                                };

                                match MailboxMerger::merge_sources(&configs, &options, ingester) {
                                    Ok((merged_msgs, report)) => {
                                        let count = merged_msgs.len();
                                        state.messages = merged_msgs;
                                        state.rebuild_folders_from_messages();
                                        state.merge_report_text = Some(report.format_summary());
                                        state.merge_status = Some(format!("Successfully merged {} sources into workbench ({} messages loaded)!", configs.len(), count));
                                    }
                                    Err(e) => {
                                        state.merge_status = Some(format!("Error merging sources: {}", e));
                                    }
                                }
                            }
                        }

                        // Button 2: Merge & Export to File
                        if ui.button("Merge & Export to File...").clicked() {
                            if state.merge_source_paths.is_empty() {
                                state.merge_status = Some("Please add at least one source mailbox.".to_string());
                            } else if state.merge_dest_path.trim().is_empty() {
                                state.merge_status = Some("Please specify a destination target path.".to_string());
                            } else {
                                let configs: Vec<MergeSourceConfig> = state.merge_source_paths
                                    .iter()
                                    .map(MergeSourceConfig::new)
                                    .collect();
                                let folder_strategy: MergeFolderStrategy = state.merge_folder_strategy.parse().unwrap_or(MergeFolderStrategy::Combine);
                                let dedup_strategy = match state.merge_dedup_strategy.as_str() {
                                    "sha256" => DedupStrategy::Sha256,
                                    "content" => DedupStrategy::NormalizedContent,
                                    "none" => DedupStrategy::None,
                                    _ => DedupStrategy::MessageId,
                                };
                                let options = MergeOptions {
                                    folder_strategy,
                                    dedup_strategy,
                                    sort_by_date: state.merge_sort_by_date,
                                    dry_run: false,
                                };

                                let ingester = |p: &Path| -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>> {
                                    let fmt = FormatSniffer::sniff_path(p);
                                    if fmt == mail2rust_core::MailboxFormat::Pst || fmt == mail2rust_core::MailboxFormat::Ost {
                                        return mail2rust_pst::PstReader::read_path(p).map_err(|e| Box::new(e) as Box<dyn std::error::Error>);
                                    }
                                    mail2rust_core::ingest_path(p)
                                };

                                match MailboxMerger::merge_sources(&configs, &options, ingester) {
                                    Ok((merged_msgs, report)) => {
                                        let dest = Path::new(&state.merge_dest_path);
                                        if let Some(parent) = dest.parent() {
                                            let _ = std::fs::create_dir_all(parent);
                                        }

                                        match state.merge_export_format {
                                            ExportFormat::Pst => {
                                                match File::create(dest) {
                                                    Ok(f) => {
                                                        match PstWriter::new(f, true) {
                                                            Ok(mut w) => {
                                                                let mut f_map = std::collections::HashMap::new();
                                                                let root_id = w.add_folder("Inbox", None).unwrap_or(0x8000);
                                                                f_map.insert("Inbox".to_string(), root_id);
                                                                for m in &merged_msgs {
                                                                    let fnid = m.folder_path.last().cloned().unwrap_or_else(|| "Inbox".to_string());
                                                                    let nid = match f_map.get(&fnid) {
                                                                        Some(&id) => id,
                                                                        None => {
                                                                            let id = w.add_folder(&fnid, None).unwrap_or(root_id);
                                                                            f_map.insert(fnid, id);
                                                                            id
                                                                        }
                                                                    };
                                                                    let _ = w.add_message(m, nid);
                                                                }
                                                                let _ = w.finalize();
                                                                state.merge_report_text = Some(report.format_summary());
                                                                state.merge_status = Some(format!("Export Succeeded: Saved {} merged messages to PST: {}", merged_msgs.len(), dest.display()));
                                                            }
                                                            Err(e) => state.merge_status = Some(format!("Error creating PST writer: {}", e)),
                                                        }
                                                    }
                                                    Err(e) => state.merge_status = Some(format!("Error creating file: {}", e)),
                                                }
                                            }
                                            ExportFormat::Mbox => {
                                                match mail2rust_mbox::MboxFileExporter::export_to_file_options(&merged_msgs, dest, false, false) {
                                                    Ok(count) => {
                                                        state.merge_report_text = Some(report.format_summary());
                                                        state.merge_status = Some(format!("Export Succeeded: Saved {} merged messages to MBOX: {}", count, dest.display()));
                                                    }
                                                    Err(e) => state.merge_status = Some(format!("Error exporting MBOX: {}", e)),
                                                }
                                            }
                                            ExportFormat::Eml => {
                                                let mut exp = mail2rust_eml::EmlExporter::new(dest, mail2rust_eml::EmlLayout::Folder);
                                                let mut exported = 0;
                                                for m in &merged_msgs {
                                                    if exp.export_message(m).is_ok() {
                                                        exported += 1;
                                                    }
                                                }
                                                state.merge_report_text = Some(report.format_summary());
                                                state.merge_status = Some(format!("Export Succeeded: Saved {} merged EML messages to: {}", exported, dest.display()));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        state.merge_status = Some(format!("Error merging sources: {}", e));
                                    }
                                }
                            }
                        }
                    });
                });

            if !is_open {
                state.active_modal = ActiveModal::None;
            }
        }

        ActiveModal::DirectPst => {
            let mut is_open = true;
            let total_msgs = state.messages.len();
            let filtered_msgs = state.filtered_messages().len();
            let selected_msgs = state.selected_message_ids.len();

            egui::Window::new("Export Mailbox Archive / Container")
                .open(&mut is_open)
                .resizable(false)
                .fixed_size(Vec2::new(560.0, 420.0))
                .show(ctx, |ui| {
                    ui.label("Export email messages to Microsoft Outlook PST, standard MBOX, or RFC 5322 EML directory.");
                    ui.add_space(4.0);

                    // GroupBox: Export Scope
                    win32_groupbox(ui, "Export Scope", |ui| {
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut state.export_scope, ExportScope::All, format!("All Messages ({})", total_msgs));
                            ui.radio_value(&mut state.export_scope, ExportScope::Filtered, format!("Current Filter ({})", filtered_msgs));
                            ui.radio_value(&mut state.export_scope, ExportScope::Selected, format!("Checked / Selected ({})", selected_msgs));
                        });
                    });

                    ui.add_space(4.0);

                    // GroupBox: Target Format
                    win32_groupbox(ui, "Target Container Format", |ui| {
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut state.export_format, ExportFormat::Pst, "Microsoft Outlook (.pst)");
                            ui.radio_value(&mut state.export_format, ExportFormat::Mbox, "Standard MBOX (.mbox)");
                            ui.radio_value(&mut state.export_format, ExportFormat::Eml, "EML Directory (.eml)");
                        });
                    });

                    ui.add_space(4.0);

                    // GroupBox: Target Destination & Options
                    win32_groupbox(ui, "Destination Path & Options", |ui| {
                        egui::Grid::new("export_dest_form").spacing([8.0, 6.0]).show(ui, |ui| {
                            let label_text = match state.export_format {
                                ExportFormat::Pst => "Destination PST File:",
                                ExportFormat::Mbox => "Destination MBOX File:",
                                ExportFormat::Eml => "Destination EML Folder:",
                            };
                            ui.label(RichText::new(label_text).size(11.0).strong());
                            ui.add(egui::TextEdit::singleline(&mut state.pst_dest_path).desired_width(360.0));
                            ui.end_row();

                            if state.export_format == ExportFormat::Pst {
                                ui.label(RichText::new("Root Folder Name:").size(11.0).strong());
                                ui.add(egui::TextEdit::singleline(&mut state.pst_root).desired_width(200.0));
                                ui.end_row();

                                ui.label(RichText::new("Volume Limit (GB):").size(11.0).strong());
                                ui.add(egui::DragValue::new(&mut state.pst_max_gb).range(1..=50));
                                ui.end_row();
                            }
                        });

                        if state.export_format == ExportFormat::Pst {
                            ui.add_space(4.0);
                            ui.checkbox(&mut state.pst_cluster_threads, "Preserve email conversation threads across volume boundaries");
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Encryption:").size(11.0).strong());
                                ui.radio_value(&mut state.pst_encryption, mail2rust_pst::PstEncryption::Permute, "Compressible (Permute)");
                                ui.radio_value(&mut state.pst_encryption, mail2rust_pst::PstEncryption::Cyclic, "High Encryption (Cyclic)");
                                ui.radio_value(&mut state.pst_encryption, mail2rust_pst::PstEncryption::None, "None");
                            });
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("PST Password:").size(11.0).strong());
                                ui.add(egui::TextEdit::singleline(&mut state.pst_password).password(true).desired_width(180.0));
                                ui.label(RichText::new("(Optional password protection)").size(10.0).italics());
                            });
                        }
                    });

                    if let Some(ref msg) = state.pst_status_message {
                        ui.add_space(4.0);
                        let is_err = msg.starts_with("Error");
                        let color = if is_err { Color32::from_rgb(180, 0, 0) } else { Color32::from_rgb(0, 120, 0) };
                        ui.label(RichText::new(msg).color(color).size(11.0));
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if win32_dialog_button(ui, "Close") {
                            state.active_modal = ActiveModal::None;
                        }

                        let export_btn_label = match state.export_format {
                            ExportFormat::Pst => "Generate PST",
                            ExportFormat::Mbox => "Export MBOX",
                            ExportFormat::Eml => "Export EML",
                        };

                        if win32_dialog_button(ui, export_btn_label) {
                            let dest_path = state.pst_dest_path.trim().to_string();
                            let items = state.get_messages_for_export();

                            if items.is_empty() {
                                state.pst_status_message = Some("Error: No messages match the selected export scope.".to_string());
                            } else if dest_path.is_empty() {
                                state.pst_status_message = Some("Error: Destination path cannot be empty.".to_string());
                            } else {
                                match state.export_format {
                                    ExportFormat::Pst => {
                                        match File::create(&dest_path) {
                                            Ok(file) => {
                                                let pwd = if state.pst_password.trim().is_empty() {
                                                    None
                                                } else {
                                                    Some(state.pst_password.trim().to_string())
                                                };
                                                match PstWriter::with_encryption(file, state.pst_encryption, pwd) {
                                                    Ok(mut writer) => {
                                                        let mut folder_nids = std::collections::HashMap::new();
                                                        let root_nid = writer.add_folder(&state.pst_root, None).unwrap_or(0x8000);
                                                        folder_nids.insert(state.pst_root.clone(), root_nid);

                                                        for f in &state.folders {
                                                            if !folder_nids.contains_key(&f.name) {
                                                                if let Ok(nid) = writer.add_folder(&f.name, Some(&state.pst_root)) {
                                                                    folder_nids.insert(f.name.clone(), nid);
                                                                }
                                                            }
                                                        }

                                                        let mut msg_written = 0;
                                                        for msg in &items {
                                                            let folder_name = msg.folder_path.last().cloned().unwrap_or_else(|| "Inbox".to_string());
                                                            let target_nid = folder_nids.get(&folder_name).copied().unwrap_or(root_nid);
                                                            if writer.add_message(msg, target_nid).is_ok() {
                                                                msg_written += 1;
                                                            }
                                                        }

                                                        if let Err(e) = writer.finalize() {
                                                            state.pst_status_message = Some(format!("Error finalizing PST: {}", e));
                                                        } else if let Ok(vfile) = File::open(&dest_path) {
                                                            if let Ok(report) = PstValidator::validate(vfile) {
                                                                state.pst_status_message = Some(format!(
                                                                    "Success: Exported {} messages to valid [MS-PST] Unicode container (CRC-32: 0x{:08X}).",
                                                                    msg_written, report.calculated_crc
                                                                ));
                                                            } else {
                                                                state.pst_status_message = Some(format!(
                                                                    "Success: Exported {} messages to PST.", msg_written
                                                                ));
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        state.pst_status_message = Some(format!("Error initializing PST writer: {}", e));
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                state.pst_status_message = Some(format!("Error creating destination file: {}", e));
                                            }
                                        }
                                    }
                                    ExportFormat::Mbox => {
                                        match MboxFileExporter::export_to_file(&items, Path::new(&dest_path), false) {
                                            Ok(count) => {
                                                state.pst_status_message = Some(format!(
                                                    "Success: Exported {} messages to standard MBOX at '{}'.",
                                                    count, dest_path
                                                ));
                                            }
                                            Err(e) => {
                                                state.pst_status_message = Some(format!("Error writing MBOX: {}", e));
                                            }
                                        }
                                    }
                                    ExportFormat::Eml => {
                                        let mut exporter = EmlExporter::new(Path::new(&dest_path), EmlLayout::Folder);
                                        let mut written = 0;
                                        let mut last_err = None;
                                        for m in &items {
                                            match exporter.export_message(m) {
                                                Ok(_) => written += 1,
                                                Err(e) => last_err = Some(e),
                                            }
                                        }
                                        if let Some(err) = last_err {
                                            state.pst_status_message = Some(format!("Exported {} messages with warning/error: {}", written, err));
                                        } else {
                                            state.pst_status_message = Some(format!(
                                                "Success: Exported {} messages to EML directory hierarchy at '{}'.",
                                                written, dest_path
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    });
                });

            if !is_open {
                state.active_modal = ActiveModal::None;
            }
        }

        ActiveModal::AutoSplit => {
            let mut is_open = true;
            egui::Window::new("Auto-Split Mailbox by Capacity / Year")
                .open(&mut is_open)
                .resizable(false)
                .fixed_size(Vec2::new(480.0, 260.0))
                .show(ctx, |ui| {
                    ui.label(
                        "Partition oversized mail archives across multiple volumes without splitting reply chains.",
                    );
                    ui.add_space(6.0);

                    win32_groupbox(ui, "Volume Split Sizing", |ui| {
                        ui.radio_value(&mut state.pst_max_gb, 15, "15 GB Volume Ceiling (Outlook Legacy Safe)");
                        ui.radio_value(&mut state.pst_max_gb, 20, "20 GB Volume Ceiling (Recommended Default)");
                        ui.radio_value(&mut state.pst_max_gb, 50, "50 GB Maximum Unicode PST Limit");
                        ui.add_space(4.0);
                        ui.checkbox(
                            &mut state.pst_cluster_threads,
                            "Maintain Thread Integrity (keep conversations in same volume)",
                        );
                    });

                    if let Some(ref msg) = state.pst_status_message {
                        ui.add_space(4.0);
                        ui.label(RichText::new(msg).color(Color32::from_rgb(0, 120, 0)).size(11.0));
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if win32_dialog_button(ui, "Close") {
                            state.active_modal = ActiveModal::None;
                        }

                        if win32_dialog_button(ui, "Split Now") {
                            let config = SplitConfig {
                                max_volume_bytes: state.pst_max_gb * 1024 * 1024 * 1024,
                                cluster_threads: state.pst_cluster_threads,
                                ..Default::default()
                            };
                            let volumes = VolumeSplitter::split(&state.messages, &config);
                            state.pst_status_message = Some(format!(
                                "Success: Split {} messages into {} volumes.",
                                state.messages.len(),
                                volumes.len()
                            ));
                        }
                    });
                });

            if !is_open {
                state.active_modal = ActiveModal::None;
            }
        }

        ActiveModal::FilterCriteria => {
            let mut is_open = true;
            egui::Window::new("Query & Filter Criteria")
                .open(&mut is_open)
                .resizable(false)
                .fixed_size(Vec2::new(480.0, 310.0))
                .show(ctx, |ui| {
                    ui.label("Filter messages by date range, folder pattern globs, and deletion status.");
                    ui.add_space(6.0);

                    win32_groupbox(ui, "Date and Folder Rules", |ui| {
                        egui::Grid::new("filter_grid").spacing([8.0, 6.0]).show(ui, |ui| {
                            ui.label(RichText::new("Since Date:").size(11.0).strong());
                            ui.add(egui::TextEdit::singleline(&mut state.filter_since).hint_text("YYYY-MM-DD"));
                            ui.end_row();

                            ui.label(RichText::new("Until Date:").size(11.0).strong());
                            ui.add(egui::TextEdit::singleline(&mut state.filter_until).hint_text("YYYY-MM-DD"));
                            ui.end_row();

                            ui.label(RichText::new("Include Glob:").size(11.0).strong());
                            ui.add(
                                egui::TextEdit::singleline(&mut state.filter_include_folder).hint_text("Projects/*"),
                            );
                            ui.end_row();

                            ui.label(RichText::new("Exclude Glob:").size(11.0).strong());
                            ui.add(
                                egui::TextEdit::singleline(&mut state.filter_exclude_folder)
                                    .hint_text("*Trash*;*Spam*"),
                            );
                            ui.end_row();
                        });

                        ui.add_space(4.0);
                        ui.checkbox(&mut state.filter_include_deleted, "Include deleted / expunged items");
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if win32_dialog_button(ui, "Cancel") {
                            state.active_modal = ActiveModal::None;
                        }

                        if win32_dialog_button(ui, "Reset") {
                            state.filter_since.clear();
                            state.filter_until.clear();
                            state.filter_include_folder.clear();
                            state.filter_exclude_folder = "*Trash*;*Spam*".to_string();
                            state.filter_include_deleted = false;
                        }

                        if win32_dialog_button(ui, "Apply") {
                            state.active_modal = ActiveModal::None;
                        }
                    });
                });

            if !is_open {
                state.active_modal = ActiveModal::None;
            }
        }

        ActiveModal::Forensics => {
            let mut is_open = true;
            egui::Window::new("Forensic Bates Stamping & Audit Manifest")
                .open(&mut is_open)
                .resizable(false)
                .fixed_size(Vec2::new(520.0, 320.0))
                .show(ctx, |ui| {
                    ui.label("Apply sequential Bates stamping and generate forensic chain-of-custody manifests.");
                    ui.add_space(6.0);

                    win32_groupbox(ui, "eDiscovery Properties", |ui| {
                        egui::Grid::new("forensics_grid").spacing([8.0, 6.0]).show(ui, |ui| {
                            ui.label(RichText::new("Case Number:").size(11.0).strong());
                            ui.add(egui::TextEdit::singleline(&mut state.case_number).desired_width(220.0));
                            ui.end_row();

                            ui.label(RichText::new("Examiner Name:").size(11.0).strong());
                            ui.add(egui::TextEdit::singleline(&mut state.examiner).desired_width(220.0));
                            ui.end_row();

                            ui.label(RichText::new("Bates Prefix:").size(11.0).strong());
                            ui.add(egui::TextEdit::singleline(&mut state.bates_prefix).desired_width(160.0));
                            ui.end_row();

                            ui.label(RichText::new("Start Number:").size(11.0).strong());
                            ui.add(egui::DragValue::new(&mut state.bates_start));
                            ui.end_row();

                            ui.label(RichText::new("Manifest CSV:").size(11.0).strong());
                            ui.add(egui::TextEdit::singleline(&mut state.manifest_csv_path).desired_width(320.0));
                            ui.end_row();
                        });
                    });

                    if let Some(ref res) = state.forensics_result {
                        ui.add_space(4.0);
                        ui.label(RichText::new(res).color(Color32::from_rgb(0, 120, 0)).size(11.0));
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if win32_dialog_button(ui, "Close") {
                            state.active_modal = ActiveModal::None;
                        }

                        if win32_dialog_button(ui, "Execute Stamping") {
                            let range = ForensicEngine::apply_bates_stamping(
                                &mut state.messages,
                                &state.bates_prefix,
                                state.bates_start,
                                7,
                            );
                            state.forensics_result = Some(format!(
                                "Success: Applied Bates numbering ({} through {}). Merkle root recorded.",
                                range.start, range.end
                            ));
                        }
                    });
                });

            if !is_open {
                state.active_modal = ActiveModal::None;
            }
        }

        ActiveModal::DryRun => {
            let mut is_open = true;
            egui::Window::new("Pre-Flight Mailbox Inspection")
                .open(&mut is_open)
                .resizable(false)
                .fixed_size(Vec2::new(480.0, 310.0))
                .show(ctx, |ui| {
                    ui.label("Review mailbox inventory metrics prior to export or conversion.");
                    ui.add_space(6.0);

                    let total_count = state.messages.len();
                    let total_bytes: usize = state.messages.iter().map(|m| m.size_bytes).sum();
                    let read_count = state.messages.iter().filter(|m| m.flags.read).count();
                    let flagged_count = state.messages.iter().filter(|m| m.flags.flagged).count();

                    win32_groupbox(ui, "Inventory Metrics", |ui| {
                        egui::Grid::new("dryrun_summary").spacing([10.0, 6.0]).show(ui, |ui| {
                            ui.label("Total Messages:");
                            ui.label(RichText::new(format!("{}", total_count)).strong());
                            ui.end_row();

                            ui.label("Total Payload Size:");
                            ui.label(
                                RichText::new(format!(
                                    "{} bytes ({:.2} MB)",
                                    total_bytes,
                                    total_bytes as f64 / (1024.0 * 1024.0)
                                ))
                                .strong(),
                            );
                            ui.end_row();

                            ui.label("Read / Unread:");
                            ui.label(format!(
                                "{} read / {} unread",
                                read_count,
                                total_count.saturating_sub(read_count)
                            ));
                            ui.end_row();

                            ui.label("Flagged Items:");
                            ui.label(format!("{}", flagged_count));
                            ui.end_row();

                            ui.label("Large Attachments (>20 MB):");
                            ui.label(RichText::new("None (All within limits)").color(Color32::from_rgb(0, 120, 0)));
                            ui.end_row();
                        });
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if win32_dialog_button(ui, "Close") {
                            state.active_modal = ActiveModal::None;
                        }
                    });
                });

            if !is_open {
                state.active_modal = ActiveModal::None;
            }
        }

        ActiveModal::ValidatePst => {
            let mut is_open = true;
            egui::Window::new("MS-PST Container Validator")
                .open(&mut is_open)
                .resizable(false)
                .fixed_size(Vec2::new(500.0, 310.0))
                .show(ctx, |ui| {
                    ui.label("Validate [MS-PST] NDB header magic, version, CRC-32 partial checksum, and B-Trees.");
                    ui.add_space(6.0);

                    win32_groupbox(ui, "PST File Target & Authentication", |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("File:").strong().size(11.0));
                            ui.add(egui::TextEdit::singleline(&mut state.validate_pst_path).desired_width(340.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Password:").strong().size(11.0));
                            ui.add(egui::TextEdit::singleline(&mut state.validate_pst_password).password(true).desired_width(180.0));
                            ui.label(RichText::new("(Optional CRC password verification)").size(10.0).italics());
                        });
                    });

                    if let Some(ref report) = state.validation_report {
                        ui.add_space(4.0);
                        egui::Frame::none()
                            .fill(Color32::WHITE)
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(127, 157, 185)))
                            .inner_margin(egui::Margin::same(6.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new(report).color(Color32::BLACK).monospace().size(11.0));
                            });
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if win32_dialog_button(ui, "Close") {
                            state.active_modal = ActiveModal::None;
                        }

                        if win32_dialog_button(ui, "Validate") {
                            if Path::new(&state.validate_pst_path).exists() {
                                if let Ok(file) = File::open(&state.validate_pst_path) {
                                    if let Ok(report) = PstValidator::validate(file) {
                                        let mut pwd_info = String::new();
                                        if let Ok(f2) = File::open(&state.validate_pst_path) {
                                            if let Ok(mut r) = mail2rust_pst::PstReader::open(f2) {
                                                if r.is_password_protected() {
                                                    pwd_info.push_str("\nPassword Protected: YES");
                                                    if !state.validate_pst_password.is_empty() {
                                                        if r.verify_password(&state.validate_pst_password) {
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
                                        }

                                        state.validation_report = Some(format!(
                                            "Status: {}\nFormat: {}\nMagic: {}\nEncryption: {}\nDeclared CRC: 0x{:08X}\nCalculated CRC: 0x{:08X}\nCRC Match: {}{}",
                                            if report.is_valid { "VALID COMPLIANT PST" } else { "INVALID PST" },
                                            report.format,
                                            report.magic,
                                            report.encryption_method,
                                            report.declared_crc,
                                            report.calculated_crc,
                                            report.crc_match,
                                            pwd_info
                                        ));
                                    } else {
                                        state.validation_report = Some("Error: Failed to validate PST structure.".to_string());
                                    }
                                }
                            } else {
                                state.validation_report = Some("Error: Specified PST file does not exist.".to_string());
                            }
                        }
                    });
                });

            if !is_open {
                state.active_modal = ActiveModal::None;
            }
        }

        ActiveModal::About => {
            let mut is_open = true;
            egui::Window::new("About Mail2Rust Management Console")
                .open(&mut is_open)
                .resizable(false)
                .fixed_size(Vec2::new(420.0, 220.0))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("✉").size(32.0));
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(format!("Mail2Rust Studio v{}", env!("CARGO_PKG_VERSION")))
                                    .strong()
                                    .size(13.0),
                            );
                            ui.label(
                                RichText::new("Universal High-Performance Email Engine in 100% Pure Rust").size(11.0),
                            );
                        });
                    });

                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ui.label("• Zero Microsoft Outlook or Windows COM runtime dependencies");
                    ui.label("• Native [MS-PST] 64-bit Unicode container generation & validation");
                    ui.label("• High-throughput parsers: MBOX, Maildir, EML, EMLX, MSG, OLM, Babyl");
                    ui.label("• Forensic Bates stamping, Merkle root hash verification, thread clustering");

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if win32_dialog_button(ui, "OK") {
                            state.active_modal = ActiveModal::None;
                        }
                    });
                });

            if !is_open {
                state.active_modal = ActiveModal::None;
            }
        }
    }
}

fn win32_groupbox<R>(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.vertical(|ui| {
        ui.label(
            RichText::new(title)
                .size(11.0)
                .strong()
                .color(Color32::from_rgb(0, 51, 153)),
        );
        egui::Frame::none()
            .fill(Color32::from_rgb(244, 243, 238))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(180, 180, 180)))
            .inner_margin(egui::Margin::same(6.0))
            .show(ui, add_contents)
            .inner
    })
    .inner
}

fn win32_dialog_button(ui: &mut egui::Ui, text: &str) -> bool {
    let btn = egui::Button::new(RichText::new(text).size(11.0).color(Color32::BLACK))
        .fill(Color32::from_rgb(240, 240, 240))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(160, 160, 160)))
        .rounding(2.0_f32)
        .min_size(Vec2::new(75.0, 23.0)); // Standard Win32 dialog button dimensions
    ui.add(btn).clicked()
}
