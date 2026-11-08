use mail2rust_core::{
    ingest_path, DedupEngine, DedupStrategy, EmailMessage, FormatSniffer, MailboxFormat, MailboxMerger,
    MergeFolderStrategy, MergeOptions, MergeSourceConfig,
};
use mail2rust_eml::{EmlExporter, EmlLayout, EmlNaming};
use mail2rust_filter::{SplitConfig, SplitMode, VolumeSplitter};
use mail2rust_forensics::{BatesRange, ForensicEngine};
use mail2rust_mbox::MboxFileExporter;
use mail2rust_pst::{PstReader, PstValidator, PstWriter};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use tracing::{debug, error, info};

pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";
pub const SERVER_NAME: &str = "mail2rust-mcp";
pub const SERVER_VERSION: &str = "3.3.0";

/// Universal reader helper that supports both PST/OST and other formats
pub fn read_mailbox_messages(source: &Path) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>> {
    let fmt = FormatSniffer::sniff_path(source);
    if fmt == MailboxFormat::Pst || fmt == MailboxFormat::Ost {
        return Ok(PstReader::read_path(source)?);
    }
    ingest_path(source)
}

/// Helper to write messages into a 64-bit Unicode MS-PST file
pub fn export_pst_file(messages: &[EmailMessage], dest_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(dest_path)?;
    let mut writer = PstWriter::new(file, true)?;
    let mut folder_map = HashMap::new();

    for msg in messages {
        let folder_name = msg.folder_path.last().map(|s| s.as_str()).unwrap_or("Inbox");
        let nid = if let Some(&id) = folder_map.get(folder_name) {
            id
        } else {
            let id = writer.add_folder(folder_name, None)?;
            folder_map.insert(folder_name.to_string(), id);
            id
        };
        writer.add_message(msg, nid)?;
    }

    writer.finalize()?;
    Ok(())
}

/// Helper to export messages into a directory tree of standard .eml files
pub fn export_eml_tree(messages: &[EmailMessage], dest_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dest_dir)?;
    let mut exporter = EmlExporter::new(dest_dir, EmlLayout::Folder).with_naming(EmlNaming::Subject);
    for msg in messages {
        exporter.export_message(msg)?;
    }
    Ok(())
}

/// Helper to export messages into an MBOX archive
pub fn export_mbox_file(
    messages: &[EmailMessage],
    dest_path: &Path,
    compress: bool,
) -> Result<usize, Box<dyn std::error::Error>> {
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(MboxFileExporter::export_to_file(messages, dest_path, compress)?)
}

