use crate::error::OutlookResult;
use crate::nk2::query_provider_to_json;
use crate::pst_ext::{is_mail_message, load_messages_from_source};
use chrono::{DateTime, Utc};
use guap_mcp::{ActionCommand, ActionRiskLevel, McpToolError};
use guap_table::{
    CellValue, ColumnDef, ColumnType, Row, TableData, TableError, TableProvider, TableQuery, TableSchema, apply_query,
};
use mail2rust_core::EmailMessage;
use mail2rust_pst::{MSGFLAG_FROMME, PR_MESSAGE_FLAGS};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StatsGroupBy {
    #[default]
    Correspondent,
    Domain,
    Folder,
    Month,
}

impl StatsGroupBy {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "correspondent" | "sender" | "email" => Some(Self::Correspondent),
            "domain" => Some(Self::Domain),
            "folder" => Some(Self::Folder),
            "month" | "date" => Some(Self::Month),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatsOptions {
    pub group_by: StatsGroupBy,
    pub self_emails: Vec<String>,
    pub self_domains: Vec<String>,
    pub dedup_by_message_id: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatRecord {
    pub row_id: u64,
    pub correspondent: String,
    pub email: String,
    pub domain: String,
    pub sent_count: u64,
    pub received_count: u64,
    pub total_count: u64,
    pub total_bytes: u64,
    pub first_message: DateTime<Utc>,
    pub last_message: DateTime<Utc>,
    pub attachment_count: u64,
    pub attachment_bytes: u64,
    pub folders: BTreeMap<String, u64>,
    pub monthly_counts: BTreeMap<String, u64>,
}

struct StatAcc {
    correspondent: String,
    email: String,
    domain: String,
    sent_count: u64,
    received_count: u64,
    total_count: u64,
    total_bytes: u64,
    first_message: DateTime<Utc>,
    last_message: DateTime<Utc>,
    attachment_count: u64,
    attachment_bytes: u64,
    folders: BTreeMap<String, u64>,
    monthly_counts: BTreeMap<String, u64>,
}

pub fn compute_mailbox_stats(source: &Path, opts: &StatsOptions) -> OutlookResult<Vec<StatRecord>> {
    let messages = load_messages_from_source(source)?;
    Ok(aggregate_messages_stats(&messages, opts))
}

pub fn aggregate_messages_stats(messages: &[EmailMessage], opts: &StatsOptions) -> Vec<StatRecord> {
    let self_emails: HashSet<String> = opts.self_emails.iter().map(|s| s.trim().to_ascii_lowercase()).collect();
    let self_domains: HashSet<String> = opts
        .self_domains
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();
    let mut seen_msg_ids: HashSet<String> = HashSet::new();
    let mut buckets: BTreeMap<String, StatAcc> = BTreeMap::new();

    for msg in messages {
        if !is_mail_message(msg) {
            continue;
        }
        if opts.dedup_by_message_id && !msg.message_id.is_empty() && !seen_msg_ids.insert(msg.message_id.clone()) {
            continue;
        }
        let folder = if msg.folder_path.is_empty() {
            "Inbox".to_string()
        } else {
            msg.folder_path.join("/")
        };
        let is_sent = classify_sent(msg, &folder, &self_emails, &self_domains);
        let month_key = msg.date.format("%Y-%m").to_string();
        let msg_bytes = (msg.size_bytes.max(msg.body_text.len())
            + msg
                .attachments
                .iter()
                .map(|a| a.size_bytes.max(a.data.len()))
                .sum::<usize>()) as u64;
        let att_count = msg.attachments.len() as u64;
        let att_bytes: u64 = msg
            .attachments
            .iter()
            .map(|a| a.size_bytes.max(a.data.len()) as u64)
            .sum();

        let keys: Vec<(String, String, String, String)> = match opts.group_by {
            StatsGroupBy::Correspondent => {
                if is_sent && !msg.to.is_empty() {
                    msg.to
                        .iter()
                        .map(|r| {
                            let em = r.email.trim().to_ascii_lowercase();
                            let dom = extract_domain(&em);
                            let name = if r.name.is_empty() { em.clone() } else { r.name.clone() };
                            (em.clone(), name, em, dom)
                        })
                        .collect()
                } else {
                    let em = msg.from.email.trim().to_ascii_lowercase();
                    let dom = extract_domain(&em);
                    let name = if msg.from.name.is_empty() {
                        em.clone()
                    } else {
                        msg.from.name.clone()
                    };
                    vec![(em.clone(), name, em, dom)]
                }
            }
            StatsGroupBy::Domain => {
                if is_sent && !msg.to.is_empty() {
                    let mut seen_domains = HashSet::new();
                    msg.to
                        .iter()
                        .filter_map(|r| {
                            let em = r.email.trim().to_ascii_lowercase();
                            let dom = extract_domain(&em);
                            let label = if dom.is_empty() { "(unknown)".to_string() } else { dom };
                            if seen_domains.insert(label.clone()) {
                                Some((label.clone(), label.clone(), format!("*@{label}"), label))
                            } else {
                                None
                            }
                        })
                        .collect()
                } else {
                    let em = msg.from.email.trim().to_ascii_lowercase();
                    let dom = extract_domain(&em);
                    let label = if dom.is_empty() { "(unknown)".to_string() } else { dom };
                    vec![(label.clone(), label.clone(), format!("*@{label}"), label)]
                }
            }
            StatsGroupBy::Folder => {
                vec![(folder.clone(), folder.clone(), String::new(), String::new())]
            }
            StatsGroupBy::Month => {
                vec![(month_key.clone(), month_key.clone(), String::new(), String::new())]
            }
        };

        for (key, display_name, email, domain) in keys {
            let entry = buckets.entry(key).or_insert_with(|| StatAcc {
                correspondent: display_name.clone(),
                email: email.clone(),
                domain: domain.clone(),
                sent_count: 0,
                received_count: 0,
                total_count: 0,
                total_bytes: 0,
                first_message: msg.date,
                last_message: msg.date,
                attachment_count: 0,
                attachment_bytes: 0,
                folders: BTreeMap::new(),
                monthly_counts: BTreeMap::new(),
            });
            if is_sent {
                entry.sent_count += 1;
            } else {
                entry.received_count += 1;
            }
            entry.total_count += 1;
            entry.total_bytes += msg_bytes;
            entry.attachment_count += att_count;
            entry.attachment_bytes += att_bytes;
            if msg.date < entry.first_message {
                entry.first_message = msg.date;
            }
            if msg.date > entry.last_message {
                entry.last_message = msg.date;
            }
            *entry.folders.entry(folder.clone()).or_insert(0) += 1;
            *entry.monthly_counts.entry(month_key.clone()).or_insert(0) += 1;
        }
    }

    let mut out: Vec<StatRecord> = buckets
        .into_values()
        .enumerate()
        .map(|(i, acc)| StatRecord {
            row_id: i as u64,
            correspondent: acc.correspondent,
            email: acc.email,
            domain: acc.domain,
            sent_count: acc.sent_count,
            received_count: acc.received_count,
            total_count: acc.total_count,
            total_bytes: acc.total_bytes,
            first_message: acc.first_message,
            last_message: acc.last_message,
            attachment_count: acc.attachment_count,
            attachment_bytes: acc.attachment_bytes,
            folders: acc.folders,
            monthly_counts: acc.monthly_counts,
        })
        .collect();

    out.sort_by(|a, b| {
        b.total_count
            .cmp(&a.total_count)
            .then_with(|| a.correspondent.cmp(&b.correspondent))
    });
    for (idx, r) in out.iter_mut().enumerate() {
        r.row_id = idx as u64;
    }
    out
}

fn classify_sent(
    msg: &EmailMessage,
    folder: &str,
    self_emails: &HashSet<String>,
    self_domains: &HashSet<String>,
) -> bool {
    let f_lower = folder.to_ascii_lowercase();
    if f_lower == "sent"
        || f_lower.ends_with("/sent")
        || f_lower.contains("sent items")
        || f_lower.contains("sent mail")
    {
        return true;
    }
    if msg
        .mapi_properties
        .iter()
        .find(|p| p.tag == PR_MESSAGE_FLAGS)
        .and_then(|p| p.value.parse::<u32>().ok())
        .is_some_and(|f| (f & MSGFLAG_FROMME) != 0)
    {
        return true;
    }
    let from_email = msg.from.email.trim().to_ascii_lowercase();
    if self_emails.contains(&from_email) {
        return true;
    }
    let from_dom = extract_domain(&from_email);
    !from_dom.is_empty() && self_domains.contains(&from_dom)
}

fn extract_domain(email: &str) -> String {
    email
        .rsplit_once('@')
        .map(|(_, d)| d.trim().to_ascii_lowercase())
        .unwrap_or_default()
}

/// `guap-table::TableProvider` for Outlook communication stats (`mail2rust_outlook_stats`).
pub struct StatsTableProvider {
    schema: TableSchema,
    rows: Vec<Row>,
}

impl StatsTableProvider {
    pub fn from_records(records: &[StatRecord]) -> Self {
        let schema = TableSchema::new(
            "mail2rust_outlook_stats",
            "Outlook Communication Statistics",
            vec![
                ColumnDef::new("correspondent", "Correspondent / Bucket", ColumnType::Text).with_width_hint(200),
                ColumnDef::new("email", "Email", ColumnType::Text).with_width_hint(210),
                ColumnDef::new("sent_count", "Sent", ColumnType::Uint).with_width_hint(75),
                ColumnDef::new("received_count", "Received", ColumnType::Uint).with_width_hint(85),
                ColumnDef::new("total_count", "Total", ColumnType::Uint).with_width_hint(75),
                ColumnDef::new("total_bytes", "Total Size", ColumnType::BytesSize).with_width_hint(100),
                ColumnDef::new("first_message", "First Message", ColumnType::Timestamp).with_width_hint(160),
                ColumnDef::new("last_message", "Last Message", ColumnType::Timestamp).with_width_hint(160),
            ],
        )
        .with_default_sort(4, guap_table::SortDirection::Desc);

        let rows = records
            .iter()
            .map(|r| {
                let detail = json!({
                    "domain": r.domain,
                    "attachment_count": r.attachment_count,
                    "attachment_bytes": r.attachment_bytes,
                    "folders": r.folders,
                    "monthly_counts": r.monthly_counts,
                });
                Row::new(
                    r.row_id,
                    vec![
                        CellValue::Text(r.correspondent.clone()),
                        CellValue::Text(r.email.clone()),
                        CellValue::Uint(r.sent_count),
                        CellValue::Uint(r.received_count),
                        CellValue::Uint(r.total_count),
                        CellValue::Bytes(r.total_bytes),
                        CellValue::Timestamp(r.first_message),
                        CellValue::Timestamp(r.last_message),
                    ],
                )
                .with_detail_json(detail)
            })
            .collect();

        Self { schema, rows }
    }

    pub fn from_source(source: &Path, opts: &StatsOptions) -> OutlookResult<Self> {
        let recs = compute_mailbox_stats(source, opts)?;
        Ok(Self::from_records(&recs))
    }
}

impl TableProvider for StatsTableProvider {
    fn tool_id(&self) -> &'static str {
        "mail2rust_outlook_stats"
    }

    fn description(&self) -> &'static str {
        "Aggregate sent/received message counts, sizes, and date ranges by correspondent, domain, folder, or month"
    }

