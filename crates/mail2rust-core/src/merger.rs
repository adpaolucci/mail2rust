use crate::dedup::{DedupEngine, DedupStrategy};
use crate::models::EmailMessage;
use crate::sniffer::FormatSniffer;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::info;

/// Strategy for organizing folder hierarchies when merging disparate mailboxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MergeFolderStrategy {
    /// Combine identical folder names across sources into a unified folder.
    #[default]
    Combine,
    /// Partition folders by prefixing with the source alias (e.g. `[SourceA]/Inbox`).
    Prefix,
    /// Flatten all folder hierarchies into a single unified root/inbox.
    Flatten,
}

impl std::str::FromStr for MergeFolderStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "combine" | "merge" | "unified" => Ok(MergeFolderStrategy::Combine),
            "prefix" | "partition" | "separate" => Ok(MergeFolderStrategy::Prefix),
            "flatten" | "flat" | "root" => Ok(MergeFolderStrategy::Flatten),
            other => Err(format!(
                "Invalid folder merge strategy '{}'. Allowed: combine, prefix, flatten",
                other
            )),
        }
    }
}

/// Configuration for an individual source in a merge operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeSourceConfig {
    pub path: PathBuf,
    pub alias: Option<String>,
    pub custom_prefix: Option<String>,
}

impl MergeSourceConfig {
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        Self {
            path: path.into(),
            alias: None,
            custom_prefix: None,
        }
    }

    pub fn with_alias<P: Into<PathBuf>, S: Into<String>>(path: P, alias: S) -> Self {
        Self {
            path: path.into(),
            alias: Some(alias.into()),
            custom_prefix: None,
        }
    }

    pub fn resolved_alias(&self) -> String {
        if let Some(ref a) = self.alias {
            if !a.trim().is_empty() {
                return a.trim().to_string();
            }
        }
        self.path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Mailbox")
            .to_string()
    }
}

/// Options controlling how mailboxes are merged, deduplicated, and organized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeOptions {
    pub folder_strategy: MergeFolderStrategy,
    pub dedup_strategy: DedupStrategy,
    pub sort_by_date: bool,
    pub dry_run: bool,
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self {
            folder_strategy: MergeFolderStrategy::Combine,
            dedup_strategy: DedupStrategy::MessageId,
            sort_by_date: true,
            dry_run: false,
        }
    }
}

/// Ingestion statistics for a single mailbox source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMergeStats {
    pub source_path: PathBuf,
    pub alias: String,
    pub detected_format: String,
    pub total_messages: usize,
    pub duplicates_removed: usize,
    pub unique_messages: usize,
    pub total_bytes: usize,
    pub folder_counts: HashMap<String, usize>,
}

/// Comprehensive report detailing the results of a multi-format mailbox merge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeReport {
    pub sources: Vec<SourceMergeStats>,
    pub total_input_messages: usize,
    pub total_unique_messages: usize,
    pub total_duplicates_removed: usize,
    pub total_bytes: usize,
    pub folder_distribution: HashMap<String, usize>,
    pub earliest_date: Option<DateTime<Utc>>,
    pub latest_date: Option<DateTime<Utc>>,
}

