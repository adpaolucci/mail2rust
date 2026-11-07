use crate::models::EmailMessage;
use crate::readers::eml::EmlReader;
use crate::readers::emlx::EmlxReader;
use crate::readers::mbox::MboxReader;
use crate::readers::msg::MsgReader;
use crate::readers::olm::OlmReader;
use flate2::read::GzDecoder;
use std::io::{Cursor, Read, Seek};
use tracing::{debug, info};
use zip::ZipArchive;

pub struct ArchiveReader;

impl ArchiveReader {
    /// Read emails directly from a ZIP archive (e.g. Google Takeout .zip, or compressed mailbox trees)
    pub fn read_zip<R: Read + Seek>(reader: R) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>> {
        let mut archive = ZipArchive::new(reader)?;
        let mut messages = Vec::new();
        info!(entries = archive.len(), "Reading emails from ZIP archive");

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            if file.is_dir() {
                continue;
            }

            let entry_name = file.name().to_string();
            let lower_name = entry_name.to_lowercase();

            // Extract folder path from ZIP path
            let folder_parts: Vec<String> = entry_name
                .split('/')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();
            let folder_hierarchy = if folder_parts.len() > 1 {
                folder_parts[..folder_parts.len() - 1].to_vec()
            } else {
                vec!["Archive".to_string()]
            };

            if lower_name.ends_with(".mbox") {
                debug!(entry = %entry_name, "Found MBOX inside ZIP");
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                let cursor = Cursor::new(data);
                let mut mbox_msgs = MboxReader::read_all(cursor, folder_hierarchy)?;
                messages.append(&mut mbox_msgs);
            } else if lower_name.ends_with(".eml") {
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                if let Ok(msg) = EmlReader::parse(&data, folder_hierarchy) {
                    messages.push(msg);
                }
            } else if lower_name.ends_with(".emlx") {
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                if let Ok(msg) = EmlxReader::parse(&data, folder_hierarchy) {
                    messages.push(msg);
                }
            } else if lower_name.ends_with(".msg") {
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                if let Ok(msg) = MsgReader::parse(&data, folder_hierarchy) {
                    messages.push(msg);
                }
            } else if lower_name.ends_with(".olm") {
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                let cursor = Cursor::new(data);
                if let Ok(mut olm_msgs) = OlmReader::parse(cursor, folder_hierarchy) {
                    messages.append(&mut olm_msgs);
                }
            }
        }

        info!(count = messages.len(), "Completed ZIP archive extraction");
        Ok(messages)
    }

    /// Read emails directly from a GZ-compressed stream (e.g. .mbox.gz or .eml.gz)
    pub fn read_gz<R: Read>(reader: R, original_name: &str) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>> {
        let mut decoder = GzDecoder::new(reader);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;

        let folder = vec![original_name.trim_end_matches(".gz").trim_end_matches(".mbox").to_string()];
        if original_name.to_lowercase().ends_with(".mbox.gz")
            || decompressed.starts_with(b"From ")
            || original_name.to_lowercase().ends_with(".mbox")
        {
            let cursor = Cursor::new(decompressed);
            MboxReader::read_all(cursor, folder)
        } else {
            let msg = EmlReader::parse(&decompressed, folder)?;
            Ok(vec![msg])
        }
    }
}
