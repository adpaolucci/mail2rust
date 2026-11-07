use crate::models::{EmailMessage, MessageFlags};
use crate::readers::eml::EmlReader;
use std::io::Read;
use tracing::info;

pub struct BabylReader;

impl BabylReader {
    pub const BABYL_HEADER: &'static str = "BABYL OPTIONS:";

    pub fn parse<R: Read>(
        mut reader: R,
        folder_path: Vec<String>,
    ) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>> {
        let mut text = String::new();
        reader.read_to_string(&mut text)?;

        let mut messages = Vec::new();
        let parts: Vec<&str> = text.split("\u{1f}").collect(); // Split by 0x1F unit separator

        info!(parts = parts.len(), "Parsing Emacs Babyl mailbox");

        for part in parts {
            let trimmed = part.trim();
            if trimmed.is_empty() || trimmed.starts_with(Self::BABYL_HEADER) {
                continue;
            }

            // Babyl messages have attribute line followed by ***, then headers, then body
            let mut read_flag = true;
            let mut replied = false;
            let mut flagged = false;

            let mut msg_text = trimmed;
            if let Some(star_idx) = trimmed.find("*** EOOH ***") {
                let attr_header = &trimmed[..star_idx];
                if attr_header.contains("unseen") {
                    read_flag = false;
                }
                if attr_header.contains("answered") {
                    replied = true;
                }
                if attr_header.contains("flagged") {
                    flagged = true;
                }
                msg_text = &trimmed[star_idx + 12..].trim();
            }

            if let Ok(mut msg) = EmlReader::parse(msg_text.as_bytes(), folder_path.clone()) {
                msg.source_format = "Babyl".to_string();
                msg.flags = MessageFlags {
                    read: read_flag,
                    replied,
                    flagged,
                    ..Default::default()
                };
                messages.push(msg);
            }
        }

        info!(count = messages.len(), "Finished Babyl parse");
        Ok(messages)
    }
}
