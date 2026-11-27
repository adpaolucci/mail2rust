//! Lossless field preservation for mail2rust's own PST containers.
//!
//! `EmailMessage` deliberately skips `raw_mime` and attachment payload bytes when
//! serialised (they are derived data in most formats), and `Attachment::data` is
//! `skip_serializing`. The PST container therefore stores an explicit preservation
//! blob so an EML -> PST -> EML round trip does not silently drop recipients,
//! dates, raw MIME or attachments (audit finding 4). The fields are also written
//! as standard MAPI properties where a conforming reader can use them; the blob is
//! the authoritative lossless copy for mail2rust-to-mail2rust round trips.

use chrono::{DateTime, Utc};
use mail2rust_core::{Attachment, EmailAddress, EmailMessage, Importance, MapiProperty, MessageFlags};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreservedAttachment {
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: usize,
    pub content_id: Option<String>,
    pub sha256: String,
    pub data: Vec<u8>,
}

impl From<&Attachment> for PreservedAttachment {
    fn from(att: &Attachment) -> Self {
        Self {
            id: att.id.clone(),
            filename: att.filename.clone(),
            mime_type: att.mime_type.clone(),
            size_bytes: att.size_bytes,
            content_id: att.content_id.clone(),
            sha256: att.sha256.clone(),
            data: att.data.clone(),
        }
    }
}

impl From<PreservedAttachment> for Attachment {
    fn from(p: PreservedAttachment) -> Self {
        let mut att = Attachment::new(p.filename, p.mime_type, p.data, p.content_id);
        // Retain the original identifiers/hash instead of recomputing them.
        att.id = p.id;
        att.size_bytes = p.size_bytes;
        att.sha256 = p.sha256;
        att
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreservedMessage {
    pub id: String,
    pub message_id: String,
    pub conversation_id: String,
    pub subject: String,
    pub from: EmailAddress,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub bcc: Vec<EmailAddress>,
    pub date: DateTime<Utc>,
    pub size_bytes: usize,
    pub body_text: String,
    pub body_html: Option<String>,
    pub raw_mime: Vec<u8>,
    pub attachments: Vec<PreservedAttachment>,
    pub mapi_properties: Vec<MapiProperty>,
    pub flags: MessageFlags,
    pub importance: Importance,
    pub sha256: String,
    pub md5: String,
    pub bates_number: Option<String>,
    pub legal_hold: bool,
}

impl PreservedMessage {
    pub fn from_message(msg: &EmailMessage) -> Self {
        Self {
            id: msg.id.clone(),
            message_id: msg.message_id.clone(),
            conversation_id: msg.conversation_id.clone(),
            subject: msg.subject.clone(),
            from: msg.from.clone(),
            to: msg.to.clone(),
            cc: msg.cc.clone(),
            bcc: msg.bcc.clone(),
            date: msg.date,
            size_bytes: msg.size_bytes,
            body_text: msg.body_text.clone(),
            body_html: msg.body_html.clone(),
            raw_mime: msg.raw_mime.clone(),
            attachments: msg.attachments.iter().map(PreservedAttachment::from).collect(),
            mapi_properties: msg.mapi_properties.clone(),
            flags: msg.flags.clone(),
            importance: msg.importance,
            sha256: msg.sha256.clone(),
            md5: msg.md5.clone(),
            bates_number: msg.bates_number.clone(),
            legal_hold: msg.legal_hold,
        }
    }

    pub fn into_message(self, folder_path: Vec<String>, source_format: String) -> EmailMessage {
        EmailMessage {
            id: self.id,
            message_id: self.message_id,
            conversation_id: self.conversation_id,
            folder_path,
            subject: self.subject,
            from: self.from,
            to: self.to,
            cc: self.cc,
            bcc: self.bcc,
            date: self.date,
            size_bytes: self.size_bytes,
            body_text: self.body_text,
            body_html: self.body_html,
            raw_mime: self.raw_mime,
            attachments: self.attachments.into_iter().map(Attachment::from).collect(),
            mapi_properties: self.mapi_properties,
            flags: self.flags,
            importance: self.importance,
            sha256: self.sha256,
            md5: self.md5,
            bates_number: self.bates_number,
            legal_hold: self.legal_hold,
            source_format,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserved_blob_round_trips_every_field() {
        let att = Attachment::new(
            "payload.bin".to_string(),
            "application/octet-stream".to_string(),
            vec![1, 2, 3],
            None,
        );
        let msg = EmailMessage {
            id: "id".to_string(),
            message_id: "<m@e>".to_string(),
            conversation_id: "c".to_string(),
            folder_path: vec!["Inbox".to_string()],
            subject: "s".to_string(),
            from: EmailAddress::new("A", "a@e"),
            to: vec![EmailAddress::new("B", "b@e")],
            cc: vec![EmailAddress::new("C", "c@e")],
            bcc: Vec::new(),
            date: Utc::now(),
            size_bytes: 3,
            body_text: "body".to_string(),
            body_html: None,
            raw_mime: b"raw".to_vec(),
            attachments: vec![att],
            mapi_properties: Vec::new(),
            flags: MessageFlags::default(),
            importance: Importance::Normal,
            sha256: "sha".to_string(),
            md5: "md5".to_string(),
            bates_number: None,
            legal_hold: false,
            source_format: "EML".to_string(),
        };
        let blob = serde_json::to_vec(&PreservedMessage::from_message(&msg)).unwrap();
        let restored = serde_json::from_slice::<PreservedMessage>(&blob)
            .unwrap()
            .into_message(vec!["Inbox".to_string()], "PST".to_string());
        assert_eq!(restored.to, msg.to);
        assert_eq!(restored.cc, msg.cc);
        assert_eq!(restored.date, msg.date);
        assert_eq!(restored.raw_mime, msg.raw_mime);
        assert_eq!(restored.attachments[0].data, vec![1, 2, 3]);
        assert_eq!(restored.attachments[0].filename, "payload.bin");
    }
}
