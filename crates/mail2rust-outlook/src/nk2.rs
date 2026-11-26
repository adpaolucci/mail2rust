use crate::error::{OutlookError, OutlookResult};
use crate::time::{datetime_to_filetime, decode_cp1252, decode_utf16le, encode_utf16le_nul, filetime_to_datetime};
use chrono::{DateTime, Utc};
use guap_mcp::{ActionCommand, ActionRiskLevel, McpToolError};
use guap_table::{
    CellValue, ColumnDef, ColumnType, Row, TableData, TableError, TableProvider, TableQuery, TableSchema, apply_query,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const NK2_MAGIC: u32 = 0x0D0A_04BA;
const MAX_NK2_ROWS: usize = 100_000;
const MAX_PROPS_PER_ROW: usize = 4_096;
const MAX_PROP_BYTES: usize = 16 * 1024 * 1024;

// Standard MAPI property tags in NK2 / Stream_Autocomplete_*.dat
pub const PR_ENTRYID: u32 = 0x0FFF_0102;
pub const PR_DISPLAY_NAME_W: u32 = 0x3001_001F;
pub const PR_ADDRTYPE_W: u32 = 0x3002_001F;
pub const PR_EMAIL_ADDRESS_W: u32 = 0x3003_001F;
pub const PR_LAST_MODIFICATION_TIME: u32 = 0x3008_0040;
pub const PR_SEARCH_KEY: u32 = 0x300B_0102;
pub const PR_SMTP_ADDRESS_W: u32 = 0x39FE_001F;
pub const PR_NICK_NAME_W: u32 = 0x6001_001F;
pub const PR_NK2_WEIGHT: u32 = 0x6002_0003;
pub const PR_NK2_SEND_RICH_INFO: u32 = 0x6003_000B;

// Provider GUIDs inside EntryID
const ONEOFF_PROVIDER_GUID: [u8; 16] = [
    0x81, 0x2B, 0x1F, 0xA4, 0xBE, 0xE3, 0x10, 0x19, 0x95, 0x3B, 0x00, 0xAA, 0x00, 0x4B, 0xB8, 0x51,
];
const EXCHANGE_AB_PROVIDER_GUID: [u8; 16] = [
    0xDC, 0xA7, 0x40, 0xC8, 0xC0, 0x42, 0x10, 0x1A, 0xB4, 0xB9, 0x08, 0x00, 0x2B, 0x2F, 0xE1, 0x82,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryIdKind {
    OneOff {
        display_name: String,
        address_type: String,
        email: String,
    },
    ExchangeAb {
        distinguished_name: String,
    },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Nk2PropValue {
    Inline(u64),
    String8(Vec<u8>),
    Unicode(Vec<u8>),
    Binary(Vec<u8>),
    MultiString8(Vec<Vec<u8>>),
    MultiUnicode(Vec<Vec<u8>>),
    MultiBinary(Vec<Vec<u8>>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Nk2RawProperty {
    pub prop_type: u16,
    pub prop_id: u16,
    pub reserved: u32,
    pub value: Nk2PropValue,
}

impl Nk2RawProperty {
    pub fn tag(&self) -> u32 {
        (u32::from(self.prop_id) << 16) | u32::from(self.prop_type)
    }

    pub fn as_string(&self) -> Option<String> {
        match &self.value {
            Nk2PropValue::Unicode(b) => Some(decode_utf16le(b)),
            Nk2PropValue::String8(b) => Some(decode_cp1252(b)),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match &self.value {
            Nk2PropValue::Inline(v) => Some(*v as i32),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match &self.value {
            Nk2PropValue::Inline(v) => Some((*v & 0xFFFF) != 0),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match &self.value {
            Nk2PropValue::Inline(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match &self.value {
            Nk2PropValue::Binary(b) => Some(b),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nk2Entry {
    pub index: usize,
    pub display_name: String,
    pub email_address: String,
    pub smtp_address: String,
    pub address_type: String,
    pub dropdown_name: String,
    pub weight: i32,
    pub send_rich_info: Option<bool>,
    pub modified_time: Option<DateTime<Utc>>,
    pub entry_id: Vec<u8>,
    pub entry_id_kind: EntryIdKind,
    pub search_key: Vec<u8>,
    /// All raw properties in exact original slot order so unknown properties round-trip verbatim.
    pub raw_props: Vec<Nk2RawProperty>,
}

impl Nk2Entry {
    pub fn domain(&self) -> String {
        let candidate = if !self.smtp_address.is_empty() {
            &self.smtp_address
        } else {
            &self.email_address
        };
        candidate
            .rsplit_once('@')
            .map(|(_, d)| d.trim().to_ascii_lowercase())
            .unwrap_or_default()
    }

    pub fn new_smtp(display_name: impl Into<String>, email: impl Into<String>, weight: i32) -> Self {
        let display_name = display_name.into();
        let email = email.into();
        let entry_id = encode_oneoff_entry_id(&display_name, "SMTP", &email);
        let search_key = format!("SMTP:{}\0", email.to_ascii_uppercase()).into_bytes();
        let now = Utc::now();

        let mut entry = Self {
            index: 0,
            display_name: display_name.clone(),
            email_address: email.clone(),
            smtp_address: email.clone(),
            address_type: "SMTP".to_string(),
            dropdown_name: display_name,
            weight,
            send_rich_info: Some(false),
            modified_time: Some(now),
            entry_id: entry_id.clone(),
            entry_id_kind: decode_entry_id(&entry_id),
            search_key,
            raw_props: Vec::new(),
        };
        entry.sync_raw_props();
        entry
    }

    /// Synchronizes canonical fields back into `raw_props` while preserving unknown tags verbatim.
    pub fn sync_raw_props(&mut self) {
        self.set_or_insert_unicode(0x3001, &self.display_name.clone());
        self.set_or_insert_unicode(0x3002, &self.address_type.clone());
        self.set_or_insert_unicode(0x3003, &self.email_address.clone());
        if !self.smtp_address.is_empty() {
            self.set_or_insert_unicode(0x39FE, &self.smtp_address.clone());
        }
        if !self.dropdown_name.is_empty() {
            self.set_or_insert_unicode(0x6001, &self.dropdown_name.clone());
        }
        self.set_or_insert_inline(0x6002, 0x0003, (self.weight as u32) as u64);
        if let Some(rich) = self.send_rich_info {
            self.set_or_insert_inline(0x6003, 0x000B, u64::from(rich));
        }
        if let Some(dt) = self.modified_time {
            self.set_or_insert_inline(0x3008, 0x0040, datetime_to_filetime(dt));
        }
        if !self.entry_id.is_empty() {
            self.set_or_insert_binary(0x0FFF, &self.entry_id.clone());
        }
        if !self.search_key.is_empty() {
            self.set_or_insert_binary(0x300B, &self.search_key.clone());
        }
    }

    fn set_or_insert_unicode(&mut self, prop_id: u16, val: &str) {
        let bytes = encode_utf16le_nul(val);
        if let Some(p) = self
            .raw_props
            .iter_mut()
            .find(|p| p.prop_id == prop_id && (p.prop_type == 0x001F || p.prop_type == 0x001E))
        {
            p.prop_type = 0x001F;
            p.value = Nk2PropValue::Unicode(bytes);
        } else {
            self.raw_props.push(Nk2RawProperty {
                prop_type: 0x001F,
                prop_id,
                reserved: 0,
                value: Nk2PropValue::Unicode(bytes),
            });
        }
    }

    fn set_or_insert_inline(&mut self, prop_id: u16, prop_type: u16, val: u64) {
        if let Some(p) = self
            .raw_props
            .iter_mut()
            .find(|p| p.prop_id == prop_id && p.prop_type == prop_type)
        {
            p.value = Nk2PropValue::Inline(val);
        } else {
            self.raw_props.push(Nk2RawProperty {
                prop_type,
                prop_id,
                reserved: 0,
                value: Nk2PropValue::Inline(val),
            });
        }
    }

    fn set_or_insert_binary(&mut self, prop_id: u16, bytes: &[u8]) {
        if let Some(p) = self
            .raw_props
            .iter_mut()
            .find(|p| p.prop_id == prop_id && p.prop_type == 0x0102)
        {
            p.value = Nk2PropValue::Binary(bytes.to_vec());
        } else {
            self.raw_props.push(Nk2RawProperty {
                prop_type: 0x0102,
                prop_id,
                reserved: 0,
                value: Nk2PropValue::Binary(bytes.to_vec()),
            });
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nk2Archive {
    pub meta_major: u32,
    pub meta_minor: u32,
    pub entries: Vec<Nk2Entry>,
    pub footer: Vec<u8>,
}

impl Nk2Archive {
    pub fn is_nk2_bytes(raw: &[u8]) -> bool {
        raw.len() >= 16 && u32::from_le_bytes(raw[0..4].try_into().unwrap()) == NK2_MAGIC
    }

    pub fn from_file(path: &Path) -> OutlookResult<Self> {
        let bytes = fs::read(path)?;
        Self::parse(&bytes)
    }

    pub fn parse(raw: &[u8]) -> OutlookResult<Self> {
        if raw.len() < 16 {
            return Err(OutlookError::InvalidFormat(
                "NK2/Autocomplete file shorter than 16-byte header".to_string(),
            ));
        }
        let magic = u32::from_le_bytes(raw[0..4].try_into().unwrap());
        if magic != NK2_MAGIC {
            return Err(OutlookError::InvalidFormat(format!(
                "Invalid NK2 magic: 0x{magic:08X} (expected 0x{NK2_MAGIC:08X})"
            )));
        }
        let meta_major = u32::from_le_bytes(raw[4..8].try_into().unwrap());
        let meta_minor = u32::from_le_bytes(raw[8..12].try_into().unwrap());
        let row_count = u32::from_le_bytes(raw[12..16].try_into().unwrap()) as usize;
        if row_count > MAX_NK2_ROWS {
            return Err(OutlookError::CorruptStructure(format!(
                "NK2 row count {row_count} exceeds safety cap {MAX_NK2_ROWS}"
            )));
        }

        let mut cur = 16usize;
        let mut entries = Vec::with_capacity(row_count.min(4096));

        for row_idx in 0..row_count {
            if cur + 4 > raw.len() {
                return Err(OutlookError::CorruptStructure(format!(
                    "Unexpected EOF reading prop_count for NK2 row {row_idx}"
                )));
            }
            let prop_count = u32::from_le_bytes(raw[cur..cur + 4].try_into().unwrap()) as usize;
            cur += 4;
            if prop_count > MAX_PROPS_PER_ROW {
                return Err(OutlookError::CorruptStructure(format!(
                    "NK2 row {row_idx} property count {prop_count} exceeds limit"
                )));
            }
            let slots_bytes = prop_count
                .checked_mul(16)
                .ok_or_else(|| OutlookError::CorruptStructure("NK2 slot table overflow".to_string()))?;
            if cur + slots_bytes > raw.len() {
                return Err(OutlookError::CorruptStructure(format!(
                    "Unexpected EOF reading property slots for NK2 row {row_idx}"
                )));
            }

            let mut slots = Vec::with_capacity(prop_count);
            for s in 0..prop_count {
                let off = cur + s * 16;
                let prop_type = u16::from_le_bytes(raw[off..off + 2].try_into().unwrap());
                let prop_id = u16::from_le_bytes(raw[off + 2..off + 4].try_into().unwrap());
                let reserved = u32::from_le_bytes(raw[off + 4..off + 8].try_into().unwrap());
                let inline_val = u64::from_le_bytes(raw[off + 8..off + 16].try_into().unwrap());
                slots.push((prop_type, prop_id, reserved, inline_val));
            }
            cur += slots_bytes;

            let mut raw_props = Vec::with_capacity(prop_count);
            for (prop_type, prop_id, reserved, inline_val) in slots {
                let value = match prop_type {
                    0x0002 | 0x0003 | 0x0004 | 0x0005 | 0x000B | 0x0014 | 0x0040 => Nk2PropValue::Inline(inline_val),
                    0x001E => Nk2PropValue::String8(read_len_blob(raw, &mut cur, row_idx)?),
                    0x001F => Nk2PropValue::Unicode(read_len_blob(raw, &mut cur, row_idx)?),
                    0x0102 => Nk2PropValue::Binary(read_len_blob(raw, &mut cur, row_idx)?),
                    0x101E => Nk2PropValue::MultiString8(read_mv_blobs(raw, &mut cur, row_idx)?),
                    0x101F => Nk2PropValue::MultiUnicode(read_mv_blobs(raw, &mut cur, row_idx)?),
                    0x1102 => Nk2PropValue::MultiBinary(read_mv_blobs(raw, &mut cur, row_idx)?),
                    other if (other & 0x1000) != 0 => Nk2PropValue::MultiBinary(read_mv_blobs(raw, &mut cur, row_idx)?),
                    _ => Nk2PropValue::Inline(inline_val),
                };
                raw_props.push(Nk2RawProperty {
                    prop_type,
                    prop_id,
                    reserved,
                    value,
                });
            }

            entries.push(build_nk2_entry(row_idx, raw_props));
        }

        let footer = raw[cur..].to_vec();
        Ok(Self {
            meta_major,
            meta_minor,
            entries,
            footer,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(64 + self.entries.len() * 256);
        out.extend_from_slice(&NK2_MAGIC.to_le_bytes());
        out.extend_from_slice(&self.meta_major.to_le_bytes());
        out.extend_from_slice(&self.meta_minor.to_le_bytes());
        out.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());

        for entry in &self.entries {
            out.extend_from_slice(&(entry.raw_props.len() as u32).to_le_bytes());
            for p in &entry.raw_props {
                out.extend_from_slice(&p.prop_type.to_le_bytes());
                out.extend_from_slice(&p.prop_id.to_le_bytes());
                out.extend_from_slice(&p.reserved.to_le_bytes());
                let inline = match &p.value {
                    Nk2PropValue::Inline(v) => *v,
                    _ => 0,
                };
                out.extend_from_slice(&inline.to_le_bytes());
            }
            for p in &entry.raw_props {
                match &p.value {
                    Nk2PropValue::Inline(_) => {}
                    Nk2PropValue::String8(b) | Nk2PropValue::Unicode(b) | Nk2PropValue::Binary(b) => {
                        out.extend_from_slice(&(b.len() as u32).to_le_bytes());
                        out.extend_from_slice(b);
                    }
                    Nk2PropValue::MultiString8(items)
                    | Nk2PropValue::MultiUnicode(items)
                    | Nk2PropValue::MultiBinary(items) => {
                        out.extend_from_slice(&(items.len() as u32).to_le_bytes());
                        for item in items {
                            out.extend_from_slice(&(item.len() as u32).to_le_bytes());
                            out.extend_from_slice(item);
                        }
                    }
                }
            }
        }

        out.extend_from_slice(&self.footer);
        out
    }

    pub fn reindex(&mut self) {
        for (i, e) in self.entries.iter_mut().enumerate() {
            e.index = i;
        }
    }

    pub fn add_entry(&mut self, mut entry: Nk2Entry) {
        entry.sync_raw_props();
        entry.index = self.entries.len();
        self.entries.push(entry);
    }

    pub fn remove_by_email(&mut self, email: &str) -> usize {
        let needle = email.trim().to_ascii_lowercase();
        let before = self.entries.len();
        self.entries.retain(|e| {
            e.email_address.to_ascii_lowercase() != needle && e.smtp_address.to_ascii_lowercase() != needle
        });
        let removed = before - self.entries.len();
        self.reindex();
        removed
    }

    pub fn remove_by_index(&mut self, index: usize) -> bool {
        if index < self.entries.len() {
            self.entries.remove(index);
            self.reindex();
            true
        } else {
            false
        }
    }

    pub fn update_weight(&mut self, email: &str, weight: i32) -> usize {
        let needle = email.trim().to_ascii_lowercase();
        let mut updated = 0usize;
        for e in &mut self.entries {
            if e.email_address.to_ascii_lowercase() == needle || e.smtp_address.to_ascii_lowercase() == needle {
                e.weight = weight;
                e.sync_raw_props();
                updated += 1;
            }
        }
        updated
    }

    pub fn dedup(&mut self) -> usize {
        let mut seen = HashSet::new();
        let before = self.entries.len();
        self.entries.retain(|e| seen.insert(entry_dedup_key(e)));
        let removed = before - self.entries.len();
        self.reindex();
        removed
    }

    pub fn merge(&mut self, other: &Nk2Archive) -> usize {
        let mut existing: HashSet<String> = self.entries.iter().map(entry_dedup_key).collect();
        let mut added = 0usize;
        for entry in &other.entries {
            let key = entry_dedup_key(entry);
            if existing.insert(key) {
                let mut cloned = entry.clone();
                cloned.index = self.entries.len();
                self.entries.push(cloned);
                added += 1;
            }
        }
        added
    }

    /// Saves atomically: writes `.tmp`, re-parses to verify integrity, optionally copies `.bak`, and renames.
    pub fn save_atomic(&self, dest: &Path, create_backup: bool) -> OutlookResult<()> {
        let bytes = self.serialize();
        // Verify round-trip parse before touching destination
        let verified = Self::parse(&bytes)?;
        if verified.entries.len() != self.entries.len() {
            return Err(OutlookError::CorruptStructure(
                "NK2 serialization verification failed row count check".to_string(),
            ));
        }

        if let Some(parent) = dest.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }

        let mut tmp_os = dest.as_os_str().to_os_string();
        tmp_os.push(".tmp");
        let tmp_path = PathBuf::from(tmp_os);
        fs::write(&tmp_path, &bytes)?;

        if create_backup && dest.exists() {
            let mut bak_os = dest.as_os_str().to_os_string();
            bak_os.push(".bak");
            let bak_path = PathBuf::from(bak_os);
            fs::copy(dest, bak_path)?;
        }

        if dest.exists() {
            fs::remove_file(dest)?;
        }
        fs::rename(&tmp_path, dest)?;
        Ok(())
    }
}

fn entry_dedup_key(e: &Nk2Entry) -> String {
    if !e.smtp_address.is_empty() {
        e.smtp_address.to_ascii_lowercase()
    } else {
        e.email_address.to_ascii_lowercase()
    }
}

fn read_len_blob(raw: &[u8], cur: &mut usize, row_idx: usize) -> OutlookResult<Vec<u8>> {
    if *cur + 4 > raw.len() {
        return Err(OutlookError::CorruptStructure(format!(
            "Unexpected EOF reading variable length in NK2 row {row_idx}"
        )));
    }
    let len = u32::from_le_bytes(raw[*cur..*cur + 4].try_into().unwrap()) as usize;
    *cur += 4;
    if len > MAX_PROP_BYTES || *cur + len > raw.len() {
        return Err(OutlookError::CorruptStructure(format!(
            "Invalid NK2 property blob length {len} in row {row_idx}"
        )));
    }
    let slice = raw[*cur..*cur + len].to_vec();
    *cur += len;
    Ok(slice)
}

fn read_mv_blobs(raw: &[u8], cur: &mut usize, row_idx: usize) -> OutlookResult<Vec<Vec<u8>>> {
    if *cur + 4 > raw.len() {
        return Err(OutlookError::CorruptStructure(format!(
            "Unexpected EOF reading multi-value count in NK2 row {row_idx}"
        )));
    }
    let count = u32::from_le_bytes(raw[*cur..*cur + 4].try_into().unwrap()) as usize;
    *cur += 4;
    if count > MAX_PROPS_PER_ROW {
        return Err(OutlookError::CorruptStructure(format!(
            "NK2 multi-value count {count} exceeds limit in row {row_idx}"
        )));
    }
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        out.push(read_len_blob(raw, cur, row_idx)?);
    }
    Ok(out)
}

fn build_nk2_entry(index: usize, raw_props: Vec<Nk2RawProperty>) -> Nk2Entry {
    let mut by_id: HashMap<u16, &Nk2RawProperty> = HashMap::new();
    for p in &raw_props {
        by_id.entry(p.prop_id).or_insert(p);
    }

    let entry_id = by_id
        .get(&0x0FFF)
        .and_then(|p| p.as_bytes())
        .map(|b| b.to_vec())
        .unwrap_or_default();
    let entry_id_kind = decode_entry_id(&entry_id);

    let mut display_name = by_id.get(&0x3001).and_then(|p| p.as_string()).unwrap_or_default();
    let mut address_type = by_id.get(&0x3002).and_then(|p| p.as_string()).unwrap_or_default();
    let mut email_address = by_id.get(&0x3003).and_then(|p| p.as_string()).unwrap_or_default();
    let mut smtp_address = by_id.get(&0x39FE).and_then(|p| p.as_string()).unwrap_or_default();
    let dropdown_name = by_id.get(&0x6001).and_then(|p| p.as_string()).unwrap_or_default();
    let weight = by_id.get(&0x6002).and_then(|p| p.as_i32()).unwrap_or(0);
    let send_rich_info = by_id.get(&0x6003).and_then(|p| p.as_bool());
    let modified_time = by_id
        .get(&0x3008)
        .and_then(|p| p.as_u64())
        .and_then(filetime_to_datetime);
    let search_key = by_id
        .get(&0x300B)
        .and_then(|p| p.as_bytes())
        .map(|b| b.to_vec())
        .unwrap_or_default();

    if let EntryIdKind::OneOff {
        display_name: ref d,
        address_type: ref a,
        email: ref e,
    } = entry_id_kind
    {
        if display_name.is_empty() {
            display_name = d.clone();
        }
        if address_type.is_empty() {
            address_type = a.clone();
        }
        if email_address.is_empty() {
            email_address = e.clone();
        }
    }
    if smtp_address.is_empty() && address_type.eq_ignore_ascii_case("SMTP") {
        smtp_address = email_address.clone();
    }
    if address_type.is_empty() && !email_address.is_empty() {
        address_type = if email_address.contains('@') {
            "SMTP".to_string()
        } else {
            "EX".to_string()
        };
    }

    Nk2Entry {
        index,
        display_name,
        email_address,
        smtp_address,
        address_type,
        dropdown_name,
        weight,
        send_rich_info,
        modified_time,
        entry_id,
        entry_id_kind,
        search_key,
        raw_props,
    }
}

/// Decodes a MAPI One-Off or Exchange AB EntryID blob.
pub fn decode_entry_id(bytes: &[u8]) -> EntryIdKind {
    if bytes.len() < 24 {
        return EntryIdKind::Unknown;
    }
    let guid = &bytes[4..20];
    if guid == ONEOFF_PROVIDER_GUID {
        let mut pos = 24usize;
        let is_unicode = (u16::from_le_bytes([bytes[22], bytes[23]]) & 0x8000) != 0 || is_likely_utf16(&bytes[pos..]);
        if is_unicode {
            let d = read_utf16_z(bytes, &mut pos);
            let a = read_utf16_z(bytes, &mut pos);
            let e = read_utf16_z(bytes, &mut pos);
            return EntryIdKind::OneOff {
                display_name: d,
                address_type: a,
                email: e,
            };
        } else {
            let d = read_ascii_z(bytes, &mut pos);
            let a = read_ascii_z(bytes, &mut pos);
            let e = read_ascii_z(bytes, &mut pos);
            return EntryIdKind::OneOff {
                display_name: d,
                address_type: a,
                email: e,
            };
        }
    } else if guid == EXCHANGE_AB_PROVIDER_GUID && bytes.len() > 28 {
        let mut pos = 28usize;
        let dn = read_ascii_z(bytes, &mut pos);
        return EntryIdKind::ExchangeAb { distinguished_name: dn };
    }
    EntryIdKind::Unknown
}

pub fn encode_oneoff_entry_id(display_name: &str, addr_type: &str, email: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(&0u32.to_le_bytes()); // abFlags
    out.extend_from_slice(&ONEOFF_PROVIDER_GUID);
    out.extend_from_slice(&0u16.to_le_bytes()); // Version = 0
    out.extend_from_slice(&0x8000u16.to_le_bytes()); // Flags: U=1 (Unicode), M=0
    out.extend_from_slice(&encode_utf16le_nul(display_name));
    out.extend_from_slice(&encode_utf16le_nul(addr_type));
    out.extend_from_slice(&encode_utf16le_nul(email));
    out
}

pub fn encode_exchange_ab_entry_id(distinguished_name: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + distinguished_name.len());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&EXCHANGE_AB_PROVIDER_GUID);
    out.extend_from_slice(&1u32.to_le_bytes()); // Version
    out.extend_from_slice(&0u32.to_le_bytes()); // Type = DT_MAILUSER
    out.extend_from_slice(distinguished_name.as_bytes());
    out.push(0);
    out
}

fn is_likely_utf16(slice: &[u8]) -> bool {
    slice.len() >= 4 && slice[1] == 0
}

fn read_utf16_z(bytes: &[u8], pos: &mut usize) -> String {
    let mut units = Vec::new();
    while *pos + 1 < bytes.len() {
        let u = u16::from_le_bytes([bytes[*pos], bytes[*pos + 1]]);
        *pos += 2;
        if u == 0 {
            break;
        }
        units.push(u);
    }
    String::from_utf16_lossy(&units)
}

fn read_ascii_z(bytes: &[u8], pos: &mut usize) -> String {
    let start = *pos;
    while *pos < bytes.len() && bytes[*pos] != 0 {
        *pos += 1;
    }
    let s = decode_cp1252(&bytes[start..*pos]);
    if *pos < bytes.len() {
        *pos += 1;
    }
    s
}

/// `guap-table::TableProvider` for Outlook AutoComplete (.NK2) (`mail2rust_nk2_inspect`).
pub struct Nk2TableProvider {
    schema: TableSchema,
    rows: Vec<Row>,
}

impl Nk2TableProvider {
    pub fn from_archive(archive: &Nk2Archive) -> Self {
        let schema = TableSchema::new(
            "mail2rust_nk2_inspect",
            "Outlook AutoComplete (.NK2 / Stream_Autocomplete)",
            vec![
                ColumnDef::new("index", "Index", ColumnType::Uint).with_width_hint(65),
                ColumnDef::new("display_name", "Display Name", ColumnType::Text).with_width_hint(180),
                ColumnDef::new("email_address", "Email Address", ColumnType::Text).with_width_hint(220),
                ColumnDef::new("address_type", "Type", ColumnType::Text).with_width_hint(70),
                ColumnDef::new("weight", "Weight", ColumnType::Int).with_width_hint(85),
                ColumnDef::new("domain", "Domain", ColumnType::Text).with_width_hint(150),
                ColumnDef::new("dropdown_name", "Drop-Down Name", ColumnType::Text).with_width_hint(180),
                ColumnDef::new("send_format", "Send Format", ColumnType::Text).with_width_hint(95),
            ],
        )
        .with_default_sort(4, guap_table::SortDirection::Desc);

        let rows = archive
            .entries
            .iter()
            .map(|e| {
                let send_fmt = match e.send_rich_info {
                    Some(true) => "Rich/RTF".to_string(),
                    Some(false) => "Plain/Auto".to_string(),
                    None => "Default".to_string(),
                };
                let detail = json!({
                    "smtp_address": e.smtp_address,
                    "entry_id_kind": e.entry_id_kind,
                    "search_key_hex": hex_lower(&e.search_key),
                    "modified_time": e.modified_time.map(|d| d.to_rfc3339()),
                    "property_count": e.raw_props.len(),
                });
                let raw_bytes = if !e.entry_id.is_empty() {
                    e.entry_id.clone()
                } else {
                    e.search_key.clone()
                };
                Row::new(
                    e.index as u64,
                    vec![
                        CellValue::Uint(e.index as u64),
                        CellValue::Text(e.display_name.clone()),
                        CellValue::Text(e.email_address.clone()),
                        CellValue::Text(e.address_type.clone()),
                        CellValue::Int(i64::from(e.weight)),
                        CellValue::Text(e.domain()),
                        CellValue::Text(e.dropdown_name.clone()),
                        CellValue::Text(send_fmt),
                    ],
                )
                .with_detail_json(detail)
                .with_raw_bytes(raw_bytes)
            })
            .collect();

        Self { schema, rows }
    }

    pub fn from_path(path: &Path) -> OutlookResult<Self> {
        let archive = Nk2Archive::from_file(path)?;
        Ok(Self::from_archive(&archive))
    }
}

impl TableProvider for Nk2TableProvider {
    fn tool_id(&self) -> &'static str {
        "mail2rust_nk2_inspect"
    }

    fn description(&self) -> &'static str {
        "Inspect Outlook AutoComplete (.NK2 and Stream_Autocomplete_*.dat) entries without modifying files"
    }

    fn schema(&self) -> TableSchema {
        self.schema.clone()
    }

    fn query(&self, req: &TableQuery) -> Result<TableData, TableError> {
        apply_query(&self.schema, &self.rows, req)
    }
}

/// Read-only MCP `ActionCommand` for `mail2rust_nk2_inspect`.
pub struct Nk2InspectCommand;

impl ActionCommand for Nk2InspectCommand {
    fn command_id(&self) -> &'static str {
        "mail2rust_nk2_inspect"
    }

    fn description(&self) -> &'static str {
        "Parse and inspect an Outlook .NK2 or Stream_Autocomplete_*.dat file (read-only)."
    }

    fn risk_level(&self) -> ActionRiskLevel {
        ActionRiskLevel::ReadOnly
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to .nk2 or Stream_Autocomplete_*.dat file" },
                "filter": { "type": "string", "description": "Optional Tabular filter expression" },
                "limit": { "type": "integer", "minimum": 1 },
                "offset": { "type": "integer", "minimum": 0 }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, args: &Value, _dry_run: bool) -> Result<Value, McpToolError> {
        let path_str = args
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| McpToolError::InvalidParams("Missing required 'path' argument".to_string()))?;
        let provider = Nk2TableProvider::from_path(Path::new(path_str))
            .map_err(|e| McpToolError::ExecutionFailed(e.to_string()))?;
        query_provider_to_json(&provider, args)
    }
}

/// Mutating MCP `ActionCommand` for `mail2rust_nk2_edit`.
pub struct Nk2EditCommand;

impl ActionCommand for Nk2EditCommand {
    fn command_id(&self) -> &'static str {
        "mail2rust_nk2_edit"
    }

    fn description(&self) -> &'static str {
        "Edit an Outlook .NK2 / Stream_Autocomplete_*.dat file (add/remove/update weight/dedup/merge) with atomic save."
    }

    fn risk_level(&self) -> ActionRiskLevel {
        ActionRiskLevel::Mutating
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Source .nk2 / Stream_Autocomplete_*.dat path" },
                "output": { "type": "string", "description": "Optional destination path (defaults to in-place atomic save)" },
                "add_name": { "type": "string" },
                "add_email": { "type": "string" },
                "remove_email": { "type": "array", "items": { "type": "string" } },
                "set_weight_email": { "type": "string" },
                "set_weight": { "type": "integer" },
                "dedup": { "type": "boolean", "default": false },
                "merge": { "type": "string", "description": "Optional second .nk2 file to merge" }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, args: &Value, dry_run: bool) -> Result<Value, McpToolError> {
        let path_str = args
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| McpToolError::InvalidParams("Missing required 'path' argument".to_string()))?;
        let src_path = PathBuf::from(path_str);
        let dest_path = args
            .get("output")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(|| src_path.clone());

        let mut archive = Nk2Archive::from_file(&src_path).map_err(|e| McpToolError::ExecutionFailed(e.to_string()))?;
        let before_count = archive.entries.len();

        let mut added = 0usize;
        if let Some(email) = args.get("add_email").and_then(Value::as_str) {
            let name = args.get("add_name").and_then(Value::as_str).unwrap_or(email);
            let weight = args.get("set_weight").and_then(Value::as_i64).unwrap_or(100) as i32;
            archive.add_entry(Nk2Entry::new_smtp(name, email, weight));
            added += 1;
        }

        let mut removed = 0usize;
        if let Some(arr) = args.get("remove_email").and_then(Value::as_array) {
            for item in arr {
                if let Some(em) = item.as_str() {
                    removed += archive.remove_by_email(em);
                }
            }
        }

        let mut weights_updated = 0usize;
        if let (Some(em), Some(w)) = (
            args.get("set_weight_email").and_then(Value::as_str),
            args.get("set_weight").and_then(Value::as_i64),
        ) {
            weights_updated = archive.update_weight(em, w as i32);
        }

        if let Some(merge_path) = args.get("merge").and_then(Value::as_str) {
            let other = Nk2Archive::from_file(Path::new(merge_path))
                .map_err(|e| McpToolError::ExecutionFailed(e.to_string()))?;
            added += archive.merge(&other);
        }

        let deduped = if args.get("dedup").and_then(Value::as_bool).unwrap_or(false) {
            archive.dedup()
        } else {
            0
        };

        if !dry_run {
            let create_bak = dest_path == src_path;
            archive
                .save_atomic(&dest_path, create_bak)
                .map_err(|e| McpToolError::ExecutionFailed(e.to_string()))?;
        }

        Ok(json!({
            "status": if dry_run { "dry_run" } else { "saved" },
            "source": src_path.display().to_string(),
            "destination": dest_path.display().to_string(),
            "before_rows": before_count,
            "after_rows": archive.entries.len(),
            "added": added,
            "removed": removed,
            "weights_updated": weights_updated,
            "deduped": deduped,
        }))
    }
}

pub(crate) fn query_provider_to_json(provider: &dyn TableProvider, args: &Value) -> Result<Value, McpToolError> {
    let filter = match args.get("filter").and_then(Value::as_str) {
        Some(f) if !f.trim().is_empty() => Some(guap_table::GuapQuery::parse_filter(f)?),
        _ => None,
    };
    let offset = args.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let limit = args.get("limit").and_then(Value::as_u64).map(|n| n.max(1) as usize);
    let data = provider.query(&TableQuery {
        filter,
        sort: Vec::new(),
        columns: None,
        offset,
        limit,
    })?;

    let rows: Vec<Value> = data
        .rows
        .iter()
        .map(|row| {
            let mut obj = serde_json::Map::new();
            obj.insert("row_id".to_string(), json!(row.id));
            for (idx, col) in data.schema.columns.iter().enumerate() {
                let v = row.cells.get(idx).map(|c| c.to_json_value()).unwrap_or(Value::Null);
                obj.insert(col.id.clone(), v);
            }
            if let Some(ref detail) = row.detail_json {
                obj.insert("detail_json".to_string(), detail.clone());
            }
            Value::Object(obj)
        })
        .collect();

    Ok(json!({
        "tool_id": data.schema.tool_id,
        "title": data.schema.title,
        "total_rows": data.total_rows,
        "returned_rows": rows.len(),
        "columns": data.schema.columns,
        "rows": rows,
    }))
}

pub(crate) fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}
