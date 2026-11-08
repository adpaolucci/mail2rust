use chrono::{DateTime, FixedOffset, NaiveDate};
use imap::types::Flag;
use imap::Session;
use mail2rust_core::models::MapiProperty;
use mail2rust_core::readers::eml::EmlReader;
use mail2rust_core::EmailMessage;
use mail2rust_sync::SyncCheckpoint;
use native_tls::{TlsConnector, TlsStream};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use tracing::{debug, info, warn};

pub mod oauth;
pub use oauth::{
    build_authorization_url, diagnose_auth_failure, exchange_code_for_token, refresh_access_token, ImapAuthMethod,
    OAuthBearerAuthenticator, OAuthProvider, OAuthTokenResponse, XOAuth2Authenticator,
};

#[derive(Debug)]
pub enum ImapClientError {
    ConnectionFailed(String),
    TlsError(String),
    AuthenticationFailed(String),
    ProtocolError(String),
    MailboxNotFound(String),
    Io(io::Error),
    EmailParseError(String),
}

impl fmt::Display for ImapClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImapClientError::ConnectionFailed(msg) => write!(f, "IMAP connection failed: {}", msg),
            ImapClientError::TlsError(msg) => write!(f, "TLS handshake failed: {}", msg),
            ImapClientError::AuthenticationFailed(msg) => write!(f, "IMAP authentication failed: {}", msg),
            ImapClientError::ProtocolError(msg) => write!(f, "IMAP protocol error: {}", msg),
            ImapClientError::MailboxNotFound(msg) => write!(f, "Mailbox not found: {}", msg),
            ImapClientError::Io(err) => write!(f, "I/O error: {}", err),
            ImapClientError::EmailParseError(err) => write!(f, "Email parsing error: {}", err),
        }
    }
}

impl std::error::Error for ImapClientError {}

impl From<io::Error> for ImapClientError {
    fn from(err: io::Error) -> Self {
        ImapClientError::Io(err)
    }
}

impl From<native_tls::Error> for ImapClientError {
    fn from(err: native_tls::Error) -> Self {
        ImapClientError::TlsError(err.to_string())
    }
}

