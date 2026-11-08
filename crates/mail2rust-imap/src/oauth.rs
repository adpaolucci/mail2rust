use native_tls::TlsConnector;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;

/// Supported authentication mechanisms for IMAP connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ImapAuthMethod {
    /// Standard password or provider App Password (IMAP LOGIN command).
    #[default]
    Password,
    /// SASL XOAUTH2 token-based authentication (Google Gmail, Yahoo, Microsoft 365).
    XOAuth2,
    /// SASL OAUTHBEARER token-based authentication (RFC 7628).
    OAuthBearer,
}

impl fmt::Display for ImapAuthMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImapAuthMethod::Password => write!(f, "Password / App Password"),
            ImapAuthMethod::XOAuth2 => write!(f, "OAuth 2.0 (XOAUTH2)"),
            ImapAuthMethod::OAuthBearer => write!(f, "OAuth 2.0 (OAUTHBEARER)"),
        }
    }
}

impl std::str::FromStr for ImapAuthMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().replace(['-', '_'], "").as_str() {
            "password" | "login" | "plain" | "apppassword" => Ok(ImapAuthMethod::Password),
            "xoauth2" | "xoauth" | "oauth2" | "oauth" => Ok(ImapAuthMethod::XOAuth2),
            "oauthbearer" | "bearer" => Ok(ImapAuthMethod::OAuthBearer),
            other => Err(format!(
                "Unknown auth method '{}'. Valid values: password, xoauth2, oauthbearer",
                other
            )),
        }
    }
}

/// SASL XOAUTH2 Authenticator conforming to Google, Yahoo, and Microsoft specifications.
pub struct XOAuth2Authenticator {
    pub username: String,
    pub access_token: String,
}

impl imap::Authenticator for XOAuth2Authenticator {
    type Response = Vec<u8>;

    fn process(&self, _challenge: &[u8]) -> Self::Response {
        // Format: user={user}\x01auth=Bearer {access_token}\x01\x01
        format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.username, self.access_token
        )
        .into_bytes()
    }
}

/// SASL OAUTHBEARER Authenticator conforming to RFC 7628 specifications.
pub struct OAuthBearerAuthenticator {
    pub username: String,
    pub host: String,
    pub port: u16,
    pub access_token: String,
}

impl imap::Authenticator for OAuthBearerAuthenticator {
    type Response = Vec<u8>;

    fn process(&self, _challenge: &[u8]) -> Self::Response {
        // RFC 7628 format: n,a={username},\x01host={host}\x01port={port}\x01auth=Bearer {token}\x01\x01
        format!(
            "n,a={},\x01host={}\x01port={}\x01auth=Bearer {}\x01\x01",
            self.username, self.host, self.port, self.access_token
        )
        .into_bytes()
    }
}

/// Major email providers with pre-configured OAuth 2.0 endpoints and default IMAP settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OAuthProvider {
    Google,
    Microsoft,
    Yahoo,
}

impl OAuthProvider {
    /// Detects the probable OAuth provider from an IMAP host name.
    pub fn from_host(host: &str) -> Option<Self> {
        let h = host.to_lowercase();
        if h.contains("gmail") || h.contains("google") {
            Some(OAuthProvider::Google)
        } else if h.contains("outlook") || h.contains("office365") || h.contains("live.com") || h.contains("hotmail") {
            Some(OAuthProvider::Microsoft)
        } else if h.contains("yahoo") || h.contains("aol") {
            Some(OAuthProvider::Yahoo)
        } else {
            None
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            OAuthProvider::Google => "Google (Gmail / Google Workspace)",
            OAuthProvider::Microsoft => "Microsoft (Outlook.com / Office 365 / M365)",
            OAuthProvider::Yahoo => "Yahoo Mail (Yahoo / AOL)",
        }
    }

