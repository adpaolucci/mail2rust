use crate::cfb_ole2::{CfbContainer, strip_msg_cfb_bytes};
use crate::error::{OutlookError, OutlookResult};
use crate::nk2::query_provider_to_json;
use crate::pst_ext::{load_messages_from_source, strip_pst_to_file};
use chrono::{DateTime, Utc};
use guap_export::{CustodyLeaf, CustodyManifest, build_custody_manifest, sha256_hex, write_custody_manifest_json};
use guap_mcp::{ActionCommand, ActionRiskLevel, McpToolError};
use guap_table::{
    CellValue, ColumnDef, ColumnType, Row, TableData, TableError, TableProvider, TableQuery, TableSchema, apply_query,
};
use mail2rust_core::{FormatSniffer, MailboxFormat, sanitize_filename};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttachmentFilter {
    pub extensions: Vec<String>,
    pub exclude_extensions: Vec<String>,
    pub mime_pattern: Option<String>,
    pub min_size: Option<usize>,
    pub max_size: Option<usize>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub sender_pattern: Option<String>,
    pub filename_pattern: Option<String>,
    pub include_inline: bool,
}

impl AttachmentFilter {
    pub fn matches(&self, rec: &AttachmentRecord) -> bool {
        if !self.include_inline && rec.is_inline {
            return false;
        }
        if !self.extensions.is_empty() {
            let ext = rec.extension.to_ascii_lowercase();
            if !self
                .extensions
                .iter()
                .any(|e| e.trim_start_matches('.').eq_ignore_ascii_case(&ext))
            {
                return false;
            }
        }
        if self
            .exclude_extensions
            .iter()
            .any(|e| e.trim_start_matches('.').eq_ignore_ascii_case(&rec.extension))
        {
            return false;
        }
        if let Some(ref mp) = self.mime_pattern
            && !rec.mime_type.to_ascii_lowercase().contains(&mp.to_ascii_lowercase())
        {
            return false;
        }
        if let Some(min) = self.min_size
            && rec.size_bytes < min
        {
            return false;
        }
        if let Some(max) = self.max_size
            && rec.size_bytes > max
        {
            return false;
        }
        if let Some(since) = self.since
            && rec.received_time < since
        {
            return false;
        }
        if let Some(until) = self.until
            && rec.received_time > until
        {
            return false;
        }
        if let Some(ref sp) = self.sender_pattern {
            let needle = sp.to_ascii_lowercase();
            if !rec.from.to_ascii_lowercase().contains(&needle) {
                return false;
            }
        }
        if let Some(ref fp) = self.filename_pattern {
            if let Ok(re) = Regex::new(fp) {
                if !re.is_match(&rec.attachment_name) {
                    return false;
                }
            } else if !rec
                .attachment_name
                .to_ascii_lowercase()
                .contains(&fp.to_ascii_lowercase())
            {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentRecord {
    pub row_id: u64,
    pub message_id: String,
    pub message_subject: String,
    pub from: String,
    pub received_time: DateTime<Utc>,
    pub folder: String,
    pub attachment_name: String,
    pub extension: String,
    pub mime_type: String,
    pub size_bytes: usize,
    pub sha256: String,
    pub content_id: Option<String>,
    pub is_inline: bool,
    #[serde(skip)]
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub extracted_files: Vec<PathBuf>,
    pub total_bytes: u64,
    pub manifest: CustodyManifest,
    pub manifest_path: PathBuf,
}

pub fn collect_attachments(source: &Path, filter: &AttachmentFilter) -> OutlookResult<Vec<AttachmentRecord>> {
    let messages = load_messages_from_source(source)?;
    let mut records = Vec::new();
    let mut row_id = 0u64;

    for msg in messages {
        let folder = if msg.folder_path.is_empty() {
            "Inbox".to_string()
        } else {
            msg.folder_path.join("/")
        };
        let from_display = msg.from.to_string();

        for att in msg.attachments {
            let ext = Path::new(&att.filename)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let sha256 = if !att.sha256.is_empty() {
                att.sha256.clone()
            } else {
                sha256_hex(&att.data)
            };
            let is_inline = att.content_id.is_some();

            let rec = AttachmentRecord {
                row_id,
                message_id: msg.message_id.clone(),
                message_subject: msg.subject.clone(),
                from: from_display.clone(),
                received_time: msg.date,
                folder: folder.clone(),
                attachment_name: att.filename,
                extension: ext,
                mime_type: att.mime_type,
                size_bytes: att.data.len().max(att.size_bytes),
                sha256,
                content_id: att.content_id,
                is_inline,
                data: att.data,
            };
            if filter.matches(&rec) {
                records.push(rec);
                row_id += 1;
            }
        }
    }

    Ok(records)
}

pub fn extract_attachments_to_dir(records: &[AttachmentRecord], dest_dir: &Path) -> OutlookResult<ExtractionResult> {
    fs::create_dir_all(dest_dir)?;
    let mut used_names: HashSet<String> = HashSet::new();
    let mut extracted_files = Vec::with_capacity(records.len());
    let mut leaves = Vec::with_capacity(records.len());
    let mut total_bytes = 0u64;

    for (idx, rec) in records.iter().enumerate() {
        let safe = sanitize_filename(&rec.attachment_name, 180);
        let sha256 = guap_export::sha256_hex(&rec.data);
        let sha512 = guap_export::sha512_hex(&rec.data);
        let hash_pfx = &sha256[..sha256.len().min(8)];

        let candidate_name = if !dest_dir.join(&safe).exists() && used_names.insert(safe.to_ascii_lowercase()) {
            safe
        } else {
            let p = Path::new(&safe);
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("attachment");
            let ext_suffix = p
                .extension()
                .and_then(|s| s.to_str())
                .filter(|s| !s.is_empty())
                .map(|ext| format!(".{ext}"))
                .unwrap_or_default();
            let base_dedup = format!("{stem}_{hash_pfx}{ext_suffix}");
            if !dest_dir.join(&base_dedup).exists() && used_names.insert(base_dedup.to_ascii_lowercase()) {
                base_dedup
            } else {
                let mut counter = 2usize;
                loop {
                    let numbered = format!("{stem}_{hash_pfx}_{counter}{ext_suffix}");
                    if !dest_dir.join(&numbered).exists() && used_names.insert(numbered.to_ascii_lowercase()) {
                        break numbered;
                    }
                    counter += 1;
                }
            }
        };

        let out_path = dest_dir.join(&candidate_name);
        fs::write(&out_path, &rec.data)?;
        let sz = rec.data.len() as u64;
        total_bytes += sz;

        leaves.push(CustodyLeaf {
            index: idx,
            path: out_path.display().to_string(),
            size_bytes: sz,
            sha256,
            sha512,
        });
        extracted_files.push(out_path);
    }

    let manifest = build_custody_manifest("mail2rust_outlook_attach", env!("CARGO_PKG_VERSION"), leaves);
    let manifest_path = dest_dir.join("custody_manifest.json");
    write_custody_manifest_json(&manifest, &manifest_path).map_err(|e| OutlookError::Io(std::io::Error::other(e)))?;

    Ok(ExtractionResult {
        extracted_files,
        total_bytes,
        manifest,
        manifest_path,
    })
}

pub fn strip_attachments_from_source(
    source: &Path,
    output: Option<&Path>,
    filter: &AttachmentFilter,
    in_place: bool,
    dry_run: bool,
) -> OutlookResult<(PathBuf, usize)> {
    let records = collect_attachments(source, filter)?;
    let dest = match output {
        Some(p) => p.to_path_buf(),
        None if in_place => source.to_path_buf(),
        None => {
            let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("mailbox");
            let ext = source.extension().and_then(|s| s.to_str()).unwrap_or("pst");
            source.with_file_name(format!("{stem}_stripped.{ext}"))
        }
    };

    if dry_run {
        return Ok((dest, records.len()));
    }

    let names: HashSet<String> = records.iter().map(|r| r.attachment_name.to_ascii_lowercase()).collect();
    if names.is_empty() && source == dest {
        return Ok((dest, 0));
    }

    let fmt = FormatSniffer::sniff_path(source);
    match fmt {
        MailboxFormat::Pst | MailboxFormat::Ost => {
            let removed = strip_pst_to_file(source, &dest, &names)?;
            Ok((dest, removed))
        }
        MailboxFormat::Msg => {
            let raw = fs::read(source)?;
            let (stripped_bytes, removed) = strip_msg_cfb_bytes(&raw, &names)?;
            CfbContainer::parse(&stripped_bytes)?; // validate rewritten CFB
            let tmp = dest.with_extension("msg.tmp");
            fs::write(&tmp, &stripped_bytes)?;
            if source == dest && dest.exists() {
                fs::copy(dest.as_path(), dest.with_extension("msg.bak"))?;
            }
            if dest.exists() {
                fs::remove_file(&dest)?;
            }
            fs::rename(&tmp, &dest)?;
            Ok((dest, removed))
        }
        other => Err(OutlookError::UnsupportedFeature(format!(
            "Attachment stripping is supported for PST/OST/MSG containers, not {other:?}"
        ))),
    }
}

/// `guap-table::TableProvider` for Outlook attachment inventory (`mail2rust_outlook_attach`).
pub struct AttachTableProvider {
    schema: TableSchema,
    rows: Vec<Row>,
}

impl AttachTableProvider {
    pub fn from_records(records: &[AttachmentRecord]) -> Self {
        let schema = TableSchema::new(
            "mail2rust_outlook_attach",
            "Outlook Attachment Inventory",
            vec![
                ColumnDef::new("message_subject", "Subject", ColumnType::Text).with_width_hint(220),
                ColumnDef::new("from", "From", ColumnType::Text).with_width_hint(180),
                ColumnDef::new("received_time", "Received", ColumnType::Timestamp).with_width_hint(160),
                ColumnDef::new("folder", "Folder", ColumnType::Text).with_width_hint(120),
                ColumnDef::new("attachment_name", "Attachment", ColumnType::Text).with_width_hint(180),
                ColumnDef::new("extension", "Ext", ColumnType::Text).with_width_hint(65),
                ColumnDef::new("mime_type", "MIME Type", ColumnType::Text).with_width_hint(130),
                ColumnDef::new("size_bytes", "Size", ColumnType::BytesSize).with_width_hint(90),
                ColumnDef::new("sha256", "SHA-256", ColumnType::Text).with_width_hint(200),
            ],
        )
        .with_default_sort(2, guap_table::SortDirection::Desc);

        let rows = records
            .iter()
            .map(|r| {
                let preview = r.data.iter().take(4096).copied().collect::<Vec<u8>>();
                let detail = json!({
                    "message_id": r.message_id,
                    "content_id": r.content_id,
                    "is_inline": r.is_inline,
                    "size_bytes": r.size_bytes,
                    "sha256": r.sha256,
                });
                Row::new(
                    r.row_id,
                    vec![
                        CellValue::Text(r.message_subject.clone()),
                        CellValue::Text(r.from.clone()),
                        CellValue::Timestamp(r.received_time),
                        CellValue::Text(r.folder.clone()),
                        CellValue::Text(r.attachment_name.clone()),
                        CellValue::Text(r.extension.clone()),
                        CellValue::Text(r.mime_type.clone()),
                        CellValue::Bytes(r.size_bytes as u64),
                        CellValue::Text(r.sha256.clone()),
                    ],
                )
                .with_detail_json(detail)
                .with_raw_bytes(preview)
            })
            .collect();

        Self { schema, rows }
    }

    pub fn from_source(source: &Path, filter: &AttachmentFilter) -> OutlookResult<Self> {
        let recs = collect_attachments(source, filter)?;
        Ok(Self::from_records(&recs))
    }
}

impl TableProvider for AttachTableProvider {
    fn tool_id(&self) -> &'static str {
        "mail2rust_outlook_attach"
    }

    fn description(&self) -> &'static str {
        "Enumerate attachments inside PST/OST/MSG/EML/MBOX mailboxes with MIME, size, and SHA-256 digests"
    }

    fn schema(&self) -> TableSchema {
        self.schema.clone()
    }

    fn query(&self, req: &TableQuery) -> Result<TableData, TableError> {
        apply_query(&self.schema, &self.rows, req)
    }
}

/// Read-only MCP `ActionCommand` for `mail2rust_outlook_attach`.
pub struct OutlookAttachCommand;

impl ActionCommand for OutlookAttachCommand {
    fn command_id(&self) -> &'static str {
        "mail2rust_outlook_attach"
    }

    fn description(&self) -> &'static str {
        "List attachments in a PST, OST, MSG, MBOX, or EML source (read-only)."
    }

