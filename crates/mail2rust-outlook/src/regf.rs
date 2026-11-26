use crate::error::{OutlookError, OutlookResult};
use crate::time::{datetime_to_filetime, decode_cp1252, decode_utf16le, filetime_to_datetime};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

const REGF_HEADER_SIZE: usize = 4096;
const HBIN_MAGIC: &[u8; 4] = b"hbin";
const REGF_MAGIC: &[u8; 4] = b"regf";
const NIL_OFFSET: u32 = 0xFFFF_FFFF;
const MAX_TREE_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegValue {
    String(String),
    ExpandString(String),
    Dword(u32),
    Qword(u64),
    Binary(Vec<u8>),
    MultiString(Vec<String>),
}

impl RegValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            RegValue::String(s) | RegValue::ExpandString(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_u32(&self) -> Option<u32> {
        match self {
            RegValue::Dword(v) => Some(*v),
            RegValue::Qword(v) => u32::try_from(*v).ok(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RegKeyNode {
    pub name: String,
    pub last_written: Option<DateTime<Utc>>,
    pub values: BTreeMap<String, RegValue>,
    pub subkeys: BTreeMap<String, RegKeyNode>,
}

impl RegKeyNode {
    pub fn get_path(&self, path: &str) -> Option<&RegKeyNode> {
        let mut cur = self;
        for part in path.split(['\\', '/']).filter(|s| !s.is_empty()) {
            let found = cur
                .subkeys
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(part))
                .map(|(_, v)| v)?;
            cur = found;
        }
        Some(cur)
    }

    pub fn get_value_ci(&self, name: &str) -> Option<&RegValue> {
        self.values
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v)
    }
}

#[derive(Debug, Clone)]
pub struct RegfHive {
    pub root: RegKeyNode,
    pub warnings: Vec<String>,
}

impl RegfHive {
    pub fn from_file(path: &Path) -> OutlookResult<Self> {
        let bytes = fs::read(path)?;
        Self::parse(&bytes)
    }

    pub fn parse(raw: &[u8]) -> OutlookResult<Self> {
        if raw.len() < REGF_HEADER_SIZE + 32 || &raw[0..4] != REGF_MAGIC {
            return Err(OutlookError::InvalidFormat(
                "Not a valid Windows Registry regf hive".to_string(),
            ));
        }

        let mut warnings = Vec::new();
        let seq1 = u32::from_le_bytes(raw[4..8].try_into().unwrap());
        let seq2 = u32::from_le_bytes(raw[8..12].try_into().unwrap());
        if seq1 != seq2 {
            warnings.push(format!(
                "regf sequence mismatch (seq1={seq1}, seq2={seq2}); hive may be dirty"
            ));
        }

        let declared_csum = u32::from_le_bytes(raw[0x1FC..0x200].try_into().unwrap());
        let calc_csum = compute_regf_checksum(&raw[..REGF_HEADER_SIZE]);
        if declared_csum != calc_csum {
            warnings.push(format!(
                "regf header checksum mismatch (declared=0x{declared_csum:08X}, calculated=0x{calc_csum:08X})"
            ));
        }

        let root_cell_offset = u32::from_le_bytes(raw[0x24..0x28].try_into().unwrap());
        let bins = &raw[REGF_HEADER_SIZE..];
        if bins.len() < 32 || &bins[0..4] != HBIN_MAGIC {
            return Err(OutlookError::CorruptStructure(
                "regf missing initial hbin page".to_string(),
            ));
        }

        let mut visited_cells = HashSet::new();
        let root = parse_nk_cell(bins, root_cell_offset, &mut visited_cells, 0)?;

        Ok(Self { root, warnings })
    }
}

pub fn compute_regf_checksum(header_4096: &[u8]) -> u32 {
    let mut x = 0u32;
    for chunk in header_4096[0..0x1FC].as_chunks::<4>().0 {
        x ^= u32::from_le_bytes(*chunk);
    }
    if x == 0xFFFF_FFFF {
        0xFFFF_FFFE
    } else if x == 0 {
        1
    } else {
        x
    }
}

fn read_cell_payload(bins: &[u8], rel_off: u32) -> OutlookResult<&[u8]> {
    if rel_off == NIL_OFFSET {
        return Err(OutlookError::CorruptStructure("Null regf cell offset".to_string()));
    }
    let off = rel_off as usize;
    if off + 4 > bins.len() {
        return Err(OutlookError::CorruptStructure(format!(
            "regf cell offset 0x{off:x} out of bounds"
        )));
    }
    let size_raw = i32::from_le_bytes(bins[off..off + 4].try_into().unwrap());
    let size = if size_raw < 0 {
        size_raw.unsigned_abs() as usize
    } else {
        size_raw as usize
    };
    if size < 4 || off + size > bins.len() {
        return Err(OutlookError::CorruptStructure(format!(
            "Invalid regf cell size {size} at 0x{off:x}"
        )));
    }
    Ok(&bins[off + 4..off + size])
}

fn parse_nk_cell(bins: &[u8], cell_off: u32, visited: &mut HashSet<u32>, depth: usize) -> OutlookResult<RegKeyNode> {
    if depth > MAX_TREE_DEPTH {
        return Err(OutlookError::CorruptStructure(
            "regf key nesting exceeds maximum depth".to_string(),
        ));
    }
    if !visited.insert(cell_off) {
        return Err(OutlookError::CorruptStructure(format!(
            "Cycle detected in regf nk cells at 0x{cell_off:x}"
        )));
    }

    let payload = read_cell_payload(bins, cell_off)?;
    if payload.len() < 0x4C || &payload[0..2] != b"nk" {
        return Err(OutlookError::CorruptStructure(format!(
            "Expected nk signature at cell 0x{cell_off:x}"
        )));
    }

    let flags = u16::from_le_bytes([payload[0x02], payload[0x03]]);
    let last_written = filetime_to_datetime(u64::from_le_bytes(payload[0x04..0x0C].try_into().unwrap()));
    let num_subkeys = u32::from_le_bytes(payload[0x14..0x18].try_into().unwrap()) as usize;
    let subkey_list_off = u32::from_le_bytes(payload[0x1C..0x20].try_into().unwrap());
    let num_values = u32::from_le_bytes(payload[0x24..0x28].try_into().unwrap()) as usize;
    let value_list_off = u32::from_le_bytes(payload[0x28..0x2C].try_into().unwrap());
    let name_len = u16::from_le_bytes([payload[0x48], payload[0x49]]) as usize;
    if 0x4C + name_len > payload.len() {
        return Err(OutlookError::CorruptStructure("Truncated nk key name".to_string()));
    }
    let name = if (flags & 0x0020) != 0 {
        decode_cp1252(&payload[0x4C..0x4C + name_len])
    } else {
        decode_utf16le(&payload[0x4C..0x4C + name_len])
    };

    let mut values = BTreeMap::new();
    if num_values > 0
        && value_list_off != NIL_OFFSET
        && let Ok(vlist_payload) = read_cell_payload(bins, value_list_off)
    {
        let max_vals = (vlist_payload.len() / 4).min(num_values);
        for i in 0..max_vals {
            let vk_off = u32::from_le_bytes(vlist_payload[i * 4..i * 4 + 4].try_into().unwrap());
            if let Ok((vname, vval)) = parse_vk_cell(bins, vk_off) {
                values.insert(vname, vval);
            }
        }
    }

    let mut subkeys = BTreeMap::new();
    if num_subkeys > 0 && subkey_list_off != NIL_OFFSET {
        let mut child_offsets = Vec::new();
        let mut visited_lists = HashSet::new();
        collect_subkey_offsets(bins, subkey_list_off, &mut child_offsets, &mut visited_lists, 0)?;
        for child_off in child_offsets {
            let child_node = parse_nk_cell(bins, child_off, visited, depth + 1)?;
            subkeys.insert(child_node.name.clone(), child_node);
        }
    }

    Ok(RegKeyNode {
        name,
        last_written,
        values,
        subkeys,
    })
}

fn collect_subkey_offsets(
    bins: &[u8],
    list_off: u32,
    out: &mut Vec<u32>,
    visited_lists: &mut HashSet<u32>,
    depth: usize,
) -> OutlookResult<()> {
    if list_off == NIL_OFFSET {
        return Ok(());
    }
    if depth > 16 {
        return Err(OutlookError::CorruptStructure(
            "regf subkey list nesting exceeds maximum depth".to_string(),
        ));
    }
    if !visited_lists.insert(list_off) {
        return Err(OutlookError::CorruptStructure(format!(
            "Cycle detected in regf subkey list at 0x{list_off:x}"
        )));
    }
    let payload = read_cell_payload(bins, list_off)?;
    if payload.len() < 4 {
        return Ok(());
    }
    let sig = &payload[0..2];
    let count = u16::from_le_bytes([payload[2], payload[3]]) as usize;
    match sig {
        b"lf" | b"lh" => {
            let max_c = ((payload.len() - 4) / 8).min(count);
            for i in 0..max_c {
                let off = 4 + i * 8;
                let nk_off = u32::from_le_bytes(payload[off..off + 4].try_into().unwrap());
                out.push(nk_off);
            }
        }
        b"li" => {
            let max_c = ((payload.len() - 4) / 4).min(count);
            for i in 0..max_c {
                let off = 4 + i * 4;
                let nk_off = u32::from_le_bytes(payload[off..off + 4].try_into().unwrap());
                out.push(nk_off);
            }
        }
        b"ri" => {
            let max_c = ((payload.len() - 4) / 4).min(count);
            for i in 0..max_c {
                let off = 4 + i * 4;
                let sub_off = u32::from_le_bytes(payload[off..off + 4].try_into().unwrap());
                collect_subkey_offsets(bins, sub_off, out, visited_lists, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn parse_vk_cell(bins: &[u8], vk_off: u32) -> OutlookResult<(String, RegValue)> {
    let payload = read_cell_payload(bins, vk_off)?;
    if payload.len() < 0x14 || &payload[0..2] != b"vk" {
        return Err(OutlookError::CorruptStructure(format!(
            "Expected vk signature at 0x{vk_off:x}"
        )));
    }
    let name_len = u16::from_le_bytes([payload[0x02], payload[0x03]]) as usize;
    let data_len_raw = u32::from_le_bytes(payload[0x04..0x08].try_into().unwrap());
    let data_off_or_inline = u32::from_le_bytes(payload[0x08..0x0C].try_into().unwrap());
    let data_type = u32::from_le_bytes(payload[0x0C..0x10].try_into().unwrap());
    let flags = u16::from_le_bytes([payload[0x10], payload[0x11]]);

    let name = if name_len == 0 {
        String::new()
    } else if 0x14 + name_len <= payload.len() {
        if (flags & 0x0001) != 0 {
            decode_cp1252(&payload[0x14..0x14 + name_len])
        } else {
            decode_utf16le(&payload[0x14..0x14 + name_len])
        }
    } else {
        String::new()
    };

    let is_inline = (data_len_raw & 0x8000_0000) != 0;
    let data_len = (data_len_raw & 0x7FFF_FFFF) as usize;

    let data_bytes: Vec<u8> = if data_len == 0 {
        Vec::new()
    } else if is_inline {
        payload[0x08..0x0C][..data_len.min(4)].to_vec()
    } else if data_off_or_inline != NIL_OFFSET {
        let cell = read_cell_payload(bins, data_off_or_inline)?;
        if cell.len() >= 8 && &cell[0..2] == b"db" {
            read_db_big_data(bins, cell, data_len)?
        } else {
            cell[..data_len.min(cell.len())].to_vec()
        }
    } else {
        Vec::new()
    };

    let val = match data_type {
        1 => RegValue::String(decode_utf16le(&data_bytes)),
        2 => RegValue::ExpandString(decode_utf16le(&data_bytes)),
        4 if data_bytes.len() >= 4 => RegValue::Dword(u32::from_le_bytes(data_bytes[0..4].try_into().unwrap())),
        11 if data_bytes.len() >= 8 => RegValue::Qword(u64::from_le_bytes(data_bytes[0..8].try_into().unwrap())),
        7 => RegValue::MultiString(decode_utf16le_multi(&data_bytes)),
        _ => RegValue::Binary(data_bytes),
    };

    Ok((name, val))
}

fn read_db_big_data(bins: &[u8], db_payload: &[u8], total_len: usize) -> OutlookResult<Vec<u8>> {
    const MAX_DB_SEGMENT_BYTES: usize = 16_344;
    let seg_count = u16::from_le_bytes([db_payload[2], db_payload[3]]) as usize;
    let seg_list_off = u32::from_le_bytes(db_payload[4..8].try_into().unwrap());
    let seg_list = read_cell_payload(bins, seg_list_off)?;
    let mut out = Vec::with_capacity(total_len.min(16 * 1024 * 1024));
    for i in 0..seg_count.min(seg_list.len() / 4) {
        let seg_off = u32::from_le_bytes(seg_list[i * 4..i * 4 + 4].try_into().unwrap());
        let seg = read_cell_payload(bins, seg_off)?;
        out.extend_from_slice(&seg[..seg.len().min(MAX_DB_SEGMENT_BYTES)]);
    }
    out.truncate(total_len);
    Ok(out)
}

fn decode_utf16le_multi(bytes: &[u8]) -> Vec<String> {
    let mut items = Vec::new();
    let mut cur = Vec::new();
    for chunk in bytes.as_chunks::<2>().0 {
        let u = u16::from_le_bytes(*chunk);
        if u == 0 {
            if cur.is_empty() {
                break;
            }
            items.push(String::from_utf16_lossy(&cur));
            cur.clear();
        } else {
            cur.push(u);
        }
    }
    if !cur.is_empty() {
        items.push(String::from_utf16_lossy(&cur));
    }
    items
}

/// Serializes an in-memory `RegKeyNode` tree into a valid Windows `regf` hive binary (`NTUSER.DAT`/`SOFTWARE`)
/// for deterministic synthetic fixture tests.
pub fn build_synthetic_regf_hive(root: &RegKeyNode) -> Vec<u8> {
    let mut bins = vec![0u8; 32]; // hbin header placeholder
    let root_off = write_nk_recursive(root, NIL_OFFSET, &mut bins);

    // Pad bins to 4096-byte multiple
    let padded_len = bins.len().div_ceil(4096) * 4096;
    bins.resize(padded_len, 0);

    // Write hbin header(s) at 4096 boundaries
    bins[0..4].copy_from_slice(HBIN_MAGIC);
    bins[4..8].copy_from_slice(&0u32.to_le_bytes());
    bins[8..12].copy_from_slice(&(padded_len as u32).to_le_bytes());

    let mut header = vec![0u8; REGF_HEADER_SIZE];
    header[0..4].copy_from_slice(REGF_MAGIC);
    header[4..8].copy_from_slice(&1u32.to_le_bytes());
    header[8..12].copy_from_slice(&1u32.to_le_bytes());
    let ft = datetime_to_filetime(Utc::now());
    header[12..20].copy_from_slice(&ft.to_le_bytes());
    header[20..24].copy_from_slice(&1u32.to_le_bytes());
    header[24..28].copy_from_slice(&5u32.to_le_bytes());
    header[0x24..0x28].copy_from_slice(&root_off.to_le_bytes());
    header[0x28..0x2C].copy_from_slice(&(padded_len as u32).to_le_bytes());
    let csum = compute_regf_checksum(&header);
    header[0x1FC..0x200].copy_from_slice(&csum.to_le_bytes());

    let mut out = Vec::with_capacity(REGF_HEADER_SIZE + bins.len());
    out.extend_from_slice(&header);
    out.extend_from_slice(&bins);
    out
}

fn alloc_regf_cell(bins: &mut Vec<u8>, payload: &[u8]) -> u32 {
    let raw_len = 4 + payload.len();
    let aligned_len = raw_len.div_ceil(8) * 8;
    let rel_off = bins.len() as u32;
    let neg_size = -(aligned_len as i32);
    bins.extend_from_slice(&neg_size.to_le_bytes());
    bins.extend_from_slice(payload);
    bins.resize(bins.len() + (aligned_len - raw_len), 0);
    rel_off
}

fn write_nk_recursive(node: &RegKeyNode, parent_off: u32, bins: &mut Vec<u8>) -> u32 {
    // Write values first
    let mut vk_offsets = Vec::with_capacity(node.values.len());
    for (vname, vval) in &node.values {
        vk_offsets.push(write_vk_cell(vname, vval, bins));
    }
    let value_list_off = if vk_offsets.is_empty() {
        NIL_OFFSET
    } else {
        let mut vl = Vec::with_capacity(vk_offsets.len() * 4);
        for off in &vk_offsets {
            vl.extend_from_slice(&off.to_le_bytes());
        }
        alloc_regf_cell(bins, &vl)
    };

    // Reserve placeholder nk offset? Actually we need nk_off as parent, or parent_off isn't read by parser.
    // Write subkeys recursively first!
    let mut sub_offsets = Vec::with_capacity(node.subkeys.len());
    for child in node.subkeys.values() {
        sub_offsets.push((child.name.clone(), write_nk_recursive(child, parent_off, bins)));
    }
    let subkey_list_off = if sub_offsets.is_empty() {
        NIL_OFFSET
    } else {
        let mut lf = Vec::with_capacity(4 + sub_offsets.len() * 8);
        lf.extend_from_slice(b"lf");
        lf.extend_from_slice(&(sub_offsets.len() as u16).to_le_bytes());
        for (cname, coff) in &sub_offsets {
            lf.extend_from_slice(&coff.to_le_bytes());
            let mut hint = [0u8; 4];
            let b = cname.as_bytes();
            let n = b.len().min(4);
            hint[..n].copy_from_slice(&b[..n]);
            lf.extend_from_slice(&hint);
        }
        alloc_regf_cell(bins, &lf)
    };

    let name_bytes = node.name.as_bytes();
    let mut nk = vec![0u8; 0x4C + name_bytes.len()];
    nk[0..2].copy_from_slice(b"nk");
    nk[0x02..0x04].copy_from_slice(&0x0020u16.to_le_bytes()); // ASCII name
    let ft = node
        .last_written
        .map(datetime_to_filetime)
        .unwrap_or_else(|| datetime_to_filetime(Utc::now()));
    nk[0x04..0x0C].copy_from_slice(&ft.to_le_bytes());
    nk[0x10..0x14].copy_from_slice(&parent_off.to_le_bytes());
    nk[0x14..0x18].copy_from_slice(&(sub_offsets.len() as u32).to_le_bytes());
    nk[0x1C..0x20].copy_from_slice(&subkey_list_off.to_le_bytes());
    nk[0x24..0x28].copy_from_slice(&(vk_offsets.len() as u32).to_le_bytes());
    nk[0x28..0x2C].copy_from_slice(&value_list_off.to_le_bytes());
    nk[0x48..0x4A].copy_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    nk[0x4C..0x4C + name_bytes.len()].copy_from_slice(name_bytes);

    alloc_regf_cell(bins, &nk)
}

fn write_vk_cell(name: &str, val: &RegValue, bins: &mut Vec<u8>) -> u32 {
    let (data_type, raw_data): (u32, Vec<u8>) = match val {
        RegValue::String(s) => (1, crate::time::encode_utf16le_nul(s)),
        RegValue::ExpandString(s) => (2, crate::time::encode_utf16le_nul(s)),
        RegValue::Binary(b) => (3, b.clone()),
        RegValue::Dword(d) => (4, d.to_le_bytes().to_vec()),
        RegValue::Qword(q) => (11, q.to_le_bytes().to_vec()),
        RegValue::MultiString(items) => {
            let mut buf = Vec::new();
            for item in items {
                buf.extend_from_slice(&crate::time::encode_utf16le_nul(item));
            }
            buf.extend_from_slice(&[0, 0]);
            (7, buf)
        }
    };

    let (len_raw, data_off_or_inline) = if raw_data.len() <= 4 {
        let mut inline = [0u8; 4];
        inline[..raw_data.len()].copy_from_slice(&raw_data);
        (0x8000_0000u32 | (raw_data.len() as u32), u32::from_le_bytes(inline))
    } else {
        let off = alloc_regf_cell(bins, &raw_data);
        (raw_data.len() as u32, off)
    };

    let name_bytes = name.as_bytes();
    let mut vk = vec![0u8; 0x14 + name_bytes.len()];
    vk[0..2].copy_from_slice(b"vk");
    vk[0x02..0x04].copy_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    vk[0x04..0x08].copy_from_slice(&len_raw.to_le_bytes());
    vk[0x08..0x0C].copy_from_slice(&data_off_or_inline.to_le_bytes());
    vk[0x0C..0x10].copy_from_slice(&data_type.to_le_bytes());
    vk[0x10..0x12].copy_from_slice(&0x0001u16.to_le_bytes()); // ASCII name
    vk[0x14..0x14 + name_bytes.len()].copy_from_slice(name_bytes);

    alloc_regf_cell(bins, &vk)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn wrap_regf_bins(mut bins: Vec<u8>, root_off: u32) -> Vec<u8> {
        let padded_len = bins.len().div_ceil(4096) * 4096;
        bins.resize(padded_len, 0);
        bins[0..4].copy_from_slice(HBIN_MAGIC);
        bins[4..8].copy_from_slice(&0u32.to_le_bytes());
        bins[8..12].copy_from_slice(&(padded_len as u32).to_le_bytes());

        let mut header = vec![0u8; REGF_HEADER_SIZE];
        header[0..4].copy_from_slice(REGF_MAGIC);
        header[4..8].copy_from_slice(&1u32.to_le_bytes());
        header[8..12].copy_from_slice(&1u32.to_le_bytes());
        header[20..24].copy_from_slice(&1u32.to_le_bytes());
        header[24..28].copy_from_slice(&5u32.to_le_bytes());
        header[0x24..0x28].copy_from_slice(&root_off.to_le_bytes());
        header[0x28..0x2C].copy_from_slice(&(padded_len as u32).to_le_bytes());
        let csum = compute_regf_checksum(&header);
        header[0x1FC..0x200].copy_from_slice(&csum.to_le_bytes());

        let mut out = Vec::with_capacity(REGF_HEADER_SIZE + bins.len());
        out.extend_from_slice(&header);
        out.extend_from_slice(&bins);
        out
    }

    fn build_nk_payload(
        name: &str,
        num_subkeys: u32,
        subkey_list_off: u32,
        num_values: u32,
        value_list_off: u32,
    ) -> Vec<u8> {
        let name_bytes = name.as_bytes();
        let mut nk = vec![0u8; 0x4C + name_bytes.len()];
        nk[0..2].copy_from_slice(b"nk");
        nk[0x02..0x04].copy_from_slice(&0x0020u16.to_le_bytes());
        nk[0x10..0x14].copy_from_slice(&NIL_OFFSET.to_le_bytes());
        nk[0x14..0x18].copy_from_slice(&num_subkeys.to_le_bytes());
        nk[0x1C..0x20].copy_from_slice(&subkey_list_off.to_le_bytes());
        nk[0x24..0x28].copy_from_slice(&num_values.to_le_bytes());
        nk[0x28..0x2C].copy_from_slice(&value_list_off.to_le_bytes());
        nk[0x48..0x4A].copy_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        nk[0x4C..0x4C + name_bytes.len()].copy_from_slice(name_bytes);
        nk
    }

    #[test]
    fn regf_cyclic_ri_and_nk_return_corrupt_structure_immediately() {
        // 1. Cyclic 3-entry `ri -> [ri, ri, ri]`
        {
            let mut bins = vec![0u8; 32];
            let ri_off = bins.len() as u32;
            let mut ri = Vec::new();
            ri.extend_from_slice(b"ri");
            ri.extend_from_slice(&3u16.to_le_bytes());
            ri.extend_from_slice(&ri_off.to_le_bytes());
            ri.extend_from_slice(&ri_off.to_le_bytes());
            ri.extend_from_slice(&ri_off.to_le_bytes());
            let allocated_ri = alloc_regf_cell(&mut bins, &ri);
            assert_eq!(allocated_ri, ri_off);

            let root_nk = build_nk_payload("ROOT", 3, ri_off, 0, NIL_OFFSET);
            let root_off = alloc_regf_cell(&mut bins, &root_nk);
            let raw = wrap_regf_bins(bins, root_off);

            let t0 = Instant::now();
            let err = RegfHive::parse(&raw).unwrap_err();
            assert!(t0.elapsed().as_millis() < 100);
            assert!(matches!(err, OutlookError::CorruptStructure(_)), "got {err:?}");
        }

        // 2. Cyclic 1-entry `ri -> [ri]`
        {
            let mut bins = vec![0u8; 32];
            let ri_off = bins.len() as u32;
            let mut ri = Vec::new();
            ri.extend_from_slice(b"ri");
            ri.extend_from_slice(&1u16.to_le_bytes());
            ri.extend_from_slice(&ri_off.to_le_bytes());
            alloc_regf_cell(&mut bins, &ri);

            let root_nk = build_nk_payload("ROOT", 1, ri_off, 0, NIL_OFFSET);
            let root_off = alloc_regf_cell(&mut bins, &root_nk);
            let raw = wrap_regf_bins(bins, root_off);

            let t0 = Instant::now();
            let err = RegfHive::parse(&raw).unwrap_err();
            assert!(t0.elapsed().as_millis() < 100);
            assert!(matches!(err, OutlookError::CorruptStructure(_)), "got {err:?}");
        }

        // 3. Cyclic `nk` (`root -> lf -> root`)
        {
            let mut bins = vec![0u8; 32];
            let root_off = bins.len() as u32;
            // Reserve space for lf right after root nk: root_nk size = 0x4C + 4 = 80, +4 cell hdr = 84 -> aligned 88
            let lf_off = root_off + 88;
            let root_nk = build_nk_payload("ROOT", 1, lf_off, 0, NIL_OFFSET);
            let actual_root = alloc_regf_cell(&mut bins, &root_nk);
            assert_eq!(actual_root, root_off);

            let mut lf = Vec::new();
            lf.extend_from_slice(b"lf");
            lf.extend_from_slice(&1u16.to_le_bytes());
            lf.extend_from_slice(&root_off.to_le_bytes());
            lf.extend_from_slice(b"ROOT");
            let actual_lf = alloc_regf_cell(&mut bins, &lf);
            assert_eq!(actual_lf, lf_off);

            let raw = wrap_regf_bins(bins, root_off);
            let t0 = Instant::now();
            let err = RegfHive::parse(&raw).unwrap_err();
            assert!(t0.elapsed().as_millis() < 100);
            assert!(matches!(err, OutlookError::CorruptStructure(_)), "got {err:?}");
        }
    }

    #[test]
    fn regf_db_big_data_16344_segment_boundary_decodes_17000_chars_intact() {
        let mut bins = vec![0u8; 32];
        let expected: String = "A".repeat(17_000);
        let full_bytes = crate::time::encode_utf16le_nul(&expected);
        assert_eq!(full_bytes.len(), 34_002);

        // Split across standard Windows 16,344-byte db segments (each padded to 8B boundary -> 4 trailing 0x00 bytes)
        let mut seg_offsets = Vec::new();
        for chunk in full_bytes.chunks(16_344) {
            seg_offsets.push(alloc_regf_cell(&mut bins, chunk));
        }
        assert_eq!(seg_offsets.len(), 3);

        let mut seg_list = Vec::new();
        for off in &seg_offsets {
            seg_list.extend_from_slice(&off.to_le_bytes());
        }
        let seg_list_off = alloc_regf_cell(&mut bins, &seg_list);

        let mut db_cell = Vec::new();
        db_cell.extend_from_slice(b"db");
        db_cell.extend_from_slice(&(seg_offsets.len() as u16).to_le_bytes());
        db_cell.extend_from_slice(&seg_list_off.to_le_bytes());
        let db_off = alloc_regf_cell(&mut bins, &db_cell);

        let name_bytes = b"BigStr";
        let mut vk = vec![0u8; 0x14 + name_bytes.len()];
        vk[0..2].copy_from_slice(b"vk");
        vk[0x02..0x04].copy_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        vk[0x04..0x08].copy_from_slice(&(full_bytes.len() as u32).to_le_bytes());
        vk[0x08..0x0C].copy_from_slice(&db_off.to_le_bytes());
        vk[0x0C..0x10].copy_from_slice(&1u32.to_le_bytes()); // REG_SZ
        vk[0x10..0x12].copy_from_slice(&0x0001u16.to_le_bytes());
        vk[0x14..0x14 + name_bytes.len()].copy_from_slice(name_bytes);
        let vk_off = alloc_regf_cell(&mut bins, &vk);

        let vlist_off = alloc_regf_cell(&mut bins, &vk_off.to_le_bytes());
        let root_nk = build_nk_payload("ROOT", 0, NIL_OFFSET, 1, vlist_off);
        let root_off = alloc_regf_cell(&mut bins, &root_nk);
        let raw = wrap_regf_bins(bins, root_off);

        let hive = RegfHive::parse(&raw).unwrap();
        let val = hive.root.get_value_ci("BigStr").unwrap().as_str().unwrap();
        assert_eq!(val.len(), 17_000);
        assert_eq!(val, expected);
    }
}
