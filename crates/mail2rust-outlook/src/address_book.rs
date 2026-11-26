use crate::error::{OutlookError, OutlookResult};
use crate::nk2::{Nk2Archive, query_provider_to_json};
use crate::oab::parse_oab_file;
use crate::pst_ext::extract_pst_contacts;
use chrono::{DateTime, Utc};
use guap_mcp::{ActionCommand, ActionRiskLevel, McpToolError};
use guap_table::{
    CellValue, ColumnDef, ColumnType, Row, TableData, TableError, TableProvider, TableQuery, TableSchema, apply_query,
};
use mail2rust_core::{FormatSniffer, MailboxFormat, VcardReader};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactRecord {
    pub row_id: u64,
    pub entry_type: String,
    pub display_name: String,
    pub email1: String,
    pub email2: String,
    pub company: String,
    pub job_title: String,
    pub phone: String,
    pub department: String,
    pub modified_time: Option<DateTime<Utc>>,
    pub mobile_phone: String,
    pub office_location: String,
    pub notes: String,
    pub members: Vec<(String, String)>,
    pub source_file: String,
}

pub fn collect_contacts(source: &Path) -> OutlookResult<Vec<ContactRecord>> {
    if !source.exists() {
        return Err(OutlookError::NotFound(format!(
            "Contacts source does not exist: {}",
            source.display()
        )));
    }
    let source_str = source.display().to_string();
    let ext = source
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let fmt = FormatSniffer::sniff_path(source);

    if fmt == MailboxFormat::Pst || fmt == MailboxFormat::Ost {
        let raw = extract_pst_contacts(source)?;
        let out = raw
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                let entry_type = if c.message_class.to_ascii_uppercase().contains("DISTLIST") {
                    "DistList"
                } else {
                    "Contact"
                };
                ContactRecord {
                    row_id: i as u64,
                    entry_type: entry_type.to_string(),
                    display_name: c.display_name,
                    email1: c.email1,
                    email2: c.email2,
                    company: c.company,
                    job_title: c.job_title,
                    phone: c.phone,
                    department: c.department,
                    modified_time: c.modified_time,
                    mobile_phone: c.mobile_phone,
                    office_location: c.office_location,
                    notes: c.notes,
                    members: c.members,
                    source_file: source_str.clone(),
                }
            })
            .collect();
        return Ok(out);
    }

    if ext == "oab" {
        let oab_entries = parse_oab_file(source)?;
        return Ok(oab_entries
            .into_iter()
            .enumerate()
            .map(|(i, e)| ContactRecord {
                row_id: i as u64,
                entry_type: "OAB".to_string(),
                display_name: e.display_name,
                email1: e.email1,
                email2: e.email2,
                company: e.company,
                job_title: e.job_title,
                phone: e.phone,
                department: e.department,
                modified_time: None,
                mobile_phone: String::new(),
                office_location: String::new(),
                notes: String::new(),
                members: Vec::new(),
                source_file: source_str.clone(),
            })
            .collect());
    }

    if ext == "nk2" || ext == "dat" {
        let bytes = fs::read(source)?;
        if Nk2Archive::is_nk2_bytes(&bytes) {
            let arc = Nk2Archive::parse(&bytes)?;
            return Ok(arc
                .entries
                .into_iter()
                .enumerate()
                .map(|(i, e)| ContactRecord {
                    row_id: i as u64,
                    entry_type: "NK2".to_string(),
                    display_name: e.display_name,
                    email1: if e.smtp_address.is_empty() {
                        e.email_address
                    } else {
                        e.smtp_address
                    },
                    email2: String::new(),
                    company: String::new(),
                    job_title: String::new(),
                    phone: String::new(),
                    department: String::new(),
                    modified_time: e.modified_time,
                    mobile_phone: String::new(),
                    office_location: String::new(),
                    notes: String::new(),
                    members: Vec::new(),
                    source_file: source_str.clone(),
                })
                .collect());
        }
    }

    if fmt == MailboxFormat::Vcard || ext == "vcf" || ext == "vcard" {
        let bytes = fs::read(source)?;
        let msgs = VcardReader::parse(&bytes, vec!["Contacts".to_string()])
            .map_err(|e| OutlookError::ParseError(e.to_string()))?;
        return Ok(msgs
            .into_iter()
            .enumerate()
            .map(|(i, m)| {
                let company = m
                    .mapi_properties
                    .iter()
                    .find(|p| p.tag == 0x3A16_001F)
                    .map(|p| p.value.clone())
                    .unwrap_or_default();
                let job_title = m
                    .mapi_properties
                    .iter()
                    .find(|p| p.tag == 0x3A17_001F)
                    .map(|p| p.value.clone())
                    .unwrap_or_default();
                let phone = m
                    .body_text
                    .lines()
                    .find_map(|l| l.strip_prefix("Phone: "))
                    .unwrap_or("")
                    .to_string();
                ContactRecord {
                    row_id: i as u64,
                    entry_type: "vCard".to_string(),
                    display_name: m.subject,
                    email1: m.from.email,
                    email2: String::new(),
                    company,
                    job_title,
                    phone,
                    department: String::new(),
                    modified_time: None,
                    mobile_phone: String::new(),
                    office_location: String::new(),
                    notes: m.body_text,
                    members: Vec::new(),
                    source_file: source_str.clone(),
                }
            })
            .collect());
    }

    Err(OutlookError::UnsupportedFeature(format!(
        "Unsupported address book source format: {}",
        source.display()
    )))
}

