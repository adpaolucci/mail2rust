use chrono::{DateTime, Datelike, TimeZone, Utc};
use mail2rust_core::{EmailMessage, Importance};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use tracing::info;

// ----------------------- Human-Friendly Size Parsing -----------------------

/// Parse human-friendly sizes like "20GB", "500MB", "15.5G", "100KB", "1024B", "0", "unlimited"
pub fn parse_size_bytes(val: &str) -> Result<usize, String> {
    let s = val.trim();
    if s.is_empty()
        || s == "0"
        || s.eq_ignore_ascii_case("unlimited")
        || s.eq_ignore_ascii_case("none")
        || s.eq_ignore_ascii_case("off")
    {
        return Ok(0);
    }

    let (num_part, unit_part) = match s.find(|c: char| c.is_alphabetic()) {
        Some(pos) => (s[..pos].trim(), s[pos..].trim().to_uppercase()),
        None => (s, "B".to_string()),
    };

    let num: f64 = num_part
        .parse()
        .map_err(|_| format!("Invalid numeric value in size: '{}'", val))?;

    if num < 0.0 || num.is_nan() || num.is_infinite() {
        return Err(format!("Invalid size number: '{}'", val));
    }

    let bytes = match unit_part.as_str() {
        "B" | "BYTES" | "" => num,
        "K" | "KB" | "KIB" => num * 1024.0,
        "M" | "MB" | "MIB" => num * 1024.0 * 1024.0,
        "G" | "GB" | "GIB" => num * 1024.0 * 1024.0 * 1024.0,
        "T" | "TB" | "TIB" => num * 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return Err(format!("Unrecognized size unit in '{}': '{}'", val, unit_part)),
    };

    if bytes > usize::MAX as f64 {
        return Err(format!("Size exceeds maximum allowed: '{}'", val));
    }

    Ok(bytes as usize)
}

// ----------------------- Date Parsing & Filtering -----------------------

/// Parse CLI date input into timezone-aware UTC datetime.
/// Supports YYYY, YYYY-MM-DD, YYYY-MM-DDTHH:MM:SS, and RFC 2822.
pub fn parse_cli_datetime(val: &str, is_end_of_day: bool) -> Result<DateTime<Utc>, String> {
    let s = val.trim();
    if s.is_empty() {
        return Err("Empty date string".to_string());
    }

    // 1. Year only: "2023"
    if s.len() == 4 && s.chars().all(|c| c.is_ascii_digit()) {
        let year: i32 = s.parse().map_err(|e| format!("{}", e))?;
        if is_end_of_day {
            return Utc
                .with_ymd_and_hms(year, 12, 31, 23, 59, 59)
                .single()
                .ok_or_else(|| "Invalid year boundary".to_string());
        } else {
            return Utc
                .with_ymd_and_hms(year, 1, 1, 0, 0, 0)
                .single()
                .ok_or_else(|| "Invalid year boundary".to_string());
        }
    }

    // 2. Date only: "YYYY-MM-DD" or "YYYY/MM/DD"
    if s.len() == 10
        && s.is_ascii()
        && (s.as_bytes()[4] == b'-' || s.as_bytes()[4] == b'/')
        && (s.as_bytes()[7] == b'-' || s.as_bytes()[7] == b'/')
    {
        let y: i32 = s[0..4].parse().map_err(|_| "Invalid year")?;
        let m: u32 = s[5..7].parse().map_err(|_| "Invalid month")?;
        let d: u32 = s[8..10].parse().map_err(|_| "Invalid day")?;
        if is_end_of_day {
            return Utc
                .with_ymd_and_hms(y, m, d, 23, 59, 59)
                .single()
                .ok_or_else(|| "Invalid date boundary".to_string());
        } else {
            return Utc
                .with_ymd_and_hms(y, m, d, 0, 0, 0)
                .single()
                .ok_or_else(|| "Invalid date boundary".to_string());
        }
    }

    // 3. RFC 3339 / ISO 8601
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }

    // 4. RFC 2822
    if let Ok(dt) = DateTime::parse_from_rfc2822(s) {
        return Ok(dt.with_timezone(&Utc));
    }

    // 5. Naive Datetime formats
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Ok(DateTime::from_naive_utc_and_offset(naive, Utc));
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Ok(DateTime::from_naive_utc_and_offset(naive, Utc));
    }

    Err(format!(
        "Unable to parse date string: '{}'. Supported formats: YYYY, YYYY-MM-DD, ISO 8601, RFC 2822",
        val
    ))
}

