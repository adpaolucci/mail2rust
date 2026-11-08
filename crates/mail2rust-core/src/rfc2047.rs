//! RFC 2047 MIME Header and Encoded-Word Decoder & Sanitizer

/// Decode an RFC 2047 MIME encoded-word string (e.g. `=?UTF-8?Q?...?=` or `=?UTF-8?B?...?=`) into a clean Unicode string.
/// Accurately handles linear whitespace suppression between adjacent encoded words,
/// base64 and quoted-printable encodings, and common character sets (UTF-8, ISO-8859-1, Windows-1252, ASCII).
pub fn decode_rfc2047(input: &str) -> String {
    if !input.contains("=?") {
        return input.to_string();
    }

    let mut result = String::with_capacity(input.len());
    let mut last_was_encoded = false;
    let mut pending_whitespace = String::new();

    let bytes = input.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        if bytes[pos..].starts_with(b"=?") {
            // Potential encoded word
            let start = pos;
            if let Some((encoded_str, next_pos)) = try_parse_encoded_word(&bytes[start..]) {
                if !last_was_encoded {
                    result.push_str(&pending_whitespace);
                }
                pending_whitespace.clear();
                result.push_str(&encoded_str);
                last_was_encoded = true;
                pos = start + next_pos;
                continue;
            }
        }

        let ch = input[pos..].chars().next().unwrap_or(' ');
        let ch_len = ch.len_utf8();

        if ch.is_whitespace() {
            if last_was_encoded {
                pending_whitespace.push(ch);
            } else {
                result.push(ch);
            }
        } else {
            if last_was_encoded {
                result.push_str(&pending_whitespace);
                pending_whitespace.clear();
                last_was_encoded = false;
            }
            result.push(ch);
        }

        pos += ch_len;
    }

    result.push_str(&pending_whitespace);
    result
}

fn try_parse_encoded_word(slice: &[u8]) -> Option<(String, usize)> {
    if !slice.starts_with(b"=?") {
        return None;
    }
    // Format: =?charset?encoding?encoded_text?=
    let rest = &slice[2..];
    let first_delim = rest.iter().position(|&b| b == b'?')?;
    let charset_bytes = &rest[..first_delim];
    let charset = std::str::from_utf8(charset_bytes).ok()?.to_lowercase();

    let enc_rest = &rest[first_delim + 1..];
    let second_delim = enc_rest.iter().position(|&b| b == b'?')?;
    let enc_type_bytes = &enc_rest[..second_delim];
    let enc_type = std::str::from_utf8(enc_type_bytes).ok()?.to_uppercase();

    let payload_rest = &enc_rest[second_delim + 1..];
    let end_delim = payload_rest.windows(2).position(|w| w == b"?=")?;
    let payload = &payload_rest[..end_delim];

    let total_len = 2 + first_delim + 1 + second_delim + 1 + end_delim + 2;

    let raw_bytes = match enc_type.as_str() {
        "B" => decode_base64_bytes(payload).ok()?,
        "Q" => decode_quoted_printable_bytes(payload),
        _ => return None,
    };

    let decoded_str = decode_bytes_with_charset(&raw_bytes, &charset);
    Some((decoded_str, total_len))
}

fn decode_bytes_with_charset(bytes: &[u8], charset: &str) -> String {
    match charset {
        "utf-8" | "utf8" | "us-ascii" | "ascii" => String::from_utf8_lossy(bytes).to_string(),
        "iso-8859-1" | "latin1" => bytes.iter().map(|&b| b as char).collect(),
        "windows-1252" | "cp1252" => bytes
            .iter()
            .map(|&b| match b {
                0x80 => '€',
                0x82 => '‚',
                0x83 => 'ƒ',
                0x84 => '„',
                0x85 => '…',
                0x86 => '†',
                0x87 => '‡',
                0x88 => 'ˆ',
                0x89 => '‰',
                0x8A => 'Š',
                0x8B => '‹',
                0x8C => 'Œ',
                0x8E => 'Ž',
                0x91 => '‘',
                0x92 => '’',
                0x93 => '“',
                0x94 => '”',
                0x95 => '•',
                0x96 => '–',
                0x97 => '—',
                0x98 => '˜',
                0x99 => '™',
                0x9A => 'š',
                0x9B => '›',
                0x9C => 'œ',
                0x9E => 'ž',
                0x9F => 'Ÿ',
                _ => b as char,
            })
            .collect(),
        _ => {
            // Default to lossy UTF-8
            String::from_utf8_lossy(bytes).to_string()
        }
    }
}