/// Serializes contacts to RFC 2426 vCard 3.0 text (round-trip compatible with `VcardReader::parse`).
pub fn export_contacts_vcf(contacts: &[ContactRecord]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for c in contacts {
        let _ = writeln!(out, "BEGIN:VCARD");
        let _ = writeln!(out, "VERSION:3.0");
        let _ = writeln!(out, "FN:{}", c.display_name);
        if !c.email1.is_empty() {
            let _ = writeln!(out, "EMAIL;TYPE=INTERNET,PREF:{}", c.email1);
        }
        if !c.email2.is_empty() {
            let _ = writeln!(out, "EMAIL;TYPE=INTERNET:{}", c.email2);
        }
        if !c.company.is_empty() || !c.department.is_empty() {
            if c.department.is_empty() {
                let _ = writeln!(out, "ORG:{}", c.company);
            } else {
                let _ = writeln!(out, "ORG:{};{}", c.company, c.department);
            }
        }
        if !c.job_title.is_empty() {
            let _ = writeln!(out, "TITLE:{}", c.job_title);
        }
        if !c.phone.is_empty() {
            let _ = writeln!(out, "TEL;TYPE=WORK,VOICE:{}", c.phone);
        }
        if !c.mobile_phone.is_empty() {
            let _ = writeln!(out, "TEL;TYPE=CELL:{}", c.mobile_phone);
        }
        if !c.notes.is_empty() {
            let clean_note = c.notes.replace(['\r', '\n'], " ");
            let _ = writeln!(out, "NOTE:{clean_note}");
        }
        let _ = writeln!(out, "END:VCARD");
        let _ = writeln!(out);
    }
    out
}

/// `guap-table::TableProvider` for Outlook contacts & address book (`mail2rust_outlook_contacts`).
pub struct ContactsTableProvider {
    schema: TableSchema,
    rows: Vec<Row>,
}

impl ContactsTableProvider {
    pub fn from_records(records: &[ContactRecord]) -> Self {
        let schema = TableSchema::new(
            "mail2rust_outlook_contacts",
            "Outlook Address Book & Contacts",
            vec![
                ColumnDef::new("display_name", "Display Name", ColumnType::Text).with_width_hint(180),
                ColumnDef::new("email1", "Email 1", ColumnType::Text).with_width_hint(200),
                ColumnDef::new("email2", "Email 2", ColumnType::Text).with_width_hint(180),
                ColumnDef::new("company", "Company", ColumnType::Text).with_width_hint(160),
                ColumnDef::new("job_title", "Job Title", ColumnType::Text).with_width_hint(150),
                ColumnDef::new("phone", "Phone", ColumnType::Text).with_width_hint(130),
                ColumnDef::new("department", "Department", ColumnType::Text).with_width_hint(140),
                ColumnDef::new("modified_time", "Modified", ColumnType::Timestamp).with_width_hint(160),
            ],
        )
        .with_default_sort(0, guap_table::SortDirection::Asc);

        let rows = records
            .iter()
            .map(|c| {
                let detail = json!({
                    "entry_type": c.entry_type,
                    "members": c.members,
                    "mobile_phone": c.mobile_phone,
                    "office_location": c.office_location,
                    "notes": c.notes,
                    "source_file": c.source_file,
                });
                Row::new(
                    c.row_id,
                    vec![
                        CellValue::Text(c.display_name.clone()),
                        CellValue::Text(c.email1.clone()),
                        CellValue::Text(c.email2.clone()),
                        CellValue::Text(c.company.clone()),
                        CellValue::Text(c.job_title.clone()),
                        CellValue::Text(c.phone.clone()),
                        CellValue::Text(c.department.clone()),
                        c.modified_time.map_or(CellValue::Null, CellValue::Timestamp),
                    ],
                )
                .with_detail_json(detail)
            })
            .collect();

        Self { schema, rows }
    }

    pub fn from_source(source: &Path) -> OutlookResult<Self> {
        let recs = collect_contacts(source)?;
        Ok(Self::from_records(&recs))
    }
}

impl TableProvider for ContactsTableProvider {
    fn tool_id(&self) -> &'static str {
        "mail2rust_outlook_contacts"
    }

    fn description(&self) -> &'static str {
        "Decode Outlook contacts & distribution lists from PST/OST, uncompressed OAB, NK2, or vCard"
    }

    fn schema(&self) -> TableSchema {
        self.schema.clone()
    }

    fn query(&self, req: &TableQuery) -> Result<TableData, TableError> {
        apply_query(&self.schema, &self.rows, req)
    }
}

/// Read-only MCP `ActionCommand` for `mail2rust_outlook_contacts`.
pub struct OutlookContactsCommand;

impl ActionCommand for OutlookContactsCommand {
    fn command_id(&self) -> &'static str {
        "mail2rust_outlook_contacts"
    }

    fn description(&self) -> &'static str {
        "Read and decode Outlook contacts and distribution lists from PST/OST, OAB, NK2, or vCard (read-only)."
    }

    fn risk_level(&self) -> ActionRiskLevel {
        ActionRiskLevel::ReadOnly
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "description": "Path to PST/OST, OAB, NK2, or VCF file" },
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
        let provider = ContactsTableProvider::from_source(Path::new(src))
            .map_err(|e| McpToolError::ExecutionFailed(e.to_string()))?;
        query_provider_to_json(&provider, args)
    }
}
