use clap::Parser;
use mail2rust_mcp::{get_tools_manifest, run_stdio_server, SERVER_NAME, SERVER_VERSION};
use std::process;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    name = SERVER_NAME,
    version = SERVER_VERSION,
    about = "Model Context Protocol (MCP) server for mail2rust email migration and analysis"
)]
struct Args {
    /// Print available tools in JSON schema format and exit
    #[arg(long)]
    tools: bool,

    /// Log level filter (sent to stderr)
    #[arg(long, default_value = "warn")]
    log_level: String,
}

fn main() {
    let args = Args::parse();

    if args.tools {
        let manifest = get_tools_manifest();
        println!("{}", serde_json::to_string_pretty(&manifest).unwrap());
        process::exit(0);
    }

    // Direct all structured tracing output to stderr so stdout is strictly preserved for JSON-RPC
    let filter = EnvFilter::try_new(&args.log_level).unwrap_or_else(|_| EnvFilter::new("warn"));
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .init();

    if let Err(e) = run_stdio_server() {
        eprintln!("[mail2rust-mcp error] Server exited: {}", e);
        process::exit(1);
    }
}
