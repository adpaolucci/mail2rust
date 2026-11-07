use chrono::{DateTime, Utc};
use mail2rust_core::EmailMessage;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCheckpoint {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub source_id: String,
    pub target_id: String,
    pub migrated_message_ids: HashSet<String>,
    pub folder_high_watermarks: HashMap<String, u64>,
    pub total_bytes_migrated: usize,
    pub total_messages_migrated: usize,
    pub status: String,
}

pub struct CheckpointManager;

impl CheckpointManager {
    pub fn new_checkpoint(source_id: &str, target_id: &str) -> SyncCheckpoint {
        SyncCheckpoint {
            id: format!("chk_{}", Utc::now().timestamp_millis()),
            timestamp: Utc::now(),
            source_id: source_id.to_string(),
            target_id: target_id.to_string(),
            migrated_message_ids: HashSet::new(),
            folder_high_watermarks: HashMap::new(),
            total_bytes_migrated: 0,
            total_messages_migrated: 0,
            status: "in_progress".to_string(),
        }
    }

    pub fn load(path: &Path) -> io::Result<SyncCheckpoint> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let chk: SyncCheckpoint = serde_json::from_reader(reader)?;
        info!(path = ?path, count = chk.total_messages_migrated, "Loaded sync checkpoint");
        Ok(chk)
    }

    pub fn save(checkpoint: &SyncCheckpoint, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = File::create(path)?;
        serde_json::to_writer_pretty(file, checkpoint)?;
        debug!(path = ?path, count = checkpoint.total_messages_migrated, "Saved sync checkpoint");
        Ok(())
    }

    pub fn calculate_delta(
        source_messages: &[EmailMessage],
        checkpoint: &SyncCheckpoint,
    ) -> (Vec<EmailMessage>, usize) {
        let mut new_messages = Vec::new();
        let mut already_migrated_count = 0;

        for m in source_messages {
            let key = if !m.message_id.is_empty() {
                m.message_id.clone()
            } else {
                m.sha256.clone()
            };

            if checkpoint.migrated_message_ids.contains(&key) {
                already_migrated_count += 1;
            } else {
                new_messages.push(m.clone());
            }
        }

        info!(
            new = new_messages.len(),
            already_done = already_migrated_count,
            "Delta calculation completed"
        );
        (new_messages, already_migrated_count)
    }

    pub fn commit_run(
        checkpoint: &mut SyncCheckpoint,
        migrated: &[EmailMessage],
    ) {
        for m in migrated {
            let key = if !m.message_id.is_empty() {
                m.message_id.clone()
            } else {
                m.sha256.clone()
            };
            checkpoint.migrated_message_ids.insert(key);
            checkpoint.total_bytes_migrated += m.size_bytes;
            checkpoint.total_messages_migrated += 1;

            let folder_key = m.folder_path.join("/");
            let mark = checkpoint.folder_high_watermarks.entry(folder_key).or_insert(0);
            *mark += 1;
        }
        checkpoint.timestamp = Utc::now();
        checkpoint.status = "completed".to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mail2rust_core::{EmailAddress, EmailMessage, Importance, MessageFlags};

    fn make_test_msg(id: &str) -> EmailMessage {
        EmailMessage {
            id: id.to_string(),
            message_id: format!("<{}@test>", id),
            conversation_id: "c1".to_string(),
            folder_path: vec!["Inbox".to_string()],
            subject: "Delta Test".to_string(),
            from: EmailAddress::new("Sender", "sender@test.com"),
            to: vec![],
            cc: vec![],
            bcc: vec![],
            date: Utc::now(),
            size_bytes: 100,
            body_text: "Body".to_string(),
            body_html: None,
            raw_mime: vec![],
            attachments: vec![],
            mapi_properties: vec![],
            flags: MessageFlags::default(),
            importance: Importance::Normal,
            sha256: format!("sha_{}", id),
            md5: format!("md5_{}", id),
            bates_number: None,
            legal_hold: false,
            source_format: "test".to_string(),
        }
    }

    #[test]
    fn test_delta_sync_cycle() {
        let m1 = make_test_msg("1");
        let m2 = make_test_msg("2");
        let m3 = make_test_msg("3");

        let mut chk = CheckpointManager::new_checkpoint("src", "dst");
        CheckpointManager::commit_run(&mut chk, &[m1.clone(), m2.clone()]);
        assert_eq!(chk.total_messages_migrated, 2);

        let all = vec![m1, m2, m3];
        let (delta, already_done) = CheckpointManager::calculate_delta(&all, &chk);
        assert_eq!(already_done, 2);
        assert_eq!(delta.len(), 1);
        assert_eq!(delta[0].id, "3");
    }
}
