use crate::models::{EmailMessage, MessageFlags};
use crate::readers::eml::EmlReader;
use std::path::Path;
use tracing::{debug, info};
use walkdir::WalkDir;

pub struct MaildirReader;

impl MaildirReader {
    pub fn read_dir(root_dir: &Path) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>> {
        let mut messages = Vec::new();
        info!(path = ?root_dir, "Scanning Maildir directory structure");

        for entry in WalkDir::new(root_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let parent = match path.parent() {
                Some(p) => p,
                None => continue,
            };

            let parent_name = parent.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if parent_name != "cur" && parent_name != "new" {
                continue;
            }

            // Determine folder path relative to root
            let mut folder_path = Vec::new();
            if let Ok(rel) = path.strip_prefix(root_dir) {
                let comps: Vec<_> = rel.iter().filter_map(|c| c.to_str()).collect();
                if comps.len() > 1 {
                    for &comp in &comps[..comps.len() - 2] {
                        let clean = comp.trim_start_matches('.');
                        if !clean.is_empty() {
                            folder_path.push(clean.to_string());
                        }
                    }
                }
            }
            if folder_path.is_empty() {
                folder_path.push("Inbox".to_string());
            }

            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let is_new = parent_name == "new";
            let flags = Self::parse_maildir_flags(file_name, is_new);

            let raw_bytes = match std::fs::read(path) {
                Ok(b) => b,
                Err(e) => {
                    debug!(path = ?path, error = %e, "Skipping unreadable Maildir file");
                    continue;
                }
            };

            if let Ok(mut msg) = EmlReader::parse(&raw_bytes, folder_path) {
                msg.source_format = "Maildir".to_string();
                msg.flags = flags;
                messages.push(msg);
            }
        }

        info!(count = messages.len(), "Maildir extraction complete");
        Ok(messages)
    }

    fn parse_maildir_flags(file_name: &str, is_new: bool) -> MessageFlags {
        let mut flags = MessageFlags::default();
        if is_new {
            flags.read = false;
            return flags;
        }

        // Flags are separated by :2, or ;2,
        let info_part = if let Some(idx) = file_name.find(":2,") {
            &file_name[idx + 3..]
        } else if let Some(idx) = file_name.find(";2,") {
            &file_name[idx + 3..]
        } else {
            ""
        };

        for ch in info_part.chars() {
            match ch {
                'S' => flags.read = true,
                'R' => flags.replied = true,
                'F' => flags.flagged = true,
                'P' => flags.forwarded = true,
                'T' => flags.deleted = true,
                'D' => flags.draft = true,
                _ => {}
            }
        }

        flags
    }
}
