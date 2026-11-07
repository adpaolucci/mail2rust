use chrono::Datelike;
use clap::{Parser, Subcommand, ValueEnum};
use mail2rust_core::{init_logging, EmailMessage, FormatSniffer};
use mail2rust_eml::{EmlExporter, EmlLayout, EmlNaming};
use mail2rust_filter::{
    parse_cli_datetime, parse_size_bytes, DateFilter, DedupEngine, DedupStrategy, FolderFilter, SplitConfig, SplitMode,
    VolumeSplitter,
};
use mail2rust_forensics::{BatesRange, ForensicEngine};
use mail2rust_mbox::MboxFileExporter;
use mail2rust_pst::{PstValidator, PstWriter};
use mail2rust_sync::CheckpointManager;
use std::fs::File;
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(Parser, Debug)]
#[command(
    name = "mail2rust",
    author = "Anthony D. Paolucci <adpaolucci@gmail.com>",
    version = "3.2.0",
    about = "Universal high-performance offline email & mailbox converter in 100% pure Rust"
)]
struct Cli {
    #[arg(short, long, global = true, default_value = "info")]
    log_level: String,

    #[arg(long, global = true)]
    log_file: Option<PathBuf>,

    #[arg(long, global = true, alias = "audit-manifest")]
    audit_log: Option<PathBuf>,

    #[arg(long, global = true)]
    error_log: Option<PathBuf>,

    #[arg(long, global = true)]
    bates_prefix: Option<String>,

    #[arg(long, global = true, default_value = "1000")]
    bates_start: usize,

    #[arg(long, global = true)]
    resume: Option<PathBuf>,

    #[arg(long, global = true, alias = "checkpoint-out")]
    state_file: Option<PathBuf>,

    #[arg(long, global = true)]
    dedup: Option<CliDedupStrategy>,

    #[arg(long, global = true)]
    dry_run: bool,

    // Date filtering flags
    #[arg(long, global = true, alias = "after")]
    since: Option<String>,

    #[arg(long, global = true, alias = "before")]
    until: Option<String>,

    #[arg(long, global = true)]
    start_year: Option<i32>,

    #[arg(long, global = true)]
    end_year: Option<i32>,

    // Folder filtering flags
    #[arg(long, global = true)]
    include_folder: Vec<String>,

    #[arg(long, global = true)]
    exclude_folder: Vec<String>,

