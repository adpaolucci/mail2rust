use crate::models::EmailMessage;
use crate::readers::eml::EmlReader;
use std::io::Read;
use tracing::info;

pub struct MmdfReader;

impl MmdfReader {
    pub const DELIMITER: [u8; 4] = [0x01, 0x01, 0x01, 0x01];

    pub fn parse<R: Read>(
        mut reader: R,
        folder_path: Vec<String>,
    ) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;

        let mut messages = Vec::new();
        let mut idx = 0;
        let mut last_boundary = 0;

        info!(total_bytes = bytes.len(), "Parsing MMDF mailbox format");

        while idx + 4 <= bytes.len() {
            if &bytes[idx..idx + 4] == Self::DELIMITER {
                if idx > last_boundary {
                    let msg_bytes = &bytes[last_boundary..idx];
                    let trimmed = msg_bytes.trim_ascii();
                    if !trimmed.is_empty() {
                        if let Ok(mut msg) = EmlReader::parse(trimmed, folder_path.clone()) {
                            msg.source_format = "MMDF".to_string();
                            messages.push(msg);
                        }
                    }
                }
                idx += 4;
                last_boundary = idx;
                continue;
            }
            idx += 1;
        }

        info!(count = messages.len(), "Finished MMDF parse");
        Ok(messages)
    }
}
