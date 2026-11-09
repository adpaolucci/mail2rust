use chrono::{DateTime, Utc};
use mail2rust_core::models::{EmailAddress, EmailMessage, Importance, MessageFlags};
use mail2rust_sync::SyncCheckpoint;
use native_tls::TlsConnector;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use tracing::info;

pub trait ReadWrite: Read + Write {}
impl<T: Read + Write> ReadWrite for T {}

/// Represents errors encountered during JMAP protocol interactions.
#[derive(Debug)]
pub enum JmapError {
    HttpError(u16, String),
    SessionError(String),
    ProtocolError(String),
    NetworkError(String),
}

impl fmt::Display for JmapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JmapError::HttpError(code, body) => write!(f, "JMAP HTTP Error {}: {}", code, body),
            JmapError::SessionError(msg) => write!(f, "JMAP Session Error: {}", msg),
            JmapError::ProtocolError(msg) => write!(f, "JMAP Protocol Error: {}", msg),
            JmapError::NetworkError(msg) => write!(f, "JMAP Network Error: {}", msg),
        }
    }
}

impl std::error::Error for JmapError {}

/// JMAP Session Resource (RFC 8620 Section 2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JmapSession {
    #[serde(rename = "apiUrl")]
    pub api_url: String,
    #[serde(rename = "downloadUrl")]
    pub download_url: Option<String>,
    #[serde(rename = "uploadUrl")]
    pub upload_url: Option<String>,
    #[serde(rename = "primaryAccounts", default)]
    pub primary_accounts: HashMap<String, String>,
    #[serde(default)]
    pub accounts: HashMap<String, JmapAccount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JmapAccount {
    pub name: String,
    #[serde(rename = "isPersonal", default)]
    pub is_personal: bool,
}

/// JMAP Mailbox representation (RFC 8621 Section 2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JmapMailbox {
    pub id: String,
    pub name: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    pub role: Option<String>,
    #[serde(rename = "totalEmails", default)]
    pub total_emails: usize,
    #[serde(rename = "unreadEmails", default)]
    pub unread_emails: usize,
}

