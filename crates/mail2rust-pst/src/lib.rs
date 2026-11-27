#![forbid(unsafe_code)]

pub mod ltp;
pub mod ndb;
pub mod preserve;
pub mod reader;
pub mod validator;
pub mod writer;

pub use ltp::{
    MSGFLAG_FROMME, MSGFLAG_READ, MSGFLAG_UNSENT, PR_ADDRTYPE_W, PR_ATTACH_CONTENT_ID_W, PR_ATTACH_DATA_BIN,
    PR_ATTACH_FILENAME_W, PR_ATTACH_FLAGS, PR_ATTACH_LONG_FILENAME_W, PR_ATTACH_METHOD, PR_ATTACH_MIME_TAG_W,
    PR_ATTACH_NUM, PR_ATTACH_SIZE, PR_BODY_HTML_W, PR_BODY_W, PR_BUSINESS_TELEPHONE_NUMBER_W, PR_CLIENT_SUBMIT_TIME,
    PR_COMPANY_NAME_W, PR_CONTACT_EMAIL1_W, PR_CONTACT_EMAIL2_W, PR_CONTACT_EMAIL3_W, PR_CREATION_TIME,
    PR_DEPARTMENT_NAME_W, PR_DISPLAY_NAME_W, PR_DISTLIST_MEMBERS, PR_DISTLIST_ONEOFF_MEMBERS, PR_EMAIL_ADDRESS_W,
    PR_ENTRYID, PR_HASATTACH, PR_HOME_TELEPHONE_NUMBER_W, PR_IMPORTANCE, PR_INTERNET_MESSAGE_ID_W,
    PR_LAST_MODIFICATION_TIME, PR_MESSAGE_CLASS_W, PR_MESSAGE_DELIVERY_TIME, PR_MESSAGE_FLAGS,
    PR_MOBILE_TELEPHONE_NUMBER_W, PR_NICK_NAME_W, PR_OFFICE_LOCATION_W, PR_PST_PASSWORD, PR_RECIPIENT_TYPE,
    PR_SEARCH_KEY, PR_SEND_RICH_INFO, PR_SENDER_EMAIL_ADDRESS_W, PR_SENDER_NAME_W, PR_SMTP_ADDRESS_W, PR_SUBJECT_W,
    PR_TITLE_W, ParsedPropertyContext, PropertyContext, TableContext, TableContextRow, datetime_to_filetime,
    filetime_to_datetime,
};
pub use ndb::{
    AMAP_START_OFFSET, Bref, DATA_START_OFFSET, DEPERMUTE_TABLE, MPBB, NDB_CRYPT_CYCLIC, NDB_CRYPT_NONE,
    NDB_CRYPT_PERMUTE, NID_ATTACHMENT_TABLE, NID_MESSAGE_STORE, NID_NAME_TO_ID_MAP, NID_RECIPIENT_TABLE,
    NID_ROOT_FOLDER, NID_TYPE_ATTACHMENT, NID_TYPE_ATTACHMENT_TABLE, NID_TYPE_RECIPIENT_TABLE, PERMUTE_TABLE,
    PMAP_START_OFFSET, PST_HEADER_SIZE, PST_PAGE_SIZE, PST_TRAILER_SIG, PST_UNICODE_MAGIC, PST_UNICODE_VER, PTYPE_AMAP,
    PTYPE_BBT, PTYPE_NBT, PTYPE_PMAP, PageTrailer, PstEncryption, calculate_partial_header_crc,
    compute_pst_password_crc, cyclic_crypt, depermute_bytes, permute_bytes, weak_crc32,
};
pub use reader::{NameToIdMap, NbtNode, PstReader, SubnodeEntry};
pub use validator::{PstValidationReport, PstValidator};
pub use writer::PstWriter;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use mail2rust_core::{Attachment, EmailAddress, EmailMessage, Importance, MessageFlags};
    use std::io::Cursor;

    #[test]
    fn test_weak_crc32_vector() {
        assert_eq!(weak_crc32(b""), 0);
        assert_eq!(weak_crc32(b"abc"), 0xca6598d0);
    }

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

    #[test]
    fn test_pst_attachments_recipients_dates_and_xblock_roundtrip() {
        use chrono::TimeZone;
        use mail2rust_core::Attachment;

        let mut buffer = Cursor::new(Vec::new());
        let sent_at = Utc.with_ymd_and_hms(2026, 9, 15, 11, 22, 33).unwrap();
        let big_payload = vec![0x5Au8; 16_500]; // > 8192 bytes triggers XBLOCK

        {
            let mut writer = PstWriter::new(&mut buffer, true).expect("init writer");
            let inbox_nid = writer.add_folder("Inbox", None).expect("inbox");
            let sub_nid = writer.add_folder("Project Titan", Some("Inbox")).expect("sub");

            let msg = EmailMessage {
                id: "att_rt".to_string(),
                message_id: "<att-rt@titan.corp>".to_string(),
                conversation_id: "conv_att".to_string(),
                folder_path: vec!["Inbox".to_string(), "Project Titan".to_string()],
                subject: "Subnode Attachment & Recipient Test".to_string(),
                from: EmailAddress::new("Evelyn Vance", "evelyn@titan.corp"),
                to: vec![EmailAddress::new("Alex Wright", "alex@corp.local")],
                cc: vec![EmailAddress::new("Audit", "audit@titan.corp")],
                bcc: vec![],
                date: sent_at,
                size_bytes: 20_000,
                body_text: "See attached PDF and CSV.".to_string(),
                body_html: None,
                raw_mime: vec![],
                attachments: vec![
                    Attachment::new(
                        "report.pdf".to_string(),
                        "application/pdf".to_string(),
                        big_payload.clone(),
                        None,
                    ),
                    Attachment::new(
                        "logo.png".to_string(),
                        "image/png".to_string(),
                        b"\x89PNG\r\n\x1a\n".to_vec(),
                        Some("cid:logo@titan.corp".to_string()),
                    ),
                ],
                mapi_properties: vec![],
                flags: MessageFlags {
                    read: true,
                    ..Default::default()
                },
                importance: Importance::High,
                sha256: "s".to_string(),
                md5: "m".to_string(),
                bates_number: None,
                legal_hold: false,
                source_format: "Test".to_string(),
            };
            writer.add_message(&msg, sub_nid).expect("add msg");
            let _ = inbox_nid;
            writer.finalize().expect("finalize");
        }

        let report = PstValidator::validate(&mut buffer).expect("validate");
        assert!(report.is_valid, "PST errors: {:?}", report.errors);

        let mut reader = PstReader::open(&mut buffer).expect("open reader");
        let msgs = reader.read_all_messages().expect("read_all_messages");
        assert_eq!(msgs.len(), 1);
        let m = &msgs[0];
        assert_eq!(m.folder_path, vec!["Inbox".to_string(), "Project Titan".to_string()]);
        assert_eq!(m.date.timestamp(), sent_at.timestamp());
        assert_eq!(m.to.len(), 1);
        assert_eq!(m.to[0].email, "alex@corp.local");
        assert_eq!(m.cc.len(), 1);
        assert_eq!(m.cc[0].email, "audit@titan.corp");
        assert_eq!(m.attachments.len(), 2);
        let pdf = m.attachments.iter().find(|a| a.filename == "report.pdf").unwrap();
        assert_eq!(pdf.mime_type, "application/pdf");
        assert_eq!(pdf.data, big_payload);
        let png = m.attachments.iter().find(|a| a.filename == "logo.png").unwrap();
        assert_eq!(png.content_id.as_deref(), Some("cid:logo@titan.corp"));
    }

    #[test]
    fn test_pst_roundtrip_preserves_large_attachment() {
        let payload: Vec<u8> = (0..10_000).map(|i| (i % 251) as u8).collect();
        let msg = EmailMessage {
            id: "large".to_string(),
            message_id: "<large@example.com>".to_string(),
            conversation_id: "large".to_string(),
            folder_path: vec!["Inbox".to_string()],
            subject: "Large attachment".to_string(),
            from: EmailAddress::new("Alice", "alice@example.com"),
            to: vec![EmailAddress::new("Bob", "bob@example.com")],
            cc: Vec::new(),
            bcc: Vec::new(),
            date: Utc::now(),
            size_bytes: payload.len(),
            body_text: "body".to_string(),
            body_html: None,
            raw_mime: Vec::new(),
            attachments: vec![Attachment::new(
                "big.bin".to_string(),
                "application/octet-stream".to_string(),
                payload.clone(),
                None,
            )],
            mapi_properties: Vec::new(),
            flags: MessageFlags::default(),
            importance: Importance::Normal,
            sha256: String::new(),
            md5: String::new(),
            bates_number: None,
            legal_hold: false,
            source_format: "EML".to_string(),
        };

        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = PstWriter::new(&mut buffer, true).unwrap();
            let inbox = writer.add_folder("Inbox", None).unwrap();
            writer.add_message(&msg, inbox).unwrap();
            writer.finalize().unwrap();
        }
        let mut reader = PstReader::open(&mut buffer).unwrap();
        let messages = reader.read_all_messages().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].attachments[0].data, payload);
    }

    #[test]
    fn test_pst_roundtrip_preserves_recipients_date_and_attachments() {
        let original_date = Utc.timestamp_opt(1_577_836_496, 0).single().unwrap();
        let att = Attachment::new(
            "payload.bin".to_string(),
            "application/octet-stream".to_string(),
            b"ABCDEFGH".to_vec(),
            None,
        );
        let msg = EmailMessage {
            id: "rt_full".to_string(),
            message_id: "<rt-full@example.com>".to_string(),
            conversation_id: "conv_full".to_string(),
            folder_path: vec!["Inbox".to_string()],
            subject: "Attachment and date test".to_string(),
            from: EmailAddress::new("Alice", "alice@example.com"),
            to: vec![
                EmailAddress::new("Bob", "bob@example.com"),
                EmailAddress::new("Carol", "carol@example.com"),
            ],
            cc: vec![EmailAddress::new("Dave", "dave@example.com")],
            bcc: vec![EmailAddress::new("Eve", "eve@example.com")],
            date: original_date,
            size_bytes: 8,
            body_text: "Hello body.".to_string(),
            body_html: None,
            raw_mime: b"From: alice@example.com\r\n\r\nHello body.".to_vec(),
            attachments: vec![att],
            mapi_properties: Vec::new(),
            flags: MessageFlags::default(),
            importance: Importance::Normal,
            sha256: "sha".to_string(),
            md5: "md5".to_string(),
            bates_number: Some("B-0001".to_string()),
            legal_hold: true,
            source_format: "EML".to_string(),
        };

        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = PstWriter::new(&mut buffer, true).unwrap();
            let inbox = writer.add_folder("Inbox", None).unwrap();
            writer.add_message(&msg, inbox).unwrap();
            writer.finalize().unwrap();
        }

        let mut reader = PstReader::open(&mut buffer).unwrap();
        let messages = reader.read_all_messages().unwrap();
        assert_eq!(messages.len(), 1);
        let out = &messages[0];
        assert_eq!(out.subject, msg.subject);
        assert_eq!(out.to, msg.to);
        assert_eq!(out.cc, msg.cc);
        assert_eq!(out.bcc, msg.bcc);
        assert_eq!(out.date, original_date);
        assert_eq!(out.raw_mime, msg.raw_mime);
        assert_eq!(out.attachments.len(), 1);
        assert_eq!(out.attachments[0].filename, "payload.bin");
        assert_eq!(out.attachments[0].data, b"ABCDEFGH");
        assert_eq!(out.bates_number, Some("B-0001".to_string()));
        assert!(out.legal_hold);
    }
}