/// Returns list of available tools with their JSON schema
pub fn get_tools_manifest() -> Value {
    json!([
        {
            "name": "mail2rust_sniff",
            "description": "Inspect magic bytes and identify mailbox format (PST, OST, MBOX, EML, EMLX, MSG, OLM, Maildir).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to mailbox file or directory"
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "mail2rust_validate_pst",
            "description": "Validate 64-bit Unicode MS-PST binary integrity, CRC-32 header, and B-Trees without Outlook.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to .pst file to validate"
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "mail2rust_convert",
            "description": "Convert mailbox between formats (e.g. MBOX to PST, PST to EML tree, EML to PST, Maildir to PST).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "Source mailbox file or folder"
                    },
                    "destination": {
                        "type": "string",
                        "description": "Target destination file (.pst/.mbox) or directory (for EML tree)"
                    },
                    "target_format": {
                        "type": "string",
                        "enum": ["pst", "eml", "mbox"],
                        "description": "Optional explicit target format ('pst', 'eml', 'mbox'). If omitted, inferred from destination extension."
                    },
                    "dedup": {
                        "type": "boolean",
                        "description": "Deduplicate messages by SHA-256 hash (default: false)"
                    },
                    "checkpoint_file": {
                        "type": "string",
                        "description": "Optional path to JSON or JSONL state file to track progress and enable incremental restarts"
                    },
                    "resume": {
                        "type": "boolean",
                        "description": "Resume from checkpoint_file, skipping already migrated messages (default: false)"
                    }
                },
                "required": ["source", "destination"]
            }
        },
        {
            "name": "mail2rust_audit",
            "description": "Perform forensic audit on mailbox: compute SHA-256 hashes, Bates stamping, and chain of custody.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Mailbox file or folder to audit"
                    },
                    "case_number": {
                        "type": "string",
                        "description": "Case or investigation reference number (e.g. CASE-2026-001)"
                    },
                    "examiner": {
                        "type": "string",
                        "description": "Forensic examiner name or agent handle"
                    },
                    "bates_prefix": {
                        "type": "string",
                        "description": "Bates stamp prefix (e.g. EVIDENCE)"
                    },
                    "output_csv": {
                        "type": "string",
                        "description": "Optional CSV output file path for forensic manifest"
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "mail2rust_split",
            "description": "Split oversized mailbox (e.g. >20GB or >50GB) into chunked volume archives preserving folder hierarchies.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "Source mailbox file or folder"
                    },
                    "destination_dir": {
                        "type": "string",
                        "description": "Directory where split volumes will be saved"
                    },
                    "max_volume_mb": {
                        "type": "integer",
                        "description": "Maximum size per volume in megabytes (default: 20000 = 20GB)"
                    },
                    "strategy": {
                        "type": "string",
                        "enum": ["size", "year"],
                        "description": "Split strategy: 'size' (default) or 'year'"
                    },
                    "target_format": {
                        "type": "string",
                        "enum": ["pst", "mbox", "eml"],
                        "description": "Output format for split volumes (default: pst)"
                    }
                },
                "required": ["source", "destination_dir"]
            }
        },
        {
            "name": "mail2rust_merge",
            "description": "Merge multiple disparate mailbox archives into a unified container with cross-archive deduplication.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "sources": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of source mailbox file/directory paths"
                    },
                    "destination": {
                        "type": "string",
                        "description": "Destination file path (.pst/.mbox) or folder (.eml)"
                    },
                    "dedup": {
                        "type": "boolean",
                        "description": "Deduplicate identical messages across archives (default: true)"
                    }
                },
                "required": ["sources", "destination"]
            }
        }
    ])
}