/// JMAP Email item (RFC 8621 Section 4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JmapEmailItem {
    pub id: String,
    #[serde(rename = "blobId")]
    pub blob_id: Option<String>,
    #[serde(rename = "threadId")]
    pub thread_id: Option<String>,
    #[serde(rename = "mailboxIds", default)]
    pub mailbox_ids: HashMap<String, bool>,
    #[serde(default)]
    pub keywords: HashMap<String, bool>,
    pub size: Option<usize>,
    #[serde(rename = "receivedAt")]
    pub received_at: Option<String>,
    #[serde(rename = "messageId")]
    pub message_id: Option<Vec<String>>,
    pub subject: Option<String>,
    pub from: Option<Vec<JmapEmailAddress>>,
    pub to: Option<Vec<JmapEmailAddress>>,
    pub cc: Option<Vec<JmapEmailAddress>>,
    #[serde(rename = "textBody")]
    pub text_body: Option<Vec<JmapBodyPart>>,
    #[serde(rename = "htmlBody")]
    pub html_body: Option<Vec<JmapBodyPart>>,
    #[serde(rename = "bodyValues", default)]
    pub body_values: HashMap<String, JmapBodyValue>,
    pub attachments: Option<Vec<JmapAttachmentPart>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JmapEmailAddress {
    pub name: Option<String>,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JmapBodyPart {
    #[serde(rename = "partId")]
    pub part_id: Option<String>,
    #[serde(rename = "blobId")]
    pub blob_id: Option<String>,
    pub size: Option<usize>,
    #[serde(rename = "type")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JmapBodyValue {
    pub value: String,
    #[serde(rename = "isEncodingProblem", default)]
    pub is_encoding_problem: bool,
    #[serde(rename = "isTruncated", default)]
    pub is_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JmapAttachmentPart {
    #[serde(rename = "partId")]
    pub part_id: Option<String>,
    #[serde(rename = "blobId")]
    pub blob_id: Option<String>,
    pub size: Option<usize>,
    #[serde(rename = "type")]
    pub mime_type: Option<String>,
    pub name: Option<String>,
}

/// Pure-Rust client for the modern IETF JSON Meta Application Protocol (JMAP, RFC 8620 / 8621).
pub struct JmapClient {
    pub session_url: String,
    pub token: String,
    session: Option<JmapSession>,
    account_id: Option<String>,
}

impl JmapClient {
    /// Create a new JMAP client with a target session URL and Bearer API token.
    pub fn new(session_url: &str, token: &str) -> Self {
        Self {
            session_url: session_url.to_string(),
            token: token.to_string(),
            session: None,
            account_id: None,
        }
    }

    /// Fastmail standard session entry point.
    pub fn fastmail(token: &str) -> Self {
        Self::new("https://api.fastmail.com/jmap/session", token)
    }

    /// Connect and discover the JMAP session object and primary mail account.
    pub fn connect(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!(session_url = %self.session_url, "Discovering JMAP Session");
        let (status, body) = http_request("GET", &self.session_url, Some(&self.token), None)
            .map_err(|e| JmapError::NetworkError(e.to_string()))?;

        if status != 200 {
            return Err(JmapError::HttpError(status, body).into());
        }

        let session: JmapSession = serde_json::from_str(&body)
            .map_err(|e| JmapError::SessionError(format!("Failed to parse session JSON: {} (Body: {})", e, body)))?;

        // Resolve primary mail account id
        let primary_id = session
            .primary_accounts
            .get("urn:ietf:params:jmap:mail")
            .cloned()
            .or_else(|| session.accounts.keys().next().cloned())
            .ok_or_else(|| JmapError::SessionError("No mail-capable JMAP account found in session".to_string()))?;

        info!(account_id = %primary_id, api_url = %session.api_url, "JMAP Session established");
        self.account_id = Some(primary_id);
        self.session = Some(session);
        Ok(())
    }

    /// Primary account identifier resolved from the session.
    pub fn account_id(&self) -> Option<&str> {
        self.account_id.as_deref()
    }

    /// Discovered session metadata.
    pub fn session(&self) -> Option<&JmapSession> {
        self.session.as_ref()
    }

    /// Retrieve all accessible mailboxes for the primary account.
    pub fn list_mailboxes(&self) -> Result<Vec<JmapMailbox>, Box<dyn std::error::Error>> {
        let session = self.session.as_ref().ok_or("JMAP session not connected")?;
        let account_id = self.account_id.as_deref().ok_or("No account ID selected")?;

        let request_payload = serde_json::json!({
            "using": [
                "urn:ietf:params:jmap:core",
                "urn:ietf:params:jmap:mail"
            ],
            "methodCalls": [
                [
                    "Mailbox/get",
                    {
                        "accountId": account_id,
                        "ids": null
                    },
                    "m0"
                ]
            ]
        });

        let (status, body) = http_request(
            "POST",
            &session.api_url,
            Some(&self.token),
            Some(&request_payload.to_string()),
        )
        .map_err(|e| JmapError::NetworkError(e.to_string()))?;

        if status != 200 {
            return Err(JmapError::HttpError(status, body).into());
        }

        let parsed: serde_json::Value = serde_json::from_str(&body)?;
        let mut mailboxes = Vec::new();

        if let Some(responses) = parsed.get("methodResponses").and_then(|r| r.as_array()) {
            for resp in responses {
                if let Some(arr) = resp.as_array() {
                    if arr.first().and_then(|v| v.as_str()) == Some("Mailbox/get") {
                        if let Some(list) = arr.get(1).and_then(|args| args.get("list")).and_then(|l| l.as_array()) {
                            for mb_val in list {
                                if let Ok(mb) = serde_json::from_value::<JmapMailbox>(mb_val.clone()) {
                                    mailboxes.push(mb);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(mailboxes)
    }

    /// Query email IDs within a specific mailbox.
    pub fn query_emails(
        &self,
        mailbox_id: &str,
        limit: usize,
        position: usize,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let session = self.session.as_ref().ok_or("JMAP session not connected")?;
        let account_id = self.account_id.as_deref().ok_or("No account ID selected")?;

        let request_payload = serde_json::json!({
            "using": [
                "urn:ietf:params:jmap:core",
                "urn:ietf:params:jmap:mail"
            ],
            "methodCalls": [
                [
                    "Email/query",
                    {
                        "accountId": account_id,
                        "filter": {
                            "inMailbox": mailbox_id
                        },
                        "sort": [
                            { "property": "receivedAt", "isAscending": true }
                        ],
                        "limit": limit,
                        "position": position
                    },
                    "q0"
                ]
            ]
        });

        let (status, body) = http_request(
            "POST",
            &session.api_url,
            Some(&self.token),
            Some(&request_payload.to_string()),
        )
        .map_err(|e| JmapError::NetworkError(e.to_string()))?;

        if status != 200 {
            return Err(JmapError::HttpError(status, body).into());
        }

        let parsed: serde_json::Value = serde_json::from_str(&body)?;
        let mut ids = Vec::new();

        if let Some(responses) = parsed.get("methodResponses").and_then(|r| r.as_array()) {
            for resp in responses {
                if let Some(arr) = resp.as_array() {
                    if arr.first().and_then(|v| v.as_str()) == Some("Email/query") {
                        if let Some(id_list) = arr.get(1).and_then(|args| args.get("ids")).and_then(|l| l.as_array()) {
                            for id_val in id_list {
                                if let Some(id_str) = id_val.as_str() {
                                    ids.push(id_str.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(ids)
    }

    /// Retrieve full email representations for a set of email IDs.
    pub fn get_emails(&self, email_ids: &[String]) -> Result<Vec<JmapEmailItem>, Box<dyn std::error::Error>> {
        if email_ids.is_empty() {
            return Ok(Vec::new());
        }

        let session = self.session.as_ref().ok_or("JMAP session not connected")?;
        let account_id = self.account_id.as_deref().ok_or("No account ID selected")?;

        let request_payload = serde_json::json!({
            "using": [
                "urn:ietf:params:jmap:core",
                "urn:ietf:params:jmap:mail"
            ],
            "methodCalls": [
                [
                    "Email/get",
                    {
                        "accountId": account_id,
                        "ids": email_ids,
                        "properties": [
                            "id", "blobId", "threadId", "mailboxIds", "keywords", "size",
                            "receivedAt", "messageId", "subject", "from", "to", "cc",
                            "textBody", "htmlBody", "bodyValues", "attachments"
                        ],
                        "bodyProperties": ["partId", "blobId", "size", "type"],
                        "fetchTextBodyValues": true,
                        "fetchHTMLBodyValues": true
                    },
                    "e0"
                ]
            ]
        });

        let (status, body) = http_request(
            "POST",
            &session.api_url,
            Some(&self.token),
            Some(&request_payload.to_string()),
        )
        .map_err(|e| JmapError::NetworkError(e.to_string()))?;

        if status != 200 {
            return Err(JmapError::HttpError(status, body).into());
        }

        let parsed: serde_json::Value = serde_json::from_str(&body)?;
        let mut emails = Vec::new();

        if let Some(responses) = parsed.get("methodResponses").and_then(|r| r.as_array()) {
            for resp in responses {
                if let Some(arr) = resp.as_array() {
                    if arr.first().and_then(|v| v.as_str()) == Some("Email/get") {
                        if let Some(list) = arr.get(1).and_then(|args| args.get("list")).and_then(|l| l.as_array()) {
                            for item_val in list {
                                if let Ok(item) = serde_json::from_value::<JmapEmailItem>(item_val.clone()) {
                                    emails.push(item);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(emails)
    }

    /// Stream all emails across mailboxes into unified EmailMessage models with delta checkpointing.
    pub fn sync_all<F>(
        &self,
        folder_filter: Option<&[String]>,
        checkpoint: &mut SyncCheckpoint,
        on_message: F,
    ) -> Result<usize, Box<dyn std::error::Error>>
    where
        F: FnMut(EmailMessage),
    {
        self.sync_all_with_checkpoint_path(folder_filter, checkpoint, None, 250, None, on_message)
    }

    /// Stream all emails across mailboxes into unified EmailMessage models with delta checkpointing
    /// and periodic atomic state flushes.
    pub fn sync_all_with_checkpoint_path<F>(
        &self,
        folder_filter: Option<&[String]>,
        checkpoint: &mut SyncCheckpoint,
        checkpoint_path: Option<&std::path::Path>,
        checkpoint_interval: usize,
        cancellation_flag: Option<&std::sync::atomic::AtomicBool>,
        mut on_message: F,
    ) -> Result<usize, Box<dyn std::error::Error>>
    where
        F: FnMut(EmailMessage),
    {
        let mailboxes = self.list_mailboxes()?;
        let mut mailbox_name_map = HashMap::new();
        for mb in &mailboxes {
            mailbox_name_map.insert(mb.id.clone(), mb.name.clone());
        }

        let mut total_ingested = 0;
        let mut msgs_since_last_flush = 0;

        for mb in mailboxes {
            if cancellation_flag.is_some_and(|f| f.load(std::sync::atomic::Ordering::Relaxed)) {
                mail2rust_sync::CheckpointManager::mark_interrupted(
                    checkpoint,
                    "Interrupted by user signal (SIGINT/Ctrl+C)",
                );
                if let Some(p) = checkpoint_path {
                    let _ = mail2rust_sync::CheckpointManager::save_atomic(checkpoint, p);
                }
                break;
            }

            if let Some(filter) = folder_filter {
                if !filter.iter().any(|f| f.eq_ignore_ascii_case(&mb.name)) {
                    continue;
                }
            }

            info!(mailbox = %mb.name, total = mb.total_emails, "Syncing JMAP mailbox");

            let mut position = 0;
            let page_size = 200;

            loop {
                if cancellation_flag.is_some_and(|f| f.load(std::sync::atomic::Ordering::Relaxed)) {
                    break;
                }

                let ids = self.query_emails(&mb.id, page_size, position)?;
                if ids.is_empty() {
                    break;
                }

                let items = self.get_emails(&ids)?;
                for item in items {
                    if cancellation_flag.is_some_and(|f| f.load(std::sync::atomic::Ordering::Relaxed)) {
                        break;
                    }

                    // Check deduplication / checkpointing
                    if checkpoint.migrated_message_ids.contains(&item.id) {
                        continue;
                    }

                    let msg = jmap_item_to_email_message(item, &mailbox_name_map);
                    checkpoint.migrated_message_ids.insert(msg.id.clone());
                    checkpoint.total_messages_migrated += 1;
                    checkpoint.total_bytes_migrated += msg.size_bytes;
                    total_ingested += 1;
                    msgs_since_last_flush += 1;

                    if let Some(p) = checkpoint_path {
                        if msgs_since_last_flush >= checkpoint_interval {
                            msgs_since_last_flush = 0;
                            let _ = mail2rust_sync::CheckpointManager::save_atomic(checkpoint, p);
                        }
                    }

                    on_message(msg);
                }

                position += ids.len();
                if ids.len() < page_size {
                    break;
                }
            }

            if let Some(p) = checkpoint_path {
                let _ = mail2rust_sync::CheckpointManager::save_atomic(checkpoint, p);
            }
        }

        checkpoint.timestamp = Utc::now();
        Ok(total_ingested)
    }
}

/// Transforms a JMAP Email entity into a Mail2Rust standard EmailMessage container.
pub fn jmap_item_to_email_message(item: JmapEmailItem, mailbox_map: &HashMap<String, String>) -> EmailMessage {
    let folder_path = item
        .mailbox_ids
        .keys()
        .filter_map(|mid| mailbox_map.get(mid).cloned())
        .collect::<Vec<String>>();

    let folder = if folder_path.is_empty() {
        vec!["Inbox".to_string()]
    } else {
        folder_path
    };

    let message_id = item
        .message_id
        .and_then(|v| v.first().cloned())
        .unwrap_or_else(|| format!("<{}@jmap.local>", item.id));

    let conversation_id = item.thread_id.unwrap_or_else(|| item.id.clone());
    let subject = item.subject.unwrap_or_else(|| "(No Subject)".to_string());

    let from = item
        .from
        .and_then(|v| v.into_iter().next())
        .map(|a| EmailAddress::new(a.name.as_deref().unwrap_or(&a.email), &a.email))
        .unwrap_or_else(|| EmailAddress::new("Unknown", "unknown@jmap.local"));

    let to = item
        .to
        .unwrap_or_default()
        .into_iter()
        .map(|a| EmailAddress::new(a.name.as_deref().unwrap_or(&a.email), &a.email))
        .collect();

    let cc = item
        .cc
        .unwrap_or_default()
        .into_iter()
        .map(|a| EmailAddress::new(a.name.as_deref().unwrap_or(&a.email), &a.email))
        .collect();

    let date = item
        .received_at
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc)))
        .unwrap_or_else(Utc::now);

    let mut body_text = String::new();
    if let Some(ref text_parts) = item.text_body {
        for part in text_parts {
            if let Some(ref pid) = part.part_id {
                if let Some(val) = item.body_values.get(pid) {
                    body_text.push_str(&val.value);
                }
            }
        }
    }

    let mut body_html = None;
    if let Some(ref html_parts) = item.html_body {
        let mut html_str = String::new();
        for part in html_parts {
            if let Some(ref pid) = part.part_id {
                if let Some(val) = item.body_values.get(pid) {
                    html_str.push_str(&val.value);
                }
            }
        }
        if !html_str.is_empty() {
            body_html = Some(html_str);
        }
    }

    let mut flags = MessageFlags::default();
    if item.keywords.contains_key("$seen") {
        flags.read = true;
    }
    if item.keywords.contains_key("$flagged") {
        flags.flagged = true;
    }
    if item.keywords.contains_key("$answered") {
        flags.replied = true;
    }
    if item.keywords.contains_key("$draft") {
        flags.draft = true;
    }

    let size_bytes = item.size.unwrap_or_else(|| body_text.len() + 100);
    let (sha256, md5) = EmailMessage::compute_hashes(body_text.as_bytes());

    EmailMessage {
        id: item.id,
        message_id,
        conversation_id,
        folder_path: folder,
        subject,
        from,
        to,
        cc,
        bcc: Vec::new(),
        date,
        size_bytes,
        body_text,
        body_html,
        raw_mime: Vec::new(),
        attachments: Vec::new(),
        mapi_properties: Vec::new(),
        flags,
        importance: Importance::Normal,
        sha256,
        md5,
        bates_number: None,
        legal_hold: false,
        source_format: "JMAP".to_string(),
    }
}

/// Minimal, zero-external-dependency HTTP/HTTPS client supporting JSON payloads and basic chunking.
fn http_request(
    method: &str,
    url_str: &str,
    auth_bearer: Option<&str>,
    json_body: Option<&str>,
) -> Result<(u16, String), Box<dyn std::error::Error>> {
    let (is_https, without_proto) = if let Some(p) = url_str.strip_prefix("https://") {
        (true, p)
    } else if let Some(p) = url_str.strip_prefix("http://") {
        (false, p)
    } else {
        return Err(format!("Unsupported URL scheme (expected http:// or https://): {}", url_str).into());
    };

    let (host_port, path) = match without_proto.find('/') {
        Some(pos) => (&without_proto[..pos], &without_proto[pos..]),
        None => (without_proto, "/"),
    };

    let (host, port) = if let Some(colon) = host_port.find(':') {
        let h = &host_port[..colon];
        let p: u16 = host_port[colon + 1..]
            .parse()
            .map_err(|e| format!("Invalid port in URL: {}", e))?;
        (h, p)
    } else {
        (host_port, if is_https { 443 } else { 80 })
    };

    let addr = format!("{}:{}", host, port);
    let tcp_stream = TcpStream::connect(&addr)?;
    tcp_stream.set_read_timeout(Some(std::time::Duration::from_secs(20)))?;
    tcp_stream.set_write_timeout(Some(std::time::Duration::from_secs(20)))?;

    let mut stream: Box<dyn ReadWrite> = if is_https {
        let connector = TlsConnector::builder().build()?;
        let tls_stream = connector.connect(host, tcp_stream)?;
        Box::new(tls_stream)
    } else {
        Box::new(tcp_stream)
    };

    let auth_header = match auth_bearer {
        Some(t) => format!("Authorization: Bearer {}\r\n", t),
        None => String::new(),
    };

    let body_str = json_body.unwrap_or("");
    let req = format!(
        "{} {} HTTP/1.1\r\n\
         Host: {}\r\n\
         User-Agent: mail2rust/3.3.0\r\n\
         Accept: application/json\r\n\
         Content-Type: application/json\r\n\
         {}Content-Length: {}\r\n\
         Connection: close\r\n\r\n\
         {}",
        method,
        path,
        host,
        auth_header,
        body_str.len(),
        body_str
    );

    stream.write_all(req.as_bytes())?;
    stream.flush()?;

    let mut raw_response = Vec::new();
    let mut buf = [0u8; 4096];
    let mut expected_total_len = None;
    let mut is_chunked = false;

    loop {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            break;
        }
        raw_response.extend_from_slice(&buf[..n]);

        if expected_total_len.is_none() {
            if let Some(header_end) = find_subsequence(&raw_response, b"\r\n\r\n") {
                let header_part = String::from_utf8_lossy(&raw_response[..header_end]);
                for line in header_part.lines() {
                    let lower = line.to_ascii_lowercase();
                    if lower.starts_with("content-length:") {
                        if let Some(val_str) = line.split(':').nth(1) {
                            if let Ok(cl) = val_str.trim().parse::<usize>() {
                                expected_total_len = Some(header_end + 4 + cl);
                            }
                        }
                    } else if lower.contains("transfer-encoding: chunked") {
                        is_chunked = true;
                    }
                }
            } else if let Some(header_end) = find_subsequence(&raw_response, b"\n\n") {
                let header_part = String::from_utf8_lossy(&raw_response[..header_end]);
                for line in header_part.lines() {
                    let lower = line.to_ascii_lowercase();
                    if lower.starts_with("content-length:") {
                        if let Some(val_str) = line.split(':').nth(1) {
                            if let Ok(cl) = val_str.trim().parse::<usize>() {
                                expected_total_len = Some(header_end + 2 + cl);
                            }
                        }
                    } else if lower.contains("transfer-encoding: chunked") {
                        is_chunked = true;
                    }
                }
            }
        }

        if let Some(expected) = expected_total_len {
            if raw_response.len() >= expected {
                break;
            }
        }

        if is_chunked && raw_response.ends_with(b"0\r\n\r\n") {
            break;
        }
    }

    let resp_str = String::from_utf8_lossy(&raw_response);
    let (header_part, body_part) = match resp_str.find("\r\n\r\n") {
        Some(idx) => (&resp_str[..idx], &resp_str[idx + 4..]),
        None => match resp_str.find("\n\n") {
            Some(idx) => (&resp_str[..idx], &resp_str[idx + 2..]),
            None => ("", resp_str.as_ref()),
        },
    };

    let status_code: u16 = header_part
        .lines()
        .next()
        .and_then(|line| {
            let mut parts = line.split_whitespace();
            parts.next(); // HTTP/1.1
            parts.next().and_then(|c| c.parse().ok())
        })
        .unwrap_or(200);

    let is_chunked = header_part
        .lines()
        .any(|line| line.to_lowercase().contains("transfer-encoding: chunked"));

    let final_body = if is_chunked {
        decode_chunked_body(body_part)
    } else {
        body_part.to_string()
    };

    Ok((status_code, final_body))
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn decode_chunked_body(body: &str) -> String {
    let mut out = String::new();
    let mut rest = body;
    while !rest.is_empty() {
        if let Some(nl) = rest.find("\r\n") {
            let hex_str = rest[..nl].trim();
            if let Ok(size) = usize::from_str_radix(hex_str, 16) {
                if size == 0 {
                    break;
                }
                let data_start = nl + 2;
                if data_start + size <= rest.len() {
                    out.push_str(&rest[data_start..data_start + size]);
                    let next = data_start + size + 2;
                    if next <= rest.len() {
                        rest = &rest[next..];
                    } else {
                        break;
                    }
                } else {
                    out.push_str(&rest[data_start..]);
                    break;
                }
            } else {
                out.push_str(rest);
                break;
            }
        } else {
            out.push_str(rest);
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn test_jmap_mock_sync_end_to_end() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().unwrap().port();

        thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut reader = std::io::BufReader::new(socket.try_clone().unwrap());

            // 1. Session request
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap() > 0 {
                if line == "\r\n" || line == "\n" {
                    break;
                }
                line.clear();
            }

            let session_json = serde_json::json!({
                "apiUrl": format!("http://127.0.0.1:{}/jmap/api", port),
                "downloadUrl": format!("http://127.0.0.1:{}/jmap/download/{{accountId}}/{{blobId}}/{{name}}", port),
                "primaryAccounts": {
                    "urn:ietf:params:jmap:mail": "acc_fastmail_01"
                },
                "accounts": {
                    "acc_fastmail_01": {
                        "name": "tester@fastmail.com",
                        "isPersonal": true
                    }
                }
            });

            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                session_json.to_string().len(),
                session_json
            );
            socket.write_all(resp.as_bytes()).unwrap();
            socket.flush().unwrap();
            drop(socket);

            // 2. Mailbox/get request
            let (mut socket2, _) = listener.accept().unwrap();
            let mut reader2 = std::io::BufReader::new(socket2.try_clone().unwrap());
            line.clear();
            let mut content_len = 0;
            while reader2.read_line(&mut line).unwrap() > 0 {
                if line.to_lowercase().starts_with("content-length:") {
                    content_len = line.split(':').nth(1).unwrap().trim().parse().unwrap_or(0);
                }
                if line == "\r\n" || line == "\n" {
                    break;
                }
                line.clear();
            }
            let mut body_bytes = vec![0u8; content_len];
            reader2.read_exact(&mut body_bytes).unwrap();

            let mailbox_resp = serde_json::json!({
                "methodResponses": [
                    [
                        "Mailbox/get",
                        {
                            "accountId": "acc_fastmail_01",
                            "state": "state_1",
                            "list": [
                                {
                                    "id": "mb_inbox",
                                    "name": "Inbox",
                                    "totalEmails": 1,
                                    "unreadEmails": 1
                                }
                            ]
                        },
                        "m0"
                    ]
                ]
            });
            let resp2 = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                mailbox_resp.to_string().len(),
                mailbox_resp
            );
            socket2.write_all(resp2.as_bytes()).unwrap();
            socket2.flush().unwrap();
            drop(socket2);

            // 2b. Second Mailbox/get request (invoked inside client.sync_all)
            let (mut socket2b, _) = listener.accept().unwrap();
            let mut reader2b = std::io::BufReader::new(socket2b.try_clone().unwrap());
            line.clear();
            content_len = 0;
            while reader2b.read_line(&mut line).unwrap() > 0 {
                if line.to_lowercase().starts_with("content-length:") {
                    content_len = line.split(':').nth(1).unwrap().trim().parse().unwrap_or(0);
                }
                if line == "\r\n" || line == "\n" {
                    break;
                }
                line.clear();
            }
            let mut body_bytes2b = vec![0u8; content_len];
            reader2b.read_exact(&mut body_bytes2b).unwrap();
            socket2b.write_all(resp2.as_bytes()).unwrap();
            socket2b.flush().unwrap();
            drop(socket2b);

            // 3. Email/query request
            let (mut socket3, _) = listener.accept().unwrap();
            let mut reader3 = std::io::BufReader::new(socket3.try_clone().unwrap());
            line.clear();
            content_len = 0;
            while reader3.read_line(&mut line).unwrap() > 0 {
                if line.to_lowercase().starts_with("content-length:") {
                    content_len = line.split(':').nth(1).unwrap().trim().parse().unwrap_or(0);
                }
                if line == "\r\n" || line == "\n" {
                    break;
                }
                line.clear();
            }
            let mut body_bytes3 = vec![0u8; content_len];
            reader3.read_exact(&mut body_bytes3).unwrap();

            let query_resp = serde_json::json!({
                "methodResponses": [
                    [
                        "Email/query",
                        {
                            "accountId": "acc_fastmail_01",
                            "ids": ["jmap_msg_001"],
                            "total": 1
                        },
                        "q0"
                    ]
                ]
            });
            let resp3 = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                query_resp.to_string().len(),
                query_resp
            );
            socket3.write_all(resp3.as_bytes()).unwrap();
            socket3.flush().unwrap();
            drop(socket3);

            // 4. Email/get request
            let (mut socket4, _) = listener.accept().unwrap();
            let mut reader4 = std::io::BufReader::new(socket4.try_clone().unwrap());
            line.clear();
            content_len = 0;
            while reader4.read_line(&mut line).unwrap() > 0 {
                if line.to_lowercase().starts_with("content-length:") {
                    content_len = line.split(':').nth(1).unwrap().trim().parse().unwrap_or(0);
                }
                if line == "\r\n" || line == "\n" {
                    break;
                }
                line.clear();
            }
            let mut body_bytes4 = vec![0u8; content_len];
            reader4.read_exact(&mut body_bytes4).unwrap();

            let get_resp = serde_json::json!({
                "methodResponses": [
                    [
                        "Email/get",
                        {
                            "accountId": "acc_fastmail_01",
                            "list": [
                                {
                                    "id": "jmap_msg_001",
                                    "blobId": "blob_001",
                                    "threadId": "thread_fastmail_01",
                                    "mailboxIds": {
                                        "mb_inbox": true
                                    },
                                    "keywords": {
                                        "$seen": true,
                                        "$flagged": true
                                    },
                                    "size": 512,
                                    "receivedAt": "2026-11-07T14:30:00Z",
                                    "messageId": ["<jmap-test-01@fastmail.com>"],
                                    "subject": "Fastmail JMAP Native Stream Test",
                                    "from": [
                                        {
                                             "name": "Rob Mueller",
                                             "email": "rob@fastmail.com"
                                        }
                                    ],
                                    "to": [
                                        {
                                             "name": "Fastmail Tester",
                                             "email": "tester@fastmail.com"
                                        }
                                    ],
                                    "textBody": [
                                        {
                                             "partId": "text_0",
                                             "size": 45,
                                             "type": "text/plain"
                                        }
                                    ],
                                    "bodyValues": {
                                        "text_0": {
                                             "value": "Ingested directly via JMAP RFC 8620/8621!"
                                        }
                                    }
                                }
                            ]
                        },
                        "e0"
                    ]
                ]
            });
            let resp4 = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                get_resp.to_string().len(),
                get_resp
            );
            socket4.write_all(resp4.as_bytes()).unwrap();
            socket4.flush().unwrap();
            drop(socket4);
        });

        let session_url = format!("http://127.0.0.1:{}/jmap/session", port);
        let mut client = JmapClient::new(&session_url, "fmu1-sample-fastmail-token");
        client.connect().expect("connect JMAP");

        let mailboxes = client.list_mailboxes().expect("list mailboxes");
        assert_eq!(mailboxes.len(), 1);
        assert_eq!(mailboxes[0].name, "Inbox");

        let mut checkpoint = mail2rust_sync::CheckpointManager::new_checkpoint("jmap_test", "local");
        let mut ingested = Vec::new();

        let count = client
            .sync_all(None, &mut checkpoint, |msg| {
                ingested.push(msg);
            })
            .expect("sync all");

        assert_eq!(count, 1);
        assert_eq!(ingested.len(), 1);

        let msg = &ingested[0];
        assert_eq!(msg.id, "jmap_msg_001");
        assert_eq!(msg.subject, "Fastmail JMAP Native Stream Test");
        assert_eq!(msg.from.name, "Rob Mueller");
        assert_eq!(msg.from.email, "rob@fastmail.com");
        assert_eq!(msg.body_text, "Ingested directly via JMAP RFC 8620/8621!");
        assert!(msg.flags.read);
        assert!(msg.flags.flagged);
        assert_eq!(msg.source_format, "JMAP");
    }
}
