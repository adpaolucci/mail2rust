use chrono::{DateTime, Utc};
use mail2rust_core::EmailMessage;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncCheckpoint {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub source_id: String,
    pub target_id: String,
    pub migrated_message_ids: HashSet<String>,
    pub folder_high_watermarks: HashMap<String, u64>,
    #[serde(default)]
    pub folder_uidvalidity: HashMap<String, u32>,
    #[serde(default)]
    pub interrupted_reason: Option<String>,
    pub total_bytes_migrated: usize,
    pub total_messages_migrated: usize,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonlRecord {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub message_id: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub source_file: String,
    #[serde(default)]
    pub folder: String,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub size_bytes: usize,
    #[serde(default)]
    pub uid: Option<u64>,
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
            folder_uidvalidity: HashMap::new(),
            interrupted_reason: None,
            total_bytes_migrated: 0,
            total_messages_migrated: 0,
            status: "in_progress".to_string(),
        }
    }

    /// Load checkpoint from file, auto-detecting JSON vs JSONL format.
    pub fn load(path: &Path) -> io::Result<SyncCheckpoint> {
        let is_jsonl = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.eq_ignore_ascii_case("jsonl"))
            .unwrap_or(false);

        if is_jsonl {
            Self::load_jsonl(path)
        } else {
            let file = File::open(path)?;
            let reader = BufReader::new(file);
            let chk: SyncCheckpoint = serde_json::from_reader(reader)?;
            info!(path = ?path, count = chk.total_messages_migrated, "Loaded sync checkpoint");
            Ok(chk)
        }
    }

    /// Load checkpoint from a JSON Lines (.jsonl) stream.
    pub fn load_jsonl(path: &Path) -> io::Result<SyncCheckpoint> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut chk = Self::new_checkpoint(&path.display().to_string(), "resumed_destination");

        for (line_no, line_res) in reader.lines().enumerate() {
            let line = line_res?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            match serde_json::from_str::<JsonlRecord>(trimmed) {
                Ok(rec) => {
                    if !rec.key.is_empty() {
                        chk.migrated_message_ids.insert(rec.key.clone());
                    }
                    if !rec.message_id.is_empty() {
                        chk.migrated_message_ids.insert(rec.message_id.clone());
                        chk.migrated_message_ids.insert(format!("mid:{}", rec.message_id));
                    }
                    if !rec.sha256.is_empty() {
                        chk.migrated_message_ids.insert(rec.sha256.clone());
                        chk.migrated_message_ids.insert(format!("sha256:{}", rec.sha256));
                    }
                    chk.total_messages_migrated += 1;
                    chk.total_bytes_migrated += rec.size_bytes;

                    if let Some(uid) = rec.uid {
                        let mark = chk.folder_high_watermarks.entry(rec.folder).or_insert(0);
                        if uid > *mark {
                            *mark = uid;
                        }
                    }
                }
                Err(e) => {
                    warn!(line = line_no + 1, error = %e, "Skipped invalid JSONL checkpoint record");
                }
            }
        }

        chk.status = "resumed".to_string();
        info!(path = ?path, count = chk.total_messages_migrated, "Loaded JSONL checkpoint stream");
        Ok(chk)
    }

    /// Save checkpoint atomically by writing to a temporary file then renaming.
    /// This prevents 0-byte or corrupted checkpoint files during sudden interruptions.
    pub fn save_atomic(checkpoint: &SyncCheckpoint, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let is_jsonl = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.eq_ignore_ascii_case("jsonl"))
            .unwrap_or(false);

        let pid = std::process::id();
        let nanos = Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let tmp_file_name = format!(
            "{}.tmp.{}.{}",
            path.file_name().and_then(|s| s.to_str()).unwrap_or("chk"),
            pid,
            nanos
        );
        let tmp_path = path.with_file_name(tmp_file_name);

        if is_jsonl {
            {
                let file = File::create(&tmp_path)?;
                let mut writer = BufWriter::new(file);
                for mid in &checkpoint.migrated_message_ids {
                    let clean_mid = if let Some(stripped) = mid.strip_prefix("mid:") {
                        stripped.to_string()
                    } else {
                        mid.clone()
                    };
                    let clean_sha = if let Some(stripped) = mid.strip_prefix("sha256:") {
                        stripped.to_string()
                    } else {
                        String::new()
                    };
                    let rec = JsonlRecord {
                        key: mid.clone(),
                        message_id: clean_mid,
                        sha256: clean_sha,
                        source_file: checkpoint.source_id.clone(),
                        folder: String::new(),
                        timestamp: checkpoint.timestamp.to_rfc3339(),
                        size_bytes: 0,
                        uid: None,
                    };
                    let line = serde_json::to_string(&rec).map_err(io::Error::other)?;
                    writeln!(writer, "{}", line)?;
                }
                writer.flush()?;
                writer.get_ref().sync_all()?;
            }
            std::fs::rename(&tmp_path, path)?;
        } else {
            {
                let file = File::create(&tmp_path)?;
                let mut writer = BufWriter::new(file);
                serde_json::to_writer_pretty(&mut writer, checkpoint).map_err(io::Error::other)?;
                writer.flush()?;
                writer.get_ref().sync_all()?;
            }
            std::fs::rename(&tmp_path, path)?;
        }

        debug!(path = ?path, count = checkpoint.total_messages_migrated, "Saved sync checkpoint atomically");
        Ok(())
    }

    /// Save checkpoint (defaults to atomic save).
    pub fn save(checkpoint: &SyncCheckpoint, path: &Path) -> io::Result<()> {
        Self::save_atomic(checkpoint, path)
    }

    /// Append a single completed record to a JSON Lines (.jsonl) checkpoint file.
    pub fn append_jsonl_record(path: &Path, record: &JsonlRecord) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        let line = serde_json::to_string(record).map_err(io::Error::other)?;
        writeln!(file, "{}", line)?;
        file.flush()?;
        Ok(())
    }

    /// Append an EmailMessage migration event to a JSON Lines (.jsonl) checkpoint file.
    pub fn append_jsonl_message(path: &Path, msg: &EmailMessage, uid: Option<u64>) -> io::Result<()> {
        let mid = msg.message_id.clone();
        let sha = msg.sha256.clone();
        let primary_key = if !mid.is_empty() {
            format!("mid:{}", mid)
        } else if !sha.is_empty() {
            format!("sha256:{}", sha)
        } else {
            msg.id.clone()
        };

        let record = JsonlRecord {
            key: primary_key,
            message_id: mid,
            sha256: sha,
            source_file: msg.source_format.clone(),
            folder: msg.folder_path.join("/"),
            timestamp: Utc::now().to_rfc3339(),
            size_bytes: msg.size_bytes,
            uid,
        };

        Self::append_jsonl_record(path, &record)
    }

    /// Calculate delta between source messages and checkpoint, supporting both raw and prefixed keys.
    pub fn calculate_delta(
        source_messages: &[EmailMessage],
        checkpoint: &SyncCheckpoint,
    ) -> (Vec<EmailMessage>, usize) {
        let mut new_messages = Vec::new();
        let mut already_migrated_count = 0;

        for m in source_messages {
            let mid_key = if !m.message_id.is_empty() {
                Some(m.message_id.clone())
            } else {
                None
            };
            let sha_key = if !m.sha256.is_empty() {
                Some(m.sha256.clone())
            } else {
                None
            };
            let id_key = if !m.id.is_empty() { Some(m.id.clone()) } else { None };

            let is_migrated = mid_key.as_ref().is_some_and(|k| {
                checkpoint.migrated_message_ids.contains(k)
                    || checkpoint.migrated_message_ids.contains(&format!("mid:{}", k))
            }) || sha_key.as_ref().is_some_and(|k| {
                checkpoint.migrated_message_ids.contains(k)
                    || checkpoint.migrated_message_ids.contains(&format!("sha256:{}", k))
            }) || id_key
                .as_ref()
                .is_some_and(|k| checkpoint.migrated_message_ids.contains(k));

            if is_migrated {
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

    pub fn commit_run(checkpoint: &mut SyncCheckpoint, migrated: &[EmailMessage]) {
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

    /// Mark a checkpoint as interrupted with an explanatory reason.
    pub fn mark_interrupted(checkpoint: &mut SyncCheckpoint, reason: &str) {
        checkpoint.status = "interrupted".to_string();
        checkpoint.interrupted_reason = Some(reason.to_string());
        checkpoint.timestamp = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mail2rust_core::{EmailAddress, EmailMessage, Importance, MessageFlags};
    use tempfile::tempdir;

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

    #[test]
    fn test_atomic_save_and_load() {
        let dir = tempdir().unwrap();
        let chk_path = dir.path().join("sub").join("checkpoint.json");

        let mut chk = CheckpointManager::new_checkpoint("imap://mail.test/user", "out.pst");
        chk.folder_high_watermarks.insert("Inbox".to_string(), 42);
        chk.folder_uidvalidity.insert("Inbox".to_string(), 1234567);
        chk.migrated_message_ids.insert("<msg1@test>".to_string());
        chk.total_messages_migrated = 1;
        chk.total_bytes_migrated = 500;

        CheckpointManager::save_atomic(&chk, &chk_path).expect("save atomic");
        assert!(chk_path.exists());

        let loaded = CheckpointManager::load(&chk_path).expect("load atomic checkpoint");
        assert_eq!(loaded.total_messages_migrated, 1);
        assert_eq!(loaded.folder_high_watermarks.get("Inbox"), Some(&42));
        assert_eq!(loaded.folder_uidvalidity.get("Inbox"), Some(&1234567));
        assert!(loaded.migrated_message_ids.contains("<msg1@test>"));
    }

    #[test]
    fn test_jsonl_append_and_load() {
        let dir = tempdir().unwrap();
        let jsonl_path = dir.path().join("checkpoint.jsonl");

        let m1 = make_test_msg("10");
        let m2 = make_test_msg("20");

        CheckpointManager::append_jsonl_message(&jsonl_path, &m1, Some(101)).unwrap();
        CheckpointManager::append_jsonl_message(&jsonl_path, &m2, Some(102)).unwrap();

        let loaded = CheckpointManager::load(&jsonl_path).expect("load jsonl");
        assert_eq!(loaded.total_messages_migrated, 2);
        assert_eq!(loaded.folder_high_watermarks.get("Inbox"), Some(&102));
        assert!(loaded.migrated_message_ids.contains("<10@test>"));
        assert!(loaded.migrated_message_ids.contains("mid:<20@test>"));
    }

    #[test]
    fn test_mark_interrupted() {
        let mut chk = CheckpointManager::new_checkpoint("src", "dst");
        CheckpointManager::mark_interrupted(&mut chk, "SIGINT signal received");
        assert_eq!(chk.status, "interrupted");
        assert_eq!(chk.interrupted_reason, Some("SIGINT signal received".to_string()));
    }
}