impl From<imap::Error> for ImapClientError {
    fn from(err: imap::Error) -> Self {
        match err {
            imap::Error::Io(io) => ImapClientError::Io(io),
            imap::Error::Tls(tls) => ImapClientError::TlsError(tls.to_string()),
            imap::Error::Validate(v) => ImapClientError::ProtocolError(format!("Validation error: {:?}", v)),
            imap::Error::No(no) => ImapClientError::AuthenticationFailed(no.to_string()),
            imap::Error::Bad(bad) => ImapClientError::ProtocolError(bad.to_string()),
            imap::Error::ConnectionLost => ImapClientError::ConnectionFailed("Connection lost".to_string()),
            imap::Error::Parse(p) => ImapClientError::ProtocolError(format!("Parse error: {:?}", p)),
            _ => ImapClientError::ProtocolError(err.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub use_tls: bool,
    pub insecure_certs: bool,
    pub folder_filter: Option<Vec<String>>,
    pub max_messages_per_folder: Option<usize>,
    pub since_date: Option<NaiveDate>,
    pub auth_method: ImapAuthMethod,
    pub oauth_token: Option<String>,
}

impl Default for ImapConfig {
    fn default() -> Self {
        Self {
            host: "imap.gmail.com".to_string(),
            port: 993,
            username: String::new(),
            password: String::new(),
            use_tls: true,
            insecure_certs: false,
            folder_filter: None,
            max_messages_per_folder: None,
            since_date: None,
            auth_method: ImapAuthMethod::Password,
            oauth_token: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImapFolderInfo {
    pub name: String,
    pub delimiter: Option<String>,
    pub flags: Vec<String>,
    pub total_messages: u32,
    pub uid_next: Option<u32>,
    pub uid_validity: Option<u32>,
    pub selectable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapSyncProgress {
    pub current_folder: String,
    pub current_message_index: usize,
    pub total_messages_in_folder: usize,
    pub total_messages_migrated: usize,
    pub current_uid: u64,
}

pub enum ImapStream {
    Tls(Box<TlsStream<TcpStream>>),
    Plain(TcpStream),
}

impl Read for ImapStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            ImapStream::Tls(s) => s.read(buf),
            ImapStream::Plain(s) => s.read(buf),
        }
    }
}

impl Write for ImapStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            ImapStream::Tls(s) => s.write(buf),
            ImapStream::Plain(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            ImapStream::Tls(s) => s.flush(),
            ImapStream::Plain(s) => s.flush(),
        }
    }
}

pub struct ImapClient {
    config: ImapConfig,
    session: Option<Session<ImapStream>>,
}

impl ImapClient {
    pub fn new(config: ImapConfig) -> Self {
        Self { config, session: None }
    }

    pub fn is_connected(&self) -> bool {
        self.session.is_some()
    }

    pub fn connect(&mut self) -> Result<(), ImapClientError> {
        info!(host = %self.config.host, port = self.config.port, tls = self.config.use_tls, "Connecting to IMAP server");

        let address = format!("{}:{}", self.config.host, self.config.port);
        let timeout = std::time::Duration::from_secs(15);
        use std::net::ToSocketAddrs;
        let socket_addr = address
            .to_socket_addrs()
            .map_err(|e| ImapClientError::ConnectionFailed(format!("Failed to resolve {}: {}", address, e)))?
            .next()
            .ok_or_else(|| ImapClientError::ConnectionFailed(format!("No IP addresses resolved for {}", address)))?;

        let tcp_stream = TcpStream::connect_timeout(&socket_addr, timeout)
            .map_err(|e| ImapClientError::ConnectionFailed(format!("Failed to connect to {}: {}", address, e)))?;

        tcp_stream
            .set_read_timeout(Some(std::time::Duration::from_secs(30)))
            .map_err(ImapClientError::Io)?;
        tcp_stream
            .set_write_timeout(Some(std::time::Duration::from_secs(30)))
            .map_err(ImapClientError::Io)?;

        let stream = if self.config.use_tls {
            let mut builder = TlsConnector::builder();
            if self.config.insecure_certs {
                builder.danger_accept_invalid_certs(true);
                builder.danger_accept_invalid_hostnames(true);
            }
            let connector = builder.build()?;
            let tls_stream = connector.connect(&self.config.host, tcp_stream).map_err(|e| {
                ImapClientError::TlsError(format!("TLS handshake with {} failed: {}", self.config.host, e))
            })?;
            ImapStream::Tls(Box::new(tls_stream))
        } else {
            ImapStream::Plain(tcp_stream)
        };

        let client = imap::Client::new(stream);
        let session = match self.config.auth_method {
            ImapAuthMethod::Password => client
                .login(&self.config.username, &self.config.password)
                .map_err(|(err, _)| {
                    let diag = diagnose_auth_failure(&self.config.host, &err.to_string());
                    warn!("{}", diag);
                    ImapClientError::from(err)
                })?,
            ImapAuthMethod::XOAuth2 => {
                let token = self
                    .config
                    .oauth_token
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&self.config.password);
                let auth = XOAuth2Authenticator {
                    username: self.config.username.clone(),
                    access_token: token.to_string(),
                };
                client.authenticate("XOAUTH2", &auth).map_err(|(err, _)| {
                    let diag = diagnose_auth_failure(&self.config.host, &err.to_string());
                    warn!("{}", diag);
                    ImapClientError::from(err)
                })?
            }
            ImapAuthMethod::OAuthBearer => {
                let token = self
                    .config
                    .oauth_token
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&self.config.password);
                let auth = OAuthBearerAuthenticator {
                    username: self.config.username.clone(),
                    host: self.config.host.clone(),
                    port: self.config.port,
                    access_token: token.to_string(),
                };
                client
                    .authenticate("OAUTHBEARER", &auth)
                    .map_err(|(err, _)| {
                        let diag = diagnose_auth_failure(&self.config.host, &err.to_string());
                        warn!("{}", diag);
                        ImapClientError::from(err)
                    })?
            }
        };

        info!(
            user = %self.config.username,
            auth_method = %self.config.auth_method,
            "Authenticated successfully to IMAP server"
        );
        self.session = Some(session);
        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<(), ImapClientError> {
        if let Some(mut session) = self.session.take() {
            let _ = session.logout();
        }
        Ok(())
    }

    pub fn list_folders(&mut self) -> Result<Vec<ImapFolderInfo>, ImapClientError> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| ImapClientError::ConnectionFailed("Not connected to IMAP server".to_string()))?;

        let folders_response = session.list(Some(""), Some("*"))?;
        let mut results = Vec::new();

        for folder in folders_response.iter() {
            let name = folder.name().to_string();
            let delimiter = folder.delimiter().map(|d| d.to_string());
            let attributes = folder.attributes();
            let mut flags = Vec::new();
            let mut selectable = true;

            for attr in attributes {
                let s = format!("{:?}", attr);
                if s.contains("NoSelect") || s.contains("Noselect") {
                    selectable = false;
                }
                flags.push(s);
            }

            results.push(ImapFolderInfo {
                name,
                delimiter,
                flags,
                total_messages: 0,
                uid_next: None,
                uid_validity: None,
                selectable,
            });
        }

