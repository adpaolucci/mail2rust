use chrono::Datelike;
use clap::{Parser, Subcommand, ValueEnum};
use mail2rust_core::{
    init_logging, EmailMessage, FormatSniffer, MailboxMerger, MergeFolderStrategy, MergeOptions, MergeSourceConfig,
};
use mail2rust_eml::{EmlExporter, EmlLayout, EmlNaming, ManifestRecord, ManifestWriter};
use mail2rust_filter::{
    parse_cli_datetime, parse_size_bytes, DateFilter, DedupEngine, DedupStrategy, FolderFilter, MultiPatternMatcher,
    SplitConfig, SplitMode, VolumeSplitter,
};
use mail2rust_forensics::{BatesRange, ForensicEngine};
use mail2rust_imap::{ImapAuthMethod, OAuthProvider};
use mail2rust_mbox::{MboxFileExporter, MboxWriter};
use mail2rust_pst::{PstEncryption, PstReader, PstValidator, PstWriter};
use mail2rust_sync::CheckpointManager;
use std::fs::File;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::info;

static SHUTDOWN_FLAG: AtomicBool = AtomicBool::new(false);

pub fn is_shutdown_requested() -> bool {
    SHUTDOWN_FLAG.load(Ordering::Relaxed)
}

pub fn init_signal_handler() {
    let _ = ctrlc::set_handler(move || {
        if SHUTDOWN_FLAG.swap(true, Ordering::SeqCst) {
            eprintln!("\n[!] Force terminating on second interrupt.");
            std::process::exit(130);
        } else {
            eprintln!("\n[!] Interrupt received (SIGINT/Ctrl+C). Gracefully saving progress checkpoint before exit...");
        }
    });
}

mod samples;

#[derive(Parser, Debug)]
#[command(
    name = "mail2rust",
    author = "Mail2Rust Contributors",
    version = "3.5.0",
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

    /// Checkpoint interval in number of messages before writing incremental progress
    #[arg(long, global = true, default_value = "250")]
    checkpoint_interval: usize,

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

    /// Launch native host OS file/folder picker dialog if paths are omitted
    #[arg(long, global = true, alias = "browse")]
    pick: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum CliDedupStrategy {
    Sha256,
    MessageId,
    Content,
    None,
}

