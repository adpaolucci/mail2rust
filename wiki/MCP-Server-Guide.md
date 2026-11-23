# MCP Server Guide (`mail2rust-mcp`)

> [!NOTE]
> For the full tool-by-tool walk-through and example agent conversation, see also **[Model Context Protocol (MCP) Integration Guide](Model-Context-Protocol-MCP)**.

`mail2rust` ships a native JSON-RPC 2.0 `stdio` **Model Context Protocol (MCP)** server (`protocolVersion: "2024-11-05"`), accessible via `mail2rust mcp` or the standalone `mail2rust-mcp` executable.

---

## 1. Client Discovery (`mcp.json` & `smithery.yaml`)

```json
{
  "mcpServers": {
    "mail2rust": {
      "command": "mail2rust",
      "args": ["mcp"],
      "env": {
        "RUST_LOG": "warn"
      }
    }
  }
}
```

All internal tracing is routed exclusively to `stderr`, keeping `stdout` strictly compliant with newline-delimited JSON-RPC 2.0. To dump the raw JSON tool schemas from the terminal, run:

```bash
mail2rust mcp --tools
```

---

## 2. Supported JSON-RPC 2.0 Methods

- `initialize` — Negotiates protocol version `2024-11-05`, server capabilities (`tools`), and `serverInfo` (`mail2rust-mcp` `v3.5.0`).
- `ping` — Health check returning `{}`.
- `notifications/initialized` / `notifications/cancelled` — Handled silently with no response frame.
- `tools/list` — Returns the JSON Schema definitions for all 6 tools.
- `tools/call` — Dispatches tool execution and wraps structured JSON results inside `content: [{ "type": "text", "text": "..." }]` with `isError`.

---

## 3. Exposed MCP Tools (6 Total)

| Tool Name | Required Inputs | Optional Inputs | Description |
| :--- | :--- | :--- | :--- |
| **`mail2rust_sniff`** | `path` | — | Inspects magic bytes and directory layout to classify PST, OST, MBOX, EML, EMLX, MSG, OLM, Maildir, Thunderbird, MH, MMDF, or Babyl stores. |
| **`mail2rust_validate_pst`** | `path` | — | Validates 64-bit Unicode MS-PST binary integrity (`!BDN` magic, version 23, CRC-32 header checksum, and NBT/BBT root B-Tree offsets) without Microsoft Outlook. |
| **`mail2rust_convert`** | `source`, `destination` | `target_format`, `dedup`, `checkpoint_file`, `resume` | Converts between PST, MBOX, and EML hierarchies with optional SHA-256 deduplication and incremental checkpoint resumption. |
| **`mail2rust_audit`** | `path` | `case_number`, `examiner`, `bates_prefix`, `output_csv` | Performs cryptographic eDiscovery auditing: computes SHA-256 Merkle root, applies sequential Bates stamps, and writes a forensic CSV manifest. |
| **`mail2rust_split`** | `source`, `destination_dir` | `max_volume_mb`, `strategy` (`size` \| `year`), `target_format` | Splits oversized archives (>20 GB / >50 GB) into multi-volume PST, MBOX, or EML parts while clustering conversation threads. |
| **`mail2rust_merge`** | `sources` (array), `destination` | `dedup` | Consolidates multiple disparate mailbox archives into a single container with cross-archive SHA-256 deduplication. |
