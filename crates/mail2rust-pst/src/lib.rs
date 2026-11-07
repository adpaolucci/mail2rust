pub mod ltp;
pub mod ndb;
pub mod reader;
pub mod validator;
pub mod writer;

pub use ltp::{
    ParsedPropertyContext, PropertyContext, PR_BODY_HTML_W, PR_BODY_W, PR_DISPLAY_NAME_W, PR_IMPORTANCE,
    PR_INTERNET_MESSAGE_ID_W, PR_MESSAGE_CLASS_W, PR_MESSAGE_FLAGS, PR_SENDER_EMAIL_ADDRESS_W, PR_SENDER_NAME_W,
    PR_SUBJECT_W,
};
pub use ndb::{
    calculate_partial_header_crc, depermute_bytes, permute_bytes, Bref, DEPERMUTE_TABLE, PERMUTE_TABLE,
    PST_HEADER_SIZE, PST_UNICODE_MAGIC, PST_UNICODE_VER,
};
pub use reader::PstReader;
pub use validator::{PstValidationReport, PstValidator};
pub use writer::PstWriter;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use mail2rust_core::{EmailAddress, EmailMessage, Importance, MessageFlags};
    use std::io::Cursor;

    #[test]
    fn test_permute_depermute_roundtrip() {
        let original: Vec<u8> = (0..=255).collect();
        let mut data = original.clone();

        permute_bytes(&mut data);
        assert_ne!(data, original, "Permuted data must be transformed");

        depermute_bytes(&mut data);
        assert_eq!(data, original, "Depermuted data must match original 100%");
    }

    #[test]
    fn test_pst_generation_and_validation() {
        let mut buffer = Cursor::new(Vec::new());

        // 1. Build PST
        {
            let mut writer = PstWriter::new(&mut buffer, true).expect("Failed to initialize PstWriter");
            let inbox_nid = writer.add_folder("Inbox", None).expect("Failed to add folder");

            let sample_msg = EmailMessage {
                id: "msg_test_01".to_string(),
                message_id: "<test01@example.com>".to_string(),
                conversation_id: "conv_01".to_string(),
                folder_path: vec!["Inbox".to_string()],
                subject: "Test Direct PST Builder".to_string(),
                from: EmailAddress::new("Alice Smith", "alice@example.com"),
                to: vec![EmailAddress::new("Bob Jones", "bob@example.com")],
                cc: Vec::new(),
                bcc: Vec::new(),
                date: Utc::now(),
                size_bytes: 1024,
                body_text: "Direct Rust PST generation without Outlook works seamlessly!".to_string(),
                body_html: Some("<p>Direct Rust PST generation works!</p>".to_string()),
                raw_mime: Vec::new(),
                attachments: Vec::new(),
                mapi_properties: Vec::new(),
                flags: MessageFlags {
                    read: true,
                    ..Default::default()
                },
                importance: Importance::High,
                sha256: "test_sha256".to_string(),
                md5: "test_md5".to_string(),
                bates_number: Some("LEGAL-0001".to_string()),
                legal_hold: false,
                source_format: "EML".to_string(),
            };

            writer
                .add_message(&sample_msg, inbox_nid)
                .expect("Failed to add message");
            writer.finalize().expect("Failed to finalize PST");
        }

        // 2. Validate Generated PST
        let report = PstValidator::validate(&mut buffer).expect("Validation execution failed");
        assert!(report.is_valid, "PST must be valid: {:?}", report.errors);
        assert!(report.crc_match, "CRC32 partial must match declared header CRC");
        assert_eq!(report.version, 23);
        assert_eq!(report.encryption_method, "Compressible (NDB_CRYPT_PERMUTE)");
        assert!(report.nbt_root_bid > 0);
        assert!(report.bbt_root_bid > 0);
    }

    #[test]
    fn test_pst_writer_and_reader_roundtrip() {
        let mut buffer = Cursor::new(Vec::new());

        // 1. Write PST
        {
            let mut writer = PstWriter::new(&mut buffer, true).expect("Failed to init PstWriter");
            let inbox_nid = writer.add_folder("Inbox", None).expect("Failed to add folder");

            let msg = EmailMessage {
                id: "msg_roundtrip".to_string(),
                message_id: "<roundtrip@enterprise.local>".to_string(),
                conversation_id: "conv_rt".to_string(),
                folder_path: vec!["Inbox".to_string()],
                subject: "Enterprise Migration Bidirectional Roundtrip".to_string(),
                from: EmailAddress::new("Admin Officer", "admin@enterprise.local"),
                to: vec![EmailAddress::new("Recipient", "recip@enterprise.local")],
                cc: vec![],
                bcc: vec![],
                date: Utc::now(),
                size_bytes: 512,
                body_text: "Pure-Rust bidirectional reading and writing of PST containers without Outlook COM."
                    .to_string(),
                body_html: Some("<p>Bidirectional PST reader/writer verified.</p>".to_string()),
                raw_mime: vec![],
                attachments: vec![],
                mapi_properties: vec![],
                flags: MessageFlags {
                    read: true,
                    ..Default::default()
                },
                importance: Importance::High,
                sha256: "rt_sha".to_string(),
                md5: "rt_md5".to_string(),
                bates_number: None,
                legal_hold: false,
                source_format: "Roundtrip".to_string(),
            };

            writer.add_message(&msg, inbox_nid).expect("Failed to write message");
            writer.finalize().expect("Failed to finalize PST");
        }

        // 2. Read PST back
        let mut reader = PstReader::open(&mut buffer).expect("Failed to open PST reader");
        let messages = reader.read_all_messages().expect("Failed to read messages");

        assert_eq!(messages.len(), 1, "Must read back 1 message");
        assert_eq!(messages[0].subject, "Enterprise Migration Bidirectional Roundtrip");
        assert_eq!(messages[0].from.name, "Admin Officer");
        assert_eq!(messages[0].from.email, "admin@enterprise.local");
        assert!(messages[0].body_text.contains("Pure-Rust bidirectional reading"));
        assert!(messages[0].flags.read);
        assert_eq!(messages[0].importance, Importance::High);
    }
}