impl MergeReport {
    /// Formats a clean human-readable summary of the merge operation.
    pub fn format_summary(&self) -> String {
        let mut out = String::new();
        out.push_str("======================================================================\n");
        out.push_str("              MAILBOX MERGE & CONSOLIDATION SUMMARY                  \n");
        out.push_str("======================================================================\n");
        out.push_str(&format!(
            "Total Ingested Sources:      {}\n",
            self.sources.len()
        ));
        out.push_str(&format!(
            "Total Messages Processed:    {}\n",
            self.total_input_messages
        ));
        out.push_str(&format!(
            "Duplicate Messages Removed:  {}\n",
            self.total_duplicates_removed
        ));
        out.push_str(&format!(
            "Unique Messages Retained:    {}\n",
            self.total_unique_messages
        ));
        out.push_str(&format!(
            "Total Ingested Payload Size: {:.2} MB ({} bytes)\n",
            self.total_bytes as f64 / (1024.0 * 1024.0),
            self.total_bytes
        ));
        out.push_str(&format!(
            "Unified Folders Created:     {}\n",
            self.folder_distribution.len()
        ));

        if let (Some(early), Some(late)) = (self.earliest_date, self.latest_date) {
            out.push_str(&format!(
                "Chronological Span:          {} to {}\n",
                early.format("%Y-%m-%d %H:%M:%S UTC"),
                late.format("%Y-%m-%d %H:%M:%S UTC")
            ));
        }

        out.push_str("\n--- Per-Source Ingestion Breakdown ---\n");
        for (i, s) in self.sources.iter().enumerate() {
            out.push_str(&format!(
                "  [{}] Source: \"{}\" ({})\n",
                i + 1,
                s.alias,
                s.source_path.display()
            ));
            out.push_str(&format!(
                "      Format: {} | Total: {} | Unique: {} | Duplicates: {} | Size: {:.2} MB\n",
                s.detected_format,
                s.total_messages,
                s.unique_messages,
                s.duplicates_removed,
                s.total_bytes as f64 / (1024.0 * 1024.0)
            ));
        }

        out.push_str("\n--- Merged Folder Distribution ---\n");
        let mut sorted_folders: Vec<_> = self.folder_distribution.iter().collect();
        sorted_folders.sort_by(|a, b| b.1.cmp(a.1));
        for (folder, count) in sorted_folders {
            out.push_str(&format!("  - {:<35} : {:>6} messages\n", folder, count));
        }
        out.push_str("======================================================================\n");
        out
    }
}

/// Core engine for merging disparate mailbox sources and formats into a unified message collection.
pub struct MailboxMerger;

impl MailboxMerger {
    /// Ingests multiple mailbox sources using a custom ingester function, applies folder strategies,
    /// performs cross-source deduplication, sorts chronologically, and returns the merged messages and report.
    pub fn merge_sources<F>(
        sources: &[MergeSourceConfig],
        options: &MergeOptions,
        ingester: F,
    ) -> Result<(Vec<EmailMessage>, MergeReport), Box<dyn std::error::Error>>
    where
        F: Fn(&Path) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>>,
    {
        let mut batches = Vec::new();

        for src in sources {
            let path = &src.path;
            info!(path = ?path, alias = ?src.alias, "Ingesting source for merge");
            let msgs = ingester(path)?;
            batches.push((src.clone(), msgs));
        }

        Ok(Self::merge_batches(batches, options))
    }