/// Executes a tool call and returns the JSON result
pub fn call_tool(name: &str, arguments: &Value) -> Result<Value, String> {
    match name {
        "mail2rust_sniff" => {
            let path_str = arguments
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing required parameter 'path'".to_string())?;
            let path = Path::new(path_str);
            if !path.exists() {
                return Err(format!("File or directory does not exist: {}", path_str));
            }
            let format = FormatSniffer::sniff_path(path);
            let metadata = std::fs::metadata(path).ok();
            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
            let is_dir = path.is_dir();

            let recommended = match format {
                MailboxFormat::Pst => "mail2rust_validate_pst or mail2rust_convert",
                MailboxFormat::Ost => "mail2rust_convert (OST to PST/EML)",
                _ => "mail2rust_convert or mail2rust_audit",
            };

            Ok(json!({
                "path": path_str,
                "format": format.name(),
                "enum_variant": format!("{:?}", format),
                "size_bytes": size,
                "size_human": format!("{:.2} MB", size as f64 / (1024.0 * 1024.0)),
                "is_directory": is_dir,
                "recommended_action": recommended
            }))
        }

        "mail2rust_validate_pst" => {
            let path_str = arguments
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing required parameter 'path'".to_string())?;
            let path = Path::new(path_str);
            if !path.exists() {
                return Err(format!("PST file does not exist: {}", path_str));
            }
            let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
            let report = PstValidator::validate(file).map_err(|e| format!("Validation error: {}", e))?;

            Ok(json!({
                "path": path_str,
                "is_valid": report.is_valid,
                "magic": report.magic,
                "client_magic": report.client_magic,
                "format": report.format,
                "version": report.version,
                "encryption_method": report.encryption_method,
                "declared_crc": format!("0x{:08X}", report.declared_crc),
                "calculated_crc": format!("0x{:08X}", report.calculated_crc),
                "crc_match": report.crc_match,
                "nbt_root_bid": format!("0x{:X}", report.nbt_root_bid),
                "bbt_root_bid": format!("0x{:X}", report.bbt_root_bid),
                "summary": if report.is_valid {
                    "MS-PST 64-bit Unicode file passes all header, CRC-32, and B-Tree specification checks."
                } else {
                    "File failed MS-PST specification validation."
                }
            }))
        }

        "mail2rust_convert" => {
            let src_str = arguments
                .get("source")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing parameter 'source'".to_string())?;
            let dest_str = arguments
                .get("destination")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing parameter 'destination'".to_string())?;
            let target_format = arguments.get("target_format").and_then(|v| v.as_str());
            let dedup = arguments.get("dedup").and_then(|v| v.as_bool()).unwrap_or(false);
            let checkpoint_file = arguments.get("checkpoint_file").and_then(|v| v.as_str());
            let resume = arguments.get("resume").and_then(|v| v.as_bool()).unwrap_or(false);

            let src_path = Path::new(src_str);
            let dest_path = Path::new(dest_str);

            if !src_path.exists() {
                return Err(format!("Source path does not exist: {}", src_str));
            }

            let start_time = std::time::Instant::now();
            let mut messages = read_mailbox_messages(src_path).map_err(|e| format!("Ingestion failed: {}", e))?;
            let total_ingested = messages.len();
            let mut duplicates_skipped = 0;
            let mut already_migrated = 0;

            let mut checkpoint = if let Some(chk_path) = checkpoint_file {
                let p = Path::new(chk_path);
                if p.exists() && (resume || p.extension().is_some_and(|e| e.eq_ignore_ascii_case("jsonl"))) {
                    match mail2rust_sync::CheckpointManager::load(p) {
                        Ok(loaded) => {
                            let (delta, skipped) =
                                mail2rust_sync::CheckpointManager::calculate_delta(&messages, &loaded);
                            already_migrated = skipped;
                            messages = delta;
                            loaded
                        }
                        Err(_) => mail2rust_sync::CheckpointManager::new_checkpoint(src_str, dest_str),
                    }
                } else {
                    mail2rust_sync::CheckpointManager::new_checkpoint(src_str, dest_str)
                }
            } else {
                mail2rust_sync::CheckpointManager::new_checkpoint(src_str, dest_str)
            };

            if dedup {
                let (unique, dups) = DedupEngine::deduplicate(messages, DedupStrategy::Sha256);
                duplicates_skipped = dups.len();
                messages = unique;
            }

            let ext = dest_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let target = target_format.unwrap_or(if ext == "pst" {
                "pst"
            } else if ext == "mbox" {
                "mbox"
            } else {
                "eml"
            });

            match target {
                "pst" => {
                    export_pst_file(&messages, dest_path).map_err(|e| format!("PST write failed: {}", e))?;
                }
                "mbox" => {
                    export_mbox_file(&messages, dest_path, false).map_err(|e| format!("MBOX write failed: {}", e))?;
                }
                "eml" => {
                    export_eml_tree(&messages, dest_path).map_err(|e| format!("EML export failed: {}", e))?;
                }
                other => return Err(format!("Unsupported target format: {}", other)),
            }

            if let Some(chk_path) = checkpoint_file {
                let p = Path::new(chk_path);
                if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("jsonl")) {
                    for m in &messages {
                        let _ = mail2rust_sync::CheckpointManager::append_jsonl_message(p, m, None);
                    }
                } else {
                    mail2rust_sync::CheckpointManager::commit_run(&mut checkpoint, &messages);
                    let _ = mail2rust_sync::CheckpointManager::save_atomic(&checkpoint, p);
                }
            }

            let elapsed_ms = start_time.elapsed().as_millis();

            Ok(json!({
                "status": "success",
                "source": src_str,
                "destination": dest_str,
                "target_format": target,
                "messages_ingested": total_ingested,
                "already_migrated": already_migrated,
                "messages_written": messages.len(),
                "duplicates_skipped": duplicates_skipped,
                "checkpoint_file": checkpoint_file,
                "elapsed_ms": elapsed_ms
            }))
        }

        "mail2rust_audit" => {
            let path_str = arguments
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing parameter 'path'".to_string())?;
            let case_number = arguments
                .get("case_number")
                .and_then(|v| v.as_str())
                .unwrap_or("CASE-2026-M2R");
            let examiner = arguments
                .get("examiner")
                .and_then(|v| v.as_str())
                .unwrap_or("Autonomous Agent");
            let bates_prefix = arguments.get("bates_prefix").and_then(|v| v.as_str());
            let output_csv = arguments.get("output_csv").and_then(|v| v.as_str());

            let path = Path::new(path_str);
            if !path.exists() {
                return Err(format!("Mailbox path does not exist: {}", path_str));
            }

            let mut messages = read_mailbox_messages(path).map_err(|e| format!("Ingestion failed: {}", e))?;

            let bates_range = if let Some(pfx) = bates_prefix {
                ForensicEngine::apply_bates_stamping(&mut messages, pfx, 1000, 7)
            } else {
                BatesRange {
                    start: "N/A".to_string(),
                    end: "N/A".to_string(),
                }
            };

            let manifest = ForensicEngine::generate_manifest(
                &messages,
                case_number,
                examiner,
                path_str,
                output_csv.unwrap_or("audit_manifest.csv"),
                bates_range.clone(),
            );

            if let Some(csv_path) = output_csv {
                let p = Path::new(csv_path);
                if let Some(parent) = p.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                ForensicEngine::export_manifest_csv(&messages, p)
                    .map_err(|e| format!("Failed to export CSV: {}", e))?;
            }

            Ok(json!({
                "status": "success",
                "case_number": manifest.case_number,
                "examiner": manifest.examiner,
                "source": manifest.source_container,
                "total_items": manifest.total_items,
                "total_size_bytes": manifest.total_size_bytes,
                "root_sha256": manifest.root_sha256,
                "bates_start": manifest.bates_range.start,
                "bates_end": manifest.bates_range.end,
                "output_csv": output_csv
            }))
        }

        "mail2rust_split" => {
            let src_str = arguments
                .get("source")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing parameter 'source'".to_string())?;
            let dest_dir_str = arguments
                .get("destination_dir")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing parameter 'destination_dir'".to_string())?;
            let max_volume_mb = arguments.get("max_volume_mb").and_then(|v| v.as_u64()).unwrap_or(20000) as usize;
            let strategy_str = arguments.get("strategy").and_then(|v| v.as_str()).unwrap_or("size");
            let target_ext = arguments
                .get("target_format")
                .and_then(|v| v.as_str())
                .unwrap_or("pst")
                .to_string();

            let src_path = Path::new(src_str);
            let dest_dir = Path::new(dest_dir_str);

            if !src_path.exists() {
                return Err(format!("Source does not exist: {}", src_str));
            }
            std::fs::create_dir_all(dest_dir).map_err(|e| format!("Cannot create dir: {}", e))?;

            let messages = read_mailbox_messages(src_path).map_err(|e| format!("Ingestion failed: {}", e))?;

            let config = SplitConfig {
                max_volume_bytes: max_volume_mb * 1024 * 1024,
                cluster_threads: true,
                file_prefix: "part".to_string(),
                target_ext: target_ext.clone(),
            };

            let mode = if strategy_str.eq_ignore_ascii_case("year") {
                SplitMode::Year
            } else {
                SplitMode::Size
            };

            let volumes = VolumeSplitter::split_with_mode(&messages, &config, &mode);
            let mut created = Vec::new();

            for vol in &volumes {
                let vol_path = dest_dir.join(&vol.file_name);
                if target_ext == "pst" {
                    export_pst_file(&vol.messages, &vol_path).map_err(|e| format!("Failed writing volume: {}", e))?;
                } else if target_ext == "mbox" {
                    export_mbox_file(&vol.messages, &vol_path, false)
                        .map_err(|e| format!("Failed writing volume: {}", e))?;
                } else {
                    export_eml_tree(&vol.messages, &vol_path).map_err(|e| format!("Failed writing volume: {}", e))?;
                }
                created.push(json!({
                    "volume_index": vol.volume_index,
                    "file_name": vol.file_name,
                    "path": vol_path.to_string_lossy(),
                    "messages_count": vol.messages.len(),
                    "size_bytes": vol.total_size_bytes
                }));
            }

            Ok(json!({
                "status": "success",
                "source": src_str,
                "destination_dir": dest_dir_str,
                "strategy": strategy_str,
                "volumes_count": volumes.len(),
                "volumes": created
            }))
        }

        "mail2rust_merge" => {
            let sources_val = arguments
                .get("sources")
                .and_then(|v| v.as_array())
                .ok_or_else(|| "Missing array parameter 'sources'".to_string())?;
            let dest_str = arguments
                .get("destination")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing parameter 'destination'".to_string())?;
            let dedup = arguments.get("dedup").and_then(|v| v.as_bool()).unwrap_or(true);

            let mut source_configs = Vec::new();
            for item in sources_val {
                if let Some(s) = item.as_str() {
                    let p = PathBuf::from(s);
                    if !p.exists() {
                        return Err(format!("Source file does not exist: {}", s));
                    }
                    source_configs.push(MergeSourceConfig::new(p));
                }
            }

            if source_configs.is_empty() {
                return Err("At least one valid source is required for merge".to_string());
            }

            let options = MergeOptions {
                folder_strategy: MergeFolderStrategy::Combine,
                dedup_strategy: if dedup {
                    DedupStrategy::Sha256
                } else {
                    DedupStrategy::MessageId
                },
                sort_by_date: true,
                dry_run: false,
            };

            let dest_path = Path::new(dest_str);
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }

            let (merged_messages, report) =
                MailboxMerger::merge_sources(&source_configs, &options, read_mailbox_messages)
                    .map_err(|e| format!("Merge failed: {}", e))?;

            let ext = dest_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            if ext == "pst" {
                export_pst_file(&merged_messages, dest_path)
                    .map_err(|e| format!("Failed writing merged PST: {}", e))?;
            } else if ext == "mbox" {
                export_mbox_file(&merged_messages, dest_path, false)
                    .map_err(|e| format!("Failed writing merged MBOX: {}", e))?;
            } else {
                export_eml_tree(&merged_messages, dest_path)
                    .map_err(|e| format!("Failed writing merged EML: {}", e))?;
            }

            Ok(json!({
                "status": "success",
                "sources_merged": source_configs.len(),
                "destination": dest_str,
                "total_input_messages": report.total_input_messages,
                "total_unique_messages": report.total_unique_messages,
                "total_duplicates_removed": report.total_duplicates_removed,
                "total_bytes": report.total_bytes
            }))
        }

        unknown => Err(format!("Unknown tool: {}", unknown)),
    }
}