    fn schema(&self) -> TableSchema {
        self.schema.clone()
    }

    fn query(&self, req: &TableQuery) -> Result<TableData, TableError> {
        apply_query(&self.schema, &self.rows, req)
    }
}

/// Read-only MCP `ActionCommand` for `mail2rust_outlook_stats`.
pub struct OutlookStatsCommand;

impl ActionCommand for OutlookStatsCommand {
    fn command_id(&self) -> &'static str {
        "mail2rust_outlook_stats"
    }

    fn description(&self) -> &'static str {
        "Aggregate PST/OST/MSG/MBOX/EML communication statistics by correspondent, domain, folder, or month (read-only)."
    }

    fn risk_level(&self) -> ActionRiskLevel {
        ActionRiskLevel::ReadOnly
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "description": "Path to PST/OST/MSG/MBOX/EML file or directory" },
                "group_by": { "type": "string", "enum": ["correspondent", "domain", "folder", "month"], "default": "correspondent" },
                "self_emails": { "type": "array", "items": { "type": "string" } },
                "filter": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1 },
                "offset": { "type": "integer", "minimum": 0 }
            },
            "required": ["source"]
        })
    }

    fn execute(&self, args: &Value, _dry_run: bool) -> Result<Value, McpToolError> {
        let src = args
            .get("source")
            .and_then(Value::as_str)
            .ok_or_else(|| McpToolError::InvalidParams("Missing required 'source' argument".to_string()))?;
        let group_by = args
            .get("group_by")
            .and_then(Value::as_str)
            .and_then(StatsGroupBy::parse)
            .unwrap_or(StatsGroupBy::Correspondent);
        let self_emails = args
            .get("self_emails")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default();
        let opts = StatsOptions {
            group_by,
            self_emails,
            self_domains: Vec::new(),
            dedup_by_message_id: false,
        };
        let provider = StatsTableProvider::from_source(Path::new(src), &opts)
            .map_err(|e| McpToolError::ExecutionFailed(e.to_string()))?;
        query_provider_to_json(&provider, args)
    }
}
