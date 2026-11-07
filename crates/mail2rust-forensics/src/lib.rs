use chrono::{DateTime, Utc};
use mail2rust_core::EmailMessage;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatesRange {
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForensicManifest {
    pub case_number: String,
    pub examiner: String,
    pub timestamp: DateTime<Utc>,
    pub source_container: String,
    pub target_container: String,
    pub total_items: usize,
    pub total_size_bytes: usize,
    pub root_sha256: String,
    pub bates_range: BatesRange,
    pub tool_version: String,
}

pub struct ForensicEngine;

impl ForensicEngine {
    pub fn apply_bates_stamping(
        messages: &mut [EmailMessage],
        prefix: &str,
        starting_number: usize,
        digit_padding: usize,
    ) -> BatesRange {
        if messages.is_empty() {
            return BatesRange {
                start: "N/A".to_string(),
                end: "N/A".to_string(),
            };
        }

        info!(
            prefix = prefix,
            start = starting_number,
            count = messages.len(),
            "Applying Bates stamping"
        );

        let start_bates = format!("{}-{:0width$}", prefix, starting_number, width = digit_padding);
        let mut end_bates = start_bates.clone();

        for (current, msg) in (starting_number..).zip(messages.iter_mut()) {
            let bates = format!("{}-{:0width$}", prefix, current, width = digit_padding);
            msg.bates_number = Some(bates.clone());
            end_bates = bates;
        }

        BatesRange {
            start: start_bates,
            end: end_bates,
        }
    }

    pub fn compute_root_hash(messages: &[EmailMessage]) -> String {
        let mut hashes: Vec<_> = messages.iter().map(|m| m.sha256.as_str()).collect();
        hashes.sort();
        let mut root = Sha256::new();
        for h in hashes {
            root.update(h.as_bytes());
        }
        format!("{:x}", root.finalize())
    }

    pub fn generate_manifest(
        messages: &[EmailMessage],
        case_number: &str,
        examiner: &str,
        source_container: &str,
        target_container: &str,
        bates_range: BatesRange,
    ) -> ForensicManifest {
        let total_size_bytes = messages.iter().map(|m| m.size_bytes).sum();
        let root_sha256 = Self::compute_root_hash(messages);

        ForensicManifest {
            case_number: case_number.to_string(),
            examiner: examiner.to_string(),
            timestamp: Utc::now(),
            source_container: source_container.to_string(),
            target_container: target_container.to_string(),
            total_items: messages.len(),
            total_size_bytes,
            root_sha256,
            bates_range,
            tool_version: format!("mail2rust-forensics v{}", env!("CARGO_PKG_VERSION")),
        }
    }

    pub fn export_manifest_json(manifest: &ForensicManifest, path: &Path) -> io::Result<()> {
        let file = File::create(path)?;
        serde_json::to_writer_pretty(file, manifest)?;
        info!(path = ?path, "Exported JSON forensic manifest");
        Ok(())
    }

    pub fn export_manifest_csv(messages: &[EmailMessage], path: &Path) -> io::Result<()> {
        let file = File::create(path)?;
        let mut file = BufWriter::new(file);
        writeln!(
            file,
            "bates_number,message_id,subject,sender,date,sha256,md5,size_bytes,folder_path,legal_hold"
        )?;

        for m in messages {
            let bates = m.bates_number.as_deref().unwrap_or("").replace('"', "\"\"");
            let safe_msgid = m.message_id.replace('"', "\"\"");
            let safe_subj = m.subject.replace('"', "\"\"");
            let safe_sender = m.from.email.replace('"', "\"\"");
            let safe_folder = m.folder_path.join("/").replace('"', "\"\"");
            writeln!(
                file,
                "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",{},\"{}\",{}",
                bates,
                safe_msgid,
                safe_subj,
                safe_sender,
                m.date.to_rfc3339(),
                m.sha256,
                m.md5,
                m.size_bytes,
                safe_folder,
                m.legal_hold
            )?;
        }
        file.flush()?;

        info!(path = ?path, count = messages.len(), "Exported CSV forensic manifest");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mail2rust_core::{EmailAddress, EmailMessage, Importance, MessageFlags};

    #[test]
    fn test_bates_stamping_and_manifest() {
        let mut messages = vec![
            EmailMessage {
                id: "1".to_string(),
                message_id: "<1@test>".to_string(),
                conversation_id: "c1".to_string(),
                folder_path: vec!["Inbox".to_string()],
                subject: "Test 1".to_string(),
                from: EmailAddress::new("Alice", "alice@test.com"),
                to: vec![],
                cc: vec![],
                bcc: vec![],
                date: Utc::now(),
                size_bytes: 100,
                body_text: "Body 1".to_string(),
                body_html: None,
                raw_mime: vec![],
                attachments: vec![],
                mapi_properties: vec![],
                flags: MessageFlags::default(),
                importance: Importance::Normal,
                sha256: "sha1".to_string(),
                md5: "md51".to_string(),
                bates_number: None,
                legal_hold: false,
                source_format: "test".to_string(),
            },
            EmailMessage {
                id: "2".to_string(),
                message_id: "<2@test>".to_string(),
                conversation_id: "c2".to_string(),
                folder_path: vec!["Inbox".to_string()],
                subject: "Test 2".to_string(),
                from: EmailAddress::new("Bob", "bob@test.com"),
                to: vec![],
                cc: vec![],
                bcc: vec![],
                date: Utc::now(),
                size_bytes: 200,
                body_text: "Body 2".to_string(),
                body_html: None,
                raw_mime: vec![],
                attachments: vec![],
                mapi_properties: vec![],
                flags: MessageFlags::default(),
                importance: Importance::Normal,
                sha256: "sha2".to_string(),
                md5: "md52".to_string(),
                bates_number: None,
                legal_hold: true,
                source_format: "test".to_string(),
            },
        ];

        let range = ForensicEngine::apply_bates_stamping(&mut messages, "ACME", 1000, 7);
        assert_eq!(range.start, "ACME-0001000");
        assert_eq!(range.end, "ACME-0001001");
        assert_eq!(messages[0].bates_number.as_deref(), Some("ACME-0001000"));
        assert_eq!(messages[1].bates_number.as_deref(), Some("ACME-0001001"));

        let manifest = ForensicEngine::generate_manifest(
            &messages,
            "CASE-2026",
            "Inspector Clouseau",
            "source.mbox",
            "dest.pst",
            range,
        );
        assert_eq!(manifest.total_items, 2);
        assert_eq!(manifest.total_size_bytes, 300);
        assert!(!manifest.root_sha256.is_empty());

        // Test CSV manifest output with escaped quotes
        messages[0].subject = "Subject with \"quotes\" and, commas".to_string();
        let temp_csv = std::env::temp_dir().join(format!(
            "test_manifest_{}.csv",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        ForensicEngine::export_manifest_csv(&messages, &temp_csv).expect("CSV export failed");
        let content = std::fs::read_to_string(&temp_csv).expect("Failed to read CSV");
        assert!(content.contains("\"Subject with \"\"quotes\"\" and, commas\""));
        let _ = std::fs::remove_file(temp_csv);
    }
}