/// Handles a single incoming JSON-RPC request and returns the response Value if needed
pub fn handle_jsonrpc_request(req: &Value) -> Option<Value> {
    let id = req.get("id").cloned();
    let method = match req.get("method").and_then(|m| m.as_str()) {
        Some(m) => m,
        None => {
            return id.map(|req_id| {
                json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "error": { "code": -32600, "message": "Invalid Request: missing method" }
                })
            });
        }
    };

    match method {
        "initialize" => {
            debug!("Received initialize request");
            id.map(|req_id| {
                json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "protocolVersion": MCP_PROTOCOL_VERSION,
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": SERVER_NAME,
                            "version": SERVER_VERSION
                        }
                    }
                })
            })
        }

        "notifications/initialized" => {
            info!("Client initialized successfully");
            None
        }

        "ping" => id.map(|req_id| {
            json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": {}
            })
        }),

        "tools/list" => id.map(|req_id| {
            json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": {
                    "tools": get_tools_manifest()
                }
            })
        }),

        "tools/call" => {
            let params = req.get("params").unwrap_or(&Value::Null);
            let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let arguments = params.get("arguments").unwrap_or(&Value::Null);

            let res = call_tool(tool_name, arguments);
            id.map(|req_id| match res {
                Ok(val) => {
                    let formatted = serde_json::to_string_pretty(&val).unwrap_or_default();
                    json!({
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "result": {
                            "content": [
                                {
                                    "type": "text",
                                    "text": formatted
                                }
                            ],
                            "isError": false
                        }
                    })
                }
                Err(err_msg) => json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "content": [
                            {
                                "type": "text",
                                "text": format!("Tool execution error: {}", err_msg)
                            }
                        ],
                        "isError": true
                    }
                }),
            })
        }

        unknown_method => id.map(|req_id| {
            json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "error": {
                    "code": -32601,
                    "message": format!("Method not found: {}", unknown_method)
                }
            })
        }),
    }
}