    #[arg(long, global = true)]
    include_deleted: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum CliDedupStrategy {
    Sha256,
    MessageId,
    Content,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Convert any email/mailbox format to another (auto-detects source & destination)
    Convert {
        source: PathBuf,
        destination: PathBuf,
        #[arg(long, alias = "split-pst")]
        max_pst_size: Option<String>,
        #[arg(long)]
        split_by: Option<String>,
        #[arg(long)]
        splits: Option<usize>,
        #[arg(long)]
        pst_root: Option<String>,
        #[arg(long)]
        flatten: bool,
    },
    /// Convert any source into Microsoft Outlook PST (pure Rust, zero Outlook required)
    Pst {
        source: PathBuf,
        destination: PathBuf,
        #[arg(long, default_value = "20")]
        max_pst_gb: usize,
        #[arg(long, alias = "split-pst")]
        max_pst_size: Option<String>,
        #[arg(long)]
        split_by: Option<String>,
        #[arg(long)]
        splits: Option<usize>,
        #[arg(long)]
        pst_root: Option<String>,
        #[arg(long)]
        flatten: bool,
        #[arg(long)]
        overwrite_pst: bool,
        #[arg(long, default_value = "true")]
        cluster_threads: bool,
        #[arg(long, default_value = "true")]
        compress: bool,
    },
    /// Convert any source into clean EML files with customizable directory layout
    Eml {
        source: PathBuf,
        destination: PathBuf,
        #[arg(long, default_value = "folder")]
        layout: String,
        #[arg(long, default_value = "subject")]
        naming: String,
        #[arg(long, default_value = "0")]
        max_per_dir: usize,
    },
    /// Convert any source into standard mboxrd MBOX file
    Mbox {
        source: PathBuf,
        destination: PathBuf,
        #[arg(long)]
        compress: bool,
        #[arg(long)]
        append: bool,
        #[arg(long)]
        split_size: Option<String>,
    },
    /// Split a mailbox into multi-volume parts while preserving email threads
    Split {
        source: PathBuf,
        destination_dir: PathBuf,
        #[arg(long, default_value = "20000")]
        max_volume_mb: usize,
        #[arg(long, alias = "split-size")]
        split_size: Option<String>,
        #[arg(long)]
        split_by: Option<String>,
        #[arg(long)]
        splits: Option<usize>,
        #[arg(long, default_value = "true")]
        cluster_threads: bool,
        #[arg(long, default_value = "pst")]
        target_ext: String,
    },
    /// Generate forensic chain-of-custody audit manifest and verify container integrity
    Audit {
        source: PathBuf,
        #[arg(short, long)]
        output_csv: PathBuf,
        #[arg(long)]
        output_json: Option<PathBuf>,
        #[arg(long, default_value = "CASE-2026-M2R")]
        case_number: String,
        #[arg(long, default_value = "Examiner")]
        examiner: String,
    },
    /// Validate MS-PST container header magic, version, CRC-32 checksum, and B-Trees
    ValidatePst { pst_file: PathBuf },
    /// Ingest mail directly from an IMAP server over TLS/SSL into PST, MBOX, or EML
    Imap {
        #[arg(long)]
        host: String,
        #[arg(long, default_value = "993")]
        port: u16,
        #[arg(short, long)]
        username: String,
        #[arg(short, long)]
        password: Option<String>,
        #[arg(long, alias = "disable-tls")]
        no_tls: bool,
        #[arg(long)]
        insecure: bool,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        folders: Vec<String>,
        #[arg(long)]
        max_per_folder: Option<usize>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        checkpoint: Option<PathBuf>,
    },
    /// Sniff file or folder to identify mailbox format and magic byte signature
    Sniff { path: PathBuf },
    /// Launch the pure-Rust native GUI studio (zero Node.js or WebView2 dependencies)
    Gui,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    init_logging(&cli.log_level, cli.log_file.as_deref())?;

    info!(version = "3.2.0", "Mail2Rust Engine Initialized");

    match &cli.command {
        Commands::Gui => {
            println!("Launching Mail2Rust Studio (Pure-Rust native GUI)...");
            let current_exe = std::env::current_exe().unwrap_or_default();
            let parent_dir = current_exe.parent().unwrap_or(Path::new(""));
            let gui_exe_name = if cfg!(windows) {
                "mail2rust-gui.exe"
            } else {
                "mail2rust-gui"
            };
            let candidate_path = parent_dir.join(gui_exe_name);

            let mut cmd = if candidate_path.exists() {
                std::process::Command::new(candidate_path)
            } else {
                std::process::Command::new(gui_exe_name)
            };

            let status = cmd.status()?;
            if !status.success() {
                eprintln!("Mail2Rust GUI exited with code: {:?}", status.code());
            }
        }

        Commands::Sniff { path } => {
            let format = FormatSniffer::sniff_path(path);
            println!("Target Path: {}", path.display());
            println!("Identified Format: {}", format.name());
            println!("Format Type: {:?}", format);
        }

        Commands::ValidatePst { pst_file } => {
            info!(path = ?pst_file, "Running MS-PST specification validation");
            let file = File::open(pst_file)?;
            let report = PstValidator::validate(file)?;
            println!("\n=== MS-PST Validation Report ===");
            println!("File: {}", pst_file.display());
            println!("Specification Valid: {}", report.is_valid);
            println!("Magic: {}", report.magic);
            println!("Format: {}", report.format);
            println!("Client Magic: {}", report.client_magic);
            println!("Version: {}", report.version);
            println!("Encryption: {}", report.encryption_method);
            println!("Declared CRC32: 0x{:08X}", report.declared_crc);
            println!("Calculated CRC32: 0x{:08X}", report.calculated_crc);
            println!("CRC Match: {}", report.crc_match);
            println!(
                "NBT Root BID: 0x{:X} (Offset 0x{:X})",
                report.nbt_root_bid, report.nbt_root_offset
            );
            println!(
                "BBT Root BID: 0x{:X} (Offset 0x{:X})",
                report.bbt_root_bid, report.bbt_root_offset
            );
            if !report.errors.is_empty() {
                println!("\nErrors Encountered:");
                for e in &report.errors {
                    println!("  - {}", e);
                }
            }
        }

        Commands::Convert {
            source,
            destination,
            max_pst_size,
            split_by,
            splits,
            pst_root,
            flatten,
        } => {
            let dest_ext = destination
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let dest_is_pst = dest_ext == "pst";
            let dest_is_mbox = dest_ext == "mbox" || dest_ext == "gz";

            if dest_is_pst {
                let max_gb = if let Some(ref sz) = max_pst_size {
                    parse_size_bytes(sz).unwrap_or(20 * 1024 * 1024 * 1024) / (1024 * 1024 * 1024)
                } else {
                    20
                };
                run_pst_conversion(
                    &cli,
                    source,
                    destination,
                    max_gb,
                    split_by.as_deref(),
                    *splits,
                    pst_root.as_deref(),
                    *flatten,
                    true,
                    true,
                )?;
            } else if dest_is_mbox {
                let compress = dest_ext == "gz";
                run_mbox_conversion(&cli, source, destination, compress, false)?;
            } else {
                run_eml_conversion(&cli, source, destination, "folder", "subject", 0)?;
            }
        }

        Commands::Pst {
            source,
            destination,
            max_pst_gb,
            max_pst_size,
            split_by,
            splits,
            pst_root,
            flatten,
            overwrite_pst: _,
            cluster_threads,
            compress,
        } => {
            let effective_gb = if let Some(ref sz) = max_pst_size {
                parse_size_bytes(sz).unwrap_or(max_pst_gb * 1024 * 1024 * 1024) / (1024 * 1024 * 1024)
            } else {
                *max_pst_gb
            };
            run_pst_conversion(
                &cli,
                source,
                destination,
                effective_gb,
                split_by.as_deref(),
                *splits,
                pst_root.as_deref(),
                *flatten,
                *cluster_threads,
                *compress,
            )?;
        }

        Commands::Eml {
            source,
            destination,
            layout,
            naming,
            max_per_dir,
        } => {
            run_eml_conversion(&cli, source, destination, layout, naming, *max_per_dir)?;
        }

        Commands::Mbox {
            source,
            destination,
            compress,
            append,
            split_size: _,
        } => {
            run_mbox_conversion(&cli, source, destination, *compress, *append)?;
        }

        Commands::Split {
            source,
            destination_dir,
            max_volume_mb,
            split_size,
            split_by,
            splits,
            cluster_threads,
            target_ext,
        } => {
            info!(source = ?source, dest = ?destination_dir, "Starting volume auto-split");
            let mut messages = ingest_source(source)?;
            apply_preprocessing(&cli, &mut messages)?;

            if cli.dry_run {
                perform_dry_run(&messages, source);
                return Ok(());
            }

            let max_bytes = if let Some(ref sz) = split_size {
                parse_size_bytes(sz).unwrap_or(max_volume_mb * 1024 * 1024)
            } else {
                max_volume_mb * 1024 * 1024
            };

            let split_mode = if let Some("year") = split_by.as_deref() {
                SplitMode::Year
            } else if let Some(n) = splits {
                SplitMode::Even(*n)
            } else {
                SplitMode::Size
            };

            let cfg = SplitConfig {
                max_volume_bytes: max_bytes,
                cluster_threads: *cluster_threads,
                file_prefix: "Volume".to_string(),
                target_ext: target_ext.clone(),
            };

            let volumes = VolumeSplitter::split_with_mode(&messages, &cfg, &split_mode);
            std::fs::create_dir_all(destination_dir)?;

            for vol in &volumes {
                let out_file = destination_dir.join(&vol.file_name);
                info!(volume = vol.volume_index, file = ?vol.file_name, count = vol.messages.len(), "Writing volume");
                if target_ext == "pst" {
                    let file = File::create(&out_file)?;
                    let mut writer = PstWriter::new(file, true)?;
                    let inbox = writer.add_folder("Inbox", None)?;
                    for m in &vol.messages {
                        writer.add_message(m, inbox)?;
                    }
                    writer.finalize()?;
                } else {
                    MboxFileExporter::export_to_file(&vol.messages, &out_file, false)?;
                }
            }
            println!("Successfully split mailbox into {} volumes", volumes.len());
        }

        Commands::Audit {
            source,
            output_csv,
            output_json,
            case_number,
            examiner,
        } => {
            info!(source = ?source, "Running forensic audit");
            let mut messages = ingest_source(source)?;
            apply_preprocessing(&cli, &mut messages)?;

            let bates_range = if let Some(ref pfx) = cli.bates_prefix {
                ForensicEngine::apply_bates_stamping(&mut messages, pfx, cli.bates_start, 7)
            } else {
                BatesRange {
                    start: "N/A".to_string(),
                    end: "N/A".to_string(),
                }
            };

            ForensicEngine::export_manifest_csv(&messages, output_csv)?;
            let manifest = ForensicEngine::generate_manifest(
                &messages,
                case_number,
                examiner,
                &source.display().to_string(),
                &output_csv.display().to_string(),
                bates_range,
            );

            if let Some(json_path) = output_json {
                ForensicEngine::export_manifest_json(&manifest, json_path)?;
            }

            println!("\n=== Forensic Audit Manifest Generated ===");
            println!("Case: {}", manifest.case_number);
            println!("Examiner: {}", manifest.examiner);
            println!("Total Items: {}", manifest.total_items);
            println!(
                "Total Size: {} bytes ({:.2} MB)",
                manifest.total_size_bytes,
                manifest.total_size_bytes as f64 / (1024.0 * 1024.0)
            );
            println!("Root SHA-256 Digest: {}", manifest.root_sha256);
            println!(
                "Bates Range: {} -> {}",
                manifest.bates_range.start, manifest.bates_range.end
            );
            println!("CSV Report: {}", output_csv.display());
        }

        Commands::Imap {
            host,
            port,
            username,
            password,
            no_tls,
            insecure,
            output,
            folders,
            max_per_folder,
            since,
            checkpoint,
        } => {
            let pass = match password {
                Some(p) => p.clone(),
                None => match std::env::var("MAIL2RUST_IMAP_PASSWORD") {
                    Ok(p) => p,
                    Err(_) => {
                        eprint!("Enter IMAP Password for {}: ", username);
                        let mut line = String::new();
                        std::io::stdin().read_line(&mut line)?;
                        line.trim().to_string()
                    }
                },
            };

            let since_date = if let Some(s) = since {
                Some(parse_cli_datetime(s, false)?.date_naive())
            } else {
                None
            };

            let folder_filter = if folders.is_empty() {
                None
            } else {
                Some(folders.clone())
            };

            let use_tls = !no_tls;
            let config = mail2rust_imap::ImapConfig {
                host: host.clone(),
                port: *port,
                username: username.clone(),
                password: pass,
                use_tls,
                insecure_certs: *insecure,
                folder_filter,
                max_messages_per_folder: *max_per_folder,
                since_date,
            };

            println!("Connecting to IMAP {}:{} (TLS: {})...", host, port, use_tls);
            let mut client = mail2rust_imap::ImapClient::new(config);
            client.connect()?;
            println!("Authenticated as {}", username);

            let chk_path = checkpoint.as_ref().map(|p| p.as_path());
            let mut chk = if let Some(p) = chk_path {
                if p.exists() {
                    println!("Resuming migration from checkpoint: {}", p.display());
                    CheckpointManager::load(p)?
                } else {
                    CheckpointManager::new_checkpoint(
                        &format!("imap://{}/{}", host, username),
                        &output.display().to_string(),
                    )
                }
            } else {
                CheckpointManager::new_checkpoint(
                    &format!("imap://{}/{}", host, username),
                    &output.display().to_string(),
                )
            };

            println!("Streaming mail directly to {}...", output.display());
            let mut messages = Vec::new();
            client.sync_all(&mut chk, |prog, msg| {
                use std::io::Write;
                print!(
                    "\r[IMAP Sync] Folder: {:<20} | Msg #{:<5} | UID: {:<8}",
                    prog.current_folder, prog.current_message_index, prog.current_uid
                );
                let _ = std::io::stdout().flush();
                messages.push(msg);
            })?;
            println!("\nFetched {} messages from IMAP server.", messages.len());

            apply_preprocessing(&cli, &mut messages)?;

            let ext = output
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("pst")
                .to_lowercase();
            match ext.as_str() {
                "mbox" => {
                    MboxFileExporter::export_to_file(&messages, output, false)?;
                    println!("Exported {} messages to MBOX: {}", messages.len(), output.display());
                }
                "eml" | "" if output.is_dir() => {
                    let mut exporter = EmlExporter::new(output, EmlLayout::Folder).with_naming(EmlNaming::Subject);
                    for m in &messages {
                        exporter.export_message(m)?;
                    }
                    println!(
                        "Exported {} messages to EML directory: {}",
                        messages.len(),
                        output.display()
                    );
                }
                _ => {
                    let file = File::create(output)?;
                    let mut writer = PstWriter::new(file, true)?;
                    let mut folder_map = std::collections::HashMap::new();

                    for msg in &messages {
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
                    println!(
                        "Exported {} messages to PST container: {}",
                        messages.len(),
                        output.display()
                    );
                }
            }

            if let Some(p) = chk_path {
                chk.status = "completed".to_string();
                CheckpointManager::save(&chk, p)?;
                println!("Saved final migration checkpoint to: {}", p.display());
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_pst_conversion(
    cli: &Cli,
    source: &Path,
    destination: &Path,
    max_pst_gb: usize,
    split_by: Option<&str>,
    splits: Option<usize>,
    pst_root: Option<&str>,
    flatten: bool,
    cluster_threads: bool,
    compress: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(source = ?source, dest = ?destination, "Starting PST conversion (zero Outlook dependency)");
    let mut messages = ingest_source(source)?;
    apply_preprocessing(cli, &mut messages)?;

    if cli.dry_run {
        perform_dry_run(&messages, source);
        return Ok(());
    }

    let max_bytes = max_pst_gb * 1024 * 1024 * 1024;
    let total_bytes: usize = messages.iter().map(|m| m.size_bytes).sum();

    let split_mode = if let Some("year") = split_by {
        Some(SplitMode::Year)
    } else if let Some(n) = splits {
        Some(SplitMode::Even(n))
    } else if total_bytes > max_bytes {
        Some(SplitMode::Size)
    } else {
        None
    };

    if let Some(mode) = split_mode {
        info!("Splitting PST export using mode: {:?}", mode);
        let prefix = destination.file_stem().and_then(|s| s.to_str()).unwrap_or("Archive");
        let parent_dir = destination.parent().unwrap_or_else(|| Path::new("."));
        let cfg = SplitConfig {
            max_volume_bytes: max_bytes,
            cluster_threads,
            file_prefix: prefix.to_string(),
            target_ext: "pst".to_string(),
        };
        let volumes = VolumeSplitter::split_with_mode(&messages, &cfg, &mode);
        for vol in &volumes {
            let out_file = parent_dir.join(&vol.file_name);
            write_single_pst(&vol.messages, &out_file, compress, pst_root, flatten)?;
        }
        println!("Exported {} PST volumes successfully", volumes.len());
    } else {
        write_single_pst(&messages, destination, compress, pst_root, flatten)?;
        println!("Exported 1 PST archive successfully ({} messages)", messages.len());
    }

    post_process(cli, &messages, source, destination)?;
    Ok(())
}

fn write_single_pst(
    messages: &[EmailMessage],
    destination: &Path,
    compress: bool,
    pst_root: Option<&str>,
    flatten: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(destination)?;
    let mut writer = PstWriter::new(file, compress)?;

    let mut folder_map = std::collections::HashMap::new();

    let root_nid = match pst_root {
        Some(root_name) if !root_name.trim().is_empty() => {
            let nid = writer.add_folder(root_name.trim(), None)?;
            folder_map.insert(root_name.trim().to_string(), nid);
            nid
        }
        _ => {
            let nid = writer.add_folder("Inbox", None)?;
            folder_map.insert("Inbox".to_string(), nid);
            nid
        }
    };

    let parent_folder_name = match pst_root {
        Some(root_name) if !root_name.trim().is_empty() => Some(root_name.trim()),
        _ => None,
    };

    for m in messages {
        let fnid = if flatten {
            root_nid
        } else {
            let folder_name = m.folder_path.last().cloned().unwrap_or_else(|| "Inbox".to_string());
            match folder_map.get(&folder_name) {
                Some(&nid) => nid,
                None => {
                    let new_nid = writer.add_folder(&folder_name, parent_folder_name)?;
                    folder_map.insert(folder_name.clone(), new_nid);
                    new_nid
                }
            }
        };
        writer.add_message(m, fnid)?;
    }

    writer.finalize()?;
    info!(dest = ?destination, count = messages.len(), "PST generated successfully");
    Ok(())
}

fn run_eml_conversion(
    cli: &Cli,
    source: &Path,
    destination: &Path,
    layout_str: &str,
    naming_str: &str,
    max_per_dir: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(source = ?source, dest = ?destination, layout = layout_str, "Starting EML export");
    let mut messages = ingest_source(source)?;
    apply_preprocessing(cli, &mut messages)?;

    if cli.dry_run {
        perform_dry_run(&messages, source);
        return Ok(());
    }

    let layout = match layout_str.to_lowercase().as_str() {
        "year" => EmlLayout::Year,
        "month" => EmlLayout::Month,
        "flat" => EmlLayout::Flat,
        _ => EmlLayout::Folder,
    };

    let naming = match naming_str.to_lowercase().as_str() {
        "index" => EmlNaming::Index,
        _ => EmlNaming::Subject,
    };

    let mut exporter = EmlExporter::new(destination, layout)
        .with_naming(naming)
        .with_max_per_dir(max_per_dir);

    for m in &messages {
        exporter.export_message(m)?;
    }

    println!("Exported {} EML messages to {}", messages.len(), destination.display());
    post_process(cli, &messages, source, destination)?;
    Ok(())
}

fn run_mbox_conversion(
    cli: &Cli,
    source: &Path,
    destination: &Path,
    compress: bool,
    append: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(source = ?source, dest = ?destination, compress = compress, append = append, "Starting MBOX export");
    let mut messages = ingest_source(source)?;
    apply_preprocessing(cli, &mut messages)?;

    if cli.dry_run {
        perform_dry_run(&messages, source);
        return Ok(());
    }

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }

    MboxFileExporter::export_to_file_options(&messages, destination, compress, append)?;
    println!(
        "Exported {} messages into mboxrd container: {}",
        messages.len(),
        destination.display()
    );
    post_process(cli, &messages, source, destination)?;
    Ok(())
}

fn perform_dry_run(messages: &[EmailMessage], source: &Path) {
    println!("\n======================================================================");
    println!("                   PRE-FLIGHT DRY-RUN AUDIT REPORT");
    println!("======================================================================");
    println!("Source Store:          {}", source.display());

    let total_count = messages.len();
    let total_bytes: usize = messages.iter().map(|m| m.size_bytes).sum();
    let total_mb = total_bytes as f64 / (1024.0 * 1024.0);
    let total_gb = total_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

    let read_count = messages.iter().filter(|m| m.flags.read).count();
    let unread_count = total_count.saturating_sub(read_count);
    let flagged_count = messages.iter().filter(|m| m.flags.flagged).count();
    let deleted_count = messages.iter().filter(|m| m.flags.deleted).count();

    println!("Total Messages:        {}", total_count);
    println!(
        "Total Payload Size:    {} bytes ({:.2} MB / {:.3} GB)",
        total_bytes, total_mb, total_gb
    );
    println!(
        "Message Flags:         {} Read | {} Unread | {} Flagged | {} Deleted",
        read_count, unread_count, flagged_count, deleted_count
    );

    if let (Some(earliest), Some(latest)) = (
        messages.iter().map(|m| m.date).min(),
        messages.iter().map(|m| m.date).max(),
    ) {
        println!(
            "Date Span:             {} -> {}",
            earliest.format("%Y-%m-%d %H:%M:%S UTC"),
            latest.format("%Y-%m-%d %H:%M:%S UTC")
        );
    }

    // Breakdown by year
    let mut year_counts: std::collections::BTreeMap<i32, (usize, usize)> = std::collections::BTreeMap::new();
    for m in messages {
        let entry = year_counts.entry(m.date.year()).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += m.size_bytes;
    }

    println!("\nYear Breakdown:");
    for (year, (count, bytes)) in &year_counts {
        println!(
            "  - {}: {:>6} msgs ({:.2} MB)",
            year,
            count,
            *bytes as f64 / (1024.0 * 1024.0)
        );
    }

    // Folder Breakdown
    let mut folder_counts: std::collections::BTreeMap<String, (usize, usize)> = std::collections::BTreeMap::new();
    for m in messages {
        let folder = if m.folder_path.is_empty() {
            "Inbox".to_string()
        } else {
            m.folder_path.join("/")
        };
        let entry = folder_counts.entry(folder).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += m.size_bytes;
    }

    println!("\nFolder Hierarchy Distribution:");
    for (folder, (count, bytes)) in &folder_counts {
        println!(
            "  - {:<30} {:>6} msgs ({:.2} MB)",
            folder,
            count,
            *bytes as f64 / (1024.0 * 1024.0)
        );
    }

    // Oversized Attachments Warning (> 20 MB / > 25 MB)
    let threshold_20mb = 20 * 1024 * 1024;
    let mut oversized = Vec::new();
    for m in messages {
        for att in &m.attachments {
            if att.size_bytes >= threshold_20mb {
                oversized.push((
                    att.filename.clone(),
                    att.size_bytes,
                    m.subject.clone(),
                    m.folder_path.join("/"),
                ));
            }
        }
    }

    if !oversized.is_empty() {
        println!(
            "\n[!] WARNING: Detected {} Oversized Attachment(s) (>= 20 MB):",
            oversized.len()
        );
        println!("    These files may exceed Outlook MAPI server attachment limits (20-25 MB).");
        for (name, sz, subj, fld) in oversized {
            println!(
                "    * Filename: {} ({:.2} MB) | Subject: {} | Folder: {}",
                name,
                sz as f64 / (1024.0 * 1024.0),
                subj,
                fld
            );
        }
    } else {
        println!("\nAttachment Check:      PASSED (No attachments >= 20 MB found)");
    }

    println!("======================================================================\n");
}

fn ingest_source(source: &Path) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>> {
    let fmt = FormatSniffer::sniff_path(source);
    if fmt == mail2rust_core::MailboxFormat::Pst || fmt == mail2rust_core::MailboxFormat::Ost {
        return Ok(mail2rust_pst::PstReader::read_path(source)?);
    }
    mail2rust_core::ingest_path(source)
}

fn apply_preprocessing(cli: &Cli, messages: &mut Vec<EmailMessage>) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Resume / Checkpoint delta
    if let Some(ref chk_path) = cli.resume {
        if chk_path.exists() {
            let chk = CheckpointManager::load(chk_path)?;
            let (delta, already_done) = CheckpointManager::calculate_delta(messages, &chk);
            info!(
                already_done = already_done,
                remaining = delta.len(),
                "Resuming migration"
            );
            *messages = delta;
        }
    }

    // 2. Date Filtering
    let since_dt = if let Some(ref s) = cli.since {
        Some(parse_cli_datetime(s, false).map_err(|e| format!("Invalid --since: {}", e))?)
    } else {
        None
    };
    let until_dt = if let Some(ref u) = cli.until {
        Some(parse_cli_datetime(u, true).map_err(|e| format!("Invalid --until: {}", e))?)
    } else {
        None
    };

    let date_filter = DateFilter {
        since: since_dt,
        until: until_dt,
        start_year: cli.start_year,
        end_year: cli.end_year,
    };

    if date_filter.is_active() {
        let before_count = messages.len();
        messages.retain(|m| date_filter.matches(&m.date));
        info!(
            retained = messages.len(),
            skipped = before_count - messages.len(),
            "Applied date filtering"
        );
    }

    // 3. Folder Filtering
    let folder_filter = FolderFilter::new(cli.include_folder.clone(), cli.exclude_folder.clone());
    if folder_filter.is_active() {
        let before_count = messages.len();
        messages.retain(|m| folder_filter.matches(&m.folder_path));
        info!(
            retained = messages.len(),
            skipped = before_count - messages.len(),
            "Applied folder filtering"
        );
    }

    // 4. Deleted Message Filtering
    if !cli.include_deleted {
        let before_count = messages.len();
        messages.retain(|m| !m.flags.deleted);
        let skipped = before_count - messages.len();
        if skipped > 0 {
            info!(
                skipped = skipped,
                "Skipped deleted/expunged messages (--include-deleted not set)"
            );
        }
    }

    // 5. Deduplication
    if let Some(strat) = cli.dedup {
        let rs_strat = match strat {
            CliDedupStrategy::Sha256 => DedupStrategy::Sha256,
            CliDedupStrategy::MessageId => DedupStrategy::MessageId,
            CliDedupStrategy::Content => DedupStrategy::NormalizedContent,
        };
        let (unique, dups) = DedupEngine::deduplicate(std::mem::take(messages), rs_strat);
        info!(
            unique = unique.len(),
            duplicates = dups.len(),
            "Deduplicated input messages"
        );
        *messages = unique;
    }

    // 6. Bates Stamping
    if let Some(ref pfx) = cli.bates_prefix {
        ForensicEngine::apply_bates_stamping(messages, pfx, cli.bates_start, 7);
    }

    Ok(())
}

fn post_process(
    cli: &Cli,
    messages: &[EmailMessage],
    source: &Path,
    dest: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(ref manifest_path) = cli.audit_log {
        ForensicEngine::export_manifest_csv(messages, manifest_path)?;
        info!(manifest = ?manifest_path, "Saved audit manifest");
    }

    if let Some(ref state_path) = cli.state_file {
        let mut chk = CheckpointManager::new_checkpoint(&source.display().to_string(), &dest.display().to_string());
        CheckpointManager::commit_run(&mut chk, messages);
        CheckpointManager::save(&chk, state_path)?;
        info!(state = ?state_path, "Saved migration state");
    }

    Ok(())
}