    pub fn default_imap_host(&self) -> &'static str {
        match self {
            OAuthProvider::Google => "imap.gmail.com",
            OAuthProvider::Microsoft => "outlook.office365.com",
            OAuthProvider::Yahoo => "imap.mail.yahoo.com",
        }
    }

    pub fn default_imap_port(&self) -> u16 {
        993
    }

    pub fn auth_url(&self) -> &'static str {
        match self {
            OAuthProvider::Google => "https://accounts.google.com/o/oauth2/v2/auth",
            OAuthProvider::Microsoft => "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
            OAuthProvider::Yahoo => "https://api.login.yahoo.com/oauth2/request_auth",
        }
    }

    pub fn token_url(&self) -> &'static str {
        match self {
            OAuthProvider::Google => "https://oauth2.googleapis.com/token",
            OAuthProvider::Microsoft => "https://login.microsoftonline.com/common/oauth2/v2.0/token",
            OAuthProvider::Yahoo => "https://api.login.yahoo.com/oauth2/get_token",
        }
    }

    pub fn default_scope(&self) -> &'static str {
        match self {
            OAuthProvider::Google => "https://mail.google.com/",
            OAuthProvider::Microsoft => "https://outlook.office.com/IMAP.AccessAsUser.All offline_access",
            OAuthProvider::Yahoo => "mail-r mail-w",
        }
    }

    /// Provides clear instructions for using App Passwords as an alternative to OAuth2 developer apps.
    pub fn app_password_guide(&self) -> &'static str {
        match self {
            OAuthProvider::Google => {
                "Google Gmail App Passwords:\n\
                 1. Go to your Google Account at https://myaccount.google.com/apppasswords (or https://myaccount.google.com/security)\n\
                 2. Under 'How you sign in to Google', ensure 2-Step Verification is enabled.\n\
                 3. Select 'App passwords' (or visit https://myaccount.google.com/apppasswords directly).\n\
                 4. Enter an app name (e.g., 'Mail2Rust') and click 'Create'.\n\
                 5. Copy the 16-character generated password and use it with '--password' in Mail2Rust."
            }
            OAuthProvider::Microsoft => {
                "Microsoft / Outlook.com App Passwords:\n\
                 1. Go to https://account.live.com/proofs/manage/additional\n\
                 2. Under 'App passwords', click 'Create a new app password'.\n\
                 3. Copy the generated password and use it with '--password' in Mail2Rust.\n\
                 (Note: Office 365 corporate accounts may require admin consent for IMAP or modern OAuth2)."
            }
            OAuthProvider::Yahoo => {
                "Yahoo Mail App Passwords:\n\
                 1. Go to Yahoo Account Security at https://login.yahoo.com/account/security\n\
                 2. Scroll down and click 'Generate app password'.\n\
                 3. Enter 'Mail2Rust' and click 'Generate password'.\n\
                 4. Copy the 16-character code and use it with '--password' in Mail2Rust."
            }
        }
    }
}

/// Represents the response returned by an OAuth 2.0 token endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

impl OAuthTokenResponse {
    /// Saves the token data to a secure local JSON file.
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)
    }

    /// Loads token data from a previously saved JSON file.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let token: Self = serde_json::from_str(&content)?;
        Ok(token)
    }
}

/// Generates an authorization URL for user browser login.
pub fn build_authorization_url(
    provider: OAuthProvider,
    client_id: &str,
    redirect_uri: &str,
    state: &str,
) -> String {
    format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&access_type=offline&prompt=consent",
        provider.auth_url(),
        urlencoding_simple(client_id),
        urlencoding_simple(redirect_uri),
        urlencoding_simple(provider.default_scope()),
        urlencoding_simple(state)
    )
}

/// Exchanges an authorization code for access and refresh tokens via HTTPS POST.
pub fn exchange_code_for_token(
    provider: OAuthProvider,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<OAuthTokenResponse, Box<dyn std::error::Error>> {
    let params = vec![
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("code", code),
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect_uri),
    ];

    let resp_body = https_post_form(provider.token_url(), &params)?;
    let token: OAuthTokenResponse = serde_json::from_str(&resp_body)
        .map_err(|e| format!("Failed to parse OAuth token response: {} (Response: {})", e, resp_body))?;
    Ok(token)
}

/// Refreshes an expired access token using a refresh token via HTTPS POST.
pub fn refresh_access_token(
    provider: OAuthProvider,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<OAuthTokenResponse, Box<dyn std::error::Error>> {
    let params = vec![
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ];

    let resp_body = https_post_form(provider.token_url(), &params)?;
    let token: OAuthTokenResponse = serde_json::from_str(&resp_body)
        .map_err(|e| format!("Failed to parse refreshed OAuth token: {} (Response: {})", e, resp_body))?;
    Ok(token)
}

/// Lightweight, zero-external-dependency HTTPS POST form-urlencoded client using `native-tls`.
fn https_post_form(url_str: &str, params: &[(&str, &str)]) -> Result<String, Box<dyn std::error::Error>> {
    // Parse URL (e.g. "https://oauth2.googleapis.com/token")
    let without_proto = url_str
        .strip_prefix("https://")
        .ok_or_else(|| format!("Only https:// URLs are supported: {}", url_str))?;

    let (host, path) = match without_proto.find('/') {
        Some(pos) => (&without_proto[..pos], &without_proto[pos..]),
        None => (without_proto, "/"),
    };

    // Construct form body
    let mut form_body = String::new();
    for (i, (k, v)) in params.iter().enumerate() {
        if i > 0 {
            form_body.push('&');
        }
        form_body.push_str(&urlencoding_simple(k));
        form_body.push('=');
        form_body.push_str(&urlencoding_simple(v));
    }

    let addr = format!("{}:443", host);
    let tcp_stream = TcpStream::connect(&addr)?;
    tcp_stream.set_read_timeout(Some(std::time::Duration::from_secs(15)))?;
    tcp_stream.set_write_timeout(Some(std::time::Duration::from_secs(15)))?;

    let connector = TlsConnector::builder().build()?;
    let mut tls_stream = connector.connect(host, tcp_stream)?;

    let request = format!(
        "POST {} HTTP/1.1\r\n\
         Host: {}\r\n\
         User-Agent: mail2rust/3.3.0\r\n\
         Content-Type: application/x-www-form-urlencoded\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n\
         {}",
        path,
        host,
        form_body.len(),
        form_body
    );

    tls_stream.write_all(request.as_bytes())?;
    tls_stream.flush()?;

    let mut response_bytes = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match tls_stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => response_bytes.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(e) => return Err(Box::new(e)),
        }
    }

    let response_str = String::from_utf8_lossy(&response_bytes).to_string();

    // Split headers from body
    if let Some(pos) = response_str.find("\r\n\r\n") {
        Ok(response_str[pos + 4..].to_string())
    } else if let Some(pos) = response_str.find("\n\n") {
        Ok(response_str[pos + 2..].to_string())
    } else {
        Ok(response_str)
    }
}

