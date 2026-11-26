use crate::error::{OutlookError, OutlookResult};
use crate::time::{datetime_to_filetime, decode_cp1252, decode_utf16le, encode_utf16le_nul, filetime_to_datetime};
use chrono::{DateTime, Utc};
use mail2rust_core::{Attachment, EmailAddress, EmailMessage, Importance, MessageFlags};
use std::collections::{BTreeMap, HashSet};

pub const CFB_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
const ENDOFCHAIN: u32 = 0xFFFF_FFFE;
const FREESECT: u32 = 0xFFFF_FFFF;
const FATSECT: u32 = 0xFFFF_FFFD;
const DIFSECT: u32 = 0xFFFF_FFFC;
const NOSTREAM: u32 = 0xFFFF_FFFF;

#[derive(Debug, Clone)]
pub struct CfbDirEntry {
    pub sid: u32,
    pub name: String,
    pub obj_type: u8, // 1=Storage, 2=Stream, 5=Root
    pub left_sib: u32,
    pub right_sib: u32,
    pub child_sid: u32,
    pub start_sect: u32,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct CfbStorage {
    pub name: String,
    pub streams: BTreeMap<String, Vec<u8>>,
    pub children: BTreeMap<String, CfbStorage>,
}

impl CfbStorage {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            streams: BTreeMap::new(),
            children: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CfbContainer {
    pub sector_size: usize,
    pub mini_sector_size: usize,
    pub mini_cutoff: u32,
    pub root: CfbStorage,
}

impl CfbContainer {
    pub fn parse(raw: &[u8]) -> OutlookResult<Self> {
        if raw.len() < 512 || raw[0..8] != CFB_MAGIC {
            return Err(OutlookError::InvalidFormat("Not a valid OLE2/CFB file".to_string()));
        }

        let major_ver = u16::from_le_bytes([raw[0x1A], raw[0x1B]]);
        let sector_shift = u16::from_le_bytes([raw[0x1E], raw[0x1F]]);
        let mini_shift = u16::from_le_bytes([raw[0x20], raw[0x21]]);
        if sector_shift != 9 && sector_shift != 12 {
            return Err(OutlookError::InvalidFormat(format!(
                "Unsupported CFB sector shift: {sector_shift}"
            )));
        }
        let sector_size = 1usize << sector_shift;
        let mini_sector_size = if mini_shift == 6 {
            64
        } else {
            1usize << mini_shift.min(12)
        };

        let num_fat_sects = u32::from_le_bytes(raw[0x2C..0x30].try_into().unwrap()) as usize;
        let dir_start_sect = u32::from_le_bytes(raw[0x30..0x34].try_into().unwrap());
        let mini_cutoff = u32::from_le_bytes(raw[0x38..0x3C].try_into().unwrap());
        let minifat_start_sect = u32::from_le_bytes(raw[0x3C..0x40].try_into().unwrap());
        let num_minifat_sects = u32::from_le_bytes(raw[0x40..0x44].try_into().unwrap()) as usize;
        let mut difat_sect = u32::from_le_bytes(raw[0x44..0x48].try_into().unwrap());
        let num_difat_sects = u32::from_le_bytes(raw[0x48..0x4C].try_into().unwrap()) as usize;

        // 1. Collect FAT sector indices from header DIFAT[109] + extended DIFAT chain
        let mut fat_sects = Vec::with_capacity(num_fat_sects.min(131_072));
        for i in 0..109 {
            let s = u32::from_le_bytes(raw[0x4C + i * 4..0x50 + i * 4].try_into().unwrap());
            if s != FREESECT && s != ENDOFCHAIN && fat_sects.len() < num_fat_sects {
                fat_sects.push(s);
            }
        }
        let mut difat_visited = HashSet::new();
        let entries_per_difat = (sector_size / 4).saturating_sub(1);
        for _ in 0..num_difat_sects {
            if difat_sect >= DIFSECT || !difat_visited.insert(difat_sect) {
                break;
            }
            let Some(sec) = sector_slice(raw, difat_sect, sector_size) else {
                break;
            };
            for i in 0..entries_per_difat {
                let s = u32::from_le_bytes(sec[i * 4..i * 4 + 4].try_into().unwrap());
                if s != FREESECT && s != ENDOFCHAIN && fat_sects.len() < num_fat_sects {
                    fat_sects.push(s);
                }
            }
            difat_sect = u32::from_le_bytes(
                sec[entries_per_difat * 4..entries_per_difat * 4 + 4]
                    .try_into()
                    .unwrap(),
            );
        }

        // 2. Build FAT table
        let mut fat = Vec::with_capacity(fat_sects.len() * (sector_size / 4));
        for &fs in &fat_sects {
            let Some(sec) = sector_slice(raw, fs, sector_size) else {
                break;
            };
            for chunk in sec.as_chunks::<4>().0 {
                fat.push(u32::from_le_bytes(*chunk));
            }
        }

        // 3. Read Directory stream
        let dir_bytes = read_fat_chain(raw, &fat, dir_start_sect, sector_size, None)?;
        let mut entries = Vec::with_capacity(dir_bytes.len() / 128);
        for (sid, chunk) in dir_bytes.as_chunks::<128>().0.iter().enumerate() {
            let name_len = (u16::from_le_bytes([chunk[0x40], chunk[0x41]]) as usize).min(64);
            let name = if name_len >= 2 {
                decode_utf16le(&chunk[0..name_len])
            } else {
                String::new()
            };
            let obj_type = chunk[0x42];
            let left_sib = u32::from_le_bytes(chunk[0x44..0x48].try_into().unwrap());
            let right_sib = u32::from_le_bytes(chunk[0x48..0x4C].try_into().unwrap());
            let child_sid = u32::from_le_bytes(chunk[0x4C..0x50].try_into().unwrap());
            let start_sect = u32::from_le_bytes(chunk[0x74..0x78].try_into().unwrap());
            let size = if major_ver >= 4 {
                u64::from_le_bytes(chunk[0x78..0x80].try_into().unwrap())
            } else {
                u64::from(u32::from_le_bytes(chunk[0x78..0x7C].try_into().unwrap()))
            };
            entries.push(CfbDirEntry {
                sid: sid as u32,
                name,
                obj_type,
                left_sib,
                right_sib,
                child_sid,
                start_sect,
                size,
            });
        }

        if entries.is_empty() || entries[0].obj_type != 5 {
            return Err(OutlookError::CorruptStructure(
                "CFB directory missing Root Entry".to_string(),
            ));
        }

        // 4. Read MiniStream & MiniFAT
        let mini_stream = if entries[0].start_sect != ENDOFCHAIN && entries[0].size > 0 {
            read_fat_chain(
                raw,
                &fat,
                entries[0].start_sect,
                sector_size,
                Some(entries[0].size as usize),
            )
            .unwrap_or_default()
        } else {
            Vec::new()
        };

        let mut minifat = Vec::new();
        if minifat_start_sect != ENDOFCHAIN
            && num_minifat_sects > 0
            && let Ok(mf_bytes) = read_fat_chain(
                raw,
                &fat,
                minifat_start_sect,
                sector_size,
                Some(num_minifat_sects * sector_size),
            )
        {
            for chunk in mf_bytes.as_chunks::<4>().0 {
                minifat.push(u32::from_le_bytes(*chunk));
            }
        }

        let mut visited_sids = HashSet::new();
        let root = build_storage_tree(
            &entries,
            0,
            raw,
            &fat,
            &mini_stream,
            &minifat,
            sector_size,
            mini_sector_size,
            mini_cutoff as u64,
            &mut visited_sids,
            0,
        )?;

        Ok(Self {
            sector_size,
            mini_sector_size,
            mini_cutoff,
            root,
        })
    }