        Ok(results)
    }

    pub fn split_folder_hierarchy(folder_name: &str, delimiter: Option<&str>) -> Vec<String> {
        let delim = delimiter.unwrap_or("/");
        let delim_char = delim.chars().next().unwrap_or('/');

        // Strip surrounding quotes if present
        let cleaned = folder_name.trim_matches('"');

        let parts: Vec<String> = cleaned
            .split(delim_char)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if parts.is_empty() {
            vec!["Inbox".to_string()]
        } else {
            parts
        }
    }

    pub fn fetch_folder<F>(
        &mut self,
        folder_name: &str,
        delimiter: Option<&str>,
        watermark: Option<u64>,
        mut on_message: F,
    ) -> Result<(usize, u64), ImapClientError>
    where
        F: FnMut(EmailMessage, u64),
    {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| ImapClientError::ConnectionFailed("Not connected to IMAP server".to_string()))?;

        debug!(folder = folder_name, "Selecting IMAP folder");
        let mailbox = session.select(folder_name)?;
        let total_in_folder = mailbox.exists as usize;
        if total_in_folder == 0 {
            return Ok((0, watermark.unwrap_or(0)));
        }

        // Search for all messages or messages newer than since_date
        let query = if let Some(since) = self.config.since_date {
            format!("SINCE {}", since.format("%d-%b-%Y"))
        } else {
            "ALL".to_string()
        };

        let uids_set = session.uid_search(&query)?;
        let mut uids: Vec<u32> = uids_set.into_iter().collect();
        uids.sort_unstable();

        // Filter by high watermark if resuming
        if let Some(wm) = watermark {
            uids.retain(|&uid| (uid as u64) > wm);
        }

        // Apply max_messages limit if requested
        if let Some(limit) = self.config.max_messages_per_folder {
            if uids.len() > limit {
                uids.truncate(limit);
            }
        }

        if uids.is_empty() {
            return Ok((0, watermark.unwrap_or(0)));
        }

        let folder_hierarchy = Self::split_folder_hierarchy(folder_name, delimiter);
        let mut migrated_count = 0;
        let mut max_uid = watermark.unwrap_or(0);

        // Fetch in batches of 50 to avoid long IMAP command lines
        for chunk in uids.chunks(50) {
            let uid_query = chunk.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");

            let fetches = session.uid_fetch(&uid_query, "(BODY.PEEK[] FLAGS INTERNALDATE UID)")?;

            for fetch in fetches.iter() {
                let uid = fetch.uid.unwrap_or(0) as u64;
                if uid > max_uid {
                    max_uid = uid;
                }

                if let Some(body_bytes) = fetch.body() {
                    match EmlReader::parse(body_bytes, folder_hierarchy.clone()) {
                        Ok(mut email) => {
                            // Map IMAP flags
                            for flag in fetch.flags() {
                                match flag {
                                    Flag::Seen => email.flags.read = true,
                                    Flag::Flagged => email.flags.flagged = true,
                                    Flag::Answered => email.flags.replied = true,
                                    Flag::Deleted => email.flags.deleted = true,
                                    Flag::Draft => email.flags.draft = true,
                                    _ => {}
                                }
                            }

                            // Fallback to internal date if received date is missing
                            if let Some(internal_date) = fetch.internal_date() {
                                if let Some(dt) = DateTime::from_timestamp(internal_date.timestamp(), 0) {
                                    email.date = dt;
                                }
                            }

                            // Record IMAP metadata into properties
                            email.mapi_properties.push(MapiProperty {
                                tag: 0x67F20003,
                                name: "PR_IMAP_UID".to_string(),
                                prop_type: "PT_LONG".to_string(),
                                value: uid.to_string(),
                            });

                            on_message(email, uid);
                            migrated_count += 1;
                        }
                        Err(e) => {
                            warn!(uid = uid, folder = folder_name, error = %e, "Failed to parse RFC 822 body, skipping message");
                        }
                    }
                }
            }
        }

        Ok((migrated_count, max_uid))
    }

    pub fn sync_all<F>(&mut self, checkpoint: &mut SyncCheckpoint, mut on_progress: F) -> Result<usize, ImapClientError>
    where
        F: FnMut(ImapSyncProgress, EmailMessage),
    {
        let folders = self.list_folders()?;
        let mut total_messages_migrated = 0;

        for folder_info in folders {
            if !folder_info.selectable {
                debug!(folder = %folder_info.name, "Skipping non-selectable IMAP folder");
                continue;
            }

            // Check if folder is excluded by filter
            if let Some(ref filter) = self.config.folder_filter {
                if !filter.iter().any(|f| folder_info.name.eq_ignore_ascii_case(f)) {
                    continue;
                }
            }

            let folder_name = folder_info.name.clone();
            let delimiter = folder_info.delimiter.as_deref();
            let watermark = checkpoint.folder_high_watermarks.get(&folder_name).copied();

            let mut current_idx = 0;
            let (count, new_watermark) = self.fetch_folder(&folder_name, delimiter, watermark, |msg, uid| {
                current_idx += 1;
                total_messages_migrated += 1;
                checkpoint.migrated_message_ids.insert(msg.message_id.clone());
                checkpoint.total_messages_migrated += 1;
                checkpoint.total_bytes_migrated += msg.body_text.len() + msg.body_html.as_ref().map_or(0, |h| h.len());

                let progress = ImapSyncProgress {
                    current_folder: folder_name.clone(),
                    current_message_index: current_idx,
                    total_messages_in_folder: 0,
                    total_messages_migrated,
                    current_uid: uid,
                };

                on_progress(progress, msg);
            })?;

            if new_watermark > 0 {
                checkpoint.folder_high_watermarks.insert(folder_name, new_watermark);
            }
            debug!(folder = %folder_info.name, fetched = count, watermark = new_watermark, "Folder sync complete");
        }

        Ok(total_messages_migrated)
    }

    pub fn create_folder(&mut self, folder: &str) -> Result<(), ImapClientError> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| ImapClientError::ConnectionFailed("Not connected to IMAP server".to_string()))?;
        match session.create(folder) {
            Ok(_) => Ok(()),
            Err(imap::Error::No(msg)) => {
                let msg_lower = msg.information.to_lowercase();
                if msg_lower.contains("already") || msg_lower.contains("exist") {
                    debug!(folder = %folder, "Folder already exists on server");
                    Ok(())
                } else {
                    Err(ImapClientError::ProtocolError(format!(
                        "CREATE failed: {}",
                        msg.information
                    )))
                }
            }
            Err(e) => Err(ImapClientError::from(e)),
        }
    }

    pub fn append_message(
        &mut self,
        folder: &str,
        raw_mime: &[u8],
        flags: &[Flag],
        date: Option<DateTime<FixedOffset>>,
    ) -> Result<(), ImapClientError> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| ImapClientError::ConnectionFailed("Not connected to IMAP server".to_string()))?;
        let mut cmd = session.append(folder, raw_mime);
        if !flags.is_empty() {
            cmd.flags(flags.iter().cloned());
        }
        if let Some(d) = date {
            cmd.internal_date(d);
        }
        cmd.finish()?;
        Ok(())
    }

    pub fn fetch_folder_raw<F>(
        &mut self,
        folder_name: &str,
        watermark: Option<u64>,
        mut on_message: F,
    ) -> Result<(usize, u64), ImapClientError>
    where
        F: FnMut(&[u8], Vec<Flag>, Option<DateTime<FixedOffset>>, u64, Option<String>) -> Result<(), ImapClientError>,
    {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| ImapClientError::ConnectionFailed("Not connected to IMAP server".to_string()))?;

        debug!(folder = folder_name, "Selecting IMAP folder for streaming");
        let mailbox = session.select(folder_name)?;
        let total_in_folder = mailbox.exists as usize;
        if total_in_folder == 0 {
            return Ok((0, watermark.unwrap_or(0)));
        }

        let query = if let Some(since) = self.config.since_date {
            format!("SINCE {}", since.format("%d-%b-%Y"))
        } else {
            "ALL".to_string()
        };

        let uids_set = session.uid_search(&query)?;
        let mut uids: Vec<u32> = uids_set.into_iter().collect();
        uids.sort_unstable();

        if let Some(wm) = watermark {
            uids.retain(|&uid| (uid as u64) > wm);
        }

        if let Some(limit) = self.config.max_messages_per_folder {
            if uids.len() > limit {
                uids.truncate(limit);
            }
        }

        if uids.is_empty() {
            return Ok((0, watermark.unwrap_or(0)));
        }

        let mut migrated_count = 0;
        let mut max_uid = watermark.unwrap_or(0);

        for chunk in uids.chunks(50) {
            let uid_query = chunk.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
            let fetches = session.uid_fetch(&uid_query, "(BODY.PEEK[] FLAGS INTERNALDATE UID ENVELOPE)")?;

            for fetch in fetches.iter() {
                let uid = fetch.uid.unwrap_or(0) as u64;
                if uid > max_uid {
                    max_uid = uid;
                }

                if let Some(body_bytes) = fetch.body() {
                    let flags: Vec<Flag> = fetch.flags().to_vec();
                    let date = fetch.internal_date();
                    let subject = fetch
                        .envelope()
                        .and_then(|env| env.subject.as_ref().map(|s| String::from_utf8_lossy(s).to_string()));

                    on_message(body_bytes, flags, date, uid, subject)?;
                    migrated_count += 1;
                }
            }
        }

        Ok((migrated_count, max_uid))
    }
}