/// Simple RFC 3986 percent-encoding for form parameters.
fn urlencoding_simple(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

/// Evaluates IMAP error responses and provides actionable diagnostic advice when connecting to
/// modern providers that require OAuth2 or App Passwords (e.g. Google, Yahoo, Microsoft).
pub fn diagnose_auth_failure(host: &str, error_msg: &str) -> String {
    let mut diag = format!("IMAP Authentication Failed for host '{}':\n{}\n", host, error_msg);

    if let Some(provider) = OAuthProvider::from_host(host) {
        diag.push_str("\n----------------------------------------------------------------------\n");
        diag.push_str(&format!("  DIAGNOSTIC ADVICE FOR {}:\n", provider.display_name().to_uppercase()));
        diag.push_str("----------------------------------------------------------------------\n");
        diag.push_str(
            "Modern email providers no longer allow your primary account password over IMAP.\n\
             You can connect using either of two supported methods:\n\n\
             METHOD 1: Generate a Provider 'App Password' (Quickest & Recommended):\n"
        );
        diag.push_str(provider.app_password_guide());
        diag.push_str(
            "\n\nMETHOD 2: Authenticate using OAuth 2.0 (XOAUTH2 / OAUTHBEARER):\n\
             - Pass your access token via: --oauth-token <TOKEN> --auth-method xoauth2\n\
             - Or use the built-in token helper: mail2rust oauth-login\n"
        );
        diag.push_str("----------------------------------------------------------------------\n");
    }

    diag
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xoauth2_authenticator_format() {
        use imap::Authenticator;

        let auth = XOAuth2Authenticator {
            username: "john.doe@gmail.com".to_string(),
            access_token: "ya29.a0ARrdaM-SAMPLE-TOKEN".to_string(),
        };

        let processed = auth.process(b"");
        let expected = b"user=john.doe@gmail.com\x01auth=Bearer ya29.a0ARrdaM-SAMPLE-TOKEN\x01\x01";
        assert_eq!(processed, expected);
    }

    #[test]
    fn test_oauthbearer_authenticator_format() {
        use imap::Authenticator;

        let auth = OAuthBearerAuthenticator {
            username: "user@office365.com".to_string(),
            host: "outlook.office365.com".to_string(),
            port: 993,
            access_token: "eyJhbGciOi...".to_string(),
        };

        let processed = auth.process(b"");
        let expected = b"n,a=user@office365.com,\x01host=outlook.office365.com\x01port=993\x01auth=Bearer eyJhbGciOi...\x01\x01";
        assert_eq!(processed, expected);
    }

    #[test]
    fn test_provider_detection_from_host() {
        assert_eq!(OAuthProvider::from_host("imap.gmail.com"), Some(OAuthProvider::Google));
        assert_eq!(OAuthProvider::from_host("outlook.office365.com"), Some(OAuthProvider::Microsoft));
        assert_eq!(OAuthProvider::from_host("imap.mail.yahoo.com"), Some(OAuthProvider::Yahoo));
        assert_eq!(OAuthProvider::from_host("mail.custom-domain.net"), None);
    }

    #[test]
    fn test_auth_method_parsing() {
        assert_eq!("xoauth2".parse::<ImapAuthMethod>().unwrap(), ImapAuthMethod::XOAuth2);
        assert_eq!("oauth".parse::<ImapAuthMethod>().unwrap(), ImapAuthMethod::XOAuth2);
        assert_eq!("bearer".parse::<ImapAuthMethod>().unwrap(), ImapAuthMethod::OAuthBearer);
        assert_eq!("password".parse::<ImapAuthMethod>().unwrap(), ImapAuthMethod::Password);
    }

    #[test]
    fn test_diagnose_auth_failure() {
        let msg = diagnose_auth_failure("imap.gmail.com", "[AUTHENTICATIONFAILED] Invalid credentials");
        assert!(msg.contains("DIAGNOSTIC ADVICE FOR GOOGLE"));
        assert!(msg.contains("App Passwords"));
        assert!(msg.contains("XOAUTH2"));
    }
}
