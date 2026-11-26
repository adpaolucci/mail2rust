use crate::error::{OutlookError, OutlookResult};
use crate::time::{decode_cp1252, decode_utf16le};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const OAB_UNCOMPRESSED_V4_MAGIC: u32 = 0x0000_0020;
pub const OAB_COMPRESSED_V4_MAGIC: u32 = 0x0000_0003; // LZX compressed chunk header
pub const OAB_V2_VERSION: u32 = 0x0000_0002;
pub const OAB_V4_VERSION: u32 = 0x0000_0004;

// Canonical OAB MAPI property tags
const PR_DISPLAY_NAME_W: u32 = 0x3001_001F;
const PR_DISPLAY_NAME_A: u32 = 0x3001_001E;
const PR_EMAIL_ADDRESS_A: u32 = 0x3003_001E;
const PR_EMAIL_ADDRESS_W: u32 = 0x3003_001F;
const PR_SMTP_ADDRESS_A: u32 = 0x39FE_001E;
const PR_SMTP_ADDRESS_W: u32 = 0x39FE_001F;
const PR_COMPANY_NAME_W: u32 = 0x3A16_001F;
const PR_COMPANY_NAME_A: u32 = 0x3A16_001E;
const PR_TITLE_W: u32 = 0x3A17_001F;
const PR_TITLE_A: u32 = 0x3A17_001E;
const PR_DEPARTMENT_NAME_W: u32 = 0x3A18_001F;
const PR_DEPARTMENT_NAME_A: u32 = 0x3A18_001E;
const PR_BUSINESS_TELEPHONE_NUMBER_W: u32 = 0x3A08_001F;
const PR_BUSINESS_TELEPHONE_NUMBER_A: u32 = 0x3A08_001E;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OabEntry {
    pub display_name: String,
    pub email1: String,
    pub email2: String,
    pub company: String,
    pub job_title: String,
    pub department: String,
    pub phone: String,
}

/// Parses an uncompressed MS-OXOAB (`.oab`) v2/v4 Offline Address Book file.
/// If the file is LZX-compressed (`ulVersion == 3` in `OAB_V4_HDR` or LZX block signature),
/// returns `OutlookError::UnsupportedFeature` gracefully without panicking.
pub fn parse_oab_file(path: &Path) -> OutlookResult<Vec<OabEntry>> {
    let bytes = fs::read(path)?;
    parse_oab_bytes(&bytes)
}

pub fn parse_oab_bytes(raw: &[u8]) -> OutlookResult<Vec<OabEntry>> {
    if raw.len() < 16 {
        return Err(OutlookError::InvalidFormat(
            "OAB file shorter than 16-byte header".to_string(),
        ));
    }

    let ul_version = u32::from_le_bytes(raw[0..4].try_into().unwrap());
    if ul_version == OAB_COMPRESSED_V4_MAGIC {
        return Err(OutlookError::UnsupportedFeature(
            "LZX-compressed OAB v4 file is not supported; export contacts from PST, NK2, or vCard".to_string(),
        ));
    }

    if ul_version != OAB_UNCOMPRESSED_V4_MAGIC && ul_version != OAB_V4_VERSION && ul_version != OAB_V2_VERSION {
        return Err(OutlookError::InvalidFormat(format!(
            "Unrecognized OAB version/header magic: 0x{ul_version:08X}"
        )));
    }

    // Header:
    // [0..4]   ulVersion (0x20 / 4 / 2)
    // [4..8]   ulSerial
    // [8..12]  ulTotRecs
    let total_recs = u32::from_le_bytes(raw[8..12].try_into().unwrap()) as usize;
    if total_recs > 2_000_000 {
        return Err(OutlookError::CorruptStructure(format!(
            "OAB record count {total_recs} exceeds safety bound"
        )));
    }

    // Read header metadata record (MS-OXOAB OAB_META_DATA) at offset 12:
    // [12..16] cbSize (size of metadata or property descriptor table)
    if raw.len() < 20 {
        return Err(OutlookError::CorruptStructure("Truncated OAB metadata".to_string()));
    }
    let mut cur = 12usize;
    let hdr_props_count = u32::from_le_bytes(raw[cur..cur + 4].try_into().unwrap()) as usize;
    cur += 4;
    if hdr_props_count > 1024 || cur + hdr_props_count * 8 > raw.len() {
        return Err(OutlookError::CorruptStructure(
            "Invalid OAB OAB_PROP_TABLE property count".to_string(),
        ));
    }

    let mut oab_prop_tags = Vec::with_capacity(hdr_props_count);
    for _ in 0..hdr_props_count {
        let tag = u32::from_le_bytes(raw[cur..cur + 4].try_into().unwrap());
        let _flags = u32::from_le_bytes(raw[cur + 4..cur + 8].try_into().unwrap());
        cur += 8;
        oab_prop_tags.push(tag);
    }

    let bitmask_bytes = oab_prop_tags.len().div_ceil(8);
    let mut entries = Vec::with_capacity(total_recs.min(4096));

    for rec_idx in 0..total_recs {
        if cur + 4 > raw.len() {
            break;
        }
        let rec_size = u32::from_le_bytes(raw[cur..cur + 4].try_into().unwrap()) as usize;
        cur += 4;
        if rec_size < bitmask_bytes || cur + rec_size > raw.len() {
            return Err(OutlookError::CorruptStructure(format!(
                "Truncated OAB record #{rec_idx}"
            )));
        }
        let rec_slice = &raw[cur..cur + rec_size];
        cur += rec_size;

        let presence = &rec_slice[..bitmask_bytes];
        let mut rcur = bitmask_bytes;
        let mut entry = OabEntry::default();

        for (p_idx, &tag) in oab_prop_tags.iter().enumerate() {
            let is_present = (presence[p_idx / 8] & (1u8 << (7 - (p_idx % 8)))) != 0;
            if !is_present {
                continue;
            }
            let ptype = (tag & 0xFFFF) as u16;
            let str_val = match ptype {
                0x001E => {
                    let s = read_cstr_cp1252(rec_slice, &mut rcur);
                    Some(s)
                }
                0x001F => {
                    let s = read_cstr_utf16_or_utf8(rec_slice, &mut rcur);
                    Some(s)
                }
                0x0003 | 0x000B => {
                    let _ = read_varint(rec_slice, &mut rcur);
                    None
                }
                0x0102 => {
                    let len = read_varint(rec_slice, &mut rcur) as usize;
                    rcur = (rcur + len).min(rec_slice.len());
                    None
                }
                _ => None,
            };

            if let Some(val) = str_val {
                match tag {
                    PR_DISPLAY_NAME_W | PR_DISPLAY_NAME_A => entry.display_name = val,
                    PR_SMTP_ADDRESS_W | PR_SMTP_ADDRESS_A => {
                        if entry.email1.is_empty() {
                            entry.email1 = val;
                        } else if entry.email2.is_empty() && entry.email1 != val {
                            entry.email2 = val;
                        }
                    }
                    PR_EMAIL_ADDRESS_W | PR_EMAIL_ADDRESS_A => {
                        if entry.email1.is_empty() {
                            entry.email1 = val;
                        } else if entry.email2.is_empty() && entry.email1 != val {
                            entry.email2 = val;
                        }
                    }
                    PR_COMPANY_NAME_W | PR_COMPANY_NAME_A => entry.company = val,
                    PR_TITLE_W | PR_TITLE_A => entry.job_title = val,
                    PR_DEPARTMENT_NAME_W | PR_DEPARTMENT_NAME_A => entry.department = val,
                    PR_BUSINESS_TELEPHONE_NUMBER_W | PR_BUSINESS_TELEPHONE_NUMBER_A => entry.phone = val,
                    _ => {}
                }
            }
        }

        if !entry.display_name.is_empty() || !entry.email1.is_empty() {
            entries.push(entry);
        }
    }

    Ok(entries)
}