    fn risk_level(&self) -> ActionRiskLevel {
        ActionRiskLevel::ReadOnly
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "description": "Path to PST/OST/MSG/MBOX/EML file or directory" },
                "extensions": { "type": "array", "items": { "type": "string" } },
                "include_inline": { "type": "boolean", "default": true },
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
        let mut att_filter = AttachmentFilter {
            include_inline: args.get("include_inline").and_then(Value::as_bool).unwrap_or(true),
            ..Default::default()
        };
        if let Some(arr) = args.get("extensions").and_then(Value::as_array) {
            att_filter.extensions = arr.iter().filter_map(Value::as_str).map(str::to_string).collect();
        }
        let provider = AttachTableProvider::from_source(Path::new(src), &att_filter)
            .map_err(|e| McpToolError::ExecutionFailed(e.to_string()))?;
        query_provider_to_json(&provider, args)
    }
}

/// Read-only MCP `ActionCommand` for bulk extracting attachments (`mail2rust_outlook_attach_extract`).
pub struct OutlookAttachExtractCommand;

impl ActionCommand for OutlookAttachExtractCommand {
    fn command_id(&self) -> &'static str {
        "mail2rust_outlook_attach_extract"
    }

    fn description(&self) -> &'static str {
        "Bulk extract filtered attachments from PST/OST/MSG/MBOX/EML into a destination directory with SHA-256 custody manifest (read-only on source)."
    }

    fn risk_level(&self) -> ActionRiskLevel {
        ActionRiskLevel::ReadOnly
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source": { "type": "string" },
                "extract_dir": { "type": "string" },
                "extensions": { "type": "array", "items": { "type": "string" } },
                "include_inline": { "type": "boolean", "default": true }
            },
            "required": ["source", "extract_dir"]
        })
    }

    fn execute(&self, args: &Value, dry_run: bool) -> Result<Value, McpToolError> {
        let src = args
            .get("source")
            .and_then(Value::as_str)
            .ok_or_else(|| McpToolError::InvalidParams("Missing 'source'".to_string()))?;
        let dest = args
            .get("extract_dir")
            .and_then(Value::as_str)
            .ok_or_else(|| McpToolError::InvalidParams("Missing 'extract_dir'".to_string()))?;
        let mut att_filter = AttachmentFilter {
            include_inline: args.get("include_inline").and_then(Value::as_bool).unwrap_or(true),
            ..Default::default()
        };
        if let Some(arr) = args.get("extensions").and_then(Value::as_array) {
            att_filter.extensions = arr.iter().filter_map(Value::as_str).map(str::to_string).collect();
        }
        let recs = collect_attachments(Path::new(src), &att_filter)
            .map_err(|e| McpToolError::ExecutionFailed(e.to_string()))?;
        if dry_run {
            return Ok(json!({
                "status": "dry_run",
                "matched_attachments": recs.len(),
                "total_bytes": recs.iter().map(|r| r.size_bytes as u64).sum::<u64>(),
                "extract_dir": dest,
            }));
        }
        let result = extract_attachments_to_dir(&recs, Path::new(dest))
            .map_err(|e| McpToolError::ExecutionFailed(e.to_string()))?;
        Ok(json!({
            "status": "extracted",
            "extracted_count": result.extracted_files.len(),
            "total_bytes": result.total_bytes,
            "merkle_root_sha256": result.manifest.merkle_root_sha256,
            "manifest_path": result.manifest_path.display().to_string(),
        }))
    }
}

