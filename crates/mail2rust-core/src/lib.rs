pub mod logging;
pub mod models;
pub mod readers;
pub mod sniffer;

pub use logging::init_logging;
pub use models::{Attachment, EmailAddress, EmailMessage, Importance, MailboxFolder, MapiProperty, MessageFlags};
pub use readers::archive::ArchiveReader;
pub use readers::babyl::BabylReader;
pub use readers::eml::EmlReader;
pub use readers::emlx::EmlxReader;
pub use readers::ical::IcalReader;
pub use readers::maildir::MaildirReader;
pub use readers::mbox::MboxReader;
pub use readers::mh::MhReader;
pub use readers::mmdf::MmdfReader;
pub use readers::msg::MsgReader;
pub use readers::olm::OlmReader;
pub use readers::vcard::VcardReader;
pub use sniffer::{FormatSniffer, MailboxFormat};

use std::fs::File;
use std::path::Path;
use tracing::info;
use walkdir::WalkDir;

/// Universally ingests messages from any file, folder, archive, or mailbox format
pub fn ingest_path(source: &Path) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>> {
    let mut messages = Vec::new();
    let format = FormatSniffer::sniff_path(source);
    info!(source = ?source, format = format.name(), "Ingesting mailbox source");

    match format {
        MailboxFormat::Zip => {
            let file = File::open(source)?;
            messages = ArchiveReader::read_zip(file)?;
        }
        MailboxFormat::Gz => {
            let file = File::open(source)?;
            let fname = source.file_name().and_then(|s| s.to_str()).unwrap_or("archive.gz");
            messages = ArchiveReader::read_gz(file, fname)?;
        }
        MailboxFormat::Mbox => {
            let file = File::open(source)?;
            let folder = vec![source
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Inbox")
                .to_string()];
            messages = MboxReader::read_all(file, folder)?;
        }
        MailboxFormat::Maildir => {
            messages = MaildirReader::read_dir(source)?;
        }
        MailboxFormat::Olm => {
            let file = File::open(source)?;
            messages = OlmReader::parse(file, vec!["OLM".to_string()])?;
        }
        MailboxFormat::Mh => {
            messages = MhReader::read_dir(source)?;
        }
        MailboxFormat::Mmdf => {
            let file = File::open(source)?;
            messages = MmdfReader::parse(file, vec!["MMDF".to_string()])?;
        }
        MailboxFormat::Babyl => {
            let file = File::open(source)?;
            messages = BabylReader::parse(file, vec!["Babyl".to_string()])?;
        }
        MailboxFormat::Vcard => {
            let bytes = std::fs::read(source)?;
            messages = VcardReader::parse(&bytes, vec!["Contacts".to_string()])?;
        }
        MailboxFormat::Icalendar => {
            let bytes = std::fs::read(source)?;
            messages = IcalReader::parse(&bytes, vec!["Calendar".to_string()])?;
        }
        _ => {
            if source.is_file() {
                let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                let bytes = std::fs::read(source)?;
                if ext == "emlx" {
                    messages.push(EmlxReader::parse(&bytes, vec!["Import".to_string()])?);
                } else if ext == "msg" {
                    messages.push(MsgReader::parse(&bytes, vec!["Import".to_string()])?);
                } else if ext == "vcf" || ext == "vcard" {
                    messages.extend(VcardReader::parse(&bytes, vec!["Contacts".to_string()])?);
                } else if ext == "ics" || ext == "ical" {
                    messages.extend(IcalReader::parse(&bytes, vec!["Calendar".to_string()])?);
                } else {
                    messages.push(EmlReader::parse(&bytes, vec!["Import".to_string()])?);
                }
            } else if source.is_dir() {
                for entry in WalkDir::new(source).into_iter().filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    let folder = match path.strip_prefix(source) {
                        Ok(rel) => rel
                            .parent()
                            .map(|p| p.iter().filter_map(|s| s.to_str().map(|x| x.to_string())).collect())
                            .unwrap_or_default(),
                        Err(_) => vec!["Inbox".to_string()],
                    };

                    if let Ok(bytes) = std::fs::read(path) {
                        if ext == "emlx" {
                            if let Ok(m) = EmlxReader::parse(&bytes, folder) {
                                messages.push(m);
                            }
                        } else if ext == "msg" {
                            if let Ok(m) = MsgReader::parse(&bytes, folder) {
                                messages.push(m);
                            }
                        } else if ext == "vcf" || ext == "vcard" {
                            if let Ok(c) = VcardReader::parse(&bytes, vec!["Contacts".to_string()]) {
                                messages.extend(c);
                            }
                        } else if ext == "ics" || ext == "ical" {
                            if let Ok(ev) = IcalReader::parse(&bytes, vec!["Calendar".to_string()]) {
                                messages.extend(ev);
                            }
                        } else if ext == "eml" || ext.is_empty() {
                            if let Ok(m) = EmlReader::parse(&bytes, folder) {
                                messages.push(m);
                            }
                        }
                    }
                }
            }
        }
    }

    info!(count = messages.len(), "Source ingestion complete");
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_address_parsing() {
        let addr = EmailAddress::parse("\"Alice Vance\" <alice@example.com>");
        assert_eq!(addr.name, "Alice Vance");
        assert_eq!(addr.email, "alice@example.com");

        let plain = EmailAddress::parse("bob@example.com");
        assert_eq!(plain.name, "bob@example.com");
        assert_eq!(plain.email, "bob@example.com");
    }

    #[test]
    fn test_eml_parser_simple() {
        let raw = b"From: Alice <alice@example.com>\r\n\
To: Bob <bob@example.com>\r\n\
Subject: Test Email Subject\r\n\
Date: Fri, 01 Nov 2026 12:00:00 +0000\r\n\
Message-ID: <test01@example.com>\r\n\
\r\n\
Hello Bob, this is a plain text email message.";

        let msg = EmlReader::parse(raw, vec!["Inbox".to_string()]).expect("Failed to parse EML");
        assert_eq!(msg.subject, "Test Email Subject");
        assert_eq!(msg.from.email, "alice@example.com");
        assert_eq!(msg.to[0].email, "bob@example.com");
        assert!(msg.body_text.contains("Hello Bob"));
        assert_eq!(msg.folder_path, vec!["Inbox"]);
    }

    #[test]
    fn test_eml_rfc2047_decoding() {
        // Base64 encoded UTF-8 subject: "Hello World"
        let b64 = "=?UTF-8?B?SGVsbG8gV29ybGQ=?=";
        assert_eq!(EmlReader::decode_rfc2047(b64), "Hello World");

        // Quoted-Printable encoded: "Hello_World"
        let qp = "=?ISO-8859-1?Q?Hello=20World?=";
        assert_eq!(EmlReader::decode_rfc2047(qp), "Hello World");
    }

    #[test]
    fn test_mbox_reader_streaming() {
        let mbox_data = b"From sender@example.com Fri Nov 01 12:00:00 2026\r\n\
From: sender@example.com\r\n\
Subject: Message 1\r\n\
\r\n\
First body\r\n\
From sender@example.com Fri Nov 01 12:01:00 2026\r\n\
From: sender@example.com\r\n\
Subject: Message 2\r\n\
\r\n\
>From unescaped line\r\n";

        let msgs = MboxReader::read_all(&mbox_data[..], vec!["Archive".to_string()]).expect("Mbox parse failed");
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].subject, "Message 1");
        assert_eq!(msgs[1].subject, "Message 2");
        assert!(msgs[1].body_text.contains("From unescaped line"));
    }

    #[test]
    fn test_format_sniffer() {
        assert_eq!(FormatSniffer::sniff_bytes(b"!BDN\x00\x00", "pst"), MailboxFormat::Pst);
        assert_eq!(FormatSniffer::sniff_bytes(b"!BDO\x00\x00", "ost"), MailboxFormat::Ost);
        assert_eq!(
            FormatSniffer::sniff_bytes(&[0xCF, 0xAD, 0x12, 0xFE], "dbx"),
            MailboxFormat::Dbx
        );
        assert_eq!(
            FormatSniffer::sniff_bytes(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1], "msg"),
            MailboxFormat::Msg
        );
        assert_eq!(
            FormatSniffer::sniff_bytes(b"From user@host", "mbox"),
            MailboxFormat::Mbox
        );
        assert_eq!(
            FormatSniffer::sniff_bytes(&[0x50, 0x4B, 0x03, 0x04], "zip"),
            MailboxFormat::Zip
        );
        assert_eq!(FormatSniffer::sniff_bytes(&[0x1F, 0x8B], "gz"), MailboxFormat::Gz);
    }

    #[test]
    fn test_gmail_labels_routing() {
        let (folder, flags) = EmlReader::parse_gmail_labels("Inbox,Important,Work/Finance,Starred");
        assert_eq!(folder, Some(vec!["Work".to_string(), "Finance".to_string()]));
        assert!(flags.read);
        assert!(flags.flagged);
        assert!(!flags.deleted);

        let (sent_folder, _) = EmlReader::parse_gmail_labels("Sent,Important");
        assert_eq!(sent_folder, Some(vec!["Sent Items".to_string()]));

        let (trash_folder, trash_flags) = EmlReader::parse_gmail_labels("Trash");
        assert_eq!(trash_folder, Some(vec!["Deleted Items".to_string()]));
        assert!(trash_flags.deleted);
    }

    #[test]
    fn test_received_header_date_fallback() {
        let raw = b"From: Alice <alice@example.com>\r\n\
To: Bob <bob@example.com>\r\n\
Subject: No Date Header\r\n\
Received: from mail.example.com; Thu, 15 Aug 2024 16:45:00 +0000 (UTC)\r\n\
\r\n\
Fallback date test";
        let msg = EmlReader::parse(raw, vec!["Inbox".to_string()]).expect("Parse should succeed");
        assert_eq!(msg.date.format("%Y-%m-%d").to_string(), "2024-08-15");
    }
}
