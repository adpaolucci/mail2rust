use clap::{Parser, Subcommand, ValueEnum};
use mail2rust_core::{
    init_logging, BabylReader, EmailMessage, EmlReader, EmlxReader, FormatSniffer,
    MailboxFormat, MaildirReader, MboxReader, MhReader, MmdfReader, MsgReader, OlmReader,
};
use mail2rust_eml::{EmlExporter, EmlLayout};
use mail2rust_filter::{DedupEngine, DedupStrategy, SplitConfig, VolumeSplitter};
use mail2rust_forensics::{BatesRange, ForensicEngine};
use mail2rust_mbox::MboxFileExporter;
use mail2rust_pst::{PstValidator, PstWriter};
use mail2rust_sync::CheckpointManager;
use std::fs::File;
use std::path::{Path, PathBuf};
use tracing::info;
use walkdir::WalkDir;

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

    #[arg(long, global = true)]
    audit_manifest: Option<PathBuf>,

    #[arg(long, global = true)]
    bates_prefix: Option<String>,

    #[arg(long, global = true, default_value = "1000")]
    bates_start: usize,

    #[arg(long, global = true)]
    resume: Option<PathBuf>,

    #[arg(long, global = true)]
    checkpoint_out: Option<PathBuf>,

    #[arg(long, global = true)]
    dedup: Option<CliDedupStrategy>,

    #[arg(long, global = true)]
    dry_run: bool,

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
    },
    /// Convert any source into Microsoft Outlook PST (pure Rust, zero Outlook required)
    Pst {
        source: PathBuf,
        destination: PathBuf,
        #[arg(long, default_value = "20")]
        max_pst_gb: usize,
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
    },
    /// Convert any source into standard mboxrd MBOX file
    Mbox {
        source: PathBuf,
        destination: PathBuf,
        #[arg(long)]
        compress: bool,
    },
    /// Split a mailbox into multi-volume parts while preserving email threads
    Split {
        source: PathBuf,
        destination_dir: PathBuf,
        #[arg(long, default_value = "20000")]
        max_volume_mb: usize,
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
    ValidatePst {
        pst_file: PathBuf,
    },
    /// Sniff file or folder to identify mailbox format and magic byte signature
    Sniff {
        path: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    init_logging(&cli.log_level, cli.log_file.as_deref())?;

    info!(version = "3.2.0", "Mail2Rust Engine Initialized");

    match &cli.command {
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
            println!("NBT Root BID: 0x{:X} (Offset 0x{:X})", report.nbt_root_bid, report.nbt_root_offset);
            println!("BBT Root BID: 0x{:X} (Offset 0x{:X})", report.bbt_root_bid, report.bbt_root_offset);
            if !report.errors.is_empty() {
                println!("\nErrors Encountered:");
                for e in &report.errors {
                    println!("  - {}", e);
                }
            }
        }

        Commands::Convert { source, destination } => {
            let dest_ext = destination.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            let dest_is_pst = dest_ext == "pst";
            let dest_is_mbox = dest_ext == "mbox" || dest_ext == "gz";

            if dest_is_pst {
                run_pst_conversion(&cli, source, destination, 20, true, true)?;
            } else if dest_is_mbox {
                let compress = dest_ext == "gz";
                run_mbox_conversion(&cli, source, destination, compress)?;
            } else {
                run_eml_conversion(&cli, source, destination, "folder")?;
            }
        }

        Commands::Pst { source, destination, max_pst_gb, cluster_threads, compress } => {
            run_pst_conversion(&cli, source, destination, *max_pst_gb, *cluster_threads, *compress)?;
        }

        Commands::Eml { source, destination, layout } => {
            run_eml_conversion(&cli, source, destination, layout)?;
        }

        Commands::Mbox { source, destination, compress } => {
            run_mbox_conversion(&cli, source, destination, *compress)?;
        }

        Commands::Split { source, destination_dir, max_volume_mb, cluster_threads, target_ext } => {
            info!(source = ?source, dest = ?destination_dir, "Starting volume auto-split");
            let mut messages = ingest_source(source)?;
            apply_preprocessing(&cli, &mut messages)?;

            let cfg = SplitConfig {
                max_volume_bytes: max_volume_mb * 1024 * 1024,
                cluster_threads: *cluster_threads,
                file_prefix: "Volume".to_string(),
                target_ext: target_ext.clone(),
            };

            let volumes = VolumeSplitter::split(&messages, &cfg);
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

        Commands::Audit { source, output_csv, output_json, case_number, examiner } => {
            info!(source = ?source, "Running forensic audit");
            let mut messages = ingest_source(source)?;
            apply_preprocessing(&cli, &mut messages)?;

            let bates_range = if let Some(ref pfx) = cli.bates_prefix {
                ForensicEngine::apply_bates_stamping(&mut messages, pfx, cli.bates_start, 7)
            } else {
                BatesRange { start: "N/A".to_string(), end: "N/A".to_string() }
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
            println!("Total Size: {} bytes ({:.2} MB)", manifest.total_size_bytes, manifest.total_size_bytes as f64 / (1024.0 * 1024.0));
            println!("Root SHA-256 Digest: {}", manifest.root_sha256);
            println!("Bates Range: {} -> {}", manifest.bates_range.start, manifest.bates_range.end);
            println!("CSV Report: {}", output_csv.display());
        }
    }

    Ok(())
}

fn run_pst_conversion(
    cli: &Cli,
    source: &Path,
    destination: &Path,
    max_pst_gb: usize,
    cluster_threads: bool,
    compress: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(source = ?source, dest = ?destination, "Starting PST conversion (zero Outlook dependency)");
    let mut messages = ingest_source(source)?;
    apply_preprocessing(cli, &mut messages)?;

    let max_bytes = max_pst_gb * 1024 * 1024 * 1024;
    let total_bytes: usize = messages.iter().map(|m| m.size_bytes).sum();

    if total_bytes > max_bytes {
        info!("Mailbox size ({} bytes) exceeds single PST limit ({} bytes), auto-splitting", total_bytes, max_bytes);
        let prefix = destination.file_stem().and_then(|s| s.to_str()).unwrap_or("Archive");
        let parent_dir = destination.parent().unwrap_or_else(|| Path::new("."));
        let cfg = SplitConfig {
            max_volume_bytes: max_bytes,
            cluster_threads,
            file_prefix: prefix.to_string(),
            target_ext: "pst".to_string(),
        };
        let volumes = VolumeSplitter::split(&messages, &cfg);
        for vol in &volumes {
            let out_file = parent_dir.join(&vol.file_name);
            write_single_pst(&vol.messages, &out_file, compress)?;
        }
        println!("Exported {} PST volumes successfully", volumes.len());
    } else {
        write_single_pst(&messages, destination, compress)?;
        println!("Exported 1 PST archive successfully ({} messages)", messages.len());
    }

    post_process(cli, &messages, source, destination)?;
    Ok(())
}

fn write_single_pst(messages: &[EmailMessage], destination: &Path, compress: bool) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(destination)?;
    let mut writer = PstWriter::new(file, compress)?;

    let mut folder_map = std::collections::HashMap::new();
    let root_inbox = writer.add_folder("Inbox", None)?;
    folder_map.insert("Inbox".to_string(), root_inbox);

    for m in messages {
        let folder_name = m.folder_path.last().cloned().unwrap_or_else(|| "Inbox".to_string());
        let fnid = match folder_map.get(&folder_name) {
            Some(&nid) => nid,
            None => {
                let new_nid = writer.add_folder(&folder_name, None)?;
                folder_map.insert(folder_name.clone(), new_nid);
                new_nid
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
) -> Result<(), Box<dyn std::error::Error>> {
    info!(source = ?source, dest = ?destination, layout = layout_str, "Starting EML export");
    let mut messages = ingest_source(source)?;
    apply_preprocessing(cli, &mut messages)?;

    let layout = match layout_str.to_lowercase().as_str() {
        "year" => EmlLayout::Year,
        "month" => EmlLayout::Month,
        "flat" => EmlLayout::Flat,
        _ => EmlLayout::Folder,
    };

    let mut exporter = EmlExporter::new(destination, layout);
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
) -> Result<(), Box<dyn std::error::Error>> {
    info!(source = ?source, dest = ?destination, compress = compress, "Starting MBOX export");
    let mut messages = ingest_source(source)?;
    apply_preprocessing(cli, &mut messages)?;

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }

    MboxFileExporter::export_to_file(&messages, destination, compress)?;
    println!("Exported {} messages into mboxrd container: {}", messages.len(), destination.display());
    post_process(cli, &messages, source, destination)?;
    Ok(())
}

fn ingest_source(source: &Path) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error>> {
    let mut messages = Vec::new();
    let format = FormatSniffer::sniff_path(source);
    info!(source = ?source, format = format.name(), "Ingesting mailbox source");

    match format {
        MailboxFormat::Mbox => {
            let file = File::open(source)?;
            let folder = vec![source.file_stem().and_then(|s| s.to_str()).unwrap_or("Inbox").to_string()];
            messages = MboxReader::read_all(file, folder)?;
        }
        MailboxFormat::Maildir => {
            messages = MaildirReader::read_dir(source)?;
        }
        MailboxFormat::Olm => {
            let file = File::open(source)?;
            messages = OlmReader::parse(file, vec!["OLM".to_string()])?;
        }
        MailboxFormat::Mh => {
            messages = MhReader::read_dir(source)?;
        }
        MailboxFormat::Mmdf => {
            let file = File::open(source)?;
            messages = MmdfReader::parse(file, vec!["MMDF".to_string()])?;
        }
        MailboxFormat::Babyl => {
            let file = File::open(source)?;
            messages = BabylReader::parse(file, vec!["Babyl".to_string()])?;
        }
        _ => {
            if source.is_file() {
                let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                let bytes = std::fs::read(source)?;
                if ext == "emlx" {
                    messages.push(EmlxReader::parse(&bytes, vec!["Import".to_string()])?);
                } else if ext == "msg" {
                    messages.push(MsgReader::parse(&bytes, vec!["Import".to_string()])?);
                } else {
                    messages.push(EmlReader::parse(&bytes, vec!["Import".to_string()])?);
                }
            } else if source.is_dir() {
                for entry in WalkDir::new(source).into_iter().filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    let folder = match path.strip_prefix(source) {
                        Ok(rel) => rel.parent().map(|p| p.iter().filter_map(|s| s.to_str().map(|x| x.to_string())).collect()).unwrap_or_default(),
                        Err(_) => vec!["Inbox".to_string()],
                    };

                    if let Ok(bytes) = std::fs::read(path) {
                        if ext == "emlx" {
                            if let Ok(m) = EmlxReader::parse(&bytes, folder) {
                                messages.push(m);
                            }
                        } else if ext == "msg" {
                            if let Ok(m) = MsgReader::parse(&bytes, folder) {
                                messages.push(m);
                            }
                        } else if ext == "eml" || ext.is_empty() {
                            if let Ok(m) = EmlReader::parse(&bytes, folder) {
                                messages.push(m);
                            }
                        }
                    }
                }
            }
        }
    }

    info!(count = messages.len(), "Source ingestion complete");
    Ok(messages)
}

fn apply_preprocessing(cli: &Cli, messages: &mut Vec<EmailMessage>) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(ref chk_path) = cli.resume {
        if chk_path.exists() {
            let chk = CheckpointManager::load(chk_path)?;
            let (delta, already_done) = CheckpointManager::calculate_delta(messages, &chk);
            info!(already_done = already_done, remaining = delta.len(), "Resuming migration");
            *messages = delta;
        }
    }

    if let Some(strat) = cli.dedup {
        let rs_strat = match strat {
            CliDedupStrategy::Sha256 => DedupStrategy::Sha256,
            CliDedupStrategy::MessageId => DedupStrategy::MessageId,
            CliDedupStrategy::Content => DedupStrategy::NormalizedContent,
        };
        let (unique, dups) = DedupEngine::deduplicate(std::mem::take(messages), rs_strat);
        info!(unique = unique.len(), duplicates = dups.len(), "Deduplicated input messages");
        *messages = unique;
    }

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
    if let Some(ref manifest_path) = cli.audit_manifest {
        ForensicEngine::export_manifest_csv(messages, manifest_path)?;
    }

    if let Some(ref chk_out) = cli.checkpoint_out {
        let mut chk = CheckpointManager::new_checkpoint(&source.display().to_string(), &dest.display().to_string());
        CheckpointManager::commit_run(&mut chk, messages);
        CheckpointManager::save(&chk, chk_out)?;
    }

    Ok(())
}