impl From<CliDedupStrategy> for DedupStrategy {
    fn from(val: CliDedupStrategy) -> Self {
        match val {
            CliDedupStrategy::Sha256 => DedupStrategy::Sha256,
            CliDedupStrategy::MessageId => DedupStrategy::MessageId,
            CliDedupStrategy::Content => DedupStrategy::NormalizedContent,
            CliDedupStrategy::None => DedupStrategy::None,
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Default)]
enum CliPstEncryption {
    None,
    #[default]
    Permute,
    Cyclic,
}

impl From<CliPstEncryption> for PstEncryption {
    fn from(val: CliPstEncryption) -> Self {
        match val {
            CliPstEncryption::None => PstEncryption::None,
            CliPstEncryption::Permute => PstEncryption::Permute,
            CliPstEncryption::Cyclic => PstEncryption::Cyclic,
        }
    }
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Convert any email/mailbox format to another (auto-detects source & destination)
    Convert {
        #[arg(value_name = "SOURCE")]
        source: Option<PathBuf>,
        #[arg(value_name = "DESTINATION")]
        destination: Option<PathBuf>,
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
        #[arg(long, default_value = "permute")]
        encryption: CliPstEncryption,
        #[arg(long)]
        password: Option<String>,
        /// Filter by search rules file (.toml, .json, or .rules text)
        #[arg(long)]
        search_rules: Option<PathBuf>,
        /// Filter by regex pattern
        #[arg(long)]
        search_pattern: Option<String>,
        /// Automatically generate clickable, sortable index.html in output directory
        #[arg(long)]
        html_index: bool,
        /// Path for chain-of-custody CSV manifest export
        #[arg(long)]
        manifest_csv: Option<PathBuf>,
    },
    /// Convert any source into Microsoft Outlook PST (pure Rust, zero Outlook required)
    Pst {
        #[arg(value_name = "SOURCE")]
        source: Option<PathBuf>,
        #[arg(value_name = "DESTINATION")]
        destination: Option<PathBuf>,
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
        #[arg(long, default_value = "permute")]
        encryption: CliPstEncryption,
        #[arg(long)]
        password: Option<String>,
    },
    /// Convert any source into clean EML files with customizable directory layout
    Eml {
        #[arg(value_name = "SOURCE")]
        source: Option<PathBuf>,
        #[arg(value_name = "DESTINATION")]
        destination: Option<PathBuf>,
        #[arg(long, default_value = "folder")]
        layout: String,
        #[arg(long, default_value = "subject")]
        naming: String,
        #[arg(long, default_value = "0")]
        max_per_dir: usize,
        /// Automatically generate clickable, sortable index.html in output directory
        #[arg(long)]
        html_index: bool,
        /// Path for chain-of-custody CSV manifest export
        #[arg(long)]
        manifest_csv: Option<PathBuf>,
    },
    /// High-throughput zero-copy regex e-Discovery search and extraction
    Search {
        /// Source directory, Thunderbird profile, MBOX, or PST
        #[arg(value_name = "SOURCE")]
        source: Option<PathBuf>,
        /// Destination directory for matched messages and deliverables
        #[arg(value_name = "DESTINATION")]
        destination: Option<PathBuf>,
        /// Path to search rules file (.toml, .json, or .rules text)
        #[arg(long, short = 'r')]
        rules: Option<PathBuf>,
        /// Inline regex pattern(s) to match
        #[arg(long, short = 'p')]
        pattern: Vec<String>,
        /// Target output format: 'eml', 'pst', 'mbox', or 'manifest-only'
        #[arg(long, default_value = "eml")]
        format: String,
        /// Automatically generate clickable, sortable index.html in output directory
        #[arg(long)]
        html_index: bool,
        /// Path for chain-of-custody CSV manifest export
        #[arg(long)]
        manifest_csv: Option<PathBuf>,
        /// PST container encryption ('none', 'permute', 'cyclic')
        #[arg(long, default_value = "permute")]
        encryption: CliPstEncryption,
        /// Optional password for PST container
        #[arg(long)]
        password: Option<String>,
    },
    /// Convert any source into standard mboxrd MBOX file
    Mbox {
        #[arg(value_name = "SOURCE")]
        source: Option<PathBuf>,
        #[arg(value_name = "DESTINATION")]
        destination: Option<PathBuf>,
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
    ValidatePst {
        #[arg(value_name = "PST_FILE")]
        pst_file: Option<PathBuf>,
        #[arg(long)]
        password: Option<String>,
    },
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
        /// Authentication method: 'password', 'xoauth2', 'oauthbearer'
        #[arg(long, default_value = "password")]
        auth_method: String,
        /// OAuth2 Bearer token (for Google, Yahoo, Microsoft)
        #[arg(long)]
        oauth_token: Option<String>,
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
    Sniff {
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,
    },
    /// Launch native host OS file/folder picker dialog and output the selected path
    Pick {
        /// Dialog type to launch: 'file', 'files', 'folder', or 'save'
        #[arg(short, long, default_value = "file")]
        kind: String,
        /// Dialog window title
        #[arg(long)]
        title: Option<String>,
        /// Default file extension for save dialog (e.g. "pst", "mbox", "csv")
        #[arg(long, default_value = "pst")]
        ext: String,
        /// Initial folder to browse
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Generate a comprehensive multi-format sample and demo suite
    GenerateSamples {
        #[arg(short, long, alias = "output", default_value = "samples")]
        output_dir: PathBuf,
    },
    /// Launch the pure-Rust native GUI studio (zero Node.js or WebView2 dependencies)
    Gui,
    /// Direct server-to-server IMAP migration between accounts with zero local disk storage
    ImapSync {
        /// Source IMAP server hostname
        #[arg(long)]
        src_host: String,
        /// Source IMAP port
        #[arg(long, default_value = "993")]
        src_port: u16,
        /// Source IMAP username
        #[arg(long)]
        src_user: String,
        /// Source IMAP password
        #[arg(long)]
        src_pass: Option<String>,
        /// Source authentication method: 'password', 'xoauth2', 'oauthbearer'
        #[arg(long, default_value = "password")]
        src_auth_method: String,
        /// Source OAuth2 Bearer token
        #[arg(long)]
        src_oauth_token: Option<String>,
        /// Disable TLS on source connection
        #[arg(long)]
        no_src_tls: bool,

        /// Destination IMAP server hostname
        #[arg(long)]
        dst_host: String,
        /// Destination IMAP port
        #[arg(long, default_value = "993")]
        dst_port: u16,
        /// Destination IMAP username
        #[arg(long)]
        dst_user: String,
        /// Destination IMAP password
        #[arg(long)]
        dst_pass: Option<String>,
        /// Destination authentication method: 'password', 'xoauth2', 'oauthbearer'
        #[arg(long, default_value = "password")]
        dst_auth_method: String,
        /// Destination OAuth2 Bearer token
        #[arg(long)]
        dst_oauth_token: Option<String>,
        /// Disable TLS on destination connection
        #[arg(long)]
        no_dst_tls: bool,

        /// Accept self-signed / insecure TLS certificates
        #[arg(long)]
        insecure: bool,
        /// Specific folders to migrate (comma-separated or repeated)
        #[arg(long)]
        folders: Vec<String>,
        /// Prefix for created destination folders (e.g., 'Migrated/')
        #[arg(long)]
        folder_prefix: Option<String>,
        /// Maximum messages to migrate per folder
        #[arg(long)]
        max_per_folder: Option<usize>,
        /// Do not preserve original IMAP flags (\Seen, \Flagged, etc.)
        #[arg(long)]
        no_flags: bool,
        /// Do not preserve original message internal delivery dates
        #[arg(long)]
        no_dates: bool,
        /// Dry-run mode: scan and verify connections without uploading messages
        #[arg(long)]
        dry_run: bool,
    },
    /// Merge disparate mailbox files, directories, and formats into a unified container
    Merge {
        /// Source mailbox paths to merge (files, directories, or archives)
        #[arg(required = true, num_args = 2..)]
        sources: Vec<PathBuf>,

        /// Destination output path (.pst, .mbox, directory for .eml)
        #[arg(short, long, alias = "destination", required = true)]
        output: PathBuf,

        /// Folder organization strategy: 'combine' (merge matching folders), 'prefix' (prefix with source alias), 'flatten'
        #[arg(long, default_value = "combine")]
        folder_strategy: String,

        /// Custom source aliases / prefixes (comma-separated, matched by position to sources)
        #[arg(long)]
        source_prefixes: Option<String>,

        /// Sort messages chronologically by date
        #[arg(long, default_value = "true")]
        sort_by_date: bool,

        /// Target format override ('pst', 'mbox', 'eml')
        #[arg(long)]
        format: Option<String>,

        /// PST split size if target is PST (e.g. '20GB', '500MB')
        #[arg(long, alias = "split-pst")]
        max_pst_size: Option<String>,

        /// PST encryption mode: 'none', 'permute', 'cyclic'
        #[arg(long, default_value = "permute")]
        encryption: CliPstEncryption,

        /// Optional password for PST output
        #[arg(long)]
        password: Option<String>,

        /// EML layout if destination is EML: 'folder', 'year', 'month', 'flat'
        #[arg(long, default_value = "folder")]
        eml_layout: String,

        /// Dry-run mode: analyze sources and display merge summary without writing
        #[arg(long)]
        dry_run: bool,
    },
    /// Acquire OAuth2 access and refresh tokens for Google, Microsoft, or Yahoo
    #[command(name = "oauth-login")]
    OAuthLogin {
        /// Provider: 'google', 'microsoft', 'yahoo'
        #[arg(long, default_value = "google")]
        provider: String,
        /// OAuth2 Application Client ID
        #[arg(long)]
        client_id: String,
        /// OAuth2 Application Client Secret
        #[arg(long)]
        client_secret: String,
        /// Local redirect loopback port (default: 8089)
        #[arg(long, default_value = "8089")]
        port: u16,
        /// Save tokens to local JSON file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Ingest mail directly from Fastmail or any RFC 8620/8621 JMAP server into PST, MBOX, or EML
    Jmap {
        /// JMAP session URL (default: Fastmail https://api.fastmail.com/jmap/session)
        #[arg(long, default_value = "https://api.fastmail.com/jmap/session")]
        session_url: String,

        /// JMAP API Bearer token (e.g. Fastmail API token)
        #[arg(short, long, default_value = "")]
        token: String,

        /// Destination output path (.pst, .mbox, directory for .eml)
        #[arg(short, long)]
        output: PathBuf,

        /// Specific folders/mailboxes to migrate (case-insensitive name match)
        #[arg(long)]
        folders: Vec<String>,

        /// Path to checkpoint file for resume and incremental delta synchronization
        #[arg(long)]
        checkpoint: Option<PathBuf>,
    },
    /// Print instructions for generating App Passwords for Google, Microsoft, Yahoo, or Proton Mail
    AppPasswords {
        /// Specific provider to query: 'google', 'microsoft', 'yahoo', 'proton', or 'all'
        #[arg(default_value = "all")]
        provider: String,
    },
    /// Run Model Context Protocol (MCP) server over standard I/O (JSON-RPC)
    Mcp {
        /// Print tools manifest in JSON schema format and exit
        #[arg(long)]
        tools: bool,
    },
}

fn pick_source_dialog(title_opt: Option<&str>) -> Option<PathBuf> {
    if std::io::stdin().is_terminal() {
        eprintln!("\nSelect source mail container type:");
        eprintln!("  [1] Mailbox Archive File (.pst, .mbox, .eml, .msg, .olm, .zip, etc.)");
        eprintln!("  [2] Directory (Thunderbird profile, Maildir, EML folder tree)");
        eprint!("Choice [1/2] (default: 1): ");
        let mut choice = String::new();
        let _ = std::io::stdin().read_line(&mut choice);
        let trimmed = choice.trim().to_lowercase();
        if trimmed == "2" || trimmed == "d" || trimmed == "dir" || trimmed == "folder" {
            return rfd::FileDialog::new()
                .set_title(title_opt.unwrap_or("Select Mailbox Directory"))
                .pick_folder();
        }
    }
    rfd::FileDialog::new()
        .set_title(title_opt.unwrap_or("Select Mailbox Archive File"))
        .add_filter(
            "All Supported Mail Files (*.pst, *.ost, *.mbox, *.eml, *.emlx, *.msg, *.olm, *.zip, *.gz)",
            &[
                "pst", "ost", "mbox", "mbx", "eml", "emlx", "msg", "olm", "zip", "gz", "bz2", "xz", "tar", "tgz",
            ],
        )
        .add_filter("Outlook PST / OST (*.pst, *.ost)", &["pst", "ost"])
        .add_filter("MBOX Mailbox (*.mbox, *.mbx)", &["mbox", "mbx"])
        .add_filter("Email Files (*.eml, *.emlx, *.msg)", &["eml", "emlx", "msg"])
        .add_filter("All Files (*.*)", &["*"])
        .pick_file()
}

fn pick_save_dialog(default_ext: &str, title_opt: Option<&str>) -> Option<PathBuf> {
    let clean_ext = default_ext.trim_start_matches('.');
    let mut dlg = rfd::FileDialog::new()
        .set_title(title_opt.unwrap_or("Select Destination Output File"))
        .set_file_name(format!(
            "output.{}",
            if clean_ext.is_empty() { "pst" } else { clean_ext }
        ));
    if clean_ext.eq_ignore_ascii_case("pst") {
        dlg = dlg.add_filter("Outlook PST (*.pst)", &["pst"]);
    } else if clean_ext.eq_ignore_ascii_case("mbox") {
        dlg = dlg.add_filter("MBOX Mailbox (*.mbox)", &["mbox"]);
    } else if clean_ext.eq_ignore_ascii_case("csv") {
        dlg = dlg.add_filter("CSV Spreadsheet (*.csv)", &["csv"]);
    }
    dlg = dlg.add_filter("All Files (*.*)", &["*"]);
    dlg.save_file()
}

fn resolve_source(source: Option<&PathBuf>, pick: bool, title: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(s) = source {
        return Ok(s.clone());
    }
    if pick || std::io::stdin().is_terminal() {
        println!("Opening native host OS picker for source mail file or directory...");
        if let Some(picked) = pick_source_dialog(Some(title)) {
            return Ok(picked);
        }
        return Err("Operation cancelled: No source selected.".into());
    }
    Err("A source path is required. (Hint: pass --pick to select via native file dialog)".into())
}

fn resolve_destination(
    destination: Option<&PathBuf>,
    pick: bool,
    default_ext: &str,
    is_dir: bool,
    title: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(d) = destination {
        return Ok(d.clone());
    }
    if pick || std::io::stdin().is_terminal() {
        println!("Opening native host OS picker for destination...");
        if is_dir {
            if let Some(folder) = rfd::FileDialog::new().set_title(title).pick_folder() {
                return Ok(folder);
            }
        } else if let Some(file) = pick_save_dialog(default_ext, Some(title)) {
            return Ok(file);
        }
        return Err("Operation cancelled: No destination selected.".into());
    }
    Err("A destination path is required. (Hint: pass --pick to select via native file dialog)".into())
}

fn resolve_file(
    file_opt: Option<&PathBuf>,
    pick: bool,
    ext: &str,
    title: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(f) = file_opt {
        return Ok(f.clone());
    }
    if pick || std::io::stdin().is_terminal() {
        println!("Opening native host OS file picker...");
        let mut dlg = rfd::FileDialog::new().set_title(title);
        let clean_ext = ext.trim_start_matches('.');
        if !clean_ext.is_empty() {
            dlg = dlg.add_filter(format!("*.{} Files", clean_ext), &[clean_ext]);
        }
        dlg = dlg.add_filter("All Files (*.*)", &["*"]);
        if let Some(picked) = dlg.pick_file() {
            return Ok(picked);
        }
        return Err("Operation cancelled: No file selected.".into());
    }
    Err("A file path is required. (Hint: pass --pick to select via native file dialog)".into())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_signal_handler();
    let cli = Cli::parse();

    if let Commands::Mcp { tools } = &cli.command {
        if *tools {
            println!(
                "{}",
                serde_json::to_string_pretty(&mail2rust_mcp::get_tools_manifest())?
            );
            return Ok(());
        }
        let filter = tracing_subscriber::EnvFilter::try_new(&cli.log_level)
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_env_filter(filter)
            .init();

        mail2rust_mcp::run_stdio_server()?;
        return Ok(());
    }

    init_logging(&cli.log_level, cli.log_file.as_deref())?;

    info!(version = env!("CARGO_PKG_VERSION"), "Mail2Rust Engine Initialized");

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
            let p = resolve_source(path.as_ref(), cli.pick, "Select Mail File or Directory to Sniff")?;
            let format = FormatSniffer::sniff_path(&p);
            println!("Target Path: {}", p.display());
            println!("Identified Format: {}", format.name());
            println!("Format Type: {:?}", format);
        }

        Commands::GenerateSamples { output_dir } => {
            println!(
                "Generating comprehensive multi-format sample suite into: {}",
                output_dir.display()
            );
            samples::generate_all_samples(output_dir)?;
            println!(
                "Successfully generated all sample datasets in: {}",
                output_dir.display()
            );
        }

        Commands::ValidatePst { pst_file, password } => {
            let pst = resolve_file(
                pst_file.as_ref(),
                cli.pick,
                "pst",
                "Select PST / OST Container to Validate",
            )?;
            info!(path = ?pst, "Running MS-PST specification validation");
            let file = File::open(&pst)?;
            let report = PstValidator::validate(file)?;
            println!("\n=== MS-PST Validation Report ===");
            println!("File: {}", pst.display());
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

            if let Ok(file_for_reader) = File::open(&pst) {
                if let Ok(mut reader) = PstReader::open(file_for_reader) {
                    let has_pwd = reader.is_password_protected();
                    println!("Password Protected: {}", has_pwd);
                    if has_pwd {
                        if let Some(ref pwd) = password {
                            let verified = reader.verify_password(pwd);
                            println!(
                                "Password Verification: {}",
                                if verified {
                                    "SUCCESS (MATCH)"
                                } else {
                                    "FAILED (INCORRECT)"
                                }
                            );
                        } else {
                            println!("Password Verification: (No password provided; pass --password <str> to verify)");
                        }
                    }
                }
            }
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
            encryption,
            password,
            search_rules,
            search_pattern,
            html_index,
            manifest_csv,
        } => {
            let src = resolve_source(source.as_ref(), cli.pick, "Select Mail Source File or Directory")?;
            let dst = resolve_destination(
                destination.as_ref(),
                cli.pick,
                "pst",
                false,
                "Select Output Destination",
            )?;

            if search_rules.is_some() || search_pattern.is_some() {
                let patterns: Vec<String> = search_pattern.iter().cloned().collect();
                let fmt = dst.extension().and_then(|e| e.to_str()).unwrap_or("eml");
                return run_search_discovery(
                    &cli,
                    &src,
                    &dst,
                    search_rules.as_deref(),
                    &patterns,
                    fmt,
                    *html_index,
                    manifest_csv.as_deref(),
                    (*encryption).into(),
                    password.as_deref(),
                );
            }

            let dest_ext = dst.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
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
                    &src,
                    &dst,
                    max_gb,
                    split_by.as_deref(),
                    *splits,
                    pst_root.as_deref(),
                    *flatten,
                    true,
                    (*encryption).into(),
                    password.as_deref(),
                )?;
            } else if dest_is_mbox {
                let compress = dest_ext == "gz";
                run_mbox_conversion(&cli, &src, &dst, compress, false)?;
            } else {
                run_eml_conversion(
                    &cli,
                    &src,
                    &dst,
                    "folder",
                    "subject",
                    0,
                    *html_index,
                    manifest_csv.as_deref(),
                )?;
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
            encryption,
            password,
        } => {
            let src = resolve_source(source.as_ref(), cli.pick, "Select Mail Source File or Directory")?;
            let dst = resolve_destination(destination.as_ref(), cli.pick, "pst", false, "Select Output PST File")?;
            let effective_gb = if let Some(ref sz) = max_pst_size {
                parse_size_bytes(sz).unwrap_or(max_pst_gb * 1024 * 1024 * 1024) / (1024 * 1024 * 1024)
            } else {
                *max_pst_gb
            };
            let enc: PstEncryption = if !*compress && *encryption == CliPstEncryption::Permute {
                PstEncryption::None
            } else {
                (*encryption).into()
            };
            run_pst_conversion(
                &cli,
                &src,
                &dst,
                effective_gb,
                split_by.as_deref(),
                *splits,
                pst_root.as_deref(),
                *flatten,
                *cluster_threads,
                enc,
                password.as_deref(),
            )?;
        }

        Commands::Eml {
            source,
            destination,
            layout,
            naming,
            max_per_dir,
            html_index,
            manifest_csv,
        } => {
            let src = resolve_source(source.as_ref(), cli.pick, "Select Mail Source File or Directory")?;
            let dst = resolve_destination(destination.as_ref(), cli.pick, "", true, "Select Output EML Directory")?;
            run_eml_conversion(
                &cli,
                &src,
                &dst,
                layout,
                naming,
                *max_per_dir,
                *html_index,
                manifest_csv.as_deref(),
            )?;
        }

        Commands::Search {
            source,
            destination,
            rules,
            pattern,
            format,
            html_index,
            manifest_csv,
            encryption,
            password,
        } => {
            let src = resolve_source(
                source.as_ref(),
                cli.pick,
                "Select Mail Source File or Directory to Search",
            )?;
            let dst = resolve_destination(
                destination.as_ref(),
                cli.pick,
                "",
                true,
                "Select Output Deliverables Directory",
            )?;
            run_search_discovery(
                &cli,
                &src,
                &dst,
                rules.as_deref(),
                pattern,
                format,
                *html_index,
                manifest_csv.as_deref(),
                (*encryption).into(),
                password.as_deref(),
            )?;
        }

        Commands::Mbox {
            source,
            destination,
            compress,
            append,
            split_size: _,
        } => {
            let src = resolve_source(source.as_ref(), cli.pick, "Select Mail Source File or Directory")?;
            let dst = resolve_destination(destination.as_ref(), cli.pick, "mbox", false, "Select Output MBOX File")?;
            run_mbox_conversion(&cli, &src, &dst, *compress, *append)?;
        }

        Commands::Pick { kind, title, ext, dir } => {
            let mut dlg = rfd::FileDialog::new();
            if let Some(ref t) = title {
                dlg = dlg.set_title(t);
            }
            if let Some(ref d) = dir {
                dlg = dlg.set_directory(d);
            }
            match kind.to_lowercase().as_str() {
                "folder" | "dir" | "directory" => {
                    if let Some(folder) = dlg.pick_folder() {
                        println!("{}", folder.display());
                    } else {
                        std::process::exit(1);
                    }
                }
                "save" => {
                    let clean_ext = ext.trim_start_matches('.');
                    let filter_label = format!("*.{} Files", clean_ext);
                    dlg = dlg
                        .add_filter(&filter_label, &[clean_ext])
                        .add_filter("All Files (*.*)", &["*"])
                        .set_file_name(format!("output.{}", clean_ext));
                    if let Some(file) = dlg.save_file() {
                        println!("{}", file.display());
                    } else {
                        std::process::exit(1);
                    }
                }
                "files" | "multi" => {
                    dlg = dlg
                        .add_filter(
                            "All Supported Mail Files (*.pst, *.ost, *.mbox, *.eml, *.emlx, *.msg, *.olm, *.zip, *.gz)",
                            &[
                                "pst", "ost", "mbox", "mbx", "eml", "emlx", "msg", "olm", "zip", "gz", "bz2", "xz",
                                "tar", "tgz",
                            ],
                        )
                        .add_filter("All Files (*.*)", &["*"]);
                    if let Some(files) = dlg.pick_files() {
                        for f in files {
                            println!("{}", f.display());
                        }
                    } else {
                        std::process::exit(1);
                    }
                }
                _ => {
                    dlg = dlg
                        .add_filter(
                            "All Supported Mail Files (*.pst, *.ost, *.mbox, *.eml, *.emlx, *.msg, *.olm, *.zip, *.gz)",
                            &[
                                "pst", "ost", "mbox", "mbx", "eml", "emlx", "msg", "olm", "zip", "gz", "bz2", "xz",
                                "tar", "tgz",
                            ],
                        )
                        .add_filter("Outlook PST / OST (*.pst, *.ost)", &["pst", "ost"])
                        .add_filter("MBOX Mailbox (*.mbox, *.mbx)", &["mbox", "mbx"])
                        .add_filter("All Files (*.*)", &["*"]);
                    if let Some(file) = dlg.pick_file() {
                        println!("{}", file.display());
                    } else {
                        std::process::exit(1);
                    }
                }
            }
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
            auth_method,
            oauth_token,
            no_tls,
            insecure,
            output,
            folders,
            max_per_folder,
            since,
            checkpoint,
        } => {
            let auth: ImapAuthMethod = auth_method
                .parse()
                .map_err(|e: String| format!("Invalid --auth-method: {}", e))?;
            let oauth_tok = oauth_token
                .clone()
                .or_else(|| std::env::var("MAIL2RUST_OAUTH_TOKEN").ok());

            let pass = if auth != ImapAuthMethod::Password {
                oauth_tok.clone().unwrap_or_default()
            } else {
                match password {
                    Some(p) => p.clone(),
                    None => match std::env::var("MAIL2RUST_IMAP_PASSWORD") {
                        Ok(p) => p,
                        Err(_) => {
                            eprint!("Enter IMAP authentication password: ");
                            let mut line = String::new();
                            std::io::stdin().read_line(&mut line)?;
                            line.trim().to_string()
                        }
                    },
                }
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
                auth_method: auth,
                oauth_token: oauth_tok,
            };

            println!("Connecting to IMAP {}:{} (TLS: {})...", host, port, use_tls);
            let mut client = mail2rust_imap::ImapClient::new(config);
            client.connect()?;
            println!("Authenticated successfully to IMAP server.");

            let chk_path = checkpoint
                .as_ref()
                .or(cli.state_file.as_ref())
                .or(cli.resume.as_ref())
                .map(|p| p.as_path());
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
            client.sync_all_with_checkpoint_path(
                &mut chk,
                chk_path,
                cli.checkpoint_interval,
                Some(&SHUTDOWN_FLAG),
                |prog, msg| {
                    use std::io::Write;
                    print!(
                        "\r[IMAP Sync] Folder: {:<20} | Msg #{:<5} | Item ID: {:<8}",
                        prog.current_folder, prog.current_message_index, prog.message_seq_id
                    );
                    let _ = std::io::stdout().flush();
                    messages.push(msg);
                },
            )?;
            println!("\nFetched {} messages from IMAP server.", messages.len());

            if is_shutdown_requested() {
                eprintln!("\n[!] Gracefully halted on user interrupt.");
                if let Some(p) = chk_path {
                    eprintln!("[*] Interrupted migration state saved to: {}", p.display());
                    eprintln!("[*] To resume this migration from where it was paused, run:");
                    eprintln!("    mail2rust imap ... --resume \"{}\"", p.display());
                }
                std::process::exit(130);
            }

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

        Commands::ImapSync {
            src_host,
            src_port,
            src_user,
            src_pass,
            src_auth_method,
            src_oauth_token,
            no_src_tls,
            dst_host,
            dst_port,
            dst_user,
            dst_pass,
            dst_auth_method,
            dst_oauth_token,
            no_dst_tls,
            insecure,
            folders,
            folder_prefix,
            max_per_folder,
            no_flags,
            no_dates,
            dry_run,
        } => {
            let src_auth: ImapAuthMethod = src_auth_method
                .parse()
                .map_err(|e: String| format!("Invalid --src-auth-method: {}", e))?;
            let src_oauth = src_oauth_token
                .clone()
                .or_else(|| std::env::var("MAIL2RUST_SRC_OAUTH_TOKEN").ok());

            let s_pass = if src_auth != ImapAuthMethod::Password {
                src_oauth.clone().unwrap_or_default()
            } else {
                match src_pass {
                    Some(p) => p.clone(),
                    None => match std::env::var("MAIL2RUST_SRC_IMAP_PASSWORD") {
                        Ok(p) => p,
                        Err(_) => {
                            eprint!("Enter Source IMAP Password / App Password for {}: ", src_user);
                            let mut line = String::new();
                            std::io::stdin().read_line(&mut line)?;
                            line.trim().to_string()
                        }
                    },
                }
            };

            let dst_auth: ImapAuthMethod = dst_auth_method
                .parse()
                .map_err(|e: String| format!("Invalid --dst-auth-method: {}", e))?;
            let dst_oauth = dst_oauth_token
                .clone()
                .or_else(|| std::env::var("MAIL2RUST_DST_OAUTH_TOKEN").ok());

            let d_pass = if dst_auth != ImapAuthMethod::Password {
                dst_oauth.clone().unwrap_or_default()
            } else {
                match dst_pass {
                    Some(p) => p.clone(),
                    None => match std::env::var("MAIL2RUST_DST_IMAP_PASSWORD") {
                        Ok(p) => p,
                        Err(_) => {
                            eprint!("Enter Destination IMAP Password / App Password for {}: ", dst_user);
                            let mut line = String::new();
                            std::io::stdin().read_line(&mut line)?;
                            line.trim().to_string()
                        }
                    },
                }
            };

            let folder_filter = if folders.is_empty() {
                None
            } else {
                Some(folders.clone())
            };

            let config = mail2rust_imap::ImapMigrationConfig {
                source: mail2rust_imap::ImapConfig {
                    host: src_host.clone(),
                    port: *src_port,
                    username: src_user.clone(),
                    password: s_pass,
                    use_tls: !no_src_tls,
                    insecure_certs: *insecure,
                    folder_filter: folder_filter.clone(),
                    max_messages_per_folder: *max_per_folder,
                    since_date: None,
                    auth_method: src_auth,
                    oauth_token: src_oauth,
                },
                destination: mail2rust_imap::ImapConfig {
                    host: dst_host.clone(),
                    port: *dst_port,
                    username: dst_user.clone(),
                    password: d_pass,
                    use_tls: !no_dst_tls,
                    insecure_certs: *insecure,
                    folder_filter: None,
                    max_messages_per_folder: None,
                    since_date: None,
                    auth_method: dst_auth,
                    oauth_token: dst_oauth,
                },
                folder_filter,
                folder_prefix: folder_prefix.clone(),
                folder_mapping: None,
                preserve_flags: !no_flags,
                preserve_dates: !no_dates,
                create_missing_folders: true,
                dry_run: *dry_run,
            };

            println!("\n======================================================================");
            println!("             IMAP SERVER-TO-SERVER DIRECT MIGRATION");
            println!("======================================================================");
            println!(
                "Source:        {}@{}:{} (TLS: {})",
                src_user, src_host, src_port, !no_src_tls
            );
            println!(
                "Destination:   {}@{}:{} (TLS: {})",
                dst_user, dst_host, dst_port, !no_dst_tls
            );
            println!("Storage Mode:  Direct Socket-to-Socket Streaming (Zero Local Disk Saving)");
            println!(
                "Preserve:      Flags: {} | Dates: {} | Auto-Create Folders: Yes",
                !no_flags, !no_dates
            );
            if *dry_run {
                println!("Run Mode:      DRY RUN (Scan only, no messages will be uploaded)");
            }
            println!("======================================================================\n");

            let summary = mail2rust_imap::ImapMigrator::migrate(&config, |prog| {
                let subj = prog.current_subject.as_deref().unwrap_or("(No Subject)");
                let subj_truncated = if subj.len() > 40 {
                    format!("{}...", &subj[..37])
                } else {
                    subj.to_string()
                };
                let mb = (prog.total_bytes_transferred as f64) / (1024.0 * 1024.0);
                println!(
                    "[{}/{}] [{}] Msg #{} (Item ID {}) \"{}\" | Total: {} msgs ({:.2} MB)",
                    prog.folder_index,
                    prog.total_folders,
                    prog.current_target_folder,
                    prog.message_index_in_folder,
                    prog.message_seq_id,
                    subj_truncated,
                    prog.total_messages_migrated,
                    mb
                );
            })?;

            let total_mb = (summary.total_bytes_transferred as f64) / (1024.0 * 1024.0);
            let speed_msg = if summary.duration_seconds > 0.0 {
                (summary.total_messages_migrated as f64) / summary.duration_seconds
            } else {
                0.0
            };
            let speed_mb = if summary.duration_seconds > 0.0 {
                total_mb / summary.duration_seconds
            } else {
                0.0
            };

            println!("\n======================================================================");
            println!("                     IMAP MIGRATION SUMMARY");
            println!("======================================================================");
            println!("Status:                SUCCESS");
            println!("Folders Processed:     {}", summary.total_folders_migrated);
            println!("Messages Transferred:  {}", summary.total_messages_migrated);
            println!(
                "Data Transferred:      {:.2} MB ({} bytes)",
                total_mb, summary.total_bytes_transferred
            );
            println!(
                "Duration:              {:.2}s | Speed: {:.1} msg/s ({:.2} MB/s)",
                summary.duration_seconds, speed_msg, speed_mb
            );
            println!("Disk Footprint:        0 bytes (No local files saved)");
            if !summary.errors.is_empty() {
                println!("Warnings / Errors:     {}", summary.errors.len());
                for err in &summary.errors {
                    println!("  - {}", err);
                }
            }
            println!("======================================================================\n");
        }

        Commands::Merge {
            sources,
            output,
            folder_strategy,
            source_prefixes,
            sort_by_date,
            format,
            max_pst_size,
            encryption,
            password,
            eml_layout,
            dry_run,
        } => {
            run_merge_conversion(
                &cli,
                sources,
                output,
                folder_strategy,
                source_prefixes.as_deref(),
                *sort_by_date,
                format.as_deref(),
                max_pst_size.as_deref(),
                (*encryption).into(),
                password.as_deref(),
                eml_layout,
                *dry_run,
            )?;
        }

        Commands::OAuthLogin {
            provider,
            client_id,
            client_secret,
            port,
            output,
        } => {
            let prov = match provider.to_lowercase().as_str() {
                "google" | "gmail" => OAuthProvider::Google,
                "microsoft" | "outlook" | "office365" | "m365" => OAuthProvider::Microsoft,
                "yahoo" | "aol" => OAuthProvider::Yahoo,
                other => {
                    return Err(format!("Unknown provider '{}'. Supported: google, microsoft, yahoo", other).into());
                }
            };

            let redirect_uri = format!("http://127.0.0.1:{}/callback", port);
            let state = uuid::Uuid::new_v4().to_string();
            let auth_url = mail2rust_imap::build_authorization_url(prov, client_id, &redirect_uri, &state);

            println!("\n======================================================================");
            println!("               OAUTH 2.0 BROWSER AUTHORIZATION HELPER                  ");
            println!("======================================================================");
            println!("Provider:     {}", prov.display_name());
            println!("Redirect URI: {}", redirect_uri);
            println!("\nPlease open the following authorization URL in your browser to sign in:\n");
            println!("  {}\n", auth_url);

            if cfg!(windows) {
                let _ = std::process::Command::new("cmd")
                    .args(["/C", "start", "", &auth_url])
                    .spawn();
            } else if cfg!(target_os = "macos") {
                let _ = std::process::Command::new("open").arg(&auth_url).spawn();
            } else {
                let _ = std::process::Command::new("xdg-open").arg(&auth_url).spawn();
            }

            println!("Listening for authorization redirect on port {}...", port);
            println!("(Press Ctrl+C to cancel)\n");

            let listener = std::net::TcpListener::bind(format!("127.0.0.1:{}", port))?;
            let (mut socket, _) = listener.accept()?;

            let mut buf = [0u8; 4096];
            let n = std::io::Read::read(&mut socket, &mut buf)?;
            let req = String::from_utf8_lossy(&buf[..n]);

            let code = req
                .lines()
                .next()
                .and_then(|line| {
                    if let Some(query_start) = line.find('?') {
                        let end = line.find(" HTTP/").unwrap_or(line.len());
                        let query = &line[query_start + 1..end];
                        for pair in query.split('&') {
                            if let Some(c) = pair.strip_prefix("code=") {
                                return Some(c.to_string());
                            }
                        }
                    }
                    None
                })
                .ok_or_else(|| "Failed to extract authorization code from redirect request".to_string())?;

            let html_response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
                <html><body style='font-family:sans-serif;text-align:center;padding:50px;'>\
                <h2>Authentication Successful!</h2>\
                <p>Mail2Rust has received your authorization code. You may close this tab.</p>\
                </body></html>";
            let _ = std::io::Write::write_all(&mut socket, html_response.as_bytes());

            println!("Authorization code received! Exchanging code for tokens over TLS...");
            let tokens = mail2rust_imap::exchange_code_for_token(prov, client_id, client_secret, &code, &redirect_uri)?;

            println!("\nTokens Acquired Successfully!");
            println!(
                "Access Token:  {}...",
                &tokens.access_token[..tokens.access_token.len().min(24)]
            );
            if let Some(ref rt) = tokens.refresh_token {
                println!("Refresh Token: {}...", &rt[..rt.len().min(24)]);
            }

            let cache_file = output.clone().unwrap_or_else(|| PathBuf::from("mail2rust-oauth.json"));
            tokens.save_to_file(&cache_file)?;
            println!("\nSaved token cache to: {}", cache_file.display());
            println!("\nTo ingest emails using this token, run:");
            println!("  mail2rust imap --host {} --username <YOUR_EMAIL> --auth-method xoauth2 --oauth-token \"{}\" -o mailbox.pst\n",
                prov.default_imap_host(), tokens.access_token);
            println!("======================================================================\n");
        }

        Commands::Jmap {
            session_url,
            token,
            output,
            folders,
            checkpoint,
        } => {
            let tok = if !token.is_empty() {
                token.clone()
            } else {
                match std::env::var("MAIL2RUST_JMAP_TOKEN") {
                    Ok(t) => t,
                    Err(_) => {
                        eprint!("Enter JMAP API Bearer Token: ");
                        let mut line = String::new();
                        std::io::stdin().read_line(&mut line)?;
                        line.trim().to_string()
                    }
                }
            };

            println!("Connecting to JMAP session: {}...", session_url);
            let mut client = mail2rust_imap::jmap::JmapClient::new(session_url, &tok);
            client.connect()?;
            println!("Authenticated to JMAP session successfully.");

            let chk_path = checkpoint
                .as_ref()
                .or(cli.state_file.as_ref())
                .or(cli.resume.as_ref())
                .map(|p| p.as_path());
            let mut chk = if let Some(p) = chk_path {
                if p.exists() {
                    println!("Resuming migration from checkpoint: {}", p.display());
                    CheckpointManager::load(p)?
                } else {
                    CheckpointManager::new_checkpoint(session_url, &output.display().to_string())
                }
            } else {
                CheckpointManager::new_checkpoint(session_url, &output.display().to_string())
            };

            let folder_filter = if folders.is_empty() {
                None
            } else {
                Some(folders.as_slice())
            };

            println!("Streaming JMAP messages to {}...", output.display());
            let mut messages = Vec::new();
            let count = client.sync_all_with_checkpoint_path(
                folder_filter,
                &mut chk,
                chk_path,
                cli.checkpoint_interval,
                Some(&SHUTDOWN_FLAG),
                |msg| {
                    use std::io::Write;
                    print!(
                        "\r[JMAP Ingestion] Msg #{:<5} | Subject: {:<40}",
                        messages.len() + 1,
                        msg.subject.chars().take(38).collect::<String>()
                    );
                    let _ = std::io::stdout().flush();
                    messages.push(msg);
                },
            )?;
            println!(
                "\nFetched {} messages from JMAP server ({} newly synced).",
                messages.len(),
                count
            );

            if is_shutdown_requested() {
                eprintln!("\n[!] Gracefully halted on user interrupt.");
                if let Some(p) = chk_path {
                    eprintln!("[*] Interrupted migration state saved to: {}", p.display());
                    eprintln!("[*] To resume this migration from where it was paused, run:");
                    eprintln!("    mail2rust jmap ... --resume \"{}\"", p.display());
                }
                std::process::exit(130);
            }
            println!(
                "\nFetched {} messages from JMAP server ({} newly synced).",
                messages.len(),
                count
            );

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

        Commands::AppPasswords { provider } => {
            println!("\n======================================================================");
            println!("         PROVIDER APP PASSWORDS SETUP GUIDE (RECOMMENDED)");
            println!("======================================================================");
            println!(
                "Modern email services require an 'App Password' for IMAP access\n\
                     unless using an enterprise OAuth2 registered client. App passwords\n\
                     are 16-character codes that bypass 2-Factor Authentication prompts\n\
                     specifically for desktop email utilities like Mail2Rust.\n"
            );

            let p_lower = provider.to_lowercase();
            let show_google = p_lower == "all" || p_lower == "google" || p_lower == "gmail";
            let show_ms = p_lower == "all" || p_lower == "microsoft" || p_lower == "outlook" || p_lower == "office365";
            let show_yahoo = p_lower == "all" || p_lower == "yahoo" || p_lower == "aol";
            let show_proton = p_lower == "all"
                || p_lower == "proton"
                || p_lower == "protonmail"
                || p_lower == "bridge"
                || p_lower == "pm";
            let show_apple = p_lower == "all" || p_lower == "apple" || p_lower == "icloud" || p_lower == "me";
            let show_fastmail = p_lower == "all" || p_lower == "fastmail";
            let show_zoho = p_lower == "all" || p_lower == "zoho";

            if show_google {
                println!("--- GOOGLE GMAIL & GOOGLE WORKSPACE ---");
                println!("{}", OAuthProvider::Google.setup_instructions());
                println!();
            }
            if show_ms {
                println!("--- MICROSOFT OUTLOOK.COM & OFFICE 365 ---");
                println!("{}", OAuthProvider::Microsoft.setup_instructions());
                println!();
            }
            if show_yahoo {
                println!("--- YAHOO MAIL & AOL ---");
                println!("{}", OAuthProvider::Yahoo.setup_instructions());
                println!();
            }
            if show_proton {
                println!("--- PROTON MAIL & PROTON MAIL BRIDGE ---");
                println!("{}", OAuthProvider::Proton.setup_instructions());
                println!();
            }
            if show_apple {
                println!("--- APPLE ICLOUD MAIL ---");
                println!("{}", OAuthProvider::Apple.setup_instructions());
                println!();
            }
            if show_fastmail {
                println!("--- FASTMAIL ---");
                println!("{}", OAuthProvider::Fastmail.setup_instructions());
                println!();
            }
            if show_zoho {
                println!("--- ZOHO MAIL ---");
                println!("{}", OAuthProvider::Zoho.setup_instructions());
                println!();
            }
            println!("Once generated, supply it to Mail2Rust with:\n  --password \"xxxx xxxx xxxx xxxx\"");
            println!("======================================================================\n");
        }
        Commands::Mcp { .. } => unreachable!(),
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_merge_conversion(
    cli: &Cli,
    sources: &[PathBuf],
    destination: &Path,
    folder_strategy_str: &str,
    source_prefixes: Option<&str>,
    sort_by_date: bool,
    format_override: Option<&str>,
    max_pst_size: Option<&str>,
    encryption: PstEncryption,
    password: Option<&str>,
    eml_layout_str: &str,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let folder_strategy: MergeFolderStrategy = folder_strategy_str
        .parse()
        .map_err(|e: String| format!("Invalid --folder-strategy: {}", e))?;

    let dedup_strategy = cli.dedup.map(Into::into).unwrap_or(DedupStrategy::MessageId);

    let prefix_list: Vec<&str> = if let Some(prefixes) = source_prefixes {
        prefixes.split(',').map(|s| s.trim()).collect()
    } else {
        Vec::new()
    };

    let mut configs = Vec::new();
    for (i, src) in sources.iter().enumerate() {
        let mut cfg = MergeSourceConfig::new(src);
        if let Some(&p) = prefix_list.get(i) {
            if !p.is_empty() {
                cfg.alias = Some(p.to_string());
                cfg.custom_prefix = Some(p.to_string());
            }
        }
        configs.push(cfg);
    }

    let options = MergeOptions {
        folder_strategy,
        dedup_strategy,
        sort_by_date,
        dry_run: dry_run || cli.dry_run,
    };

    println!(
        "\nStarting Multi-Format Mailbox Merge across {} sources...",
        configs.len()
    );
    let (mut merged_messages, report) = MailboxMerger::merge_sources(&configs, &options, ingest_source)?;

    println!("{}", report.format_summary());

    if options.dry_run {
        println!("Dry-run mode active: no target files written.\n");
        return Ok(());
    }

    // Apply global CLI date filters if specified
    apply_preprocessing(cli, &mut merged_messages)?;

    let dest_ext = destination
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let is_pst = if let Some(fmt) = format_override {
        fmt.eq_ignore_ascii_case("pst")
    } else {
        dest_ext == "pst"
    };

    let is_mbox = if let Some(fmt) = format_override {
        fmt.eq_ignore_ascii_case("mbox")
    } else {
        dest_ext == "mbox" || dest_ext == "gz"
    };

    if is_pst {
        let max_bytes = if let Some(sz) = max_pst_size {
            parse_size_bytes(sz).unwrap_or(20 * 1024 * 1024 * 1024)
        } else {
            20 * 1024 * 1024 * 1024
        };

        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let total_bytes: usize = merged_messages.iter().map(|m| m.size_bytes).sum();
        if total_bytes > max_bytes && max_bytes > 0 {
            let prefix = destination
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("MergedArchive");
            let parent_dir = destination.parent().unwrap_or_else(|| Path::new("."));
            let cfg = SplitConfig {
                max_volume_bytes: max_bytes,
                cluster_threads: true,
                file_prefix: prefix.to_string(),
                target_ext: "pst".to_string(),
            };
            let volumes = VolumeSplitter::split_with_mode(&merged_messages, &cfg, &SplitMode::Size);
            for vol in &volumes {
                let out_file = parent_dir.join(&vol.file_name);
                write_single_pst(&vol.messages, &out_file, encryption, password, None, false)?;
            }
            println!("Exported {} merged PST volumes successfully", volumes.len());
        } else {
            write_single_pst(&merged_messages, destination, encryption, password, None, false)?;
            println!("Merged output successfully saved to: {}", destination.display());
        }
    } else if is_mbox {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let compress = dest_ext == "gz" || destination.to_string_lossy().ends_with(".mbox.gz");
        MboxFileExporter::export_to_file_options(&merged_messages, destination, compress, false)?;
        println!("Merged output successfully saved to: {}", destination.display());
    } else {
        // EML directory tree
        let layout = match eml_layout_str.to_lowercase().as_str() {
            "year" => EmlLayout::Year,
            "month" => EmlLayout::Month,
            "flat" => EmlLayout::Flat,
            _ => EmlLayout::Folder,
        };
        let mut exporter = EmlExporter::new(destination, layout);
        for m in &merged_messages {
            exporter.export_message(m)?;
        }
        println!("Merged output successfully saved to: {}", destination.display());
    }

    println!("======================================================================\n");
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
    encryption: PstEncryption,
    password: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        source = ?source,
        dest = ?destination,
        encryption = ?encryption,
        has_password = password.is_some(),
        "Starting PST conversion (zero Outlook dependency)"
    );
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
            write_single_pst(&vol.messages, &out_file, encryption, password, pst_root, flatten)?;
        }
        println!("Exported {} PST volumes successfully", volumes.len());
    } else {
        write_single_pst(&messages, destination, encryption, password, pst_root, flatten)?;
        println!("Exported 1 PST archive successfully ({} messages)", messages.len());
    }

    post_process(cli, &messages, source, destination)?;
    Ok(())
}

fn write_single_pst(
    messages: &[EmailMessage],
    destination: &Path,
    encryption: PstEncryption,
    password: Option<&str>,
    pst_root: Option<&str>,
    flatten: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(destination)?;
    let mut writer = PstWriter::with_encryption(file, encryption, password.map(|s| s.to_string()))?;

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

#[allow(clippy::too_many_arguments)]
fn run_eml_conversion(
    cli: &Cli,
    source: &Path,
    destination: &Path,
    layout_str: &str,
    naming_str: &str,
    max_per_dir: usize,
    html_index: bool,
    manifest_csv: Option<&Path>,
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

    if html_index {
        let title = format!(
            "e-Discovery Production Index - {}",
            source.file_name().and_then(|s| s.to_str()).unwrap_or("Mailbox")
        );
        exporter = exporter.with_html_index(title);
    }

    if let Some(man_path) = manifest_csv {
        exporter = exporter.with_manifest_csv(man_path)?;
    }

    for (idx, m) in messages.iter().enumerate() {
        if is_shutdown_requested() {
            eprintln!(
                "\n[!] Gracefully halting EML export on user interrupt (message {}/{})...",
                idx,
                messages.len()
            );
            let state_dest = cli.state_file.as_ref().or(cli.resume.as_ref());
            if let Some(state_path) = state_dest {
                let mut chk = if state_path.exists() {
                    CheckpointManager::load(state_path).unwrap_or_else(|_| {
                        CheckpointManager::new_checkpoint(
                            &source.display().to_string(),
                            &destination.display().to_string(),
                        )
                    })
                } else {
                    CheckpointManager::new_checkpoint(&source.display().to_string(), &destination.display().to_string())
                };
                CheckpointManager::commit_run(&mut chk, &messages[..idx]);
                CheckpointManager::mark_interrupted(&mut chk, "Halted by user signal (SIGINT/Ctrl+C)");
                let _ = CheckpointManager::save_atomic(&chk, state_path);
                eprintln!("[*] Interrupted migration state saved to: {}", state_path.display());
                eprintln!("[*] To resume this migration from where it was paused, run:");
                eprintln!("    mail2rust convert ... --resume \"{}\"", state_path.display());
            }
            std::process::exit(130);
        }
        exporter.export_message(m)?;
    }

    exporter.finalize()?;

    println!("Exported {} EML messages to {}", messages.len(), destination.display());
    if html_index {
        println!("Interactive HTML Index: {}", destination.join("index.html").display());
    }
    if let Some(man_path) = manifest_csv {
        println!("Master Manifest CSV:    {}", man_path.display());
    }
    post_process(cli, &messages, source, destination)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_search_discovery(
    _cli: &Cli,
    source: &Path,
    destination: &Path,
    rules_file: Option<&Path>,
    patterns: &[String],
    format_str: &str,
    html_index: bool,
    manifest_csv: Option<&Path>,
    encryption: PstEncryption,
    password: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let t_start = std::time::Instant::now();
    println!("================================================================================");
    println!("   mail2rust High-Throughput e-Discovery Regex Search & Extraction Engine       ");
    println!("================================================================================");
    println!("[*] Source:       {}", source.display());
    println!("[*] Destination:  {}", destination.display());

    // 1. Compile Matcher
    let matcher = if let Some(rf) = rules_file {
        println!("[*] Loading search rules from: {}", rf.display());
        MultiPatternMatcher::from_file(rf)?
    } else if !patterns.is_empty() {
        println!("[*] Compiling {} inline regex pattern(s)...", patterns.len());
        MultiPatternMatcher::from_patterns(patterns.iter().map(|s| s.as_str()))?
    } else {
        return Err("No search rules or patterns provided. Use --rules <FILE> or -p <REGEX>.".into());
    };

    println!(
        "[+] Compiled {} search rule(s) into zero-copy DFA RegexSet successfully",
        matcher.len()
    );
    for r in matcher.rules() {
        println!("    [{:<10}] {:<40} -> {}", r.tag, r.description, r.pattern);
    }

    // 2. Setup outputs based on format
    let fmt = format_str.to_lowercase();
    let is_pst = fmt == "pst" || destination.extension().and_then(|e| e.to_str()) == Some("pst");
    let is_mbox = fmt == "mbox" || destination.extension().and_then(|e| e.to_str()) == Some("mbox");
    let is_manifest_only = fmt == "manifest-only";

    if (is_pst || is_mbox) && destination.extension().is_some() {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
    } else {
        std::fs::create_dir_all(destination)?;
    }

    let manifest_path = if let Some(man) = manifest_csv {
        man.to_path_buf()
    } else if (is_pst || is_mbox) && destination.extension().is_some() {
        destination.with_extension("csv")
    } else {
        destination.join("search_manifest.csv")
    };
    let mut manifest_writer = ManifestWriter::create_file(&manifest_path)?;

    // Output containers
    let mut pst_writer = if is_pst {
        let pst_file_path = if destination.extension().and_then(|e| e.to_str()) == Some("pst") {
            destination.to_path_buf()
        } else {
            destination.join("search_results.pst")
        };
        let file = File::create(&pst_file_path)?;
        let mut pw = PstWriter::with_encryption(file, encryption, password.map(|s| s.to_string()))?;
        let folder_nid = pw.add_folder("Search Results", None)?;
        Some((pw, folder_nid, pst_file_path))
    } else {
        None
    };

    let mut mbox_writer = if is_mbox {
        let mbox_file_path = if destination.extension().and_then(|e| e.to_str()) == Some("mbox") {
            destination.to_path_buf()
        } else {
            destination.join("search_results.mbox")
        };
        let file = File::create(&mbox_file_path)?;
        Some((MboxWriter::new(file), mbox_file_path))
    } else {
        None
    };

    let mut eml_exporter = if !is_pst && !is_mbox && !is_manifest_only {
        let mut exp = EmlExporter::new(destination, EmlLayout::Flat).with_naming(EmlNaming::Subject);
        if html_index {
            let title = format!(
                "e-Discovery Search Results - {}",
                source.file_name().and_then(|s| s.to_str()).unwrap_or("Production")
            );
            exp = exp.with_html_index(title);
        }
        Some(exp)
    } else {
        None
    };

    // 3. Streaming Ingestion & Evaluation
    let mut total_evaluated = 0usize;
    let mut total_matched = 0usize;
    let mut rule_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    println!("\n[*] Streaming mailbox source and evaluating rules in real time...");

    mail2rust_core::ingest_path_streaming(source, |msg| {
        total_evaluated += 1;
        if total_evaluated.is_multiple_of(5000) {
            eprint!(
                "\r[*] Evaluated {} messages | {} matched...",
                total_evaluated, total_matched
            );
        }

        if is_shutdown_requested() {
            return Err("User interrupted search operation".into());
        }

        // Evaluate against search rules
        if let Some(matches) = matcher.matches_message(&msg) {
            total_matched += 1;
            let mut matched_tags = Vec::new();
            for m in matches {
                *rule_counts.entry(m.tag.clone()).or_insert(0) += 1;
                matched_tags.push(m.tag);
            }
            let tags_str = matched_tags.join(";");

            let mut eml_rel = String::new();

            if let Some(ref mut exp) = eml_exporter {
                if let Ok(p) = exp.export_message_annotated(&msg, &tags_str, &msg.source_format) {
                    eml_rel = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
                }
            } else if let Some((ref mut pw, folder_nid, _)) = pst_writer {
                pw.add_message(&msg, folder_nid)?;
            } else if let Some((ref mut mb, _)) = mbox_writer {
                mb.write_message(&msg)?;
            }

            let to_str = msg.to.iter().map(|a| a.to_string()).collect::<Vec<_>>().join("; ");
            manifest_writer.write_record(&ManifestRecord {
                message_id: msg.message_id.clone(),
                date: msg.date.to_rfc3339(),
                from: msg.from.to_string(),
                to: to_str,
                subject: mail2rust_core::decode_rfc2047(&msg.subject),
                size_bytes: msg.size_bytes.max(msg.raw_mime.len()),
                matched_keywords: tags_str,
                source_container: msg.source_format.clone(),
                sha256_hash: msg.sha256.clone(),
                eml_filename: eml_rel,
            })?;
        }

        Ok(())
    })?;

    // 4. Finalize outputs
    manifest_writer.flush()?;

    if let Some(ref mut exp) = eml_exporter {
        exp.finalize()?;
    }
    if let Some((mut pw, _, ref pst_path)) = pst_writer {
        pw.finalize()?;
        println!(
            "[+] Successfully assembled verified PST: {} ({:.2} MB)",
            pst_path.display(),
            std::fs::metadata(pst_path)?.len() as f64 / (1024.0 * 1024.0)
        );
    }
    if let Some((mut mb, ref mbox_path)) = mbox_writer {
        mb.flush()?;
        println!(
            "[+] Successfully assembled MBOX: {} ({:.2} MB)",
            mbox_path.display(),
            std::fs::metadata(mbox_path)?.len() as f64 / (1024.0 * 1024.0)
        );
    }

    let elapsed = t_start.elapsed().as_secs_f64();
    println!("\n================================================================================");
    println!("                     SEARCH & EXTRACTION COMPLETED                              ");
    println!("================================================================================");
    println!("Total Messages Evaluated:     {}", total_evaluated);
    println!("Total Relevant Messages Found: {}", total_matched);
    println!(
        "Total Elapsed Time:           {:.2}s ({:.1} msgs/sec)",
        elapsed,
        if elapsed > 0.0 {
            total_evaluated as f64 / elapsed
        } else {
            0.0
        }
    );
    println!("Master Manifest CSV:          {}", manifest_path.display());
    if html_index {
        println!(
            "Interactive HTML Index:       {}",
            destination.join("index.html").display()
        );
    }
    println!("--------------------------------------------------------------------------------");
    println!("Matches per Rule:");
    for r in matcher.rules() {
        let cnt = rule_counts.get(&r.tag).copied().unwrap_or(0);
        println!("  [{:<10}] {:<40} : {:>6} matches", r.tag, r.description, cnt);
    }
    println!("================================================================================");

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
        let rs_strat: DedupStrategy = strat.into();
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

    let state_dest = cli.state_file.as_ref().or(cli.resume.as_ref());
    if let Some(state_path) = state_dest {
        let mut chk = if state_path.exists() {
            CheckpointManager::load(state_path).unwrap_or_else(|_| {
                CheckpointManager::new_checkpoint(&source.display().to_string(), &dest.display().to_string())
            })
        } else {
            CheckpointManager::new_checkpoint(&source.display().to_string(), &dest.display().to_string())
        };
        CheckpointManager::commit_run(&mut chk, messages);
        CheckpointManager::save_atomic(&chk, state_path)?;
        info!(state = ?state_path, "Saved migration state atomically");
    }

    Ok(())
}
