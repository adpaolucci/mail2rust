use crate::models::EmailMessage;
use crate::readers::eml::EmlReader;
use std::io::{BufRead, BufReader, Read};
use tracing::{debug, info, trace};

pub struct MboxReader;

impl MboxReader {
    pub fn read_all<R: Read>(
        reader: R,
        folder_path: Vec<String>,
    ) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>> {
        let mut buf_reader = BufReader::new(reader);
        let mut messages = Vec::new();
        let mut current_msg_bytes: Vec<u8> = Vec::new();
        let mut line = Vec::new();
        let mut msg_index = 0;

        info!(folder = ?folder_path, "Starting streaming MBOX parse");

        while buf_reader.read_until(b'\n', &mut line)? > 0 {
            if line.starts_with(b"From ") {
                if !current_msg_bytes.is_empty() {
                    msg_index += 1;
                    trace!(index = msg_index, bytes = current_msg_bytes.len(), "Encountered MBOX message boundary");
                    match EmlReader::parse(&current_msg_bytes, folder_path.clone()) {
                        Ok(mut msg) => {
                            msg.source_format = "MBOX".to_string();
                            messages.push(msg);
                        }
                        Err(e) => {
                            debug!(index = msg_index, error = %e, "Failed to parse MBOX message item");
                        }
                    }
                    current_msg_bytes.clear();
                }
            } else {
                // mboxrd unescaping: if line starts with >From , remove one >
                if line.starts_with(b">From ") || line.starts_with(b">>From ") {
                    current_msg_bytes.extend_from_slice(&line[1..]);
                } else {
                    current_msg_bytes.extend_from_slice(&line);
                }
            }
            line.clear();
        }

        // Final message
        if !current_msg_bytes.is_empty() {
            if let Ok(mut msg) = EmlReader::parse(&current_msg_bytes, folder_path) {
                msg.source_format = "MBOX".to_string();
                messages.push(msg);
            }
        }

        info!(count = messages.len(), "Completed MBOX parse successfully");
        Ok(messages)
    }
}
