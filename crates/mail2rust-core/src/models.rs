use chrono::{DateTime, Utc};
use md5::Md5;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmailAddress {
    pub name: String,
    pub email: String,
}

impl EmailAddress {
    pub fn new(name: impl Into<String>, email: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            email: email.into(),
        }
    }

    pub fn parse(raw: &str) -> Self {
        let trimmed = raw.trim();
        if let Some(start) = trimmed.find('<') {
            if let Some(pos) = trimmed[start + 1..].find('>') {
                let end = start + 1 + pos;
                let name = trimmed[..start].trim().trim_matches('"').trim();
                let email = trimmed[start + 1..end].trim();
                return Self {
                    name: if name.is_empty() {
                        email.to_string()
                    } else {
                        name.to_string()
                    },
                    email: email.to_string(),
                };
            }
        }
        Self {
            name: trimmed.to_string(),
            email: trimmed.to_string(),
        }
    }
}

impl std::fmt::Display for EmailAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.name.is_empty() || self.name == self.email {
            write!(f, "{}", self.email)
        } else {
            write!(f, "\"{}\" <{}>", self.name, self.email)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MessageFlags {
    pub read: bool,
    pub flagged: bool,
    pub replied: bool,
    pub forwarded: bool,
    pub deleted: bool,
    pub draft: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Importance {
    Low,
    #[default]
    Normal,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attachment {
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: usize,
    pub content_id: Option<String>,
    pub sha256: String,
    #[serde(skip_serializing)]
    pub data: Vec<u8>,
}

impl Attachment {
    pub fn new(filename: String, mime_type: String, data: Vec<u8>, content_id: Option<String>) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let sha256 = format!("{:x}", hasher.finalize());
        let size_bytes = data.len();
        let id = format!("att_{}_{}", size_bytes, &sha256[..8]);

        Self {
            id,
            filename,
            mime_type,
            size_bytes,
            content_id,
            sha256,
            data,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MapiProperty {
    pub tag: u32,
    pub name: String,
    pub prop_type: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailboxFolder {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub folder_type: String,
    pub path: Vec<String>,
}

impl MailboxFolder {
    pub fn new(name: impl Into<String>, path: Vec<String>, folder_type: impl Into<String>) -> Self {
        let name_str = name.into();
        let id = path.join("/").to_lowercase();
        let parent_id = if path.len() > 1 {
            Some(path[..path.len() - 1].join("/").to_lowercase())
        } else {
            None
        };

        Self {
            id,
            parent_id,
            name: name_str,
            folder_type: folder_type.into(),
            path,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailMessage {
    pub id: String,
    pub message_id: String,
    pub conversation_id: String,
    pub folder_path: Vec<String>,
    pub subject: String,
    pub from: EmailAddress,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub bcc: Vec<EmailAddress>,
    pub date: DateTime<Utc>,
    pub size_bytes: usize,
    pub body_text: String,
    pub body_html: Option<String>,
    #[serde(skip)]
    pub raw_mime: Vec<u8>,
    pub attachments: Vec<Attachment>,
    pub mapi_properties: Vec<MapiProperty>,
    pub flags: MessageFlags,
    pub importance: Importance,
    pub sha256: String,
    pub md5: String,
    pub bates_number: Option<String>,
    pub legal_hold: bool,
    pub source_format: String,
}

impl EmailMessage {
    pub fn compute_hashes(raw_data: &[u8]) -> (String, String) {
        let mut sha = Sha256::new();
        sha.update(raw_data);
        let sha256 = format!("{:x}", sha.finalize());

        let mut md = Md5::new();
        md.update(raw_data);
        let md5 = format!("{:x}", md.finalize());

        (sha256, md5)
    }

    pub fn generate_conversation_id(subject: &str, message_id: &str, in_reply_to: Option<&str>) -> String {
        if let Some(parent) = in_reply_to {
            let clean = parent.trim().trim_matches(|c| c == '<' || c == '>');
            if !clean.is_empty() {
                return format!("conv_{}", clean);
            }
        }
        let clean_subj = subject
            .trim()
            .to_lowercase()
            .replace("re:", "")
            .replace("fwd:", "")
            .replace("fw:", "")
            .trim()
            .to_string();

        if !clean_subj.is_empty() {
            let mut hasher = Sha256::new();
            hasher.update(clean_subj.as_bytes());
            format!("thread_{}", &format!("{:x}", hasher.finalize())[..16])
        } else {
            format!("msg_{}", message_id.trim_matches(|c| c == '<' || c == '>'))
        }
    }
}