#[derive(Debug, Clone, Default)]
pub struct DateFilter {
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub start_year: Option<i32>,
    pub end_year: Option<i32>,
}

impl DateFilter {
    pub fn is_active(&self) -> bool {
        self.since.is_some() || self.until.is_some() || self.start_year.is_some() || self.end_year.is_some()
    }

    pub fn matches(&self, dt: &DateTime<Utc>) -> bool {
        if let Some(since) = self.since {
            if dt < &since {
                return false;
            }
        }
        if let Some(until) = self.until {
            if dt > &until {
                return false;
            }
        }
        if let Some(sy) = self.start_year {
            if dt.year() < sy {
                return false;
            }
        }
        if let Some(ey) = self.end_year {
            if dt.year() > ey {
                return false;
            }
        }
        true
    }
}

// ----------------------- Folder Filtering -----------------------

#[derive(Debug, Clone, Default)]
pub struct FolderFilter {
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
}

impl FolderFilter {
    pub fn new(includes: Vec<String>, excludes: Vec<String>) -> Self {
        Self {
            include_patterns: includes
                .into_iter()
                .map(|p| p.trim().replace('\\', "/"))
                .filter(|p| !p.is_empty())
                .collect(),
            exclude_patterns: excludes
                .into_iter()
                .map(|p| p.trim().replace('\\', "/"))
                .filter(|p| !p.is_empty())
                .collect(),
        }
    }

    pub fn is_active(&self) -> bool {
        !self.include_patterns.is_empty() || !self.exclude_patterns.is_empty()
    }

    fn matches_pattern(target: &str, pattern: &str) -> bool {
        let t_low = target.to_lowercase();
        let p_low = pattern.to_lowercase();

        // Exact match
        if t_low == p_low {
            return true;
        }

        // Wildcard match
        if Self::glob_match(&p_low, &t_low) {
            return true;
        }

        // If pattern has no slash, match individual segments
        if !pattern.contains('/') {
            for segment in target.split('/') {
                let s_low = segment.to_lowercase();
                if s_low == p_low || Self::glob_match(&p_low, &s_low) {
                    return true;
                }
            }
        }

        false
    }

    fn glob_match(pattern: &str, text: &str) -> bool {
        let mut regex_str = String::from("^");
        for c in pattern.chars() {
            match c {
                '*' => regex_str.push_str(".*"),
                '?' => regex_str.push('.'),
                '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                    regex_str.push('\\');
                    regex_str.push(c);
                }
                _ => regex_str.push(c),
            }
        }
        regex_str.push('$');
        if let Ok(re) = regex::Regex::new(&regex_str) {
            re.is_match(text)
        } else {
            false
        }
    }

    pub fn matches(&self, folder_parts: &[String]) -> bool {
        if !self.is_active() {
            return true;
        }

        let folder_str = if folder_parts.is_empty() {
            "Inbox".to_string()
        } else {
            folder_parts.join("/")
        };

        // 1. Inclusion check
        if !self.include_patterns.is_empty() {
            let mut matched = false;
            for pat in &self.include_patterns {
                if Self::matches_pattern(&folder_str, pat) {
                    matched = true;
                    break;
                }
            }
            if !matched {
                return false;
            }
        }

        // 2. Exclusion check
        if !self.exclude_patterns.is_empty() {
            for pat in &self.exclude_patterns {
                if Self::matches_pattern(&folder_str, pat) {
                    return false;
                }
            }
        }

        true
    }
}

// ----------------------- Query Filter Criteria & Engine -----------------------

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
            let matches_recip = msg
                .to
                .iter()
                .any(|r| r.name.to_lowercase().contains(&q) || r.email.to_lowercase().contains(&q))
                || msg
                    .cc
                    .iter()
                    .any(|r| r.name.to_lowercase().contains(&q) || r.email.to_lowercase().contains(&q));
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
        messages
            .iter()
            .filter(|m| Self::matches(m, criteria))
            .cloned()
            .collect()
    }
}

