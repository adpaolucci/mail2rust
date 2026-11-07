use crate::models::{EmailAddress, EmailMessage, Importance, MessageFlags};
use chrono::{DateTime, Utc};
use std::io::Read;
use tracing::info;

pub struct OlmReader;

impl OlmReader {
    pub fn parse<R: std::io::Read + std::io::Seek>(
        reader: R,
        base_folder: Vec<String>,
    ) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>> {
        let mut zip = zip::ZipArchive::new(reader)?;
        let mut messages = Vec::new();

        info!("Traversing .OLM zip archive archive entries");

        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            let name = file.name().to_string();

            // OLM messages reside under com.microsoft.__Messages or end with .xml
            if (name.contains("com.microsoft.__Messages") || name.contains("message_")) && name.ends_with(".xml") {
                let mut content = Vec::new();
                file.read_to_end(&mut content)?;
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
