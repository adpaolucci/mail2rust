use chrono::{DateTime, Utc};
use mail2rust_core::{EmailMessage, Importance};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FilterCriteria {
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub sender_search: Option<String>,
    pub recipient_search: Option<String>,
    pub subject_search: Option<String>,
    pub body_search: Option<String>,
    pub has_attachments: Option<bool>,
    pub min_size_bytes: Option<usize>,
    pub max_size_bytes: Option<usize>,
    pub importance: Option<Importance>,
    pub read_only: Option<bool>,
    pub flagged_only: Option<bool>,
    pub legal_hold_only: Option<bool>,
}

pub struct FilterEngine;

impl FilterEngine {
    pub fn matches(msg: &EmailMessage, criteria: &FilterCriteria) -> bool {
        if let Some(start) = criteria.start_date {
            if msg.date < start {
                return false;
            }
        }
        if let Some(end) = criteria.end_date {
            if msg.date > end {
                return false;
            }
        }
        if let Some(ref sender) = criteria.sender_search {
            let q = sender.to_lowercase();
            if !msg.from.name.to_lowercase().contains(&q) && !msg.from.email.to_lowercase().contains(&q) {
                return false;
            }
        }
        if let Some(ref recip) = criteria.recipient_search {
            let q = recip.to_lowercase();
            let matches_recip = msg.to.iter().any(|r| r.name.to_lowercase().contains(&q) || r.email.to_lowercase().contains(&q))
                || msg.cc.iter().any(|r| r.name.to_lowercase().contains(&q) || r.email.to_lowercase().contains(&q));
            if !matches_recip {
                return false;
            }
        }
        if let Some(ref subj) = criteria.subject_search {
            if !msg.subject.to_lowercase().contains(&subj.to_lowercase()) {
                return false;
            }
        }
        if let Some(ref body) = criteria.body_search {
            if !msg.body_text.to_lowercase().contains(&body.to_lowercase()) {
                return false;
            }
        }
        if let Some(has_att) = criteria.has_attachments {
            if (msg.attachments.is_empty()) == has_att {
                return false;
            }
        }
        if let Some(min_s) = criteria.min_size_bytes {
            if msg.size_bytes < min_s {
                return false;
            }
        }
        if let Some(max_s) = criteria.max_size_bytes {
            if msg.size_bytes > max_s {
                return false;
            }
        }
        if let Some(imp) = criteria.importance {
            if msg.importance != imp {
                return false;
            }
        }
        if let Some(ro) = criteria.read_only {
            if msg.flags.read != ro {
                return false;
            }
        }
        if let Some(true) = criteria.flagged_only {
            if !msg.flags.flagged {
                return false;
            }
        }
        if let Some(true) = criteria.legal_hold_only {
            if !msg.legal_hold {
                return false;
            }
        }

        true
    }

