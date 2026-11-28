use crate::models::{EmailAddress, EmailMessage, Importance, MessageFlags};
use chrono::{DateTime, Utc};
use std::io::{self, Read};
use tracing::info;

/// Maximum decompressed bytes permitted for a single `.olm` ZIP entry (64 MiB).
pub const MAX_OLM_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
/// Maximum cumulative decompressed bytes permitted across an entire `.olm` archive (1 GiB).
pub const MAX_OLM_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;

pub struct OlmReader;

impl OlmReader {
    pub fn parse<R: std::io::Read + std::io::Seek>(
        reader: R,
        base_folder: Vec<String>,
    ) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>> {
        Self::parse_with_limits(reader, base_folder, MAX_OLM_ENTRY_BYTES, MAX_OLM_TOTAL_BYTES)
    }

    pub fn parse_with_limits<R: std::io::Read + std::io::Seek>(
        reader: R,
        base_folder: Vec<String>,
        max_entry_bytes: u64,
        max_total_bytes: u64,
    ) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>> {
        let mut zip = zip::ZipArchive::new(reader)?;
        let mut messages = Vec::new();
        let mut total_bytes_read: u64 = 0;

        info!("Traversing .OLM zip archive entries");

        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            let name = file.name()?.into_owned();

            // OLM messages reside under com.microsoft.__Messages or end with .xml
            if (name.contains("com.microsoft.__Messages") || name.contains("message_")) && name.ends_with(".xml") {
                let remaining = max_total_bytes.saturating_sub(total_bytes_read);
                let entry_cap = max_entry_bytes.min(remaining);
                let mut content = Vec::new();
                file.by_ref()
                    .take(entry_cap.saturating_add(1))
                    .read_to_end(&mut content)?;

                if content.len() as u64 > max_entry_bytes {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "OLM archive entry '{}' exceeds maximum decompressed entry size ({} bytes)",
                            name, max_entry_bytes
                        ),
                    )
                    .into());
                }

                total_bytes_read = total_bytes_read.saturating_add(content.len() as u64);
                if total_bytes_read > max_total_bytes {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "OLM archive exceeds maximum cumulative decompressed size ({} bytes)",
                            max_total_bytes
                        ),
                    )
                    .into());
                }

                let xml_str = String::from_utf8_lossy(&content);

                if let Some(msg) = Self::parse_olm_xml(&xml_str, &name, base_folder.clone()) {
                    messages.push(msg);
                }
            }
        }

        info!(count = messages.len(), "Finished parsing OLM archive");
        Ok(messages)
    }

    fn parse_olm_xml(xml: &str, _file_name: &str, mut folder_path: Vec<String>) -> Option<EmailMessage> {
        let subject = Self::extract_tag(xml, "OP:messageSubject").unwrap_or_else(|| "(No Subject)".to_string());

        let sender_email =
            Self::extract_tag(xml, "OP:messageSenderAddress").unwrap_or_else(|| "sender@olm.local".to_string());
        let sender_name = Self::extract_tag(xml, "OP:messageSenderName").unwrap_or_else(|| sender_email.clone());

        let body_html = Self::extract_tag(xml, "OP:messageHtmlBody");
        let body_text =
            Self::extract_tag(xml, "OP:messageBody").unwrap_or_else(|| body_html.clone().unwrap_or_default());

        let date_str = Self::extract_tag(xml, "OP:messageSentTime");
        let date = date_str
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let is_read = Self::extract_tag(xml, "OP:messageIsRead")
            .map(|s| s == "1" || s.to_lowercase() == "true")
            .unwrap_or(true);

        folder_path.push("OLM_Import".to_string());
        let (sha256, md5) = EmailMessage::compute_hashes(xml.as_bytes());
        let message_id = format!("<olm_{}@mail2rust.local>", &sha256[..16]);
        let conversation_id = EmailMessage::generate_conversation_id(&subject, &message_id, None);

        Some(EmailMessage {
            id: format!("olm_{}", &sha256[..16]),
            message_id,
            conversation_id,
            folder_path,
            subject,
            from: EmailAddress::new(sender_name, sender_email),
            to: vec![EmailAddress::new("Recipient", "user@olm.local")],
            cc: Vec::new(),
            bcc: Vec::new(),
            date,
            size_bytes: xml.len(),
            body_text,
            body_html,
            raw_mime: xml.as_bytes().to_vec(),
            attachments: Vec::new(),
            mapi_properties: Vec::new(),
            flags: MessageFlags {
                read: is_read,
                ..Default::default()
            },
            importance: Importance::Normal,
            sha256,
            md5,
            bates_number: None,
            legal_hold: false,
            source_format: "OLM".to_string(),
        })
    }

    fn extract_tag(xml: &str, tag: &str) -> Option<String> {
        let open_tag = format!("<{}>", tag);
        let close_tag = format!("</{}>", tag);
        if let Some(start) = xml.find(&open_tag) {
            let val_start = start + open_tag.len();
            if let Some(end) = xml[val_start..].find(&close_tag) {
                return Some(xml[val_start..val_start + end].trim().to_string());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    fn build_olm_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut cursor);
            let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            for (name, data) in entries {
                writer.start_file(*name, options).expect("start_file");
                writer.write_all(data).expect("write_all");
            }
            writer.finish().expect("finish");
        }
        cursor.into_inner()
    }

    #[test]
    fn test_olm_reader_parses_valid_entry() {
        let xml = b"<email><OP:messageSubject>Hello OLM</OP:messageSubject><OP:messageSenderAddress>a@example.com</OP:messageSenderAddress><OP:messageBody>Body text</OP:messageBody></email>";
        let archive = build_olm_zip(&[("com.microsoft.__Messages/Inbox/message_0001.xml", xml)]);
        let msgs = OlmReader::parse(Cursor::new(archive), vec!["Root".to_string()]).expect("valid OLM");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].subject, "Hello OLM");
    }

    #[test]
    fn test_olm_reader_rejects_oversized_entry_zip_bomb() {
        // Compress > MAX_OLM_ENTRY_BYTES of zeros into a tiny deflated ZIP entry.
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut cursor);
            let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            writer
                .start_file("com.microsoft.__Messages/Inbox/message_bomb.xml", options)
                .expect("start_file");
            let chunk = vec![b'A'; 64 * 1024];
            let target = (MAX_OLM_ENTRY_BYTES as usize) + 64 * 1024;
            let mut written = 0;
            while written < target {
                writer.write_all(&chunk).expect("write_all");
                written += chunk.len();
            }
            writer.finish().expect("finish");
        }
        let archive = cursor.into_inner();
        let err = OlmReader::parse(Cursor::new(archive), vec!["Root".to_string()])
            .expect_err("oversized OLM entry must be rejected");
        assert!(
            err.to_string().contains("exceeds maximum decompressed entry size"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn test_olm_reader_rejects_cumulative_zip_bomb() {
        let payload = vec![b'X'; 4096];
        let archive = build_olm_zip(&[
            ("com.microsoft.__Messages/Inbox/message_0001.xml", &payload),
            ("com.microsoft.__Messages/Inbox/message_0002.xml", &payload),
            ("com.microsoft.__Messages/Inbox/message_0003.xml", &payload),
        ]);
        let err = OlmReader::parse_with_limits(Cursor::new(archive), vec!["Root".to_string()], 8192, 10_000)
            .expect_err("cumulative oversized OLM archive must be rejected");
        assert!(
            err.to_string().contains("exceeds maximum cumulative decompressed size"),
            "unexpected error: {}",
            err
        );
    }
}
