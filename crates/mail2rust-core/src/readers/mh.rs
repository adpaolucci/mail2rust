use crate::models::EmailMessage;
use crate::readers::eml::EmlReader;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracing::info;
use walkdir::WalkDir;

pub struct MhReader;

impl MhReader {
    pub fn read_dir(root_dir: &Path) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>> {
        let mut messages = Vec::new();
        info!(path = ?root_dir, "Scanning MH / Sylpheed mailbox");

        // Parse .mh_sequences if present
        let sequences = Self::parse_sequences(root_dir);

        for entry in WalkDir::new(root_dir).max_depth(2).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if let Ok(msg_num) = file_name.parse::<u32>() {
                if let Ok(bytes) = std::fs::read(path) {
                    let folder_path = vec!["MH_Mailbox".to_string()];
                    if let Ok(mut msg) = EmlReader::parse(&bytes, folder_path) {
                        msg.source_format = "MH".to_string();
                        // Apply flags from sequences
                        if let Some(seq_set) = sequences.get("unseen") {
                            msg.flags.read = !seq_set.contains(&msg_num);
                        }
                        if let Some(seq_set) = sequences.get("flagged") {
                            if seq_set.contains(&msg_num) {
                                msg.flags.flagged = true;
                            }
                        }
                        if let Some(seq_set) = sequences.get("replied") {
                            if seq_set.contains(&msg_num) {
                                msg.flags.replied = true;
                            }
                        }
                        messages.push(msg);
                    }
                }
            }
        }

        info!(count = messages.len(), "MH parsing complete");
        Ok(messages)
    }

    fn parse_sequences(dir: &Path) -> HashMap<String, HashSet<u32>> {
        let seq_file = dir.join(".mh_sequences");
        let mut map = HashMap::new();
        if let Ok(content) = std::fs::read_to_string(seq_file) {
            for line in content.lines() {
                if let Some(colon) = line.find(':') {
                    let name = line[..colon].trim().to_lowercase();
                    let nums_part = &line[colon + 1..];
                    let mut set = HashSet::new();
                    for token in nums_part.split_whitespace() {
                        if let Some(dash) = token.find('-') {
                            if let (Ok(start), Ok(end)) =
                                (token[..dash].parse::<u32>(), token[dash + 1..].parse::<u32>())
                            {
                                for n in start..=end {
                                    set.insert(n);
                                }
                            }
                        } else if let Ok(n) = token.parse::<u32>() {
                            set.insert(n);
                        }
                    }
                    map.insert(name, set);
                }
            }
        }
        map
    }
}