    /// Merges pre-loaded message batches from different sources according to the given options.
    pub fn merge_batches(
        batches: Vec<(MergeSourceConfig, Vec<EmailMessage>)>,
        options: &MergeOptions,
    ) -> (Vec<EmailMessage>, MergeReport) {
        let mut total_input = 0;
        let mut all_staged = Vec::new();
        let mut source_stats = Vec::new();

        for (src_cfg, mut msgs) in batches {
            let alias = src_cfg.resolved_alias();
            let count = msgs.len();
            total_input += count;

            let detected_format = FormatSniffer::sniff_path(&src_cfg.path).name().to_string();
            let mut byte_size = 0;
            let mut initial_folder_counts = HashMap::new();

            for m in &mut msgs {
                let size = if m.size_bytes > 0 {
                    m.size_bytes
                } else if !m.raw_mime.is_empty() {
                    m.raw_mime.len()
                } else {
                    m.body_text.len()
                };
                byte_size += size;

                // Adjust folders based on folder strategy
                match options.folder_strategy {
                    MergeFolderStrategy::Combine => {
                        // Keep folders as-is so same folder names across sources merge together.
                        // Ensure at least "Inbox" if empty.
                        if m.folder_path.is_empty() {
                            m.folder_path = vec!["Inbox".to_string()];
                        }
                    }
                    MergeFolderStrategy::Prefix => {
                        let prefix = src_cfg
                            .custom_prefix
                            .clone()
                            .unwrap_or_else(|| alias.clone());
                        let mut new_folder = vec![prefix];
                        if m.folder_path.is_empty() {
                            new_folder.push("Inbox".to_string());
                        } else {
                            new_folder.extend(m.folder_path.clone());
                        }
                        m.folder_path = new_folder;
                    }
                    MergeFolderStrategy::Flatten => {
                        m.folder_path = vec!["Merged".to_string()];
                    }
                }

                let folder_name = m.folder_path.join("/");
                *initial_folder_counts.entry(folder_name).or_insert(0) += 1;
            }

            source_stats.push(SourceMergeStats {
                source_path: src_cfg.path.clone(),
                alias: alias.clone(),
                detected_format,
                total_messages: count,
                duplicates_removed: 0,
                unique_messages: count,
                total_bytes: byte_size,
                folder_counts: initial_folder_counts,
            });

            all_staged.push((alias, msgs));
        }

        // Flatten all messages for global cross-source deduplication
        let mut combined_messages = Vec::with_capacity(total_input);
        let mut message_source_map = Vec::with_capacity(total_input);

        for (alias, msgs) in all_staged {
            for m in msgs {
                message_source_map.push(alias.clone());
                combined_messages.push(m);
            }
        }

        // Apply Cross-Source Deduplication
        let (mut final_messages, duplicates) =
            DedupEngine::deduplicate(combined_messages, options.dedup_strategy);

        // Calculate per-source duplicate counts
        let mut dup_count_by_source: HashMap<String, usize> = HashMap::new();
        for _d in &duplicates {
            if let Some(src_alias) = message_source_map.first() {
                *dup_count_by_source.entry(src_alias.clone()).or_insert(0) += 1;
            }
        }

        let total_dups = duplicates.len();
        let total_unique = final_messages.len();

        for stat in &mut source_stats {
            let dups = dup_count_by_source.get(&stat.alias).copied().unwrap_or(0);
            stat.duplicates_removed = dups;
            stat.unique_messages = stat.total_messages.saturating_sub(dups);
        }

        // Chronological Sorting
        if options.sort_by_date {
            final_messages.sort_by_key(|m| m.date);
        }

        // Final merged folder distribution
        let mut folder_distribution = HashMap::new();
        let mut total_bytes = 0;
        let mut earliest_date: Option<DateTime<Utc>> = None;
        let mut latest_date: Option<DateTime<Utc>> = None;

        for m in &final_messages {
            let fpath = m.folder_path.join("/");
            *folder_distribution.entry(fpath).or_insert(0) += 1;

            let size = if m.size_bytes > 0 {
                m.size_bytes
            } else if !m.raw_mime.is_empty() {
                m.raw_mime.len()
            } else {
                m.body_text.len()
            };
            total_bytes += size;

            match (earliest_date, latest_date) {
                (None, None) => {
                    earliest_date = Some(m.date);
                    latest_date = Some(m.date);
                }
                (Some(e), Some(l)) => {
                    if m.date < e {
                        earliest_date = Some(m.date);
                    }
                    if m.date > l {
                        latest_date = Some(m.date);
                    }
                }
                _ => {}
            }
        }

        let report = MergeReport {
            sources: source_stats,
            total_input_messages: total_input,
            total_unique_messages: total_unique,
            total_duplicates_removed: total_dups,
            total_bytes,
            folder_distribution,
            earliest_date,
            latest_date,
        };

        (final_messages, report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{EmailAddress, MessageFlags};
    use chrono::TimeZone;

    fn make_test_message(id: &str, subject: &str, folder: &[&str], _date_str: &str) -> EmailMessage {
        let dt = Utc.with_ymd_and_hms(2026, 11, 1, 10, 0, 0).unwrap();
        EmailMessage {
            id: id.to_string(),
            message_id: format!("<{}>", id),
            conversation_id: format!("conv_{}", id),
            folder_path: folder.iter().map(|s| s.to_string()).collect(),
            subject: subject.to_string(),
            from: EmailAddress::new("Sender", "sender@example.com"),
            to: vec![EmailAddress::new("Recipient", "rcpt@example.com")],
            cc: Vec::new(),
            bcc: Vec::new(),
            date: dt,
            size_bytes: 100,
            body_text: format!("Body of {}", subject),
            body_html: None,
            raw_mime: format!("From: sender@example.com\r\nSubject: {}\r\n\r\nBody of {}", subject, subject).into_bytes(),
            attachments: Vec::new(),
            mapi_properties: Vec::new(),
            flags: MessageFlags::default(),
            importance: crate::models::Importance::Normal,
            sha256: format!("hash-{}", id),
            md5: format!("md5-{}", id),
            bates_number: None,
            legal_hold: false,
            source_format: "test".to_string(),
        }
    }

    #[test]
    fn test_merge_batches_combine_strategy() {
        let batch1 = vec![
            make_test_message("msg1@a.com", "First email", &["Inbox"], "2026-11-01"),
            make_test_message("msg2@a.com", "Second email", &["Work"], "2026-11-02"),
        ];
        let batch2 = vec![
            make_test_message("msg3@b.com", "Third email", &["Inbox"], "2026-11-03"),
            make_test_message("msg4@b.com", "Fourth email", &["Personal"], "2026-11-04"),
        ];

        let batches = vec![
            (MergeSourceConfig::new("source1.mbox"), batch1),
            (MergeSourceConfig::new("source2.pst"), batch2),
        ];

        let options = MergeOptions {
            folder_strategy: MergeFolderStrategy::Combine,
            dedup_strategy: DedupStrategy::MessageId,
            sort_by_date: true,
            dry_run: false,
        };

        let (merged, report) = MailboxMerger::merge_batches(batches, &options);

        assert_eq!(merged.len(), 4);
        assert_eq!(report.total_input_messages, 4);
        assert_eq!(report.total_unique_messages, 4);
        assert_eq!(report.total_duplicates_removed, 0);

        // Check combined inbox
        assert_eq!(report.folder_distribution.get("Inbox"), Some(&2));
        assert_eq!(report.folder_distribution.get("Work"), Some(&1));
        assert_eq!(report.folder_distribution.get("Personal"), Some(&1));
    }

    #[test]
    fn test_merge_batches_prefix_strategy() {
        let batch1 = vec![make_test_message(
            "msg1@a.com",
            "Gmail email",
            &["Inbox"],
            "2026-11-01",
        )];
        let batch2 = vec![make_test_message(
            "msg2@b.com",
            "Yahoo email",
            &["Inbox"],
            "2026-11-02",
        )];

        let batches = vec![
            (
                MergeSourceConfig::with_alias("source1.mbox", "Gmail"),
                batch1,
            ),
            (
                MergeSourceConfig::with_alias("source2.pst", "Yahoo"),
                batch2,
            ),
        ];

        let options = MergeOptions {
            folder_strategy: MergeFolderStrategy::Prefix,
            dedup_strategy: DedupStrategy::MessageId,
            sort_by_date: true,
            dry_run: false,
        };

        let (merged, report) = MailboxMerger::merge_batches(batches, &options);

        assert_eq!(merged.len(), 2);
        assert_eq!(report.folder_distribution.get("Gmail/Inbox"), Some(&1));
        assert_eq!(report.folder_distribution.get("Yahoo/Inbox"), Some(&1));
    }

    #[test]
    fn test_merge_cross_source_deduplication() {
        let batch1 = vec![make_test_message(
            "common@corp.com",
            "Overlap email",
            &["Inbox"],
            "2026-11-01",
        )];
        let mut dup_msg = make_test_message(
            "common@corp.com",
            "Overlap email",
            &["Inbox"],
            "2026-11-01",
        );
        dup_msg.flags.flagged = true; // Duplicate has flagged flag
        let batch2 = vec![dup_msg];

        let batches = vec![
            (MergeSourceConfig::new("archive1.mbox"), batch1),
            (MergeSourceConfig::new("archive2.pst"), batch2),
        ];

        let options = MergeOptions {
            folder_strategy: MergeFolderStrategy::Combine,
            dedup_strategy: DedupStrategy::MessageId,
            sort_by_date: true,
            dry_run: false,
        };

        let (merged, report) = MailboxMerger::merge_batches(batches, &options);

        assert_eq!(merged.len(), 1);
        assert_eq!(report.total_input_messages, 2);
        assert_eq!(report.total_unique_messages, 1);
        assert_eq!(report.total_duplicates_removed, 1);
        // Flag should have been merged
        assert!(merged[0].flags.flagged);
    }
}