impl Drop for ImapClient {
    fn drop(&mut self) {
        let _ = self.disconnect();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapMigrationConfig {
    pub source: ImapConfig,
    pub destination: ImapConfig,
    pub folder_filter: Option<Vec<String>>,
    pub folder_prefix: Option<String>,
    pub folder_mapping: Option<HashMap<String, String>>,
    pub preserve_flags: bool,
    pub preserve_dates: bool,
    pub create_missing_folders: bool,
    pub dry_run: bool,
}

impl Default for ImapMigrationConfig {
    fn default() -> Self {
        Self {
            source: ImapConfig::default(),
            destination: ImapConfig::default(),
            folder_filter: None,
            folder_prefix: None,
            folder_mapping: None,
            preserve_flags: true,
            preserve_dates: true,
            create_missing_folders: true,
            dry_run: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapMigrationProgress {
    pub current_source_folder: String,
    pub current_target_folder: String,
    pub folder_index: usize,
    pub total_folders: usize,
    pub message_index_in_folder: usize,
    pub total_messages_in_folder: usize,
    pub total_messages_migrated: usize,
    pub total_bytes_transferred: usize,
    pub current_subject: Option<String>,
    pub current_uid: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImapMigrationSummary {
    pub total_folders_scanned: usize,
    pub total_folders_migrated: usize,
    pub total_messages_migrated: usize,
    pub total_messages_skipped: usize,
    pub total_bytes_transferred: usize,
    pub duration_seconds: f64,
    pub errors: Vec<String>,
}

pub struct ImapMigrator;

impl ImapMigrator {
    pub fn migrate<F>(config: &ImapMigrationConfig, mut on_progress: F) -> Result<ImapMigrationSummary, ImapClientError>
    where
        F: FnMut(&ImapMigrationProgress),
    {
        let start_time = std::time::Instant::now();
        let mut summary = ImapMigrationSummary::default();

        info!(
            source_host = %config.source.host,
            dest_host = %config.destination.host,
            dry_run = config.dry_run,
            "Starting direct server-to-server IMAP migration (zero local disk storage)"
        );

        let mut src_client = ImapClient::new(config.source.clone());
        src_client.connect()?;

        let mut dst_client = ImapClient::new(config.destination.clone());
        if !config.dry_run {
            dst_client.connect()?;
        }

        // List destination folders to avoid duplicate create attempts
        let mut dest_folders: HashSet<String> = HashSet::new();
        if !config.dry_run {
            match dst_client.list_folders() {
                Ok(folders) => {
                    for f in folders {
                        dest_folders.insert(f.name);
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Could not list destination folders, will attempt creation on-demand");
                }
            }
        }

        let src_folders = src_client.list_folders()?;
        summary.total_folders_scanned = src_folders.len();

        let selectable_folders: Vec<_> = src_folders
            .into_iter()
            .filter(|f| f.selectable)
            .filter(|f| {
                if let Some(ref filter) = config.folder_filter {
                    filter.iter().any(|name| f.name.eq_ignore_ascii_case(name))
                } else {
                    true
                }
            })
            .collect();

        let total_folders = selectable_folders.len();

        for (folder_idx, folder_info) in selectable_folders.iter().enumerate() {
            let src_folder = &folder_info.name;

            // Target folder calculation
            let dest_folder = if let Some(ref mapping) = config.folder_mapping {
                mapping.get(src_folder).cloned().unwrap_or_else(|| {
                    if let Some(ref pfx) = config.folder_prefix {
                        format!("{}{}", pfx, src_folder)
                    } else {
                        src_folder.clone()
                    }
                })
            } else if let Some(ref pfx) = config.folder_prefix {
                format!("{}{}", pfx, src_folder)
            } else {
                src_folder.clone()
            };

            // Ensure destination folder exists
            if !config.dry_run && config.create_missing_folders && !dest_folders.contains(&dest_folder) {
                if let Err(e) = dst_client.create_folder(&dest_folder) {
                    warn!(folder = %dest_folder, error = %e, "Create folder returned error (may already exist)");
                } else {
                    dest_folders.insert(dest_folder.clone());
                }
            }

            info!(
                source = %src_folder,
                target = %dest_folder,
                progress = format!("{}/{}", folder_idx + 1, total_folders),
                "Migrating IMAP folder server-to-server"
            );

            let mut msg_idx = 0;
            let res = src_client.fetch_folder_raw(src_folder, None, |raw_bytes, flags, date, uid, subject| {
                msg_idx += 1;
                summary.total_messages_migrated += 1;
                summary.total_bytes_transferred += raw_bytes.len();

                let progress = ImapMigrationProgress {
                    current_source_folder: src_folder.clone(),
                    current_target_folder: dest_folder.clone(),
                    folder_index: folder_idx + 1,
                    total_folders,
                    message_index_in_folder: msg_idx,
                    total_messages_in_folder: 0,
                    total_messages_migrated: summary.total_messages_migrated,
                    total_bytes_transferred: summary.total_bytes_transferred,
                    current_subject: subject,
                    current_uid: uid,
                };

                on_progress(&progress);

                if !config.dry_run {
                    let flags_to_pass = if config.preserve_flags { &flags[..] } else { &[] };
                    let date_to_pass = if config.preserve_dates { date } else { None };
                    dst_client.append_message(&dest_folder, raw_bytes, flags_to_pass, date_to_pass)?;
                }

                Ok(())
            });

            match res {
                Ok((count, _)) => {
                    summary.total_folders_migrated += 1;
                    debug!(folder = %src_folder, count = count, "Folder migration complete");
                }
                Err(e) => {
                    let err_msg = format!("Error migrating folder {}: {}", src_folder, e);
                    warn!(%err_msg);
                    summary.errors.push(err_msg);
                }
            }
        }

        let _ = src_client.disconnect();
        let _ = dst_client.disconnect();

        summary.duration_seconds = start_time.elapsed().as_secs_f64();
        info!(
            folders = summary.total_folders_migrated,
            messages = summary.total_messages_migrated,
            bytes = summary.total_bytes_transferred,
            duration = format!("{:.2}s", summary.duration_seconds),
            "IMAP server-to-server migration complete"
        );

        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn test_split_folder_hierarchy() {
        assert_eq!(ImapClient::split_folder_hierarchy("INBOX", Some("/")), vec!["INBOX"]);
        assert_eq!(
            ImapClient::split_folder_hierarchy("INBOX/Archives/2026", Some("/")),
            vec!["INBOX", "Archives", "2026"]
        );
        assert_eq!(
            ImapClient::split_folder_hierarchy("\"[Gmail]/All Mail\"", Some("/")),
            vec!["[Gmail]", "All Mail"]
        );
        assert_eq!(
            ImapClient::split_folder_hierarchy("INBOX.Work.Projects", Some(".")),
            vec!["INBOX", "Work", "Projects"]
        );
        assert_eq!(
            ImapClient::split_folder_hierarchy("INBOX\\Exchange\\Archive", Some("\\")),
            vec!["INBOX", "Exchange", "Archive"]
        );
        assert_eq!(ImapClient::split_folder_hierarchy("", None), vec!["Inbox"]);
    }

    #[test]
    fn test_imap_config_defaults() {
        let cfg = ImapConfig::default();
        assert_eq!(cfg.host, "imap.gmail.com");
        assert_eq!(cfg.port, 993);
        assert!(cfg.use_tls);
        assert!(!cfg.insecure_certs);
    }

    #[test]
    fn test_imap_mock_sync_e2e() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind mock listener");
        let local_addr = listener.local_addr().expect("Failed to get local addr");

        let handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().expect("Failed to accept");
            let mut reader = BufReader::new(socket.try_clone().expect("Failed to clone socket"));

            // 1. Initial Greeting
            socket.write_all(b"* OK Mock IMAP Server Ready\r\n").unwrap();

            let mut line = String::new();
            while reader.read_line(&mut line).unwrap() > 0 {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.is_empty() {
                    line.clear();
                    continue;
                }
                let tag = parts[0];
                let cmd = parts.get(1).map(|c| c.to_uppercase()).unwrap_or_default();

                match cmd.as_str() {
                    "LOGIN" => {
                        socket
                            .write_all(format!("{} OK LOGIN completed\r\n", tag).as_bytes())
                            .unwrap();
                    }
                    "LIST" => {
                        socket
                            .write_all(b"* LIST (\\HasNoChildren) \"/\" \"INBOX\"\r\n")
                            .unwrap();
                        socket
                            .write_all(b"* LIST (\\HasNoChildren) \"/\" \"Archive\"\r\n")
                            .unwrap();
                        socket
                            .write_all(format!("{} OK LIST completed\r\n", tag).as_bytes())
                            .unwrap();
                    }
                    "SELECT" => {
                        socket.write_all(b"* 1 EXISTS\r\n").unwrap();
                        socket.write_all(b"* 1 RECENT\r\n").unwrap();
                        socket.write_all(b"* OK [UIDVALIDITY 999] UIDs valid\r\n").unwrap();
                        socket
                            .write_all(format!("{} OK [READ-WRITE] SELECT completed\r\n", tag).as_bytes())
                            .unwrap();
                    }
                    "UID" => {
                        let sub_cmd = parts.get(2).map(|c| c.to_uppercase()).unwrap_or_default();
                        if sub_cmd == "SEARCH" {
                            socket.write_all(b"* SEARCH 42\r\n").unwrap();
                            socket
                                .write_all(format!("{} OK SEARCH completed\r\n", tag).as_bytes())
                                .unwrap();
                        } else if sub_cmd == "FETCH" {
                            let raw_msg = "From: alice@example.com\r\nTo: bob@example.com\r\nSubject: Mock IMAP Subject\r\nDate: Sat, 07 Nov 2026 12:00:00 +0000\r\n\r\nMock email body from IMAP!";
                            let resp = format!(
                                "* 1 FETCH (UID 42 FLAGS (\\Seen \\Flagged) INTERNALDATE \"07-Nov-2026 12:00:00 +0000\" BODY[] {{{}}}\r\n{})\r\n{} OK FETCH completed\r\n",
                                raw_msg.len(),
                                raw_msg,
                                tag
                            );
                            socket.write_all(resp.as_bytes()).unwrap();
                        }
                    }
                    "LOGOUT" => {
                        socket.write_all(b"* BYE Mock logging out\r\n").unwrap();
                        socket
                            .write_all(format!("{} OK LOGOUT completed\r\n", tag).as_bytes())
                            .unwrap();
                        break;
                    }
                    _ => {
                        socket
                            .write_all(format!("{} BAD Unsupported command\r\n", tag).as_bytes())
                            .unwrap();
                    }
                }
                line.clear();
            }
        });

        let config = ImapConfig {
            host: "127.0.0.1".to_string(),
            port: local_addr.port(),
            username: "user@example.com".to_string(),
            password: "secret".to_string(),
            use_tls: false, // Plain TCP for loopback mock
            ..Default::default()
        };

        let mut client = ImapClient::new(config);
        assert!(client.connect().is_ok());

        // Test listing folders
        let folders = client.list_folders().expect("Failed to list folders");
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0].name, "INBOX");
        assert_eq!(folders[1].name, "Archive");

        // Test fetching messages with sync checkpoint
        let mut checkpoint = SyncCheckpoint {
            id: "chk_test".to_string(),
            timestamp: Utc::now(),
            source_id: "imap_test".to_string(),
            target_id: "test.pst".to_string(),
            migrated_message_ids: std::collections::HashSet::new(),
            folder_high_watermarks: std::collections::HashMap::new(),
            total_bytes_migrated: 0,
            total_messages_migrated: 0,
            status: "running".to_string(),
        };

        let mut received_messages = Vec::new();
        let total = client
            .sync_all(&mut checkpoint, |_prog, msg| {
                received_messages.push(msg);
            })
            .expect("Failed to sync_all");

        assert!(total >= 1);
        assert_eq!(received_messages.len(), 2); // INBOX (1 msg) + Archive (1 msg from mock)
        let first = &received_messages[0];
        assert_eq!(first.subject, "Mock IMAP Subject");
        assert_eq!(first.from.email, "alice@example.com");
        assert_eq!(first.to[0].email, "bob@example.com");
        assert!(first.flags.read);
        assert!(first.flags.flagged);
        assert_eq!(checkpoint.folder_high_watermarks.get("INBOX"), Some(&42));

        client.disconnect().expect("Disconnect failed");
        handle.join().expect("Mock server thread panicked");
    }

    #[test]
    fn test_imap_server_to_server_migration_e2e() {
        // 1. Source Mock IMAP Server
        let src_listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind src mock listener");
        let src_addr = src_listener.local_addr().expect("Failed to get src local addr");

        let src_handle = thread::spawn(move || {
            let (mut socket, _) = src_listener.accept().expect("Failed to accept src");
            let mut reader = BufReader::new(socket.try_clone().expect("Failed to clone socket"));

            socket.write_all(b"* OK Mock Source IMAP Ready\r\n").unwrap();

            let mut line = String::new();
            while reader.read_line(&mut line).unwrap() > 0 {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.is_empty() {
                    line.clear();
                    continue;
                }
                let tag = parts[0];
                let cmd = parts.get(1).map(|c| c.to_uppercase()).unwrap_or_default();

                match cmd.as_str() {
                    "LOGIN" => {
                        socket
                            .write_all(format!("{} OK LOGIN completed\r\n", tag).as_bytes())
                            .unwrap();
                    }
                    "LIST" => {
                        socket
                            .write_all(b"* LIST (\\HasNoChildren) \"/\" \"INBOX\"\r\n")
                            .unwrap();
                        socket
                            .write_all(format!("{} OK LIST completed\r\n", tag).as_bytes())
                            .unwrap();
                    }
                    "SELECT" => {
                        socket.write_all(b"* 1 EXISTS\r\n").unwrap();
                        socket.write_all(b"* 1 RECENT\r\n").unwrap();
                        socket.write_all(b"* OK [UIDVALIDITY 1234] UIDs valid\r\n").unwrap();
                        socket
                            .write_all(format!("{} OK [READ-WRITE] SELECT completed\r\n", tag).as_bytes())
                            .unwrap();
                    }
                    "UID" => {
                        let sub_cmd = parts.get(2).map(|c| c.to_uppercase()).unwrap_or_default();
                        if sub_cmd == "SEARCH" {
                            socket.write_all(b"* SEARCH 88\r\n").unwrap();
                            socket
                                .write_all(format!("{} OK SEARCH completed\r\n", tag).as_bytes())
                                .unwrap();
                        } else if sub_cmd == "FETCH" {
                            let raw_msg = "From: alice@source.corp\r\nTo: bob@dest.corp\r\nSubject: Direct Server Transfer\r\nDate: Sat, 07 Nov 2026 14:00:00 +0000\r\n\r\nZero local disk saving!";
                            let resp = format!(
                                "* 1 FETCH (UID 88 FLAGS (\\Seen \\Flagged) INTERNALDATE \"07-Nov-2026 14:00:00 +0000\" ENVELOPE (\"Sat, 07 Nov 2026 14:00:00 +0000\" \"Direct Server Transfer\" ((NIL NIL \"alice\" \"source.corp\")) NIL NIL NIL NIL NIL NIL NIL) BODY[] {{{}}}\r\n{})\r\n{} OK FETCH completed\r\n",
                                raw_msg.len(),
                                raw_msg,
                                tag
                            );
                            socket.write_all(resp.as_bytes()).unwrap();
                        }
                    }
                    "LOGOUT" => {
                        socket.write_all(b"* BYE Source logging out\r\n").unwrap();
                        socket
                            .write_all(format!("{} OK LOGOUT completed\r\n", tag).as_bytes())
                            .unwrap();
                        break;
                    }
                    _ => {
                        socket
                            .write_all(format!("{} BAD Unsupported\r\n", tag).as_bytes())
                            .unwrap();
                    }
                }
                line.clear();
            }
        });

        // 2. Destination Mock IMAP Server
        let dst_listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind dst mock listener");
        let dst_addr = dst_listener.local_addr().expect("Failed to get dst local addr");

        let dst_handle = thread::spawn(move || {
            let (mut socket, _) = dst_listener.accept().expect("Failed to accept dst");
            let mut reader = BufReader::new(socket.try_clone().expect("Failed to clone socket"));

            socket.write_all(b"* OK Mock Destination IMAP Ready\r\n").unwrap();

            let mut line = String::new();
            while reader.read_line(&mut line).unwrap() > 0 {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.is_empty() {
                    line.clear();
                    continue;
                }
                let tag = parts[0];
                let cmd = parts.get(1).map(|c| c.to_uppercase()).unwrap_or_default();

                match cmd.as_str() {
                    "LOGIN" => {
                        socket
                            .write_all(format!("{} OK LOGIN completed\r\n", tag).as_bytes())
                            .unwrap();
                    }
                    "LIST" => {
                        socket
                            .write_all(b"* LIST (\\HasNoChildren) \"/\" \"INBOX\"\r\n")
                            .unwrap();
                        socket
                            .write_all(format!("{} OK LIST completed\r\n", tag).as_bytes())
                            .unwrap();
                    }
                    "CREATE" => {
                        socket
                            .write_all(format!("{} OK CREATE completed\r\n", tag).as_bytes())
                            .unwrap();
                    }
                    "APPEND" => {
                        let literal_size: usize = line
                            .find('{')
                            .and_then(|start| {
                                line[start + 1..]
                                    .find('}')
                                    .map(|end| line[start + 1..start + 1 + end].parse().unwrap_or(0))
                            })
                            .unwrap_or(0);

                        socket.write_all(b"+ Ready for literal data\r\n").unwrap();

                        let mut literal_buf = vec![0u8; literal_size];
                        std::io::Read::read_exact(&mut reader, &mut literal_buf).unwrap();

                        socket
                            .write_all(format!("{} OK [APPENDUID 1 9001] APPEND completed\r\n", tag).as_bytes())
                            .unwrap();
                    }
                    "LOGOUT" => {
                        socket.write_all(b"* BYE Destination logging out\r\n").unwrap();
                        socket
                            .write_all(format!("{} OK LOGOUT completed\r\n", tag).as_bytes())
                            .unwrap();
                        break;
                    }
                    _ => {
                        socket
                            .write_all(format!("{} BAD Unsupported\r\n", tag).as_bytes())
                            .unwrap();
                    }
                }
                line.clear();
            }
        });

        // 3. Configure and execute server-to-server migration
        let config = ImapMigrationConfig {
            source: ImapConfig {
                host: "127.0.0.1".to_string(),
                port: src_addr.port(),
                username: "src_user@corp.local".to_string(),
                password: "src_pass".to_string(),
                use_tls: false,
                ..Default::default()
            },
            destination: ImapConfig {
                host: "127.0.0.1".to_string(),
                port: dst_addr.port(),
                username: "dst_user@corp.local".to_string(),
                password: "dst_pass".to_string(),
                use_tls: false,
                ..Default::default()
            },
            preserve_flags: true,
            preserve_dates: true,
            create_missing_folders: true,
            dry_run: false,
            ..Default::default()
        };

        let mut progress_events = Vec::new();
        let summary = ImapMigrator::migrate(&config, |prog| {
            progress_events.push(prog.clone());
        })
        .expect("IMAP server-to-server migration failed");

        assert_eq!(summary.total_folders_migrated, 1);
        assert_eq!(summary.total_messages_migrated, 1);
        assert!(summary.total_bytes_transferred > 0);
        assert!(summary.errors.is_empty());
        assert_eq!(progress_events.len(), 1);
        assert_eq!(progress_events[0].current_source_folder, "INBOX");
        assert_eq!(progress_events[0].current_target_folder, "INBOX");
        assert_eq!(progress_events[0].current_uid, 88);
        assert_eq!(
            progress_events[0].current_subject,
            Some("Direct Server Transfer".to_string())
        );

        src_handle.join().expect("Source server thread panicked");
        dst_handle.join().expect("Destination server thread panicked");
    }

    #[test]
    fn test_imap_xoauth2_authentication_e2e() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind mock listener");
        let local_addr = listener.local_addr().expect("Failed to get local addr");

        let handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().expect("Failed to accept");
            socket.write_all(b"* OK Mock OAuth IMAP Ready\r\n").unwrap();

            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut line = String::new();

            while reader.read_line(&mut line).unwrap() > 0 {
                let trimmed = line.trim();
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }
                let tag = parts[0];
                let cmd = parts.get(1).map(|c| c.to_uppercase()).unwrap_or_default();

                match cmd.as_str() {
                    "AUTHENTICATE" => {
                        let sub_mech = parts.get(2).map(|c| c.to_uppercase()).unwrap_or_default();
                        assert_eq!(sub_mech, "XOAUTH2");
                        if parts.len() > 3 {
                            // SASL IR (Initial Response) included on same line
                            socket
                                .write_all(format!("{} OK [CAPABILITY IMAP4rev1] XOAUTH2 authentication successful\r\n", tag).as_bytes())
                                .unwrap();
                        } else {
                            // Standard two-step challenge continuation
                            socket.write_all(b"+\r\n").unwrap();
                            let mut auth_line = String::new();
                            reader.read_line(&mut auth_line).unwrap();
                            assert!(!auth_line.trim().is_empty());
                            socket
                                .write_all(format!("{} OK [CAPABILITY IMAP4rev1] XOAUTH2 authentication successful\r\n", tag).as_bytes())
                                .unwrap();
                        }
                    }
                    "LOGOUT" => {
                        socket.write_all(b"* BYE logging out\r\n").unwrap();
                        socket
                            .write_all(format!("{} OK LOGOUT completed\r\n", tag).as_bytes())
                            .unwrap();
                        break;
                    }
                    _ => {
                        socket
                            .write_all(format!("{} BAD Unsupported\r\n", tag).as_bytes())
                            .unwrap();
                    }
                }
                line.clear();
            }
        });

        let config = ImapConfig {
            host: "127.0.0.1".to_string(),
            port: local_addr.port(),
            username: "user@gmail.com".to_string(),
            password: String::new(),
            auth_method: ImapAuthMethod::XOAuth2,
            oauth_token: Some("ya29.sample_oauth_token_12345".to_string()),
            use_tls: false,
            ..Default::default()
        };

        let mut client = ImapClient::new(config);
        assert!(client.connect().is_ok());
        assert!(client.is_connected());
        client.disconnect().expect("Disconnect failed");

        handle.join().expect("Mock thread panicked");
    }
}
