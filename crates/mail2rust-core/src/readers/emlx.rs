use crate::models::{EmailMessage, MessageFlags};
use crate::readers::eml::EmlReader;
use tracing::debug;

pub struct EmlxReader;

impl EmlxReader {
    pub fn parse(raw_bytes: &[u8], folder_path: Vec<String>) -> Result<EmailMessage, Box<dyn std::error::Error>> {
        // 1. Read first line to get byte count
        let newline_pos = raw_bytes.iter().position(|&b| b == b'\n')
            .ok_or("Invalid EMLX: missing byte count newline")?;

        let count_str = std::str::from_utf8(&raw_bytes[..newline_pos])?.trim();
        let mime_len: usize = count_str.parse()
            .map_err(|_| "Invalid EMLX: byte count not a number")?;

        let mime_start = newline_pos + 1;
        let mime_end = (mime_start + mime_len).min(raw_bytes.len());
        let mime_bytes = &raw_bytes[mime_start..mime_end];

        let mut msg = EmlReader::parse(mime_bytes, folder_path)?;
        msg.source_format = "EMLX".to_string();

        // 2. Parse trailing XML plist for flags
        if mime_end < raw_bytes.len() {
            let plist_bytes = &raw_bytes[mime_end..];
            let plist_str = String::from_utf8_lossy(plist_bytes);
            if let Some(flags) = Self::extract_plist_flags(&plist_str) {
                msg.flags = flags;
            }
        }

        debug!(id = %msg.id, subject = %msg.subject, "Parsed Apple Mail EMLX");
        Ok(msg)
    }

    fn extract_plist_flags(plist_str: &str) -> Option<MessageFlags> {
        let key_marker = "<key>flags</key>";
        if let Some(idx) = plist_str.find(key_marker) {
            let sub = &plist_str[idx + key_marker.len()..];
            if let Some(int_start) = sub.find("<integer>") {
                if let Some(int_end) = sub.find("</integer>") {
                    let num_str = sub[int_start + 9..int_end].trim();
                    if let Ok(flags_val) = num_str.parse::<u64>() {
                        return Some(MessageFlags {
                            read: (flags_val & 0x0001) != 0,
                            deleted: (flags_val & 0x0002) != 0,
                            replied: (flags_val & 0x0004) != 0,
                            flagged: (flags_val & 0x0010) != 0,
                            forwarded: (flags_val & 0x0100) != 0,
                            draft: false,
                        });
                    }
                }
            }
        }
        None
    }
}