/// Serializes an uncompressed OAB v4 test container (`OAB_UNCOMPRESSED_V4_MAGIC = 0x20`).
pub fn build_uncompressed_oab_v4(entries: &[OabEntry]) -> Vec<u8> {
    let tags = [
        PR_DISPLAY_NAME_W,
        PR_SMTP_ADDRESS_W,
        PR_COMPANY_NAME_W,
        PR_TITLE_W,
        PR_DEPARTMENT_NAME_W,
        PR_BUSINESS_TELEPHONE_NUMBER_W,
    ];
    let mut out = Vec::new();
    out.extend_from_slice(&OAB_UNCOMPRESSED_V4_MAGIC.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes()); // ulSerial
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes()); // ulTotRecs
    out.extend_from_slice(&(tags.len() as u32).to_le_bytes());
    for tag in tags {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
    }

    for entry in entries {
        let vals = [
            &entry.display_name,
            &entry.email1,
            &entry.company,
            &entry.job_title,
            &entry.department,
            &entry.phone,
        ];
        let mut mask = 0u8;
        let mut body = Vec::new();
        for (i, v) in vals.iter().enumerate() {
            if !v.is_empty() {
                mask |= 1u8 << (7 - i);
                for u in v.encode_utf16() {
                    body.extend_from_slice(&u.to_le_bytes());
                }
                body.extend_from_slice(&[0, 0]);
            }
        }
        let rec_len = 1 + body.len();
        out.extend_from_slice(&(rec_len as u32).to_le_bytes());
        out.push(mask);
        out.extend_from_slice(&body);
    }
    out
}

fn read_cstr_cp1252(buf: &[u8], cur: &mut usize) -> String {
    let start = *cur;
    while *cur < buf.len() && buf[*cur] != 0 {
        *cur += 1;
    }
    let s = decode_cp1252(&buf[start..*cur]);
    if *cur < buf.len() {
        *cur += 1;
    }
    s
}

fn read_cstr_utf16_or_utf8(buf: &[u8], cur: &mut usize) -> String {
    // Uncompressed OAB v4 stores UTF-8 NUL-terminated strings in browse files, or UTF-16LE in detail/synthetic
    let remaining = &buf[*cur..];
    if remaining.len() >= 2 && remaining[1] == 0 && remaining[0] != 0 {
        let mut end = *cur;
        while end + 1 < buf.len() && (buf[end] != 0 || buf[end + 1] != 0) {
            end += 2;
        }
        let s = decode_utf16le(&buf[*cur..end]);
        *cur = (end + 2).min(buf.len());
        s
    } else {
        read_cstr_cp1252(buf, cur)
    }
}

fn read_varint(buf: &[u8], cur: &mut usize) -> u32 {
    if *cur >= buf.len() {
        return 0;
    }
    let first = buf[*cur];
    *cur += 1;
    if (first & 0x80) == 0 {
        return u32::from(first);
    }
    let count = usize::from(first & 0x7F).min(4);
    let mut val = 0u32;
    for i in 0..count {
        if *cur >= buf.len() {
            break;
        }
        val |= u32::from(buf[*cur]) << (i * 8);
        *cur += 1;
    }
    val
}
