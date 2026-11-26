use crate::models::{Attachment, EmailAddress, EmailMessage, Importance, MessageFlags};
use chrono::{DateTime, TimeZone, Utc};
use std::collections::{BTreeMap, HashMap, HashSet};
use tracing::debug;

pub struct MsgReader;

const ENDOFCHAIN: u32 = 0xFFFF_FFFE;
const FREESECT: u32 = 0xFFFF_FFFF;
const DIFSECT: u32 = 0xFFFF_FFFC;
const NOSTREAM: u32 = 0xFFFF_FFFF;

impl MsgReader {
    pub const CFB_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

    pub fn is_msg(bytes: &[u8]) -> bool {
        bytes.len() >= 8 && bytes[0..8] == Self::CFB_MAGIC
    }

    /// Pure-Rust CFB/OLE2 stream parser for MS-OXMSG Outlook `.msg` files (with legacy stub fallback).
    pub fn parse(raw_bytes: &[u8], folder_path: Vec<String>) -> Result<EmailMessage, Box<dyn std::error::Error>> {
        if !Self::is_msg(raw_bytes) {
            return Err("Invalid Outlook MSG: Not an OLE2/CFB compound file".into());
        }

        if let Some(msg) = Self::parse_cfb_msg(raw_bytes, &folder_path) {
            debug!(id = %msg.id, subject = %msg.subject, attachments = msg.attachments.len(), "Parsed CFB Outlook MSG");
            return Ok(msg);
        }

        let (sha256, md5) = EmailMessage::compute_hashes(raw_bytes);
        let streams = Self::scan_cfb_streams(raw_bytes);

        let subject = streams
            .get("__substg1.0_0037")
            .cloned()
            .unwrap_or_else(|| "(No Subject)".to_string());

        let body_text = streams.get("__substg1.0_1000").cloned().unwrap_or_default();
        let body_html = streams.get("__substg1.0_1013").cloned();
        let sender_email = streams
            .get("__substg1.0_0C1F")
            .cloned()
            .unwrap_or_else(|| "sender@msg.local".to_string());
        let sender_name = streams
            .get("__substg1.0_0C1A")
            .cloned()
            .unwrap_or_else(|| sender_email.clone());

        let from = EmailAddress::new(sender_name, sender_email);
        let to = vec![EmailAddress::new("Recipient", "recipient@msg.local")];

        let message_id = streams
            .get("__substg1.0_1035")
            .cloned()
            .unwrap_or_else(|| format!("<msg_{}@mail2rust.local>", &sha256[..16]));

        let conversation_id = EmailMessage::generate_conversation_id(&subject, &message_id, None);

        let msg = EmailMessage {
            id: format!("msg_{}", &sha256[..16]),
            message_id,
            conversation_id,
            folder_path,
            subject,
            from,
            to,
            cc: Vec::new(),
            bcc: Vec::new(),
            date: Utc::now(),
            size_bytes: raw_bytes.len(),
            body_text,
            body_html,
            raw_mime: Vec::new(),
            attachments: Vec::new(),
            mapi_properties: Vec::new(),
            flags: MessageFlags {
                read: true,
                ..Default::default()
            },
            importance: Importance::Normal,
            sha256,
            md5,
            bates_number: None,
            legal_hold: false,
            source_format: "MSG".to_string(),
        };

        debug!(id = %msg.id, subject = %msg.subject, "Parsed Outlook MSG (stub fallback)");
        Ok(msg)
    }

