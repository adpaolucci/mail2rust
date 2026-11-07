use crate::models::{Attachment, EmailAddress, EmailMessage, Importance, MessageFlags};
use chrono::Utc;
use std::collections::HashMap;
use tracing::debug;

pub struct MsgReader;

impl MsgReader {
    pub const CFB_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

    pub fn is_msg(bytes: &[u8]) -> bool {
        bytes.len() >= 8 && bytes[0..8] == Self::CFB_MAGIC
    }

    /// Pure-Rust CFB stream scanner for MS-OXMSG Outlook .msg files
    pub fn parse(raw_bytes: &[u8], folder_path: Vec<String>) -> Result<EmailMessage, Box<dyn std::error::Error>> {
        if !Self::is_msg(raw_bytes) {
            return Err("Invalid Outlook MSG: Not an OLE2/CFB compound file".into());
        }

        let (sha256, md5) = EmailMessage::compute_hashes(raw_bytes);
        let streams = Self::scan_cfb_streams(raw_bytes);

        let subject = streams
            .get("__substg1.0_0037")
            .cloned()
            .unwrap_or_else(|| "(No Subject)".to_string());

        let body_text = streams.get("__substg1.0_1000").cloned().unwrap_or_default();

        let body_html = streams.get("__substg1.0_1013").cloned();

        let sender_email = streams
            .get("__substg1.0_0C1F")
            .cloned()
            .unwrap_or_else(|| "sender@msg.local".to_string());

        let sender_name = streams
            .get("__substg1.0_0C1A")
            .cloned()
            .unwrap_or_else(|| sender_email.clone());

        let from = EmailAddress::new(sender_name, sender_email);
        let to = vec![EmailAddress::new("Recipient", "recipient@msg.local")];

        let message_id = streams
            .get("__substg1.0_1035")
            .cloned()
            .unwrap_or_else(|| format!("<msg_{}@mail2rust.local>", &sha256[..16]));

        let conversation_id = EmailMessage::generate_conversation_id(&subject, &message_id, None);

        // Extract attachments from __attach_version1.0 streams
        let mut attachments = Vec::new();
        for (key, val) in &streams {
            if key.contains("__attach_version1.0") && key.contains("3707") {
                // filename
                let att_name = val.clone();
                let att = Attachment::new(att_name, "application/octet-stream".to_string(), Vec::new(), None);
                attachments.push(att);
            }
        }

        let msg = EmailMessage {
            id: format!("msg_{}", &sha256[..16]),
            message_id,
            conversation_id,
            folder_path,
            subject,
            from,
            to,
            cc: Vec::new(),
            bcc: Vec::new(),
            date: Utc::now(),
            size_bytes: raw_bytes.len(),
            body_text,
            body_html,
            raw_mime: Vec::new(),
            attachments,
            mapi_properties: Vec::new(),
            flags: MessageFlags {
                read: true,
                ..Default::default()
            },
            importance: Importance::Normal,
            sha256,
            md5,
            bates_number: None,
            legal_hold: false,
            source_format: "MSG".to_string(),
        };

        debug!(id = %msg.id, subject = %msg.subject, "Parsed Outlook MSG");
        Ok(msg)
    }

    /// Scans raw CFB sectors for UTF-16LE text properties matching MAPI stream markers
    fn scan_cfb_streams(raw_bytes: &[u8]) -> HashMap<String, String> {
        let mut map = HashMap::new();
        let markers = [
            ("__substg1.0_0037", "0037"), // PR_SUBJECT
            ("__substg1.0_1000", "1000"), // PR_BODY
            ("__substg1.0_1013", "1013"), // PR_HTML
            ("__substg1.0_0C1F", "0C1F"), // PR_SENDER_EMAIL_ADDRESS
            ("__substg1.0_0C1A", "0C1A"), // PR_SENDER_NAME
            ("__substg1.0_1035", "1035"), // PR_INTERNET_MESSAGE_ID
        ];

        for (tag_prefix, hex_tag) in markers {
            if let Some(text) = Self::find_tagged_stream(raw_bytes, hex_tag) {
                map.insert(tag_prefix.to_string(), text);
            }
        }

        map
    }

    fn find_tagged_stream(raw: &[u8], hex_tag: &str) -> Option<String> {
        // Tag search pattern in CFB directory entries (UTF-16LE)
        let pattern: Vec<u8> = hex_tag.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        let mut idx = 0;
        while idx + pattern.len() <= raw.len() {
            if &raw[idx..idx + pattern.len()] == pattern.as_slice() {
                // If tag is found, scan nearby sector for non-zero Unicode text
                let search_start = idx.saturating_add(64);
                let search_end = (search_start + 4096).min(raw.len());
                if search_start < search_end {
                    let slice = &raw[search_start..search_end];
                    let text = Self::extract_utf16le_string(slice);
                    if text.len() > 1 && !text.chars().all(|c| c.is_control() || c == ' ') {
                        return Some(text);
                    }
                }
            }
            idx += 1;
        }
        None
    }

    fn extract_utf16le_string(slice: &[u8]) -> String {
        let mut u16s = Vec::new();
        for chunk in slice.as_chunks::<2>().0 {
            let val = u16::from_le_bytes([chunk[0], chunk[1]]);
            if val == 0 {
                break;
            }
            if (32..=0xD7FF).contains(&val) {
                u16s.push(val);
            }
        }
        String::from_utf16_lossy(&u16s).trim().to_string()
    }
}
