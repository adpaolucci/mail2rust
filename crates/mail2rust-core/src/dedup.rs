use crate::models::EmailMessage;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use tracing::info;

/// Strategy for detecting and eliminating duplicate emails across one or multiple mailboxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DedupStrategy {
    #[default]
    Sha256,
    MessageId,
    NormalizedContent,
    None,
}

pub struct DedupEngine;

impl DedupEngine {
    /// Deduplicates a vector of email messages according to the selected strategy.
    /// When duplicates are found, flags and missing content from duplicates are merged
    /// into the retained original message.
    /// Returns `(unique_messages, duplicate_messages)`.
    pub fn deduplicate(
        messages: Vec<EmailMessage>,
        strategy: DedupStrategy,
    ) -> (Vec<EmailMessage>, Vec<EmailMessage>) {
        if strategy == DedupStrategy::None {
            return (messages, Vec::new());
        }

        let mut seen_keys = HashSet::new();
        let mut key_to_index = std::collections::HashMap::new();
        let mut unique: Vec<EmailMessage> = Vec::new();
        let mut duplicates: Vec<EmailMessage> = Vec::new();

        for msg in messages {
            let key = match strategy {
                DedupStrategy::Sha256 => {
                    if !msg.sha256.is_empty() {
                        msg.sha256.clone()
                    } else {
                        let mut h = Sha256::new();
                        h.update(msg.raw_mime.as_slice());
                        format!("{:x}", h.finalize())
                    }
                }
                DedupStrategy::MessageId => {
                    let cleaned = msg.message_id.trim();
                    let cleaned = cleaned.trim_start_matches('<').trim_end_matches('>').trim();
                    if !cleaned.is_empty() {
                        cleaned.to_lowercase()
                    } else if !msg.sha256.is_empty() {
                        msg.sha256.clone()
                    } else {
                        let mut h = Sha256::new();
                        h.update(msg.raw_mime.as_slice());
                        format!("{:x}", h.finalize())
                    }
                }
                DedupStrategy::NormalizedContent => {
                    let mut h = Sha256::new();
                    h.update(msg.from.email.to_lowercase().as_bytes());
                    h.update(msg.subject.trim().to_lowercase().as_bytes());
                    h.update(msg.body_text.trim().as_bytes());
                    format!("{:x}", h.finalize())
                }
                DedupStrategy::None => String::new(),
            };

            if !key.is_empty() && seen_keys.contains(&key) {
                // Merge flags and missing fields into existing unique entry
                if let Some(&idx) = key_to_index.get(&key) {
                    let retained: &mut EmailMessage = &mut unique[idx];
                    retained.flags.read |= msg.flags.read;
                    retained.flags.flagged |= msg.flags.flagged;
                    retained.flags.replied |= msg.flags.replied;
                    retained.flags.forwarded |= msg.flags.forwarded;
                    retained.flags.deleted |= msg.flags.deleted;
                    retained.flags.draft |= msg.flags.draft;

                    if retained.body_text.is_empty() && !msg.body_text.is_empty() {
                        retained.body_text = msg.body_text.clone();
                    }
                    if retained.body_html.is_none() && msg.body_html.is_some() {
                        retained.body_html = msg.body_html.clone();
                    }
                    if retained.attachments.is_empty() && !msg.attachments.is_empty() {
                        retained.attachments = msg.attachments.clone();
                    }
                }
                duplicates.push(msg);
            } else {
                if !key.is_empty() {
                    seen_keys.insert(key.clone());
                    key_to_index.insert(key, unique.len());
                }
                unique.push(msg);
            }
        }

        info!(
            unique = unique.len(),
            duplicates = duplicates.len(),
            "Deduplication finished"
        );
        (unique, duplicates)
    }
}