    fn parse_cfb_msg(raw: &[u8], folder_path: &[String]) -> Option<EmailMessage> {
        if raw.len() < 512 {
            return None;
        }
        let major_ver = u16::from_le_bytes([raw[0x1A], raw[0x1B]]);
        let sector_shift = u16::from_le_bytes([raw[0x1E], raw[0x1F]]);
        let mini_shift = u16::from_le_bytes([raw[0x20], raw[0x21]]);
        if sector_shift != 9 && sector_shift != 12 {
            return None;
        }
        let sector_size = 1usize << sector_shift;
        let mini_sector_size = if mini_shift == 6 {
            64
        } else {
            1usize << mini_shift.min(12)
        };
        let num_fat_sects = u32::from_le_bytes(raw[0x2C..0x30].try_into().ok()?) as usize;
        let dir_start = u32::from_le_bytes(raw[0x30..0x34].try_into().ok()?);
        let mini_cutoff = u64::from(u32::from_le_bytes(raw[0x38..0x3C].try_into().ok()?));
        let minifat_start = u32::from_le_bytes(raw[0x3C..0x40].try_into().ok()?);
        let num_minifat_sects = u32::from_le_bytes(raw[0x40..0x44].try_into().ok()?) as usize;
        let mut difat_sect = u32::from_le_bytes(raw[0x44..0x48].try_into().ok()?);
        let num_difat_sects = u32::from_le_bytes(raw[0x48..0x4C].try_into().ok()?) as usize;

        let mut fat_sects = Vec::new();
        for i in 0..109 {
            let s = u32::from_le_bytes(raw[0x4C + i * 4..0x50 + i * 4].try_into().ok()?);
            if s != FREESECT && s != ENDOFCHAIN && fat_sects.len() < num_fat_sects {
                fat_sects.push(s);
            }
        }
        let entries_per_difat = (sector_size / 4).saturating_sub(1);
        let mut difat_seen = HashSet::new();
        for _ in 0..num_difat_sects {
            if difat_sect >= DIFSECT || !difat_seen.insert(difat_sect) {
                break;
            }
            let sec = cfb_sector(raw, difat_sect, sector_size)?;
            for i in 0..entries_per_difat {
                let s = u32::from_le_bytes(sec[i * 4..i * 4 + 4].try_into().ok()?);
                if s != FREESECT && s != ENDOFCHAIN && fat_sects.len() < num_fat_sects {
                    fat_sects.push(s);
                }
            }
            difat_sect = u32::from_le_bytes(sec[entries_per_difat * 4..entries_per_difat * 4 + 4].try_into().ok()?);
        }

        let mut fat = Vec::with_capacity(fat_sects.len() * (sector_size / 4));
        for &fs in &fat_sects {
            let sec = cfb_sector(raw, fs, sector_size)?;
            for chunk in sec.as_chunks::<4>().0 {
                fat.push(u32::from_le_bytes(*chunk));
            }
        }

        let dir_bytes = cfb_fat_chain(raw, &fat, dir_start, sector_size, None)?;
        if dir_bytes.len() < 128 {
            return None;
        }
        struct Entry {
            name: String,
            obj_type: u8,
            left: u32,
            right: u32,
            child: u32,
            start: u32,
            size: u64,
        }
        let mut entries = Vec::with_capacity(dir_bytes.len() / 128);
        for chunk in dir_bytes.as_chunks::<128>().0 {
            let name_len = (u16::from_le_bytes([chunk[0x40], chunk[0x41]]) as usize).min(64);
            let name = if name_len >= 2 {
                cfb_utf16(&chunk[..name_len])
            } else {
                String::new()
            };
            entries.push(Entry {
                name,
                obj_type: chunk[0x42],
                left: u32::from_le_bytes(chunk[0x44..0x48].try_into().ok()?),
                right: u32::from_le_bytes(chunk[0x48..0x4C].try_into().ok()?),
                child: u32::from_le_bytes(chunk[0x4C..0x50].try_into().ok()?),
                start: u32::from_le_bytes(chunk[0x74..0x78].try_into().ok()?),
                size: if major_ver >= 4 {
                    u64::from_le_bytes(chunk[0x78..0x80].try_into().ok()?)
                } else {
                    u64::from(u32::from_le_bytes(chunk[0x78..0x7C].try_into().ok()?))
                },
            });
        }
        if entries[0].obj_type != 5 {
            return None;
        }

        let mini_stream = if entries[0].start < DIFSECT && entries[0].size > 0 {
            cfb_fat_chain(raw, &fat, entries[0].start, sector_size, Some(entries[0].size as usize)).unwrap_or_default()
        } else {
            Vec::new()
        };
        let mut minifat = Vec::new();
        if minifat_start < DIFSECT
            && num_minifat_sects > 0
            && let Some(mf) = cfb_fat_chain(
                raw,
                &fat,
                minifat_start,
                sector_size,
                Some(num_minifat_sects * sector_size),
            )
        {
            for c in mf.as_chunks::<4>().0 {
                minifat.push(u32::from_le_bytes(*c));
            }
        }

        let read_entry = |e: &Entry| -> Vec<u8> {
            if e.size == 0 || e.start >= DIFSECT {
                return Vec::new();
            }
            if e.size < mini_cutoff && !mini_stream.is_empty() && !minifat.is_empty() {
                cfb_minifat_chain(&mini_stream, &minifat, e.start, mini_sector_size, e.size as usize)
            } else {
                cfb_fat_chain(raw, &fat, e.start, sector_size, Some(e.size as usize)).unwrap_or_default()
            }
        };

        let collect_children = |parent_child: u32| -> Vec<usize> {
            let mut res = Vec::new();
            let mut stack = vec![parent_child];
            let mut seen = HashSet::new();
            while let Some(sid) = stack.pop() {
                if sid == NOSTREAM || (sid as usize) >= entries.len() || !seen.insert(sid) {
                    continue;
                }
                let e = &entries[sid as usize];
                res.push(sid as usize);
                if e.left != NOSTREAM {
                    stack.push(e.left);
                }
                if e.right != NOSTREAM {
                    stack.push(e.right);
                }
            }
            res
        };

        let mut root_streams: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut recips = Vec::new();
        let mut attaches = Vec::new();

        for idx in collect_children(entries[0].child) {
            let e = &entries[idx];
            if e.obj_type == 2 {
                root_streams.insert(e.name.clone(), read_entry(e));
            } else if e.obj_type == 1 {
                let mut sub_streams = BTreeMap::new();
                for cidx in collect_children(e.child) {
                    let ce = &entries[cidx];
                    if ce.obj_type == 2 {
                        sub_streams.insert(ce.name.clone(), read_entry(ce));
                    }
                }
                if e.name.starts_with("__recip_version1.0_") {
                    recips.push(sub_streams);
                } else if e.name.starts_with("__attach_version1.0_") {
                    attaches.push((e.name.clone(), sub_streams));
                }
            }
        }

        let str_prop = |map: &BTreeMap<String, Vec<u8>>, hex: &str| -> Option<String> {
            if let Some(b) = map.get(&format!("__substg1.0_{hex}001F")) {
                let s = cfb_utf16(b);
                if !s.is_empty() {
                    return Some(s);
                }
            }
            if let Some(b) = map.get(&format!("__substg1.0_{hex}001E")) {
                let end = b.iter().position(|&x| x == 0).unwrap_or(b.len());
                let s = String::from_utf8_lossy(&b[..end]).to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
            None
        };

        let props_map = parse_props_stream(root_streams.get("__properties_version1.0").map(Vec::as_slice), 32);
        let subject = str_prop(&root_streams, "0037").unwrap_or_else(|| "(No Subject)".to_string());
        let body_text = str_prop(&root_streams, "1000").unwrap_or_default();
        let body_html = root_streams
            .get("__substg1.0_10130102")
            .map(|b| String::from_utf8_lossy(b).to_string())
            .or_else(|| str_prop(&root_streams, "1013"));
        let sender_email = str_prop(&root_streams, "5D01")
            .or_else(|| str_prop(&root_streams, "0C1F"))
            .unwrap_or_else(|| "sender@msg.local".to_string());
        let sender_name = str_prop(&root_streams, "0C1A").unwrap_or_else(|| sender_email.clone());
        let date = props_map
            .get(&0x0E06_0040)
            .and_then(|&ft| cfb_filetime(ft))
            .or_else(|| props_map.get(&0x0039_0040).and_then(|&ft| cfb_filetime(ft)))
            .unwrap_or_else(Utc::now);

        let mut to = Vec::new();
        let mut cc = Vec::new();
        let mut bcc = Vec::new();
        for rmap in &recips {
            let rprops = parse_props_stream(rmap.get("__properties_version1.0").map(Vec::as_slice), 8);
            let rtype = rprops.get(&0x0C15_0003).copied().unwrap_or(1);
            let name = str_prop(rmap, "3001").unwrap_or_default();
            let email = str_prop(rmap, "39FE")
                .or_else(|| str_prop(rmap, "3003"))
                .unwrap_or_default();
            if !email.is_empty() || !name.is_empty() {
                let addr = EmailAddress::new(
                    if name.is_empty() { email.clone() } else { name },
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
        }

        let mut attachments = Vec::new();
        for (sname, amap) in attaches {
            let fname = str_prop(&amap, "3707")
                .or_else(|| str_prop(&amap, "3704"))
                .unwrap_or_else(|| format!("{sname}.bin"));
            let mime = str_prop(&amap, "370E").unwrap_or_else(|| "application/octet-stream".to_string());
            let cid = str_prop(&amap, "3712");
            let data = amap.get("__substg1.0_37010102").cloned().unwrap_or_default();
            attachments.push(Attachment::new(fname, mime, data, cid));
        }

        let (sha256, md5) = EmailMessage::compute_hashes(raw);
        let message_id =
            str_prop(&root_streams, "1035").unwrap_or_else(|| format!("<msg_{}@mail2rust.local>", &sha256[..16]));
        let conversation_id = EmailMessage::generate_conversation_id(&subject, &message_id, None);
        let flags_val = props_map.get(&0x0E07_0003).copied().unwrap_or(1);
        let imp_val = props_map.get(&0x0017_0003).copied().unwrap_or(1);

        Some(EmailMessage {
            id: format!("msg_{}", &sha256[..16]),
            message_id,
            conversation_id,
            folder_path: folder_path.to_vec(),
            subject,
            from: EmailAddress::new(sender_name, sender_email),
            to,
            cc,
            bcc,
            date,
            size_bytes: raw.len(),
            body_text,
            body_html,
            raw_mime: Vec::new(),
            attachments,
            mapi_properties: Vec::new(),
            flags: MessageFlags {
                read: (flags_val & 0x01) != 0,
                ..Default::default()
            },
            importance: match imp_val {
                2 => Importance::High,
                0 => Importance::Low,
                _ => Importance::Normal,
            },
            sha256,
            md5,
            bates_number: None,
            legal_hold: false,
            source_format: "MSG".to_string(),
        })
    }

    /// Scans raw CFB sectors for UTF-16LE text properties matching MAPI stream markers
    fn scan_cfb_streams(raw_bytes: &[u8]) -> HashMap<String, String> {
        let mut map = HashMap::new();
        let markers = [
            ("__substg1.0_0037", "0037"), // PR_SUBJECT
            ("__substg1.0_1000", "1000"), // PR_BODY
            ("__substg1.0_1013", "1013"), // PR_HTML
            ("__substg1.0_0C1F", "0C1F"), // PR_SENDER_EMAIL_ADDRESS
            ("__substg1.0_0C1A", "0C1A"), // PR_SENDER_NAME
            ("__substg1.0_1035", "1035"), // PR_INTERNET_MESSAGE_ID
        ];

        for (tag_prefix, hex_tag) in markers {
            if let Some(text) = Self::find_tagged_stream(raw_bytes, hex_tag) {
                map.insert(tag_prefix.to_string(), text);
            }
        }

        map
    }

    fn find_tagged_stream(raw: &[u8], hex_tag: &str) -> Option<String> {
        // Tag search pattern in CFB directory entries (UTF-16LE)
        let pattern: Vec<u8> = hex_tag.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        let mut idx = 0;
        while idx + pattern.len() <= raw.len() {
            if &raw[idx..idx + pattern.len()] == pattern.as_slice() {
                // If tag is found, scan nearby sector for non-zero Unicode text
                let search_start = idx.saturating_add(64);
                let search_end = (search_start + 4096).min(raw.len());
                if search_start < search_end {
                    let slice = &raw[search_start..search_end];
                    let text = Self::extract_utf16le_string(slice);
                    if text.len() > 1 && !text.chars().all(|c| c.is_control() || c == ' ') {
                        return Some(text);
                    }
                }
            }
            idx += 1;
        }
        None
    }

    fn extract_utf16le_string(slice: &[u8]) -> String {
        let mut u16s = Vec::new();
        for chunk in slice.as_chunks::<2>().0 {
            let val = u16::from_le_bytes([chunk[0], chunk[1]]);
            if val == 0 {
                break;
            }
            if (32..=0xD7FF).contains(&val) {
                u16s.push(val);
            }
        }
        String::from_utf16_lossy(&u16s).trim().to_string()
    }
}

fn cfb_sector(raw: &[u8], sect: u32, sector_size: usize) -> Option<&[u8]> {
    let start = (sect as usize).checked_add(1)?.checked_mul(sector_size)?;
    let end = start.checked_add(sector_size)?;
    raw.get(start..end)
}

fn cfb_fat_chain(raw: &[u8], fat: &[u32], start: u32, sector_size: usize, limit: Option<usize>) -> Option<Vec<u8>> {
    if start >= DIFSECT {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    let mut cur = start;
    let mut seen = HashSet::new();
    while cur < DIFSECT {
        if !seen.insert(cur) {
            return None;
        }
        let sec = cfb_sector(raw, cur, sector_size)?;
        out.extend_from_slice(sec);
        if let Some(lim) = limit
            && out.len() >= lim
        {
            out.truncate(lim);
            return Some(out);
        }
        cur = fat.get(cur as usize).copied().unwrap_or(ENDOFCHAIN);
    }
    if let Some(lim) = limit
        && out.len() > lim
    {
        out.truncate(lim);
    }
    Some(out)
}

fn cfb_minifat_chain(mini_stream: &[u8], minifat: &[u32], start: u32, mini_size: usize, limit: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(limit.min(mini_stream.len()));
    let mut cur = start;
    let mut seen = HashSet::new();
    while cur < DIFSECT && out.len() < limit {
        if !seen.insert(cur) {
            break;
        }
        let s = (cur as usize).saturating_mul(mini_size);
        let e = s.saturating_add(mini_size);
        if e > mini_stream.len() {
            break;
        }
        out.extend_from_slice(&mini_stream[s..e]);
        cur = minifat.get(cur as usize).copied().unwrap_or(ENDOFCHAIN);
    }
    out.truncate(limit);
    out
}

fn cfb_utf16(bytes: &[u8]) -> String {
    let mut units = Vec::with_capacity(bytes.len() / 2);
    for c in bytes.as_chunks::<2>().0 {
        let u = u16::from_le_bytes(*c);
        if u == 0 {
            break;
        }
        units.push(u);
    }
    String::from_utf16_lossy(&units)
}

fn parse_props_stream(stream_opt: Option<&[u8]>, header_len: usize) -> BTreeMap<u32, u64> {
    let mut map = BTreeMap::new();
    let Some(bytes) = stream_opt else {
        return map;
    };
    if bytes.len() < header_len {
        return map;
    }
    for rec in bytes[header_len..].as_chunks::<16>().0 {
        let tag = u32::from_le_bytes(rec[0..4].try_into().unwrap());
        let val = u64::from_le_bytes(rec[8..16].try_into().unwrap());
        map.insert(tag, val);
    }
    map
}

fn cfb_filetime(ft: u64) -> Option<DateTime<Utc>> {
    const EPOCH_DIFF: u64 = 116_444_736_000_000_000;
    const MAX_FT: u64 = 2_650_467_744_000_000_000;
    if !(EPOCH_DIFF..=MAX_FT).contains(&ft) {
        return None;
    }
    let ticks = ft - EPOCH_DIFF;
    let secs = i64::try_from(ticks / 10_000_000).ok()?;
    let nsecs = ((ticks % 10_000_000) * 100) as u32;
    Utc.timestamp_opt(secs, nsecs).single()
}