/// Mutating MCP `ActionCommand` for stripping attachments (`mail2rust_outlook_attach_strip`).
pub struct OutlookAttachStripCommand;

impl ActionCommand for OutlookAttachStripCommand {
    fn command_id(&self) -> &'static str {
        "mail2rust_outlook_attach_strip"
    }

    fn description(&self) -> &'static str {
        "Strip matching attachments from a PST/OST/MSG container (mutating; requires AllowMutating + confirm_destructive=true)."
    }

    fn risk_level(&self) -> ActionRiskLevel {
        ActionRiskLevel::Mutating
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source": { "type": "string" },
                "output": { "type": "string" },
                "in_place": { "type": "boolean", "default": false },
                "extensions": { "type": "array", "items": { "type": "string" } },
                "include_inline": { "type": "boolean", "default": true }
            },
            "required": ["source"]
        })
    }

    fn execute(&self, args: &Value, dry_run: bool) -> Result<Value, McpToolError> {
        let src = args
            .get("source")
            .and_then(Value::as_str)
            .ok_or_else(|| McpToolError::InvalidParams("Missing 'source'".to_string()))?;
        let output = args.get("output").and_then(Value::as_str).map(Path::new);
        let in_place = args.get("in_place").and_then(Value::as_bool).unwrap_or(false);
        let mut att_filter = AttachmentFilter {
            include_inline: args.get("include_inline").and_then(Value::as_bool).unwrap_or(true),
            ..Default::default()
        };
        if let Some(arr) = args.get("extensions").and_then(Value::as_array) {
            att_filter.extensions = arr.iter().filter_map(Value::as_str).map(str::to_string).collect();
        }
        let (dest, count) = strip_attachments_from_source(Path::new(src), output, &att_filter, in_place, dry_run)
            .map_err(|e| McpToolError::ExecutionFailed(e.to_string()))?;
        Ok(json!({
            "status": if dry_run { "dry_run" } else { "stripped" },
            "source": src,
            "destination": dest.display().to_string(),
            "stripped_attachments": count,
        }))
    }
}
