use crate::models::{Attachment, EmailAddress, EmailMessage, Importance, MessageFlags};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use tracing::{debug, trace};

pub struct EmlReader;

impl EmlReader {
    pub fn parse(raw_bytes: &[u8], folder_path: Vec<String>) -> Result<EmailMessage, Box<dyn std::error::Error>> {
        trace!("Parsing RFC 5322 MIME payload of {} bytes", raw_bytes.len());
        let (sha256, md5) = EmailMessage::compute_hashes(raw_bytes);

        // Split headers and body
        let (header_bytes, body_bytes) = Self::split_headers_and_body(raw_bytes);
        let header_str = String::from_utf8_lossy(header_bytes);
        let headers = Self::parse_headers(&header_str);

        let subject = headers.get("subject")
            .map(|s| Self::decode_rfc2047(s))
            .unwrap_or_else(|| "(No Subject)".to_string());

        let message_id = headers.get("message-id")
            .cloned()
            .unwrap_or_else(|| format!("<auto_{}@mail2rust.local>", &sha256[..16]));

        let in_reply_to = headers.get("in-reply-to").map(|s| s.as_str());

        let from = headers.get("from")
            .map(|s| EmailAddress::parse(&Self::decode_rfc2047(s)))
            .unwrap_or_else(|| EmailAddress::new("Unknown Sender", "unknown@mail2rust.local"));

        let to = headers.get("to")
            .map(|s| Self::parse_address_list(&Self::decode_rfc2047(s)))
            .unwrap_or_default();

        let cc = headers.get("cc")
            .map(|s| Self::parse_address_list(&Self::decode_rfc2047(s)))
            .unwrap_or_default();

        let bcc = headers.get("bcc")
            .map(|s| Self::parse_address_list(&Self::decode_rfc2047(s)))
            .unwrap_or_default();

        let date = headers.get("date")
            .and_then(|d| DateTime::parse_from_rfc2822(d.trim()).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let importance = match headers.get("importance").map(|s| s.to_lowercase()).as_deref() {
            Some("high") | Some("urgent") => Importance::High,
            Some("low") => Importance::Low,
            _ => Importance::Normal,
        };

        // Parse flags from common headers (X-Status, X-Mozilla-Status)
        let mut flags = MessageFlags::default();
        if let Some(moz) = headers.get("x-mozilla-status") {
            if let Ok(code) = u32::from_str_radix(moz.trim(), 16) {
                flags.read = (code & 0x0001) != 0;
                flags.replied = (code & 0x0002) != 0;
                flags.flagged = (code & 0x0004) != 0;
                flags.deleted = (code & 0x0008) != 0;
                flags.forwarded = (code & 0x1000) != 0;
            }
        }
        if let Some(status) = headers.get("status") {
            if status.contains('R') {
                flags.read = true;
            }
            if status.contains('A') {
                flags.replied = true;
            }
            if status.contains('F') {
                flags.flagged = true;
            }
        }

        let content_type = headers.get("content-type").cloned().unwrap_or_default();
        let transfer_encoding = headers.get("content-transfer-encoding").cloned().unwrap_or_default();

        let mut body_text = String::new();
        let mut body_html = None;
        let mut attachments = Vec::new();

        Self::parse_mime_part(
            body_bytes,
            &content_type,
            &transfer_encoding,
            &headers,
            &mut body_text,
            &mut body_html,
            &mut attachments,
        );

        let conversation_id = EmailMessage::generate_conversation_id(&subject, &message_id, in_reply_to);
        let id = format!("msg_{}", &sha256[..16]);

        debug!(id = %id, subject = %subject, attachments = attachments.len(), "Parsed email message");

        Ok(EmailMessage {
            id,
            message_id,
            conversation_id,
            folder_path,
            subject,
            from,
            to,
            cc,
            bcc,
            date,
            size_bytes: raw_bytes.len(),
            body_text,
            body_html,
            raw_mime: raw_bytes.to_vec(),
            attachments,
            mapi_properties: Vec::new(),
            flags,
            importance,
            sha256,
            md5,
            bates_number: None,
            legal_hold: false,
            source_format: "EML".to_string(),
        })
    }

    fn split_headers_and_body(bytes: &[u8]) -> (&[u8], &[u8]) {
        for i in 0..bytes.len().saturating_sub(3) {
            if &bytes[i..i + 4] == b"\r\n\r\n" {
                return (&bytes[..i], &bytes[i + 4..]);
            }
        }
        for i in 0..bytes.len().saturating_sub(1) {
            if &bytes[i..i + 2] == b"\n\n" {
                return (&bytes[..i], &bytes[i + 2..]);
            }
        }
        (bytes, &[])
    }

    fn parse_headers(header_str: &str) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        let mut current_key = String::new();
        let mut current_val = String::new();

        for line in header_str.lines() {
            if line.starts_with(' ') || line.starts_with('\t') {
                if !current_key.is_empty() {
                    current_val.push(' ');
                    current_val.push_str(line.trim());
                }
            } else if let Some(colon) = line.find(':') {
                if !current_key.is_empty() {
                    headers.insert(current_key.to_lowercase(), current_val.trim().to_string());
                }
                current_key = line[..colon].trim().to_string();
                current_val = line[colon + 1..].trim().to_string();
            }
        }
        if !current_key.is_empty() {
            headers.insert(current_key.to_lowercase(), current_val.trim().to_string());
        }
        headers
    }