pub fn decode_quoted_printable_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(bytes.len());
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
            if let Ok(hex_str) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(val) = u8::from_str_radix(hex_str, 16) {
                    output.push(val);
                    i += 3;
                    continue;
                }
            }
        } else if bytes[i] == b'_' {
            output.push(b' ');
        } else {
            output.push(bytes[i]);
        }
        i += 1;
    }
    output
}

pub fn decode_base64_bytes(data: &[u8]) -> Result<Vec<u8>, String> {
    const B64_TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [255u8; 256];
    for (i, &b) in B64_TABLE.iter().enumerate() {
        lookup[b as usize] = i as u8;
    }

    let mut output = Vec::with_capacity(data.len() * 3 / 4);
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

/// Sanitize any string (such as an email subject or sender name) into a safe cross-platform filename.
/// Automatically decodes any RFC 2047 encoded words first, replaces invalid characters,
/// trims Windows reserved device names (CON, PRN, AUX, etc.), and truncates to max length.
pub fn sanitize_filename(s: &str, max_len: usize) -> String {
    let decoded = decode_rfc2047(s);
    let invalid = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
    let mut clean: String = decoded
        .chars()
        .map(|c| if invalid.contains(&c) || c.is_control() { '_' } else { c })
        .collect();

    clean = clean.trim().trim_matches('.').to_string();
    if clean.is_empty() || clean == "." || clean == ".." {
        clean = "message".to_string();
    }

    // Protect against Windows DOS reserved device names
    let upper = clean.to_ascii_uppercase();
    let stem = upper.split('.').next().unwrap_or("");
    if matches!(
        stem,
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    ) {
        clean = format!("_{}", clean);
    }

    if clean.chars().count() > max_len {
        clean.chars().take(max_len).collect()
    } else {
        clean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rfc2047_plain_text() {
        assert_eq!(decode_rfc2047("Normal Subject Line"), "Normal Subject Line");
    }

    #[test]
    fn test_rfc2047_q_encoding() {
        let raw = "=?UTF-8?Q?RE:_500_Enterprise_Way_DSC_01?=";
        assert_eq!(decode_rfc2047(raw), "RE: 500 Enterprise Way DSC 01");
    }

    #[test]
    fn test_rfc2047_b_encoding() {
        // "Test Subject" in base64 is "VGVzdCBTdWJqZWN0"
        let raw = "=?UTF-8?B?VGVzdCBTdWJqZWN0?=";
        assert_eq!(decode_rfc2047(raw), "Test Subject");
    }

    #[test]
    fn test_rfc2047_adjacent_whitespace_suppression() {
        let raw = "=?UTF-8?Q?Hello_?=  \t\r\n =?UTF-8?Q?World!?=";
        assert_eq!(decode_rfc2047(raw), "Hello World!");
    }

    #[test]
    fn test_rfc2047_iso_8859_1() {
        let raw = "=?ISO-8859-1?Q?Caf=E9_de_Paris?=";
        assert_eq!(decode_rfc2047(raw), "Café de Paris");
    }

    #[test]
    fn test_sanitize_filename_with_encoded_words() {
        let raw = "=?UTF-8?Q?RE:_500_Enterprise_Way_<Confidential>?=";
        let safe = sanitize_filename(raw, 50);
        assert_eq!(safe, "RE_ 500 Enterprise Way _Confidential_");
    }
}