/// Runs the standard stdio JSON-RPC server loop
pub fn run_stdio_server() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    let mut line = String::new();
    while reader.read_line(&mut line)? > 0 {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            match serde_json::from_str::<Value>(trimmed) {
                Ok(req) => {
                    if let Some(resp) = handle_jsonrpc_request(&req) {
                        let out_str = serde_json::to_string(&resp)?;
                        writeln!(writer, "{}", out_str)?;
                        writer.flush()?;
                    }
                }
                Err(e) => {
                    error!(error = %e, "Failed to parse JSON-RPC line");
                    let err_resp = json!({
                        "jsonrpc": "2.0",
                        "id": Value::Null,
                        "error": {
                            "code": -32700,
                            "message": format!("Parse error: {}", e)
                        }
                    });
                    writeln!(writer, "{}", serde_json::to_string(&err_resp)?)?;
                    writer.flush()?;
                }
            }
        }
        line.clear();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tools_manifest_structure() {
        let manifest = get_tools_manifest();
        let tools = manifest.as_array().expect("Tools must be an array");
        assert_eq!(tools.len(), 6);

        let names: Vec<&str> = tools
            .iter()
            .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
            .collect();

        assert!(names.contains(&"mail2rust_sniff"));
        assert!(names.contains(&"mail2rust_validate_pst"));
        assert!(names.contains(&"mail2rust_convert"));
        assert!(names.contains(&"mail2rust_audit"));
        assert!(names.contains(&"mail2rust_split"));
        assert!(names.contains(&"mail2rust_merge"));
    }

    #[test]
    fn test_handle_initialize() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05"
            }
        });
        let resp = handle_jsonrpc_request(&req).expect("Response expected");
        assert_eq!(resp["id"], 1);
        assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(resp["result"]["serverInfo"]["name"], "mail2rust-mcp");
    }

    #[test]
    fn test_handle_tools_list() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        });
        let resp = handle_jsonrpc_request(&req).expect("Response expected");
        assert_eq!(resp["id"], 2);
        let tools = resp["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 6);
    }

    #[test]
    fn test_handle_unknown_method() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "non_existent_method"
        });
        let resp = handle_jsonrpc_request(&req).expect("Response expected");
        assert_eq!(resp["error"]["code"], -32601);
    }

    #[test]
    fn test_tools_call_sniff_and_validate_e2e() {
        let cargo_manifest = env!("CARGO_MANIFEST_DIR");
        let workspace_root = Path::new(cargo_manifest)
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root");

        let sample_pst = workspace_root.join("samples/pst/sample_unicode.pst");
        let sample_mbox = workspace_root.join("samples/mbox/sample_mailbox.mbox");

        // 1. Sniff Mbox
        let sniff_res = call_tool("mail2rust_sniff", &json!({ "path": sample_mbox.to_string_lossy() }))
            .expect("Sniff must succeed");
        assert_eq!(sniff_res["enum_variant"], "Mbox");

        // 2. Sniff PST
        let sniff_pst = call_tool("mail2rust_sniff", &json!({ "path": sample_pst.to_string_lossy() }))
            .expect("Sniff PST must succeed");
        assert_eq!(sniff_pst["enum_variant"], "Pst");

        // 3. Validate PST
        let val_pst = call_tool(
            "mail2rust_validate_pst",
            &json!({ "path": sample_pst.to_string_lossy() }),
        )
        .expect("Validate PST must succeed");
        assert_eq!(val_pst["is_valid"], true);
        assert_eq!(val_pst["crc_match"], true);
    }

    #[test]
    fn test_tools_call_convert_audit_and_merge_e2e() {
        let cargo_manifest = env!("CARGO_MANIFEST_DIR");
        let workspace_root = Path::new(cargo_manifest)
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root");
        let sample_mbox = workspace_root.join("samples/mbox/sample_mailbox.mbox");

        let temp_dir = tempfile::tempdir().expect("tempdir");
        let dest_pst = temp_dir.path().join("converted.pst");
        let dest_csv = temp_dir.path().join("audit.csv");
        let split_dir = temp_dir.path().join("splits");

        // 1. Convert MBOX -> PST
        let conv_res = call_tool(
            "mail2rust_convert",
            &json!({
                "source": sample_mbox.to_string_lossy(),
                "destination": dest_pst.to_string_lossy(),
                "target_format": "pst",
                "dedup": true
            }),
        )
        .expect("Convert must succeed");
        assert_eq!(conv_res["status"], "success");
        assert!(conv_res["messages_written"].as_u64().unwrap() > 0);

        // 2. Validate newly generated PST
        let val_res = call_tool("mail2rust_validate_pst", &json!({ "path": dest_pst.to_string_lossy() }))
            .expect("Validate converted PST must succeed");
        assert_eq!(val_res["is_valid"], true);
        assert_eq!(val_res["crc_match"], true);

        // 3. Audit PST
        let audit_res = call_tool(
            "mail2rust_audit",
            &json!({
                "path": dest_pst.to_string_lossy(),
                "case_number": "CASE-TEST",
                "examiner": "MCP-Agent",
                "bates_prefix": "EVIDENCE",
                "output_csv": dest_csv.to_string_lossy()
            }),
        )
        .expect("Audit must succeed");
        assert_eq!(audit_res["case_number"], "CASE-TEST");
        assert!(dest_csv.exists());

        // 4. Split PST
        let split_res = call_tool(
            "mail2rust_split",
            &json!({
                "source": dest_pst.to_string_lossy(),
                "destination_dir": split_dir.to_string_lossy(),
                "max_volume_mb": 5,
                "target_format": "mbox"
            }),
        )
        .expect("Split must succeed");
        assert!(split_res["volumes_count"].as_u64().unwrap() > 0);

        // 5. JSON-RPC Dispatch test
        let jsonrpc_req = json!({
            "jsonrpc": "2.0",
            "id": 100,
            "method": "tools/call",
            "params": {
                "name": "mail2rust_sniff",
                "arguments": {
                    "path": dest_pst.to_string_lossy()
                }
            }
        });
        let rpc_resp = handle_jsonrpc_request(&jsonrpc_req).expect("Response expected");
        assert_eq!(rpc_resp["id"], 100);
        assert_eq!(rpc_resp["result"]["isError"], false);
    }
}