    /// Serializes this CFB tree into a valid 512-byte-sector MS-CFB v3 container.
    pub fn serialize(&self) -> Vec<u8> {
        // Flatten directory entries in Red-Black (black-only sorted) BST layout.
        let mut dir_entries: Vec<FlatDirNode> = Vec::new();
        flatten_storage(&self.root, 5, &mut dir_entries);

        // Allocate all stream data sectors (streams >= 4096 in regular FAT, smaller in MiniStream if desired,
        // or store all streams in regular FAT when >= cutoff and MiniStream for < cutoff; wait: MS-CFB allows
        // streams < 4096 in MiniStream, and writing a MiniStream + MiniFAT makes it 100% spec-compliant!).
        let mut mini_stream = Vec::new();
        let mut minifat: Vec<u32> = Vec::new();
        let mut regular_sectors: Vec<Vec<u8>> = Vec::new();
        let mut fat: Vec<u32> = Vec::new();

        for node in &mut dir_entries {
            if node.obj_type == 2 {
                if node.data.is_empty() {
                    node.start_sect = ENDOFCHAIN;
                } else if (node.data.len() as u64) < 4096 {
                    let start_mini = minifat.len() as u32;
                    let chunks: Vec<&[u8]> = node.data.chunks(64).collect();
                    for (idx, chunk) in chunks.iter().enumerate() {
                        let mut sec = vec![0u8; 64];
                        sec[..chunk.len()].copy_from_slice(chunk);
                        mini_stream.extend_from_slice(&sec);
                        let next = if idx + 1 < chunks.len() {
                            start_mini + idx as u32 + 1
                        } else {
                            ENDOFCHAIN
                        };
                        minifat.push(next);
                    }
                    node.start_sect = start_mini;
                } else {
                    node.start_sect = alloc_regular_chain(&node.data, &mut regular_sectors, &mut fat);
                }
            }
        }

        // Write MiniStream into regular sectors (Root Entry)
        if !mini_stream.is_empty() {
            dir_entries[0].size = mini_stream.len() as u64;
            dir_entries[0].start_sect = alloc_regular_chain(&mini_stream, &mut regular_sectors, &mut fat);
        } else {
            dir_entries[0].size = 0;
            dir_entries[0].start_sect = ENDOFCHAIN;
        }

        // Write MiniFAT into regular sectors
        let (minifat_start, minifat_sects) = if !minifat.is_empty() {
            let mut mf_bytes = Vec::with_capacity(minifat.len() * 4);
            for entry in &minifat {
                mf_bytes.extend_from_slice(&entry.to_le_bytes());
            }
            let sects = mf_bytes.len().div_ceil(512);
            let start = alloc_regular_chain(&mf_bytes, &mut regular_sectors, &mut fat);
            (start, sects as u32)
        } else {
            (ENDOFCHAIN, 0)
        };

        // Serialize Directory sectors
        let dir_padded_len = dir_entries.len().div_ceil(4) * 512;
        let mut dir_bytes = vec![0u8; dir_padded_len];
        for (idx, node) in dir_entries.iter().enumerate() {
            let base = idx * 128;
            let name_bytes = encode_utf16le_nul(&node.name);
            let copy_len = name_bytes.len().min(64);
            dir_bytes[base..base + copy_len].copy_from_slice(&name_bytes[..copy_len]);
            dir_bytes[base + 0x40..base + 0x42].copy_from_slice(&(copy_len as u16).to_le_bytes());
            dir_bytes[base + 0x42] = node.obj_type;
            dir_bytes[base + 0x43] = 1; // black node
            dir_bytes[base + 0x44..base + 0x48].copy_from_slice(&node.left_sib.to_le_bytes());
            dir_bytes[base + 0x48..base + 0x4C].copy_from_slice(&node.right_sib.to_le_bytes());
            dir_bytes[base + 0x4C..base + 0x50].copy_from_slice(&node.child_sid.to_le_bytes());
            dir_bytes[base + 0x74..base + 0x78].copy_from_slice(&node.start_sect.to_le_bytes());
            dir_bytes[base + 0x78..base + 0x7C].copy_from_slice(&(node.size as u32).to_le_bytes());
        }
        let dir_start = alloc_regular_chain(&dir_bytes, &mut regular_sectors, &mut fat);

        // Compute FAT sector count iteratively (128 FAT entries per 512B sector)
        let mut num_fat_sects = 1usize;
        loop {
            let total_sects = regular_sectors.len() + num_fat_sects;
            let needed = total_sects.div_ceil(128).max(1);
            if needed == num_fat_sects {
                break;
            }
            num_fat_sects = needed;
        }
        // Append FAT sectors themselves marked as FATSECT
        let mut fat_sect_ids = Vec::with_capacity(num_fat_sects);
        for _ in 0..num_fat_sects {
            let sid = regular_sectors.len() as u32;
            fat_sect_ids.push(sid);
            fat.push(FATSECT);
            regular_sectors.push(vec![0u8; 512]);
        }
        while !fat.len().is_multiple_of(128) {
            fat.push(FREESECT);
        }
        for (i, &sect_id) in fat_sect_ids.iter().enumerate() {
            let mut sec = vec![0u8; 512];
            for j in 0..128 {
                let val = fat[i * 128 + j];
                sec[j * 4..j * 4 + 4].copy_from_slice(&val.to_le_bytes());
            }
            regular_sectors[sect_id as usize] = sec;
        }

        // Build 512-byte Header
        let mut out = vec![0u8; 512 + regular_sectors.len() * 512];
        out[0..8].copy_from_slice(&CFB_MAGIC);
        out[0x18..0x1A].copy_from_slice(&0x003Eu16.to_le_bytes());
        out[0x1A..0x1C].copy_from_slice(&0x0003u16.to_le_bytes());
        out[0x1C..0x1E].copy_from_slice(&0xFFFEu16.to_le_bytes());
        out[0x1E..0x20].copy_from_slice(&9u16.to_le_bytes());
        out[0x20..0x22].copy_from_slice(&6u16.to_le_bytes());
        out[0x2C..0x30].copy_from_slice(&(num_fat_sects as u32).to_le_bytes());
        out[0x30..0x34].copy_from_slice(&dir_start.to_le_bytes());
        out[0x38..0x3C].copy_from_slice(&4096u32.to_le_bytes());
        out[0x3C..0x40].copy_from_slice(&minifat_start.to_le_bytes());
        out[0x40..0x44].copy_from_slice(&minifat_sects.to_le_bytes());
        out[0x44..0x48].copy_from_slice(&ENDOFCHAIN.to_le_bytes());
        out[0x48..0x4C].copy_from_slice(&0u32.to_le_bytes());
        for i in 0..109 {
            let val = fat_sect_ids.get(i).copied().unwrap_or(FREESECT);
            out[0x4C + i * 4..0x50 + i * 4].copy_from_slice(&val.to_le_bytes());
        }
        for (i, sec) in regular_sectors.iter().enumerate() {
            let off = 512 + i * 512;
            out[off..off + 512].copy_from_slice(sec);
        }
        out
    }
}

fn sector_slice(raw: &[u8], sect: u32, sector_size: usize) -> Option<&[u8]> {
    let start = (sect as usize).checked_add(1)?.checked_mul(sector_size)?;
    let end = start.checked_add(sector_size)?;
    raw.get(start..end)
}

fn read_fat_chain(
    raw: &[u8],
    fat: &[u32],
    start: u32,
    sector_size: usize,
    limit: Option<usize>,
) -> OutlookResult<Vec<u8>> {
    if start >= DIFSECT {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut cur = start;
    let mut visited = HashSet::new();
    while cur < DIFSECT {
        if !visited.insert(cur) {
            return Err(OutlookError::CorruptStructure(format!(
                "Cycle detected in CFB FAT chain at sector {cur}"
            )));
        }
        let Some(sec) = sector_slice(raw, cur, sector_size) else {
            break;
        };
        out.extend_from_slice(sec);
        if let Some(lim) = limit
            && out.len() >= lim
        {
            out.truncate(lim);
            return Ok(out);
        }
        cur = fat.get(cur as usize).copied().unwrap_or(ENDOFCHAIN);
    }
    if let Some(lim) = limit
        && out.len() > lim
    {
        out.truncate(lim);
    }
    Ok(out)
}

fn read_minifat_chain(mini_stream: &[u8], minifat: &[u32], start: u32, mini_size: usize, limit: usize) -> Vec<u8> {
    if start >= DIFSECT || limit == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(limit.min(mini_stream.len()));
    let mut cur = start;
    let mut visited = HashSet::new();
    while cur < DIFSECT && out.len() < limit {
        if !visited.insert(cur) {
            break;
        }
        let start_off = (cur as usize).saturating_mul(mini_size);
        let end_off = start_off.saturating_add(mini_size);
        if end_off > mini_stream.len() {
            break;
        }
        out.extend_from_slice(&mini_stream[start_off..end_off]);
        cur = minifat.get(cur as usize).copied().unwrap_or(ENDOFCHAIN);
    }
    out.truncate(limit);
    out
}

#[allow(clippy::too_many_arguments)]
fn build_storage_tree(
    entries: &[CfbDirEntry],
    sid: u32,
    raw: &[u8],
    fat: &[u32],
    mini_stream: &[u8],
    minifat: &[u32],
    sector_size: usize,
    mini_sector_size: usize,
    mini_cutoff: u64,
    visited: &mut HashSet<u32>,
    depth: usize,
) -> OutlookResult<CfbStorage> {
    if depth > 64 {
        return Err(OutlookError::CorruptStructure(
            "CFB directory nesting exceeds limit".to_string(),
        ));
    }
    let entry = entries
        .get(sid as usize)
        .ok_or_else(|| OutlookError::CorruptStructure(format!("Missing CFB directory sid {sid}")))?;

    let mut storage = CfbStorage::new(&entry.name);
    let mut child_sids = Vec::new();
    collect_bst_sids(entries, entry.child_sid, &mut child_sids, visited, 0);

    for csid in child_sids {
        let Some(child) = entries.get(csid as usize) else {
            continue;
        };
        match child.obj_type {
            1 => {
                let sub = build_storage_tree(
                    entries,
                    csid,
                    raw,
                    fat,
                    mini_stream,
                    minifat,
                    sector_size,
                    mini_sector_size,
                    mini_cutoff,
                    visited,
                    depth + 1,
                )?;
                storage.children.insert(child.name.clone(), sub);
            }
            2 => {
                let data = if child.size == 0 || child.start_sect >= DIFSECT {
                    Vec::new()
                } else if child.size < mini_cutoff && !mini_stream.is_empty() && !minifat.is_empty() {
                    read_minifat_chain(
                        mini_stream,
                        minifat,
                        child.start_sect,
                        mini_sector_size,
                        child.size as usize,
                    )
                } else {
                    read_fat_chain(raw, fat, child.start_sect, sector_size, Some(child.size as usize))?
                };
                storage.streams.insert(child.name.clone(), data);
            }
            _ => {}
        }
    }

    Ok(storage)
}

fn collect_bst_sids(entries: &[CfbDirEntry], sid: u32, out: &mut Vec<u32>, visited: &mut HashSet<u32>, depth: usize) {
    if depth > 256 || sid == NOSTREAM || sid as usize >= entries.len() || !visited.insert(sid) {
        return;
    }
    let e = &entries[sid as usize];
    collect_bst_sids(entries, e.left_sib, out, visited, depth + 1);
    out.push(sid);
    collect_bst_sids(entries, e.right_sib, out, visited, depth + 1);
}

struct FlatDirNode {
    name: String,
    obj_type: u8,
    left_sib: u32,
    right_sib: u32,
    child_sid: u32,
    start_sect: u32,
    size: u64,
    data: Vec<u8>,
}

fn flatten_storage(storage: &CfbStorage, obj_type: u8, out: &mut Vec<FlatDirNode>) -> u32 {
    let my_sid = out.len() as u32;
    out.push(FlatDirNode {
        name: storage.name.clone(),
        obj_type,
        left_sib: NOSTREAM,
        right_sib: NOSTREAM,
        child_sid: NOSTREAM,
        start_sect: ENDOFCHAIN,
        size: 0,
        data: Vec::new(),
    });

    // Collect children (storages + streams) sorted per MS-CFB name order (len, then case-insensitive UTF-16)
    let mut child_sids: Vec<u32> = Vec::new();
    let mut child_names: Vec<(bool, String)> = Vec::new();
    for k in storage.children.keys() {
        child_names.push((true, k.clone()));
    }
    for k in storage.streams.keys() {
        child_names.push((false, k.clone()));
    }
    child_names.sort_by(|a, b| {
        a.1.len()
            .cmp(&b.1.len())
            .then_with(|| a.1.to_uppercase().cmp(&b.1.to_uppercase()))
    });

    for (is_storage, name) in child_names {
        if is_storage {
            let child = &storage.children[&name];
            let sid = flatten_storage(child, 1, out);
            child_sids.push(sid);
        } else {
            let data = &storage.streams[&name];
            let sid = out.len() as u32;
            out.push(FlatDirNode {
                name,
                obj_type: 2,
                left_sib: NOSTREAM,
                right_sib: NOSTREAM,
                child_sid: NOSTREAM,
                start_sect: ENDOFCHAIN,
                size: data.len() as u64,
                data: data.clone(),
            });
            child_sids.push(sid);
        }
    }

    if !child_sids.is_empty() {
        let root_child = link_balanced_bst(&child_sids, out);
        out[my_sid as usize].child_sid = root_child;
    }

    my_sid
}

fn link_balanced_bst(sids: &[u32], out: &mut [FlatDirNode]) -> u32 {
    if sids.is_empty() {
        return NOSTREAM;
    }
    let mid = sids.len() / 2;
    let sid = sids[mid];
    let left = link_balanced_bst(&sids[..mid], out);
    let right = link_balanced_bst(&sids[mid + 1..], out);
    out[sid as usize].left_sib = left;
    out[sid as usize].right_sib = right;
    sid
}

fn alloc_regular_chain(data: &[u8], regular_sectors: &mut Vec<Vec<u8>>, fat: &mut Vec<u32>) -> u32 {
    if data.is_empty() {
        return ENDOFCHAIN;
    }
    let start = regular_sectors.len() as u32;
    let chunks: Vec<&[u8]> = data.chunks(512).collect();
    for (idx, chunk) in chunks.iter().enumerate() {
        let mut sec = vec![0u8; 512];
        sec[..chunk.len()].copy_from_slice(chunk);
        regular_sectors.push(sec);
        let next = if idx + 1 < chunks.len() {
            start + idx as u32 + 1
        } else {
            ENDOFCHAIN
        };
        fat.push(next);
    }
    start
}

/// Decoded MS-OXMSG attachment metadata & payload.
#[derive(Debug, Clone)]
pub struct MsgAttachment {
    pub storage_name: String,
    pub filename: String,
    pub mime_type: String,
    pub content_id: Option<String>,
    pub is_inline: bool,
    pub data: Vec<u8>,
}

/// Decoded MS-OXMSG Outlook `.msg` item.
#[derive(Debug, Clone)]
pub struct MsgDocument {
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub sender_name: String,
    pub sender_email: String,
    pub message_id: Option<String>,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub bcc: Vec<EmailAddress>,
    pub date: Option<DateTime<Utc>>,
    pub flags: u32,
    pub importance: Importance,
    pub attachments: Vec<MsgAttachment>,
}

impl MsgDocument {
    pub fn parse_bytes(raw: &[u8]) -> OutlookResult<Self> {
        let cfb = CfbContainer::parse(raw)?;
        Self::from_cfb(&cfb)
    }

    pub fn from_cfb(cfb: &CfbContainer) -> OutlookResult<Self> {
        let root = &cfb.root;
        let props = parse_msg_properties_stream(root.streams.get("__properties_version1.0").map(Vec::as_slice), 32);

        let subject = read_substg_string(root, "0037").unwrap_or_else(|| "(No Subject)".to_string());
        let body_text = read_substg_string(root, "1000").unwrap_or_default();
        let body_html = read_substg_html(root, "1013");
        let sender_name = read_substg_string(root, "0C1A").unwrap_or_default();
        let sender_email = read_substg_string(root, "5D01")
            .or_else(|| read_substg_string(root, "0C1F"))
            .unwrap_or_else(|| "sender@msg.local".to_string());
        let message_id = read_substg_string(root, "1035");

        let date = props
            .get(&0x0E06_0040)
            .and_then(|&v| filetime_to_datetime(v))
            .or_else(|| props.get(&0x0039_0040).and_then(|&v| filetime_to_datetime(v)))
            .or_else(|| props.get(&0x3007_0040).and_then(|&v| filetime_to_datetime(v)));

        let flags = props.get(&0x0E07_0003).map(|&v| v as u32).unwrap_or(1);
        let importance = match props.get(&0x0017_0003).map(|&v| v as u32).unwrap_or(1) {
            2 => Importance::High,
            0 => Importance::Low,
            _ => Importance::Normal,
        };

        let mut to = Vec::new();
        let mut cc = Vec::new();
        let mut bcc = Vec::new();
        let mut attachments = Vec::new();

        for (name, sub) in &root.children {
            if name.starts_with("__recip_version1.0_") {
                let rprops =
                    parse_msg_properties_stream(sub.streams.get("__properties_version1.0").map(Vec::as_slice), 8);
                let rtype = rprops.get(&0x0C15_0003).map(|&v| v as u32).unwrap_or(1);
                let dname = read_substg_string(sub, "3001").unwrap_or_default();
                let email = read_substg_string(sub, "39FE")
                    .or_else(|| read_substg_string(sub, "3003"))
                    .unwrap_or_default();
                if !email.is_empty() || !dname.is_empty() {
                    let addr = EmailAddress::new(
                        if dname.is_empty() { email.clone() } else { dname },
                        if email.is_empty() {
                            "recipient@msg.local".to_string()
                        } else {
                            email
                        },
                    );
                    match rtype {
                        2 => cc.push(addr),
                        3 => bcc.push(addr),
                        _ => to.push(addr),
                    }
                }
            } else if name.starts_with("__attach_version1.0_") {
                let aprops =
                    parse_msg_properties_stream(sub.streams.get("__properties_version1.0").map(Vec::as_slice), 8);
                let aflags = aprops.get(&0x3714_0003).map(|&v| v as u32).unwrap_or(0);
                let filename = read_substg_string(sub, "3707")
                    .or_else(|| read_substg_string(sub, "3704"))
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| format!("{name}.bin"));
                let mime_type = read_substg_string(sub, "370E")
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "application/octet-stream".to_string());
                let content_id = read_substg_string(sub, "3712").filter(|s| !s.is_empty());
                let is_inline = content_id.is_some() || (aflags & 0x04) != 0;
                let data = sub.streams.get("__substg1.0_37010102").cloned().unwrap_or_default();

                attachments.push(MsgAttachment {
                    storage_name: name.clone(),
                    filename,
                    mime_type,
                    content_id,
                    is_inline,
                    data,
                });
            }
        }

        Ok(Self {
            subject,
            body_text,
            body_html,
            sender_name: if sender_name.is_empty() {
                sender_email.clone()
            } else {
                sender_name
            },
            sender_email,
            message_id,
            to,
            cc,
            bcc,
            date,
            flags,
            importance,
            attachments,
        })
    }

