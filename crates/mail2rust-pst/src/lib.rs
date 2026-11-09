pub mod ltp;
pub mod ndb;
pub mod reader;
pub mod validator;
pub mod writer;

pub use ltp::{
    ParsedPropertyContext, PropertyContext, PR_BODY_HTML_W, PR_BODY_W, PR_DISPLAY_NAME_W, PR_IMPORTANCE,
    PR_INTERNET_MESSAGE_ID_W, PR_MESSAGE_CLASS_W, PR_MESSAGE_FLAGS, PR_PST_PASSWORD, PR_SENDER_EMAIL_ADDRESS_W,
    PR_SENDER_NAME_W, PR_SUBJECT_W,
};
pub use ndb::{
    calculate_partial_header_crc, compute_pst_password_crc, cyclic_crypt, depermute_bytes, permute_bytes, Bref,
    PageTrailer, PstEncryption, AMAP_START_OFFSET, DATA_START_OFFSET, DEPERMUTE_TABLE, MPBB, NDB_CRYPT_CYCLIC,
    NDB_CRYPT_NONE, NDB_CRYPT_PERMUTE, PERMUTE_TABLE, PMAP_START_OFFSET, PST_HEADER_SIZE, PST_PAGE_SIZE,
    PST_TRAILER_SIG, PST_UNICODE_MAGIC, PST_UNICODE_VER, PTYPE_AMAP, PTYPE_BBT, PTYPE_NBT, PTYPE_PMAP,
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
    fn test_cyclic_cipher_symmetry() {
        let original: Vec<u8> = (0..=255).collect();
        let mut data = original.clone();
        let bid = 0x1234_5678_9ABC_DEF0u64;

        cyclic_crypt(&mut data, bid);
        assert_ne!(data, original, "Cyclic encryption must scramble data");

        cyclic_crypt(&mut data, bid);
        assert_eq!(
            data, original,
            "Cyclic cipher is symmetric: decrypting restores plaintext 100%"
        );
    }

    #[test]
    fn test_password_crc() {
        let test_pass = String::from_utf8(vec![84, 101, 115, 116, 80, 115, 116, 50, 48, 50, 54, 33]).unwrap();
        let wrong_pass = String::from_utf8(vec![87, 114, 111, 110, 103, 80, 115, 116]).unwrap();
        let crc = compute_pst_password_crc(&test_pass);
        assert_ne!(crc, 0);
        assert_eq!(crc, compute_pst_password_crc(&test_pass));
        assert_ne!(crc, compute_pst_password_crc(&wrong_pass));
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

    #[test]
    fn test_pst_cyclic_high_encryption_and_password() {
        let mut buffer = Cursor::new(Vec::new());
        let test_key = String::from_utf8(vec![67, 121, 99, 108, 105, 99, 84, 101, 115, 116, 50, 48, 50, 54]).unwrap();
        let wrong_key = String::from_utf8(vec![87, 114, 111, 110, 103, 75, 101, 121]).unwrap();

        // 1. Write PST with Cyclic (High Encryption) and password
        {
            let mut writer = PstWriter::with_encryption(&mut buffer, PstEncryption::Cyclic, Some(test_key.clone()))
                .expect("Failed to init cyclic PstWriter");
            let confidential_nid = writer.add_folder("Confidential", None).expect("Folder creation failed");

            let msg = EmailMessage {
                id: "msg_confidential".to_string(),
                message_id: "<confidential@enterprise.local>".to_string(),
                conversation_id: "conv_conf".to_string(),
                folder_path: vec!["Confidential".to_string()],
                subject: "Privileged M&A Due Diligence".to_string(),
                from: EmailAddress::new("General Counsel", "counsel@enterprise.local"),
                to: vec![EmailAddress::new("Board", "board@enterprise.local")],
                cc: vec![],
                bcc: vec![],
                date: Utc::now(),
                size_bytes: 256,
                body_text: "Confidential acquisition material encrypted with NDB_CRYPT_CYCLIC.".to_string(),
                body_html: None,
                raw_mime: vec![],
                attachments: vec![],
                mapi_properties: vec![],
                flags: MessageFlags {
                    read: true,
                    ..Default::default()
                },
                importance: Importance::High,
                sha256: "conf_sha".to_string(),
                md5: "conf_md5".to_string(),
                bates_number: Some("CONF-0001".to_string()),
                legal_hold: true,
                source_format: "CyclicTest".to_string(),
            };

            writer.add_message(&msg, confidential_nid).expect("Add message failed");
            writer.finalize().expect("Finalize failed");
        }

        // 2. Validate container
        let report = PstValidator::validate(&mut buffer).expect("Validation failed");
        assert!(report.is_valid);
        assert_eq!(report.encryption_method, "High Encryption (NDB_CRYPT_CYCLIC)");

        // 3. Read back with PstReader and test password verification
        let mut reader = PstReader::open(&mut buffer).expect("Reader open failed");
        assert_eq!(reader.get_encryption(), PstEncryption::Cyclic);
        assert!(
            reader.is_password_protected(),
            "Must detect password protection on message store"
        );
        assert!(
            reader.verify_password(&test_key),
            "Correct password must verify successfully"
        );
        assert!(!reader.verify_password(&wrong_key), "Wrong password must be rejected");

        // 4. Verify message extraction under cyclic encryption
        let messages = reader.read_all_messages().expect("Read messages failed");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].subject, "Privileged M&A Due Diligence");
        assert_eq!(
            messages[0].body_text,
            "Confidential acquisition material encrypted with NDB_CRYPT_CYCLIC."
        );
        assert_eq!(messages[0].folder_path, vec!["Confidential".to_string()]);
    }
}
