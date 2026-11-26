use chrono::{DateTime, TimeZone, Utc};

const FILETIME_UNIX_EPOCH_TICKS: u64 = 116_444_736_000_000_000;
const MAX_VALID_FILETIME: u64 = 2_650_467_744_000_000_000; // Year ~9999

/// Converts a Windows 64-bit FILETIME (100ns intervals since 1601-01-01 UTC) to `DateTime<Utc>`.
pub fn filetime_to_datetime(ft: u64) -> Option<DateTime<Utc>> {
    if !(FILETIME_UNIX_EPOCH_TICKS..=MAX_VALID_FILETIME).contains(&ft) {
        return None;
    }
    let ticks_since_unix = ft - FILETIME_UNIX_EPOCH_TICKS;
    let secs = i64::try_from(ticks_since_unix / 10_000_000).ok()?;
    let nsecs = ((ticks_since_unix % 10_000_000) * 100) as u32;
    Utc.timestamp_opt(secs, nsecs).single()
}

/// Converts a `DateTime<Utc>` to a Windows 64-bit FILETIME.
pub fn datetime_to_filetime(dt: DateTime<Utc>) -> u64 {
    let secs = dt.timestamp();
    if secs < 0 {
        return FILETIME_UNIX_EPOCH_TICKS;
    }
    let subsec_ticks = u64::from(dt.timestamp_subsec_nanos() / 100);
    FILETIME_UNIX_EPOCH_TICKS
        .saturating_add((secs as u64).saturating_mul(10_000_000))
        .saturating_add(subsec_ticks)
}

/// Decodes a Windows-1252 / CP1252 byte slice into UTF-8.
pub fn decode_cp1252(bytes: &[u8]) -> String {
    let trimmed = match bytes.iter().position(|&b| b == 0) {
        Some(pos) => &bytes[..pos],
        None => bytes,
    };
    if let Ok(s) = std::str::from_utf8(trimmed) {
        return s.to_string();
    }
    let mut out = String::with_capacity(trimmed.len());
    for &b in trimmed {
        let ch = match b {
            0x80 => '\u{20AC}',
            0x82 => '\u{201A}',
            0x83 => '\u{0192}',
            0x84 => '\u{201E}',
            0x85 => '\u{2026}',
            0x86 => '\u{2020}',
            0x87 => '\u{2021}',
            0x88 => '\u{02C6}',
            0x89 => '\u{2030}',
            0x8A => '\u{0160}',
            0x8B => '\u{2039}',
            0x8C => '\u{0152}',
            0x8E => '\u{017D}',
            0x91 => '\u{2018}',
            0x92 => '\u{2019}',
            0x93 => '\u{201C}',
            0x94 => '\u{201D}',
            0x95 => '\u{2022}',
            0x96 => '\u{2013}',
            0x97 => '\u{2014}',
            0x98 => '\u{02DC}',
            0x99 => '\u{2122}',
            0x9A => '\u{0161}',
            0x9B => '\u{203A}',
            0x9C => '\u{0153}',
            0x9E => '\u{017E}',
            0x9F => '\u{0178}',
            other => char::from(other),
        };
        out.push(ch);
    }
    out
}

/// Decodes a UTF-16LE byte slice (stopping at first NUL code unit) into a `String`.
pub fn decode_utf16le(bytes: &[u8]) -> String {
    let mut units = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.as_chunks::<2>().0 {
        let u = u16::from_le_bytes(*chunk);
        if u == 0 {
            break;
        }
        units.push(u);
    }
    String::from_utf16_lossy(&units)
}

/// Encodes a `&str` into NUL-terminated UTF-16LE bytes.
pub fn encode_utf16le_nul(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity((s.len() + 1) * 2);
    for u in s.encode_utf16() {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out.extend_from_slice(&[0, 0]);
    out
}