    pub fn filter_messages(messages: &[EmailMessage], criteria: &FilterCriteria) -> Vec<EmailMessage> {
        messages.iter().filter(|m| Self::matches(m, criteria)).cloned().collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DedupStrategy {
    #[default]
    Sha256,
    MessageId,
    NormalizedContent,
}

pub struct DedupEngine;

impl DedupEngine {
    pub fn deduplicate(
        messages: Vec<EmailMessage>,
        strategy: DedupStrategy,
    ) -> (Vec<EmailMessage>, Vec<EmailMessage>) {
        let mut seen = HashSet::new();
        let mut unique = Vec::new();
        let mut duplicates = Vec::new();

        for msg in messages {
            let key = match strategy {
                DedupStrategy::Sha256 => msg.sha256.clone(),
                DedupStrategy::MessageId => msg.message_id.clone(),
                DedupStrategy::NormalizedContent => {
                    let mut h = Sha256::new();
                    h.update(msg.from.email.to_lowercase().as_bytes());
                    h.update(msg.subject.trim().to_lowercase().as_bytes());
                    h.update(msg.body_text.trim().as_bytes());
                    format!("{:x}", h.finalize())
                }
            };

            if seen.contains(&key) {
                duplicates.push(msg);
            } else {
                seen.insert(key);
                unique.push(msg);
            }
        }

        info!(unique = unique.len(), duplicates = duplicates.len(), "Deduplication finished");
        (unique, duplicates)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitVolume {
    pub volume_index: usize,
    pub file_name: String,
    pub messages: Vec<EmailMessage>,
    pub total_size_bytes: usize,
    pub folder_counts: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitConfig {
    pub max_volume_bytes: usize, // e.g. 20 * 1024 * 1024 * 1024
    pub cluster_threads: bool,
    pub file_prefix: String,
    pub target_ext: String,
}

pub struct VolumeSplitter;

impl VolumeSplitter {
    pub fn split(messages: &[EmailMessage], config: &SplitConfig) -> Vec<SplitVolume> {
        if messages.is_empty() {
            return Vec::new();
        }

        let mut volumes = Vec::new();
        let mut current_idx = 1;
        let mut current_messages = Vec::new();
        let mut current_bytes = 0usize;
        let mut folder_counts = HashMap::new();

        let mut commit_vol = |msgs: &mut Vec<EmailMessage>, bytes: &mut usize, fcounts: &mut HashMap<String, usize>, idx: &mut usize| {
            if !msgs.is_empty() {
                let file_name = format!("{}_part{:03}.{}", config.file_prefix, *idx, config.target_ext);
                volumes.push(SplitVolume {
                    volume_index: *idx,
                    file_name,
                    messages: std::mem::take(msgs),
                    total_size_bytes: *bytes,
                    folder_counts: std::mem::take(fcounts),
                });
                *idx += 1;
                *bytes = 0;
            }
        };

        if config.cluster_threads {
            // Group messages by conversation_id
            let mut threads: HashMap<String, Vec<&EmailMessage>> = HashMap::new();
            for m in messages {
                threads.entry(m.conversation_id.clone()).or_default().push(m);
            }

            for (_thread_id, thread_msgs) in threads {
                let thread_size: usize = thread_msgs.iter().map(|m| m.size_bytes).sum();
                if current_bytes + thread_size > config.max_volume_bytes && !current_messages.is_empty() {
                    commit_vol(&mut current_messages, &mut current_bytes, &mut folder_counts, &mut current_idx);
                }

                for m in thread_msgs {
                    current_messages.push(m.clone());
                    current_bytes += m.size_bytes;
                    let folder_name = m.folder_path.last().cloned().unwrap_or_else(|| "Root".to_string());
                    *folder_counts.entry(folder_name).or_insert(0) += 1;
                }
            }
        } else {
            for m in messages {
                if current_bytes + m.size_bytes > config.max_volume_bytes && !current_messages.is_empty() {
                    commit_vol(&mut current_messages, &mut current_bytes, &mut folder_counts, &mut current_idx);
                }
                current_messages.push(m.clone());
                current_bytes += m.size_bytes;
                let folder_name = m.folder_path.last().cloned().unwrap_or_else(|| "Root".to_string());
                *folder_counts.entry(folder_name).or_insert(0) += 1;
            }
        }

        commit_vol(&mut current_messages, &mut current_bytes, &mut folder_counts, &mut current_idx);
        info!(volumes = volumes.len(), "Volume splitting completed");
        volumes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mail2rust_core::{EmailAddress, EmailMessage, Importance, MessageFlags};

    fn make_test_msg(id: &str, conv_id: &str, size: usize, subject: &str) -> EmailMessage {
        EmailMessage {
            id: id.to_string(),
            message_id: format!("<{}@test>", id),
            conversation_id: conv_id.to_string(),
            folder_path: vec!["Inbox".to_string()],
            subject: subject.to_string(),
            from: EmailAddress::new("Sender", "sender@test.com"),
            to: vec![],
            cc: vec![],
            bcc: vec![],
            date: Utc::now(),
            size_bytes: size,
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
    fn test_volume_splitter_thread_preservation() {
        let m1 = make_test_msg("1", "conv_a", 4000, "Thread 1 Start");
        let m2 = make_test_msg("2", "conv_a", 3000, "Re: Thread 1 Reply");
        let m3 = make_test_msg("3", "conv_b", 5000, "Thread 2 Independent");

        let messages = vec![m1, m2, m3];
        let cfg = SplitConfig {
            max_volume_bytes: 8000,
            cluster_threads: true,
            file_prefix: "Archive".to_string(),
            target_ext: "pst".to_string(),
        };

        let volumes = VolumeSplitter::split(&messages, &cfg);
        assert_eq!(volumes.len(), 2);
        // m1 and m2 must be in the same volume together
        let vol1_ids: Vec<_> = volumes[0].messages.iter().map(|m| m.id.as_str()).collect();
        assert!(vol1_ids.contains(&"1") && vol1_ids.contains(&"2") || !vol1_ids.contains(&"1") && !vol1_ids.contains(&"2"));
    }

    #[test]
    fn test_deduplication() {
        let m1 = make_test_msg("1", "c1", 100, "Hello");
        let mut m2 = make_test_msg("2", "c2", 100, "Hello");
        m2.sha256 = m1.sha256.clone();

        let (unique, dups) = DedupEngine::deduplicate(vec![m1, m2], DedupStrategy::Sha256);
        assert_eq!(unique.len(), 1);
        assert_eq!(dups.len(), 1);
    }
}
