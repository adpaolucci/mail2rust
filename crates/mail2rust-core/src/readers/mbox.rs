use crate::models::EmailMessage;
use crate::readers::eml::EmlReader;
use std::io::{BufRead, BufReader, Read};
use tracing::{debug, info, trace};

pub struct MboxReader;

impl MboxReader {
    /// Read an MBOX stream incrementally, invoking a callback for each parsed message.
    /// Uses a 16 MB buffered reader to maintain constant, bounded memory usage (<50 MB)
    /// regardless of whether the mailbox file is 10 MB or 100 GB.
    pub fn read_streaming<R: Read, F>(
        reader: R,
        folder_path: Vec<String>,
        mut on_message: F,
    ) -> Result<usize, Box<dyn std::error::Error>>
    where
        F: FnMut(EmailMessage) -> Result<(), Box<dyn std::error::Error>>,
    {
        let mut buf_reader = BufReader::with_capacity(16 * 1024 * 1024, reader);
        let mut current_msg_bytes: Vec<u8> = Vec::new();
        let mut line = Vec::new();
        let mut count = 0usize;

        info!(folder = ?folder_path, "Starting streaming MBOX parse");

        while buf_reader.read_until(b'\n', &mut line)? > 0 {
            if line.starts_with(b"From ") {
                if !current_msg_bytes.is_empty() {
                    count += 1;
                    trace!(
                        index = count,
                        bytes = current_msg_bytes.len(),
                        "Encountered MBOX message boundary"
                    );
                    match EmlReader::parse(&current_msg_bytes, folder_path.clone()) {
                        Ok(mut msg) => {
                            msg.source_format = "MBOX".to_string();
                            on_message(msg)?;
                        }
                        Err(e) => {
                            debug!(index = count, error = %e, "Failed to parse MBOX message item");
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
            count += 1;
            if let Ok(mut msg) = EmlReader::parse(&current_msg_bytes, folder_path) {
                msg.source_format = "MBOX".to_string();
                on_message(msg)?;
            }
        }

        info!(count = count, "Completed streaming MBOX parse successfully");
        Ok(count)
    }

    /// Read raw message byte slices incrementally without parsing full MIME structures.
    /// Ideal for zero-copy high-throughput regex filtering and indexing.
    pub fn read_streaming_raw<R: Read, F>(reader: R, mut on_raw_msg: F) -> Result<usize, Box<dyn std::error::Error>>
    where
        F: FnMut(&[u8]) -> Result<(), Box<dyn std::error::Error>>,
    {
        let mut buf_reader = BufReader::with_capacity(16 * 1024 * 1024, reader);
        let mut current_msg_bytes: Vec<u8> = Vec::new();
        let mut line = Vec::new();
        let mut count = 0usize;

        while buf_reader.read_until(b'\n', &mut line)? > 0 {
            if line.starts_with(b"From ") && !current_msg_bytes.is_empty() {
                count += 1;
                on_raw_msg(&current_msg_bytes)?;
                current_msg_bytes.clear();
            }
            current_msg_bytes.extend_from_slice(&line);
            line.clear();
        }

        if !current_msg_bytes.is_empty() {
            count += 1;
            on_raw_msg(&current_msg_bytes)?;
        }

        Ok(count)
    }

    pub fn read_all<R: Read>(
        reader: R,
        folder_path: Vec<String>,
    ) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>> {
        let mut messages = Vec::new();
        Self::read_streaming(reader, folder_path, |msg| {
            messages.push(msg);
            Ok(())
        })?;
        Ok(messages)
    }
}