    pub fn to_email_message(&self, folder_path: Vec<String>) -> EmailMessage {
        let (sha256, md5) = EmailMessage::compute_hashes(self.body_text.as_bytes());
        let message_id = self
            .message_id
            .clone()
            .unwrap_or_else(|| format!("<msg_{}@mail2rust.local>", &sha256[..16]));
        let conversation_id = EmailMessage::generate_conversation_id(&self.subject, &message_id, None);
        let attachments = self
            .attachments
            .iter()
            .map(|a| {
                Attachment::new(
                    a.filename.clone(),
                    a.mime_type.clone(),
                    a.data.clone(),
                    a.content_id.clone(),
                )
            })
            .collect();

        EmailMessage {
            id: format!("msg_{}", &sha256[..16]),
            message_id,
            conversation_id,
            folder_path,
            subject: self.subject.clone(),
            from: EmailAddress::new(self.sender_name.clone(), self.sender_email.clone()),
            to: self.to.clone(),
            cc: self.cc.clone(),
            bcc: self.bcc.clone(),
            date: self.date.unwrap_or_else(Utc::now),
            size_bytes: self.body_text.len() + self.attachments.iter().map(|a| a.data.len()).sum::<usize>(),
            body_text: self.body_text.clone(),
            body_html: self.body_html.clone(),
            raw_mime: Vec::new(),
            attachments,
            mapi_properties: Vec::new(),
            flags: MessageFlags {
                read: (self.flags & 0x01) != 0,
                replied: (self.flags & 0x04) != 0,
                draft: (self.flags & 0x08) != 0,
                ..Default::default()
            },
            importance: self.importance,
            sha256,
            md5,
            bates_number: None,
            legal_hold: false,
            source_format: "MSG (CFB/OLE2)".to_string(),
        }
    }
}

fn read_substg_string(storage: &CfbStorage, prop_hex: &str) -> Option<String> {
    let w_key = format!("__substg1.0_{prop_hex}001F");
    if let Some(bytes) = storage.streams.get(&w_key) {
        let s = decode_utf16le(bytes);
        if !s.is_empty() {
            return Some(s);
        }
    }
    let a_key = format!("__substg1.0_{prop_hex}001E");
    if let Some(bytes) = storage.streams.get(&a_key) {
        let s = decode_cp1252(bytes);
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

fn read_substg_html(storage: &CfbStorage, prop_hex: &str) -> Option<String> {
    let bin_key = format!("__substg1.0_{prop_hex}0102");
    if let Some(bytes) = storage.streams.get(&bin_key) {
        return Some(String::from_utf8_lossy(bytes).to_string());
    }
    read_substg_string(storage, prop_hex)
}

fn parse_msg_properties_stream(stream_opt: Option<&[u8]>, header_len: usize) -> BTreeMap<u32, u64> {
    let mut map = BTreeMap::new();
    let Some(bytes) = stream_opt else {
        return map;
    };
    if bytes.len() < header_len {
        return map;
    }
    for rec in bytes[header_len..].as_chunks::<16>().0 {
        let tag = u32::from_le_bytes([rec[0], rec[1], rec[2], rec[3]]);
        let val = u64::from_le_bytes([rec[8], rec[9], rec[10], rec[11], rec[12], rec[13], rec[14], rec[15]]);
        map.insert(tag, val);
    }
    map
}

fn encode_utf16le_no_nul(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() * 2);
    for u in s.encode_utf16() {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out
}

/// Builds a valid CFB/OLE2 `.msg` binary container from an `EmailMessage`.
pub fn build_msg_cfb_bytes(msg: &EmailMessage) -> Vec<u8> {
    let mut root = CfbStorage::new("Root Entry");
    root.streams
        .insert("__substg1.0_001A001F".to_string(), encode_utf16le_no_nul("IPM.Note"));
    root.streams
        .insert("__substg1.0_0037001F".to_string(), encode_utf16le_no_nul(&msg.subject));
    root.streams.insert(
        "__substg1.0_1000001F".to_string(),
        encode_utf16le_no_nul(&msg.body_text),
    );
    if let Some(ref html) = msg.body_html {
        root.streams
            .insert("__substg1.0_10130102".to_string(), html.as_bytes().to_vec());
    }
    root.streams.insert(
        "__substg1.0_0C1A001F".to_string(),
        encode_utf16le_no_nul(&msg.from.name),
    );
    root.streams.insert(
        "__substg1.0_0C1F001F".to_string(),
        encode_utf16le_no_nul(&msg.from.email),
    );
    root.streams.insert(
        "__substg1.0_5D01001F".to_string(),
        encode_utf16le_no_nul(&msg.from.email),
    );
    if !msg.message_id.is_empty() {
        root.streams.insert(
            "__substg1.0_1035001F".to_string(),
            encode_utf16le_no_nul(&msg.message_id),
        );
    }

    // Root __properties_version1.0 (32-byte header + 16-byte fixed property entries)
    let mut props_stream = vec![0u8; 32];
    props_stream[8..12].copy_from_slice(&(msg.to.len() as u32 + msg.cc.len() as u32).to_le_bytes());
    props_stream[12..16].copy_from_slice(&(msg.attachments.len() as u32).to_le_bytes());
    let ft = datetime_to_filetime(msg.date);
    push_msg_prop(&mut props_stream, 0x0E06_0040, ft);
    push_msg_prop(&mut props_stream, 0x0039_0040, ft);
    let flags = if msg.flags.read { 1u64 } else { 0u64 };
    push_msg_prop(&mut props_stream, 0x0E07_0003, flags);
    let imp = match msg.importance {
        Importance::High => 2u64,
        Importance::Low => 0u64,
        Importance::Normal => 1u64,
    };
    push_msg_prop(&mut props_stream, 0x0017_0003, imp);
    root.streams.insert("__properties_version1.0".to_string(), props_stream);

    // Recipients
    let mut r_idx = 0usize;
    for (rtype, list) in [(1u64, &msg.to), (2u64, &msg.cc), (3u64, &msg.bcc)] {
        for addr in list {
            let rname = format!("__recip_version1.0_#{r_idx:08X}");
            let mut rsub = CfbStorage::new(&rname);
            rsub.streams
                .insert("__substg1.0_3001001F".to_string(), encode_utf16le_no_nul(&addr.name));
            rsub.streams
                .insert("__substg1.0_3003001F".to_string(), encode_utf16le_no_nul(&addr.email));
            rsub.streams
                .insert("__substg1.0_39FE001F".to_string(), encode_utf16le_no_nul(&addr.email));
            let mut rprops = vec![0u8; 8];
            push_msg_prop(&mut rprops, 0x0C15_0003, rtype);
            rsub.streams.insert("__properties_version1.0".to_string(), rprops);
            root.children.insert(rname, rsub);
            r_idx += 1;
        }
    }

    // Attachments
    for (a_idx, att) in msg.attachments.iter().enumerate() {
        let aname = format!("__attach_version1.0_#{a_idx:08X}");
        let mut asub = CfbStorage::new(&aname);
        asub.streams
            .insert("__substg1.0_3704001F".to_string(), encode_utf16le_no_nul(&att.filename));
        asub.streams
            .insert("__substg1.0_3707001F".to_string(), encode_utf16le_no_nul(&att.filename));
        asub.streams.insert(
            "__substg1.0_370E001F".to_string(),
            encode_utf16le_no_nul(&att.mime_type),
        );
        if let Some(ref cid) = att.content_id {
            asub.streams
                .insert("__substg1.0_3712001F".to_string(), encode_utf16le_no_nul(cid));
        }
        asub.streams
            .insert("__substg1.0_37010102".to_string(), att.data.clone());
        let mut aprops = vec![0u8; 8];
        push_msg_prop(&mut aprops, 0x3705_0003, 1);
        push_msg_prop(&mut aprops, 0x0E20_0003, att.data.len() as u64);
        push_msg_prop(&mut aprops, 0x3714_0003, if att.content_id.is_some() { 4 } else { 0 });
        asub.streams.insert("__properties_version1.0".to_string(), aprops);
        root.children.insert(aname, asub);
    }

    let cfb = CfbContainer {
        sector_size: 512,
        mini_sector_size: 64,
        mini_cutoff: 4096,
        root,
    };
    cfb.serialize()
}

/// Removes matching attachment storages from a CFB `.msg` container and appends an audit note.
pub fn strip_msg_cfb_bytes(raw: &[u8], remove_names: &HashSet<String>) -> OutlookResult<(Vec<u8>, usize)> {
    let mut cfb = CfbContainer::parse(raw)?;
    let mut removed = 0usize;
    let keys: Vec<String> = cfb
        .root
        .children
        .keys()
        .filter(|k| k.starts_with("__attach_version1.0_"))
        .cloned()
        .collect();
    let mut stripped_names = Vec::new();
    for key in keys {
        let sub = &cfb.root.children[&key];
        let fname = read_substg_string(sub, "3707")
            .or_else(|| read_substg_string(sub, "3704"))
            .unwrap_or_default();
        if remove_names.is_empty() || remove_names.contains(&fname.to_lowercase()) {
            cfb.root.children.remove(&key);
            stripped_names.push(fname);
            removed += 1;
        }
    }
    if removed > 0 {
        let mut body = read_substg_string(&cfb.root, "1000").unwrap_or_default();
        for name in stripped_names {
            body.push_str(&format!("\n[Attachment stripped by mail2rust: {name}]"));
        }
        cfb.root
            .streams
            .insert("__substg1.0_1000001F".to_string(), encode_utf16le_no_nul(&body));
    }
    Ok((cfb.serialize(), removed))
}

fn push_msg_prop(buf: &mut Vec<u8>, tag: u32, val: u64) {
    buf.extend_from_slice(&tag.to_le_bytes());
    buf.extend_from_slice(&0x0000_0006u32.to_le_bytes()); // PROPATTR_READABLE | WRITABLE
    buf.extend_from_slice(&val.to_le_bytes());
}