    pub fn decode_rfc2047(input: &str) -> String {
        let mut result = String::new();
        let mut remaining = input;

        while let Some(start) = remaining.find("=?") {
            result.push_str(&remaining[..start]);
            let rest = &remaining[start + 2..];
            if let Some(first_delim) = rest.find('?') {
                let _charset = &rest[..first_delim];
                let enc_rest = &rest[first_delim + 1..];
                if let Some(second_delim) = enc_rest.find('?') {
                    let enc_type = &enc_rest[..second_delim];
                    let payload_rest = &enc_rest[second_delim + 1..];
                    if let Some(end) = payload_rest.find("?=") {
                        let payload = &payload_rest[..end];
                        let decoded = match enc_type.to_uppercase().as_str() {
                            "B" => Self::decode_base64_str(payload),
                            "Q" => Self::decode_quoted_printable_str(payload),
                            _ => payload.to_string(),
                        };
                        result.push_str(&decoded);
                        remaining = &payload_rest[end + 2..];
                        continue;
                    }
                }
            }
            result.push_str("=?");
            remaining = rest;
        }
        result.push_str(remaining);
        result
    }

    pub fn decode_base64_str(input: &str) -> String {
        let clean: String = input.chars().filter(|c| !c.is_whitespace()).collect();
        match Self::decode_base64_bytes(clean.as_bytes()) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
            Err(_) => input.to_string(),
        }
    }

    pub fn decode_quoted_printable_str(input: &str) -> String {
        let mut output = Vec::new();
        let bytes = input.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'=' && i + 2 < bytes.len() {
                if bytes[i + 1] == b'\r' && bytes[i + 2] == b'\n' {
                    i += 3;
                    continue;
                }
                if bytes[i + 1] == b'\n' {
                    i += 2;
                    continue;
                }
                if let Ok(hex_val) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
                    output.push(hex_val);
                    i += 3;
                    continue;
                }
            } else if bytes[i] == b'_' {
                output.push(b' ');
            } else {
                output.push(bytes[i]);
            }
            i += 1;
        }
        String::from_utf8_lossy(&output).to_string()
    }

    pub fn decode_base64_bytes(data: &[u8]) -> Result<Vec<u8>, String> {
        const B64_TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut lookup = [255u8; 256];
        for (i, &b) in B64_TABLE.iter().enumerate() {
            lookup[b as usize] = i as u8;
        }

        let mut output = Vec::new();
        let mut buf = 0u32;
        let mut bits = 0;

        for &byte in data {
            if byte == b'=' || byte.is_ascii_whitespace() {
                continue;
            }
            let val = lookup[byte as usize];
            if val == 255 {
                continue;
            }
            buf = (buf << 6) | (val as u32);
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                output.push(((buf >> bits) & 0xFF) as u8);
            }
        }

        Ok(output)
    }

    fn parse_address_list(raw: &str) -> Vec<EmailAddress> {
        raw.split(',')
            .map(|s| EmailAddress::parse(s))
            .filter(|a| !a.email.is_empty())
            .collect()
    }

    fn parse_mime_part(
        bytes: &[u8],
        content_type_header: &str,
        transfer_encoding: &str,
        part_headers: &HashMap<String, String>,
        body_text: &mut String,
        body_html: &mut Option<String>,
        attachments: &mut Vec<Attachment>,
    ) {
        let ct_lower = content_type_header.to_lowercase();
        if ct_lower.contains("multipart/") {
            // Find boundary
            if let Some(boundary) = Self::extract_boundary(content_type_header) {
                let parts = Self::split_multipart(bytes, &boundary);
                for part_bytes in parts {
                    let (sub_headers_bytes, sub_body_bytes) = Self::split_headers_and_body(&part_bytes);
                    let sub_header_str = String::from_utf8_lossy(sub_headers_bytes);
                    let sub_headers = Self::parse_headers(&sub_header_str);
                    let sub_ct = sub_headers.get("content-type").cloned().unwrap_or_default();
                    let sub_te = sub_headers.get("content-transfer-encoding").cloned().unwrap_or_default();

                    Self::parse_mime_part(
                        sub_body_bytes,
                        &sub_ct,
                        &sub_te,
                        &sub_headers,
                        body_text,
                        body_html,
                        attachments,
                    );
                }
                return;
            }
        }

        // Single part: decode data
        let decoded = if transfer_encoding.to_lowercase().contains("base64") {
            Self::decode_base64_bytes(bytes).unwrap_or_else(|_| bytes.to_vec())
        } else if transfer_encoding.to_lowercase().contains("quoted-printable") {
            Self::decode_quoted_printable_bytes(bytes)
        } else {
            bytes.to_vec()
        };

        // Check disposition for attachment
        let disposition = part_headers.get("content-disposition").cloned().unwrap_or_default();
        let filename = Self::extract_filename(&disposition, content_type_header);

        if let Some(fname) = filename {
            let cid = part_headers.get("content-id").map(|s| s.trim_matches(|c| c == '<' || c == '>').to_string());
            let att = Attachment::new(
                fname,
                if ct_lower.is_empty() { "application/octet-stream".to_string() } else { ct_lower.split(';').next().unwrap_or("").trim().to_string() },
                decoded,
                cid,
            );
            attachments.push(att);
        } else if ct_lower.contains("text/html") {
            let html_str = String::from_utf8_lossy(&decoded).to_string();
            *body_html = Some(html_str);
        } else {
            let text_str = String::from_utf8_lossy(&decoded).to_string();
            if body_text.is_empty() {
                *body_text = text_str;
            } else {
                body_text.push_str("\n\n");
                body_text.push_str(&text_str);
            }
        }
    }

    fn extract_boundary(ct: &str) -> Option<String> {
        let parts = ct.split(';');
        for p in parts {
            let trimmed = p.trim();
            if trimmed.to_lowercase().starts_with("boundary=") {
                let b = &trimmed[9..];
                return Some(b.trim_matches('"').trim().to_string());
            }
        }
        None
    }

    fn extract_filename(disp: &str, ct: &str) -> Option<String> {
        for source in [disp, ct] {
            for part in source.split(';') {
                let p = part.trim();
                if p.to_lowercase().starts_with("filename=") {
                    return Some(Self::decode_rfc2047(p[9..].trim_matches('"').trim()));
                }
                if p.to_lowercase().starts_with("name=") {
                    return Some(Self::decode_rfc2047(p[5..].trim_matches('"').trim()));
                }
            }
        }
        None
    }

    fn split_multipart(bytes: &[u8], boundary: &str) -> Vec<Vec<u8>> {
        let delimiter = format!("--{}", boundary);
        let delim_bytes = delimiter.as_bytes();
        let mut parts = Vec::new();

        let mut start_idx = None;
        let mut i = 0;
        while i + delim_bytes.len() <= bytes.len() {
            if &bytes[i..i + delim_bytes.len()] == delim_bytes {
                if let Some(prev) = start_idx {
                    let mut part_slice = &bytes[prev..i];
                    // Strip leading/trailing newlines
                    if part_slice.starts_with(b"\r\n") {
                        part_slice = &part_slice[2..];
                    } else if part_slice.starts_with(b"\n") {
                        part_slice = &part_slice[1..];
                    }
                    if part_slice.ends_with(b"\r\n") {
                        part_slice = &part_slice[..part_slice.len() - 2];
                    } else if part_slice.ends_with(b"\n") {
                        part_slice = &part_slice[..part_slice.len() - 1];
                    }
                    if !part_slice.is_empty() {
                        parts.push(part_slice.to_vec());
                    }
                }
                i += delim_bytes.len();
                start_idx = Some(i);
                continue;
            }
            i += 1;
        }

        parts
    }

    fn decode_quoted_printable_bytes(bytes: &[u8]) -> Vec<u8> {
        let mut output = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'=' && i + 2 < bytes.len() {
                if bytes[i + 1] == b'\r' && bytes[i + 2] == b'\n' {
                    i += 3;
                    continue;
                }
                if bytes[i + 1] == b'\n' {
                    i += 2;
                    continue;
                }
                if let Ok(hex) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
                    output.push(hex);
                    i += 3;
                    continue;
                }
            }
            output.push(bytes[i]);
            i += 1;
        }
        output
    }
}