// ----------------------- Deduplication -----------------------

pub use mail2rust_core::{DedupEngine, DedupStrategy};

// ----------------------- Volume Splitting -----------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitVolume {
    pub volume_index: usize,
    pub file_name: String,
    pub messages: Vec<EmailMessage>,
    pub total_size_bytes: usize,
    pub folder_counts: HashMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SplitMode {
    #[default]
    Size,
    Year,
    Even(usize),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SplitConfig {
    pub max_volume_bytes: usize,
    pub cluster_threads: bool,
    pub file_prefix: String,
    pub target_ext: String,
}

pub struct VolumeSplitter;

impl VolumeSplitter {
    pub fn split(messages: &[EmailMessage], config: &SplitConfig) -> Vec<SplitVolume> {
        Self::split_with_mode(messages, config, &SplitMode::Size)
    }

    pub fn split_with_mode(messages: &[EmailMessage], config: &SplitConfig, mode: &SplitMode) -> Vec<SplitVolume> {
        if messages.is_empty() {
            return Vec::new();
        }

        match mode {
            SplitMode::Year => Self::split_by_year(messages, config),
            SplitMode::Even(splits) => {
                let total_bytes: usize = messages.iter().map(|m| m.size_bytes).sum();
                let target = if *splits > 0 {
                    (total_bytes / splits).max(1)
                } else {
                    config.max_volume_bytes
                };
                let mut even_cfg = config.clone();
                even_cfg.max_volume_bytes = target;
                Self::split_by_size(messages, &even_cfg)
            }
            SplitMode::Size => Self::split_by_size(messages, config),
        }
    }

    fn split_by_year(messages: &[EmailMessage], config: &SplitConfig) -> Vec<SplitVolume> {
        let mut year_groups: BTreeMap<i32, Vec<EmailMessage>> = BTreeMap::new();
        for m in messages {
            year_groups.entry(m.date.year()).or_default().push(m.clone());
        }

        let mut volumes = Vec::new();
        let mut vol_idx = 1;

        for (year, msgs) in year_groups {
            let year_total_bytes: usize = msgs.iter().map(|m| m.size_bytes).sum();
            if year_total_bytes <= config.max_volume_bytes {
                let mut folder_counts = HashMap::new();
                for m in &msgs {
                    let folder_name = m.folder_path.last().cloned().unwrap_or_else(|| "Root".to_string());
                    *folder_counts.entry(folder_name).or_insert(0) += 1;
                }
                volumes.push(SplitVolume {
                    volume_index: vol_idx,
                    file_name: format!("{}_{}.{}", config.file_prefix, year, config.target_ext),
                    messages: msgs,
                    total_size_bytes: year_total_bytes,
                    folder_counts,
                });
                vol_idx += 1;
            } else {
                // Year group exceeds single volume limit -> split into parts
                let mut year_cfg = config.clone();
                year_cfg.file_prefix = format!("{}_{}", config.file_prefix, year);
                let sub_vols = Self::split_by_size(&msgs, &year_cfg);
                for mut sv in sub_vols {
                    sv.volume_index = vol_idx;
                    volumes.push(sv);
                    vol_idx += 1;
                }
            }
        }

        volumes
    }

    fn split_by_size(messages: &[EmailMessage], config: &SplitConfig) -> Vec<SplitVolume> {
        let mut volumes = Vec::new();
        let mut current_idx = 1;
        let mut current_messages = Vec::new();
        let mut current_bytes = 0usize;
        let mut folder_counts = HashMap::new();

        let mut commit_vol =
            |msgs: &mut Vec<EmailMessage>, bytes: &mut usize, fcounts: &mut HashMap<String, usize>, idx: &mut usize| {
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
                    commit_vol(
                        &mut current_messages,
                        &mut current_bytes,
                        &mut folder_counts,
                        &mut current_idx,
                    );
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
                    commit_vol(
                        &mut current_messages,
                        &mut current_bytes,
                        &mut folder_counts,
                        &mut current_idx,
                    );
                }
                current_messages.push(m.clone());
                current_bytes += m.size_bytes;
                let folder_name = m.folder_path.last().cloned().unwrap_or_else(|| "Root".to_string());
                *folder_counts.entry(folder_name).or_insert(0) += 1;
            }
        }

        commit_vol(
            &mut current_messages,
            &mut current_bytes,
            &mut folder_counts,
            &mut current_idx,
        );
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
    fn test_parse_size_bytes() {
        assert_eq!(parse_size_bytes("20GB").unwrap(), 20 * 1024 * 1024 * 1024);
        assert_eq!(parse_size_bytes("20 gb").unwrap(), 20 * 1024 * 1024 * 1024);
        assert_eq!(parse_size_bytes("500MB").unwrap(), 500 * 1024 * 1024);
        assert_eq!(
            parse_size_bytes("1.5G").unwrap(),
            (1.5 * 1024.0 * 1024.0 * 1024.0) as usize
        );
        assert_eq!(parse_size_bytes("100KB").unwrap(), 100 * 1024);
        assert_eq!(parse_size_bytes("1024B").unwrap(), 1024);
        assert_eq!(parse_size_bytes("0").unwrap(), 0);
        assert_eq!(parse_size_bytes("unlimited").unwrap(), 0);
    }

    #[test]
    fn test_date_filter_iso_and_rfc() {
        let since = parse_cli_datetime("2023-01-01", false).unwrap();
        let until = parse_cli_datetime("2023-12-31", true).unwrap();
        let df = DateFilter {
            since: Some(since),
            until: Some(until),
            start_year: None,
            end_year: None,
        };

        let dt_mid = Utc.with_ymd_and_hms(2023, 6, 15, 12, 0, 0).unwrap();
        assert!(df.matches(&dt_mid));

        let dt_start = Utc.with_ymd_and_hms(2023, 1, 1, 0, 0, 0).unwrap();
        assert!(df.matches(&dt_start));

        let dt_end = Utc.with_ymd_and_hms(2023, 12, 31, 23, 59, 59).unwrap();
        assert!(df.matches(&dt_end));

        let dt_before = Utc.with_ymd_and_hms(2022, 12, 31, 23, 59, 59).unwrap();
        assert!(!df.matches(&dt_before));

        let dt_after = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 1).unwrap();
        assert!(!df.matches(&dt_after));
    }

    #[test]
    fn test_folder_filter_wildcards() {
        let ff_ex = FolderFilter::new(
            vec![],
            vec!["*Trash*".to_string(), "*Spam*".to_string(), "Junk*".to_string()],
        );
        assert!(ff_ex.matches(&["Inbox".to_string()]));
        assert!(ff_ex.matches(&["Work".to_string(), "Projects".to_string()]));
        assert!(!ff_ex.matches(&["Trash".to_string()]));
        assert!(!ff_ex.matches(&["[Gmail]".to_string(), "Trash".to_string()]));
        assert!(!ff_ex.matches(&["Spam".to_string()]));

        let ff_both = FolderFilter::new(
            vec!["Work/*".to_string(), "Archive/2023*".to_string()],
            vec!["*Drafts*".to_string()],
        );
        assert!(ff_both.matches(&["Work".to_string(), "Marketing".to_string()]));
        assert!(ff_both.matches(&["Archive".to_string(), "2023-Q1".to_string()]));
        assert!(!ff_both.matches(&["Inbox".to_string()]));
        assert!(!ff_both.matches(&["Work".to_string(), "Drafts".to_string()]));
    }

    #[test]
    fn test_volume_splitter_by_year() {
        let mut m1 = make_test_msg("1", "c1", 1000, "M1");
        m1.date = Utc.with_ymd_and_hms(2022, 5, 1, 0, 0, 0).unwrap();
        let mut m2 = make_test_msg("2", "c2", 2000, "M2");
        m2.date = Utc.with_ymd_and_hms(2023, 7, 1, 0, 0, 0).unwrap();

        let cfg = SplitConfig {
            max_volume_bytes: 50000,
            cluster_threads: false,
            file_prefix: "Backup".to_string(),
            target_ext: "pst".to_string(),
        };

        let volumes = VolumeSplitter::split_with_mode(&[m1, m2], &cfg, &SplitMode::Year);
        assert_eq!(volumes.len(), 2);
        assert_eq!(volumes[0].file_name, "Backup_2022.pst");
        assert_eq!(volumes[1].file_name, "Backup_2023.pst");
    }
}
