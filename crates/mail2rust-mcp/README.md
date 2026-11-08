# `mail2rust-mcp` — Model Context Protocol (MCP) Server for Autonomous AI Agents

`mail2rust-mcp` exposes the full high-performance, offline mailbox migration, MAPI/PST binary parsing, and forensic analysis capabilities of `mail2rust` as native tools to AI agents over the [Model Context Protocol (MCP)](https://modelcontextprotocol.io/).

Compatible with **Claude Desktop**, **Cursor**, **Windsurf**, **Google Antigravity**, and **AutoGPT**.

---

## Capabilities & Native Tools

| Tool | Description | Inputs |
| :--- | :--- | :--- |
| `mail2rust_sniff` | Inspects magic bytes and classifies 15+ mailbox formats (PST, OST, MBOX, EML, EMLX, MSG, OLM, Maildir). | `path` (string) |
| `mail2rust_validate_pst` | Validates 64-bit Unicode MS-PST binary structure: 512B header, `!BDN` magic, CRC-32 partial checksum, NBT/BBT B-Trees without Microsoft Outlook. | `path` (string) |
| `mail2rust_convert` | Converts mailboxes between formats (`pst`, `mbox`, `eml`) with optional SHA-256 deduplication. | `source`, `destination`, `target_format`, `dedup` |
| `mail2rust_audit` | Forensic chain-of-custody audit: calculates item hashes, applies Bates numbering, verifies container integrity, and outputs CSV manifest. | `path`, `case_number`, `examiner`, `bates_prefix`, `output_csv` |
| `mail2rust_split` | Splits large mailboxes (>20GB or >50GB) into chunked volume archives preserving folder hierarchies. | `source`, `destination_dir`, `max_volume_mb`, `strategy`, `target_format` |
| `mail2rust_merge` | Merges multiple disparate archives into a single unified container with cross-archive deduplication. | `sources` (array), `destination`, `dedup` |

---

## Installation & Setup

### Option 1: Standalone Binary (`mail2rust-mcp`)
```bash
cargo install --path crates/mail2rust-mcp
```

### Option 2: Integrated CLI (`mail2rust mcp`)
If `mail2rust-cli` is already installed on your system:
```bash
mail2rust mcp
```

---

## Agent Configuration

### 1. Claude Desktop (`claude_desktop_config.json`)

On Windows (`%APPDATA%\Claude\claude_desktop_config.json`):
```json
{
  "mcpServers": {
    "mail2rust": {
      "command": "mail2rust",
      "args": ["mcp"]
    }
  }
}
```

On macOS / Linux (`~/Library/Application Support/Claude/claude_desktop_config.json` or `~/.config/Claude/claude_desktop_config.json`):
```json
{
  "mcpServers": {
    "mail2rust": {
      "command": "mail2rust",
      "args": ["mcp"]
    }
  }
}
```

### 2. Cursor (`.cursor/mcp.json` or Global Settings)
```json
{
  "mcpServers": {
    "mail2rust": {
      "command": "mail2rust-mcp"
    }
  }
}
```

### 3. Google Antigravity / Gemini CLI (`antigravity.json`)
```json
{
  "mcpServers": {
    "mail2rust": {
      "command": "mail2rust",
      "args": ["mcp"]
    }
  }
}
```

---

## Direct Inspection

You can inspect the tool schemas at any time without starting the stdio server:
```bash
mail2rust-mcp --tools
# or
mail2rust mcp --tools
```

All logging and tracing messages are strictly directed to `stderr`, guaranteeing clean JSON-RPC 2.0 communication over `stdout`.
