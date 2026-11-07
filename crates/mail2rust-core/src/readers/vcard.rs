use crate::models::{EmailAddress, EmailMessage, Importance, MapiProperty, MessageFlags};
use chrono::Utc;
use std::io;
use tracing::debug;

pub struct VcardReader;

impl VcardReader {
    pub fn parse(bytes: &[u8], folder_path: Vec<String>) -> io::Result<Vec<EmailMessage>> {
        let content = String::from_utf8_lossy(bytes);
        let mut messages = Vec::new();
        let mut in_vcard = false;
        let mut current_lines = Vec::new();

        // Handle line unfolding (lines beginning with space or tab continue previous line)
        let mut unfolded: Vec<String> = Vec::new();
        for line in content.lines() {
            if (line.starts_with(' ') || line.starts_with('\t')) && !unfolded.is_empty() {
                let last = unfolded.last_mut().unwrap();
                last.push_str(line.trim_start());
            } else {
                unfolded.push(line.to_string());
            }
        }

        for line in unfolded {
            let trimmed = line.trim();
            if trimmed.eq_ignore_ascii_case("BEGIN:VCARD") {
                in_vcard = true;
                current_lines.clear();
            } else if trimmed.eq_ignore_ascii_case("END:VCARD") {
                if in_vcard {
                    if let Some(msg) = Self::build_contact(&current_lines, &folder_path) {
                        messages.push(msg);
                    }
                    in_vcard = false;
                }
            } else if in_vcard {
                current_lines.push(trimmed.to_string());
            }
        }

        debug!(count = messages.len(), "Parsed vCard contacts");
        Ok(messages)
    }

    fn build_contact(lines: &[String], folder_path: &[String]) -> Option<EmailMessage> {
        let mut fn_name = String::new();
        let mut emails = Vec::new();
        let mut phones = Vec::new();
        let mut org = String::new();
        let mut title = String::new();
        let mut note = String::new();

        for line in lines {
            let (key, val) = match line.find(':') {
                Some(idx) => (line[..idx].to_uppercase(), line[idx + 1..].trim()),
                None => continue,
            };

            let base_key = key.split(';').next().unwrap_or(&key);

            match base_key {
                "FN" => fn_name = val.to_string(),
                "EMAIL" => emails.push(val.to_string()),
                "TEL" => phones.push(val.to_string()),
                "ORG" => org = val.replace(';', " ").trim().to_string(),
                "TITLE" => title = val.to_string(),
                "NOTE" => note = val.to_string(),
                _ => {}
            }
        }

        if fn_name.is_empty() && emails.is_empty() && org.is_empty() {
            return None;
        }

        let display_name = if !fn_name.is_empty() {
            fn_name
        } else if !org.is_empty() {
            org.clone()
        } else {
            emails.first().cloned().unwrap_or_else(|| "Contact".to_string())
        };

        let primary_email = emails
            .first()
            .cloned()
            .unwrap_or_else(|| "contact@addressbook.local".to_string());

        let mut body = String::new();
        body.push_str(&format!("Name: {}\n", display_name));
        if !org.is_empty() {
            body.push_str(&format!("Company: {}\n", org));
        }
        if !title.is_empty() {
            body.push_str(&format!("Title: {}\n", title));
        }
        for email in &emails {
            body.push_str(&format!("Email: {}\n", email));
        }
        for phone in &phones {
            body.push_str(&format!("Phone: {}\n", phone));
        }
        if !note.is_empty() {
            body.push_str(&format!("\nNotes:\n{}\n", note));
        }

        let folder = if folder_path.is_empty() || folder_path == ["Import"] {
            vec!["Contacts".to_string()]
        } else {
            folder_path.to_vec()
        };

        let raw_bytes = body.as_bytes();
        let (sha256, md5) = EmailMessage::compute_hashes(raw_bytes);
        let msg_id = format!("<vcard-{}@mail2rust.local>", &sha256[..12]);

        let mapi_properties = vec![
            MapiProperty {
                tag: 0x001A001F, // PR_MESSAGE_CLASS_W
                name: "PR_MESSAGE_CLASS_W".to_string(),
                prop_type: "PT_UNICODE".to_string(),
                value: "IPM.Contact".to_string(),
            },
            MapiProperty {
                tag: 0x3001001F, // PR_DISPLAY_NAME_W
                name: "PR_DISPLAY_NAME_W".to_string(),
                prop_type: "PT_UNICODE".to_string(),
                value: display_name.clone(),
            },
            MapiProperty {
                tag: 0x3A16001F, // PR_COMPANY_NAME_W
                name: "PR_COMPANY_NAME_W".to_string(),
                prop_type: "PT_UNICODE".to_string(),
                value: org,
            },
            MapiProperty {
                tag: 0x3A17001F, // PR_TITLE_W
                name: "PR_TITLE_W".to_string(),
                prop_type: "PT_UNICODE".to_string(),
                value: title,
            },
        ];

        Some(EmailMessage {
            id: format!("contact_{}", &sha256[..8]),
            message_id: msg_id.clone(),
            conversation_id: format!("contact_{}", &sha256[..8]),
            folder_path: folder,
            subject: display_name.clone(),
            from: EmailAddress::new(display_name, primary_email),
            to: vec![],
            cc: vec![],
            bcc: vec![],
            date: Utc::now(),
            size_bytes: raw_bytes.len(),
            body_text: body,
            body_html: None,
            raw_mime: Vec::new(),
            attachments: Vec::new(),
            mapi_properties,
            flags: MessageFlags::default(),
            importance: Importance::Normal,
            sha256,
            md5,
            bates_number: None,
            legal_hold: false,
            source_format: "vCard (VCF)".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vcard_parsing() {
        let vcard_data = b"BEGIN:VCARD\r\n\
VERSION:3.0\r\n\
FN:Dr. Gregory House\r\n\
ORG:Princeton-Plainsboro Teaching Hospital\r\n\
TITLE:Head of Diagnostic Medicine\r\n\
EMAIL;TYPE=INTERNET:house@ppth.org\r\n\
TEL;TYPE=WORK:+1-609-555-0142\r\n\
NOTE:Specializes in infectious disease and nephrology.\r\n\
END:VCARD\r\n";

        let contacts = VcardReader::parse(vcard_data, vec!["Contacts".to_string()]).unwrap();
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].subject, "Dr. Gregory House");
        assert_eq!(contacts[0].from.email, "house@ppth.org");
        assert!(contacts[0].body_text.contains("Princeton-Plainsboro"));
        assert!(contacts[0].mapi_properties.iter().any(|p| p.value == "IPM.Contact"));
    }
}
