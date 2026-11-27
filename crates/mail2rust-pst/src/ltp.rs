use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use chrono::{DateTime, TimeZone, Utc};
use std::collections::HashMap;
use std::io::{self, Write};

// Common MAPI property tags
pub const PR_ACKNOWLEDGEMENT_MODE: u32 = 0x00010003;
pub const PR_IMPORTANCE: u32 = 0x00170003;
pub const PR_MESSAGE_CLASS_W: u32 = 0x001A001F;
pub const PR_MESSAGE_CLASS_A: u32 = 0x001A001E;
pub const PR_SUBJECT_W: u32 = 0x0037001F;
pub const PR_SUBJECT_A: u32 = 0x0037001E;
pub const PR_CLIENT_SUBMIT_TIME: u32 = 0x00390040;
pub const PR_SENT_REPRESENTING_NAME_W: u32 = 0x0042001F;
pub const PR_SENT_REPRESENTING_EMAIL_ADDRESS_W: u32 = 0x0065001F;
pub const PR_RECIPIENT_TYPE: u32 = 0x0C150003;
pub const PR_SENDER_NAME_W: u32 = 0x0C1A001F;
pub const PR_SENDER_NAME_A: u32 = 0x0C1A001E;
pub const PR_SENDER_EMAIL_ADDRESS_W: u32 = 0x0C1F001F;
pub const PR_SENDER_EMAIL_ADDRESS_A: u32 = 0x0C1F001E;
pub const PR_DISPLAY_CC_W: u32 = 0x0E02001F;
pub const PR_DISPLAY_BCC_W: u32 = 0x0E03001F;
pub const PR_DISPLAY_TO_W: u32 = 0x0E04001F;
pub const PR_MESSAGE_DELIVERY_TIME: u32 = 0x0E060040;
pub const PR_MESSAGE_FLAGS: u32 = 0x0E070003;
pub const PR_MESSAGE_SIZE: u32 = 0x0E080003;
pub const PR_HASATTACH: u32 = 0x0E1B000B;
pub const PR_ATTACH_SIZE: u32 = 0x0E200003;
pub const PR_ATTACH_NUM: u32 = 0x0E210003;
pub const PR_ENTRYID: u32 = 0x0FFF0102;
pub const PR_BODY_W: u32 = 0x1000001F;
pub const PR_BODY_A: u32 = 0x1000001E;
pub const PR_BODY_HTML_W: u32 = 0x10130102;
pub const PR_INTERNET_MESSAGE_ID_W: u32 = 0x1035001F;
pub const PR_DISPLAY_NAME_W: u32 = 0x3001001F;
pub const PR_ADDRTYPE_W: u32 = 0x3002001F;
pub const PR_EMAIL_ADDRESS_W: u32 = 0x3003001F;
pub const PR_CREATION_TIME: u32 = 0x30070040;
pub const PR_LAST_MODIFICATION_TIME: u32 = 0x30080040;
pub const PR_SEARCH_KEY: u32 = 0x300B0102;
pub const PR_CONTENT_COUNT: u32 = 0x36020003;
pub const PR_CONTENT_UNREAD: u32 = 0x36030003;
pub const PR_SUBFOLDERS: u32 = 0x360A000B;
pub const PR_ATTACH_DATA_BIN: u32 = 0x37010102;
pub const PR_ATTACH_FILENAME_W: u32 = 0x3704001F;
pub const PR_ATTACH_METHOD: u32 = 0x37050003;
pub const PR_ATTACH_LONG_FILENAME_W: u32 = 0x3707001F;
pub const PR_ATTACH_MIME_TAG_W: u32 = 0x370E001F;
pub const PR_ATTACH_CONTENT_ID_W: u32 = 0x3712001F;
pub const PR_ATTACH_FLAGS: u32 = 0x37140003;
pub const PR_SMTP_ADDRESS_W: u32 = 0x39FE001F;
pub const PR_BUSINESS_TELEPHONE_NUMBER_W: u32 = 0x3A08001F;
pub const PR_HOME_TELEPHONE_NUMBER_W: u32 = 0x3A09001F;
pub const PR_COMPANY_NAME_W: u32 = 0x3A16001F;
pub const PR_TITLE_W: u32 = 0x3A17001F;
pub const PR_DEPARTMENT_NAME_W: u32 = 0x3A18001F;
pub const PR_OFFICE_LOCATION_W: u32 = 0x3A19001F;
pub const PR_MOBILE_TELEPHONE_NUMBER_W: u32 = 0x3A1C001F;
pub const PR_NICK_NAME_W: u32 = 0x6001001F;
pub const PR_SEND_RICH_INFO: u32 = 0x3A40000B;
pub const PR_PST_PASSWORD: u32 = 0x67FF0003;

// Canonical contact named-property IDs (mapped in NID_NAME_TO_ID_MAP or fallback)
pub const PR_CONTACT_EMAIL1_W: u32 = 0x8083001F;
pub const PR_CONTACT_EMAIL2_W: u32 = 0x8093001F;
pub const PR_CONTACT_EMAIL3_W: u32 = 0x80A3001F;
pub const PR_DISTLIST_ONEOFF_MEMBERS: u32 = 0x80541102;
pub const PR_DISTLIST_MEMBERS: u32 = 0x80551102;

pub const MSGFLAG_READ: u32 = 0x00000001;
pub const MSGFLAG_UNSENT: u32 = 0x00000008;
pub const MSGFLAG_FROMME: u32 = 0x00000020;

/// Private (non-MAPI) property tag carrying a JSON `PreservedMessage` blob.
/// This is the authoritative lossless copy for mail2rust -> mail2rust round trips.
pub const PR_MAIL2RUST_PRESERVED: u32 = 0x85420102;
/// Number of 4 KiB preservation chunks that follow.
pub const PR_MAIL2RUST_PRESERVED_COUNT: u32 = 0x85A10003;
/// Maximum useful size of one property context before 16-bit BTH offsets wrap.
pub const MAX_PROPERTY_CONTEXT_BYTES: usize = 60_000;

/// Tag of preservation chunk `index` (chunk 0 is `PR_MAIL2RUST_PRESERVED`).
pub fn mail2rust_chunk_tag(index: u32) -> u32 {
    ((0x8542u32 + index) << 16) | 0x0102
}

/// Builds a minimal conformant `0x7C` table whose only column is
/// `PR_SUBITEM_IDENTIFIER` (`0x67F2`, PT_LONG). libpff uses this value to map a
/// hierarchy/contents row to an NBT descriptor, so this is sufficient for
/// folder and message enumeration.
pub fn build_subitem_table_7c(ids: &[u32]) -> Vec<u8> {
    const HEADER: usize = 12;
    const B5: usize = 8;
    const SEVEN_C: usize = 22;
    const COLUMN: usize = 8;

    let header_end = HEADER + B5; // 0x14
    let records_end = header_end + SEVEN_C + COLUMN; // 0x32
    let values_end = records_end + ids.len() * 8;
    let index_offset = values_end + ids.len() * 4;

    let mut buf = Vec::with_capacity(index_offset + 12);

    // BTH header
    buf.write_u16::<LittleEndian>(index_offset as u16).unwrap();
    buf.write_u8(0xEC).unwrap();
    buf.write_u8(0x7C).unwrap();
    buf.write_u32::<LittleEndian>(0x40).unwrap(); // 0x7C header reference
    buf.write_u32::<LittleEndian>(0).unwrap();

    // b5 header: 4-byte identifiers, 4-byte values, level 0
    buf.write_u8(0xB5).unwrap();
    buf.write_u8(4).unwrap();
    buf.write_u8(4).unwrap();
    buf.write_u8(0).unwrap();
    buf.write_u32::<LittleEndian>(0x60).unwrap(); // record entries reference

    // 0x7C header: one column, 4-byte value rows
    buf.write_u8(0x7C).unwrap();
    buf.write_u8(1).unwrap(); // number of column definitions
    for _ in 0..4 {
        buf.write_u16::<LittleEndian>(4).unwrap(); // end offsets
    }
    buf.write_u32::<LittleEndian>(0x20).unwrap(); // b5 reference
    buf.write_u32::<LittleEndian>(0x80).unwrap(); // values array reference
    buf.write_u32::<LittleEndian>(0).unwrap();

    // Column definition: PR_SUBITEM_IDENTIFIER, PT_LONG, offset 0, size 4
    buf.write_u16::<LittleEndian>(0x0003).unwrap();
    buf.write_u16::<LittleEndian>(0x67F2).unwrap();
    buf.write_u16::<LittleEndian>(0).unwrap();
    buf.write_u8(4).unwrap();
    buf.write_u8(0).unwrap();

    // Record entries: one per row
    for (i, id) in ids.iter().enumerate() {
        buf.write_u32::<LittleEndian>(*id).unwrap();
        buf.write_u32::<LittleEndian>(i as u32).unwrap();
    }

    // Value rows
    for id in ids {
        buf.write_u32::<LittleEndian>(*id).unwrap();
    }
    debug_assert_eq!(buf.len(), index_offset);

    // BTH index: b5, 0x7C header, record entries, values array
    let offsets = [
        HEADER as u16,
        header_end as u16,
        records_end as u16,
        values_end as u16,
        index_offset as u16,
    ];
    buf.write_u16::<LittleEndian>((offsets.len() - 1) as u16).unwrap();
    buf.write_u16::<LittleEndian>(0).unwrap();
    for offset in offsets {
        buf.write_u16::<LittleEndian>(offset).unwrap();
    }
    buf
}

const FILETIME_UNIX_EPOCH_TICKS: u64 = 116_444_736_000_000_000;

/// Converts a Windows 64-bit FILETIME (100ns intervals since 1601-01-01) to `DateTime<Utc>`.
pub fn filetime_to_datetime(ft: u64) -> Option<DateTime<Utc>> {
    if ft < FILETIME_UNIX_EPOCH_TICKS {
        return None;
    }
    let ticks_since_unix = ft - FILETIME_UNIX_EPOCH_TICKS;
    let secs = (ticks_since_unix / 10_000_000) as i64;
    let nsecs = ((ticks_since_unix % 10_000_000) * 100) as u32;
    Utc.timestamp_opt(secs, nsecs).single()
}

/// Converts a `DateTime<Utc>` to a Windows 64-bit FILETIME.
pub fn datetime_to_filetime(dt: DateTime<Utc>) -> u64 {
    let secs = dt.timestamp();
    if secs < 0 {
        return FILETIME_UNIX_EPOCH_TICKS;
    }
    let subsec_ticks = (dt.timestamp_subsec_nanos() / 100) as u64;
    FILETIME_UNIX_EPOCH_TICKS
        .saturating_add((secs as u64).saturating_mul(10_000_000))
        .saturating_add(subsec_ticks)
}

#[derive(Debug, Clone, Default)]
pub struct ParsedPropertyContext {
    pub properties: HashMap<u32, Vec<u8>>,
}

impl ParsedPropertyContext {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        Self::parse_with_subnodes(bytes, &|_| None)
    }

    pub fn parse_ansi(bytes: &[u8]) -> Option<Self> {
        Self::parse_with_subnodes(bytes, &|_| None)
    }

    pub fn parse_with_subnodes(bytes: &[u8], subnode_resolver: &dyn Fn(u32) -> Option<Vec<u8>>) -> Option<Self> {
        // 1. Compact Property Context format used by mail2rust PstWriter (bType = 0xBC)
        if bytes.len() >= 6 && bytes[0] == 0xBC && bytes[1] == 0x02 && bytes[2] == 0x06 {
            return Self::parse_compact_pc(bytes);
        }

        // 2. Full MS-PST Heap-on-Node (HNHDR) with bSig = 0xEC, bClientSig = 0xBC
        if bytes.len() >= 12 && bytes[2] == 0xEC && bytes[3] == 0xBC {
            return Self::parse_hn_pc(bytes, subnode_resolver);
        }

        None
    }

    fn parse_compact_pc(bytes: &[u8]) -> Option<Self> {
        let entries_count = (&bytes[4..6]).read_u16::<LittleEndian>().ok()? as usize;
        let header_end = 6 + entries_count.checked_mul(8)?;
        if bytes.len() < header_end {
            return None;
        }

        // Collect all variable offsets first so non-string payloads are bounded instead of
        // bleeding to the end of the entire block.
        let mut raw_entries = Vec::with_capacity(entries_count);
        let mut var_offsets = Vec::new();
        for i in 0..entries_count {
            let offset = 6 + i * 8;
            let prop_id = (&bytes[offset..offset + 2]).read_u16::<LittleEndian>().ok()?;
            let prop_type = (&bytes[offset + 2..offset + 4]).read_u16::<LittleEndian>().ok()?;
            let val_or_offset = (&bytes[offset + 4..offset + 8]).read_u32::<LittleEndian>().ok()?;
            let is_inline = prop_type == 0x0003 || prop_type == 0x000B || prop_type == 0x0002;
            if !is_inline {
                var_offsets.push(val_or_offset as usize);
            }
            raw_entries.push((prop_id, prop_type, val_or_offset, is_inline));
        }
        var_offsets.sort_unstable();
        var_offsets.dedup();

        let mut properties = HashMap::new();

        for (prop_id, prop_type, val_or_offset, is_inline) in raw_entries {
            let tag = ((prop_id as u32) << 16) | (prop_type as u32);

            if is_inline {
                properties.insert(tag, val_or_offset.to_le_bytes().to_vec());
            } else {
                let data_off = val_or_offset as usize;
                if data_off < bytes.len() {
                    let next_off = var_offsets
                        .iter()
                        .copied()
                        .find(|&o| o > data_off && o <= bytes.len())
                        .unwrap_or(bytes.len());

                    if prop_type == 0x001F {
                        let mut end = data_off;
                        while end + 1 < next_off && (bytes[end] != 0 || bytes[end + 1] != 0) {
                            end += 2;
                        }
                        properties.insert(tag, bytes[data_off..end].to_vec());
                    } else if prop_type == 0x001E {
                        let mut end = data_off;
                        while end < next_off && bytes[end] != 0 {
                            end += 1;
                        }
                        properties.insert(tag, bytes[data_off..end].to_vec());
                    } else if prop_type == 0x0040 || prop_type == 0x0014 || prop_type == 0x0005 {
                        let end = (data_off + 8).min(next_off);
                        properties.insert(tag, bytes[data_off..end].to_vec());
                    } else {
                        properties.insert(tag, bytes[data_off..next_off].to_vec());
                    }
                }
            }
        }

        Some(Self { properties })
    }

    fn parse_hn_pc(bytes: &[u8], subnode_resolver: &dyn Fn(u32) -> Option<Vec<u8>>) -> Option<Self> {
        let heap = HnPage::parse(bytes)?;
        let hid_user_root = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
        let bth_hdr = heap.get_hid(hid_user_root)?;
        if bth_hdr.len() < 8 || bth_hdr[0] != 0xB5 {
            return None;
        }
        let cb_key = bth_hdr[1];
        let cb_ent = bth_hdr[2];
        let b_idx_levels = bth_hdr[3];
        let hid_root = u32::from_le_bytes(bth_hdr[4..8].try_into().ok()?);
        if cb_key != 2 || cb_ent != 6 || hid_root == 0 {
            return Some(Self::default());
        }

        let mut records = Vec::new();
        collect_bth_records(&heap, hid_root, b_idx_levels, 8, &mut records, 0);

        let mut properties = HashMap::new();
        for rec in records {
            if rec.len() < 8 {
                continue;
            }
            let prop_id = u16::from_le_bytes([rec[0], rec[1]]);
            let prop_type = u16::from_le_bytes([rec[2], rec[3]]);
            let hnid = u32::from_le_bytes([rec[4], rec[5], rec[6], rec[7]]);
            let tag = ((prop_id as u32) << 16) | (prop_type as u32);

            let val_bytes = if matches!(prop_type, 0x0002 | 0x0003 | 0x000B | 0x0004) {
                hnid.to_le_bytes().to_vec()
            } else if hnid == 0 {
                Vec::new()
            } else if (hnid & 0x1F) == 0 {
                let mut raw = heap.get_hid(hnid).map(|s| s.to_vec()).unwrap_or_default();
                trim_prop_bytes(prop_type, &mut raw);
                raw
            } else {
                let mut raw = subnode_resolver(hnid).unwrap_or_default();
                trim_prop_bytes(prop_type, &mut raw);
                raw
            };
            properties.insert(tag, val_bytes);
        }

        Some(Self { properties })
    }

    #[allow(clippy::chunks_exact_to_as_chunks)]
    pub fn get_string_w(&self, tag: u32) -> Option<String> {
        let bytes = self.properties.get(&tag)?;
        if bytes.len() < 2 {
            return None;
        }
        // Genuine Unicode PST string properties may begin with non-whitespace control bytes.
        let skip = bytes
            .chunks_exact(2)
            .take_while(|c| {
                let u = u16::from_le_bytes([c[0], c[1]]);
                u != 0 && u < 0x20 && !matches!(u, 0x09 | 0x0A | 0x0D)
            })
            .count()
            * 2;
        let bytes = &bytes[skip..];
        let u16s: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .take_while(|&u| u != 0)
            .collect();
        Some(String::from_utf16_lossy(&u16s))
    }

    pub fn get_string_a(&self, tag: u32) -> Option<String> {
        let bytes = self.properties.get(&tag)?;
        let skip = bytes
            .iter()
            .take_while(|&&b| b != 0 && b < 0x20 && !matches!(b, b'\t' | b'\n' | b'\r'))
            .count();
        let bytes = &bytes[skip..];
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        Some(String::from_utf8_lossy(&bytes[..end]).to_string())
    }

    /// Reads either `PT_UNICODE` (`0x001F`) or `PT_STRING8` (`0x001E`) for the given property ID.
    pub fn get_string_any(&self, prop_id: u16) -> Option<String> {
        let tag_w = ((prop_id as u32) << 16) | 0x001F;
        let tag_a = ((prop_id as u32) << 16) | 0x001E;
        self.get_string_w(tag_w).or_else(|| self.get_string_a(tag_a))
    }

    pub fn get_u32(&self, tag: u32) -> Option<u32> {
        let bytes = self.properties.get(&tag)?;
        if bytes.len() >= 4 {
            Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        } else {
            None
        }
    }

    pub fn get_u64(&self, tag: u32) -> Option<u64> {
        let bytes = self.properties.get(&tag)?;
        if bytes.len() >= 8 {
            Some(u64::from_le_bytes(bytes[0..8].try_into().ok()?))
        } else {
            None
        }
    }

    pub fn get_systime(&self, tag: u32) -> Option<DateTime<Utc>> {
        self.get_u64(tag).and_then(filetime_to_datetime)
    }

    pub fn get_bool(&self, tag: u32) -> Option<bool> {
        let bytes = self.properties.get(&tag)?;
        bytes.first().map(|b| *b != 0)
    }

    pub fn get_binary(&self, tag: u32) -> Option<Vec<u8>> {
        self.properties.get(&tag).cloned()
    }

    /// Decodes `PT_MV_BINARY` (`0x1102`) property payload into constituent binary slices.
    pub fn get_mv_binary(&self, tag: u32) -> Option<Vec<Vec<u8>>> {
        let bytes = self.properties.get(&tag)?;
        if bytes.len() < 4 {
            return None;
        }
        let count = u32::from_le_bytes(bytes[0..4].try_into().ok()?) as usize;
        if count == 0 || count > 65_536 {
            return Some(Vec::new());
        }
        // MS-PST MV payload: ulCount, followed by `count` u32 offsets relative to start of payload
        let table_end = 4usize.checked_add(count.checked_mul(4)?)?;
        if bytes.len() >= table_end {
            let mut offsets = Vec::with_capacity(count);
            for i in 0..count {
                let off = u32::from_le_bytes(bytes[4 + i * 4..8 + i * 4].try_into().ok()?) as usize;
                offsets.push(off);
            }
            if offsets.first().copied().unwrap_or(0) >= table_end && offsets.iter().all(|&o| o <= bytes.len()) {
                let mut out = Vec::with_capacity(count);
                for i in 0..count {
                    let start = offsets[i];
                    let end = offsets.get(i + 1).copied().unwrap_or(bytes.len());
                    if start <= end && end <= bytes.len() {
                        out.push(bytes[start..end].to_vec());
                    }
                }
                return Some(out);
            }
        }
        // Length-prefixed fallback encoding: count * (len: u32, data)
        let mut cursor = 4usize;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            if cursor + 4 > bytes.len() {
                break;
            }
            let len = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().ok()?) as usize;
            cursor += 4;
            if cursor + len > bytes.len() {
                break;
            }
            out.push(bytes[cursor..cursor + len].to_vec());
            cursor += len;
        }
        Some(out)
    }
}

fn trim_prop_bytes(prop_type: u16, raw: &mut Vec<u8>) {
    if prop_type == 0x001F {
        let mut end = 0;
        while end + 1 < raw.len() && (raw[end] != 0 || raw[end + 1] != 0) {
            end += 2;
        }
        raw.truncate(end);
    } else if prop_type == 0x001E
        && let Some(pos) = raw.iter().position(|&b| b == 0)
    {
        raw.truncate(pos);
    }
}

/// Minimal MS-PST Heap-on-Node (`HNHDR` + `HNPAGEMAP`) page reader.
pub struct HnPage<'a> {
    data: &'a [u8],
    alloc_offsets: Vec<u16>,
}

impl<'a> HnPage<'a> {
    pub fn parse(data: &'a [u8]) -> Option<Self> {
        if data.len() < 12 || data[2] != 0xEC {
            return None;
        }
        let ib_hnpm = u16::from_le_bytes([data[0], data[1]]) as usize;
        if ib_hnpm + 4 > data.len() {
            return None;
        }
        let c_alloc = u16::from_le_bytes([data[ib_hnpm], data[ib_hnpm + 1]]) as usize;
        let offsets_start = ib_hnpm + 4;
        let offsets_end = offsets_start + (c_alloc + 1).checked_mul(2)?;
        if offsets_end > data.len() {
            return None;
        }
        let mut alloc_offsets = Vec::with_capacity(c_alloc + 1);
        for i in 0..=c_alloc {
            let p = offsets_start + i * 2;
            alloc_offsets.push(u16::from_le_bytes([data[p], data[p + 1]]));
        }
        Some(Self { data, alloc_offsets })
    }

    pub fn get_hid(&self, hid: u32) -> Option<&'a [u8]> {
        if hid == 0 || (hid & 0x1F) != 0 {
            return None;
        }
        let idx = ((hid >> 5) & 0x7FF) as usize;
        if idx == 0 || idx >= self.alloc_offsets.len() {
            return None;
        }
        let start = self.alloc_offsets[idx - 1] as usize;
        let end = self.alloc_offsets[idx] as usize;
        if start <= end && end <= self.data.len() {
            Some(&self.data[start..end])
        } else {
            None
        }
    }
}

fn collect_bth_records<'a>(
    heap: &HnPage<'a>,
    hid: u32,
    level: u8,
    rec_size: usize,
    out: &mut Vec<&'a [u8]>,
    depth: usize,
) {
    if depth > 8 || hid == 0 {
        return;
    }
    let Some(buf) = heap.get_hid(hid) else {
        return;
    };
    if level == 0 {
        for chunk in buf.chunks_exact(rec_size) {
            out.push(chunk);
        }
    } else {
        let idx_entry_size = rec_size.saturating_sub(4).max(4) + 4;
        // BTHIDX: key (cbKey) + hidNextLevel (4)
        let stride = if buf.len().is_multiple_of(6) { 6 } else { idx_entry_size };
        if stride >= 6 {
            for chunk in buf.chunks_exact(stride) {
                let child_hid = u32::from_le_bytes(chunk[stride - 4..stride].try_into().unwrap_or([0; 4]));
                collect_bth_records(heap, child_hid, level - 1, rec_size, out, depth + 1);
            }
        }
    }
}

/// Decoded row from a Table Context (`0x7C`), mapping MAPI tags to raw bytes.
#[derive(Debug, Clone, Default)]
pub struct TableContextRow {
    pub row_id: u32,
    pub properties: HashMap<u32, Vec<u8>>,
}

impl TableContextRow {
    pub fn get_u32(&self, tag: u32) -> Option<u32> {
        let b = self.properties.get(&tag)?;
        if b.len() >= 4 {
            Some(u32::from_le_bytes(b[0..4].try_into().ok()?))
        } else {
            None
        }
    }

    #[allow(clippy::chunks_exact_to_as_chunks)]
    pub fn get_string_w(&self, tag: u32) -> Option<String> {
        let bytes = self.properties.get(&tag)?;
        if bytes.len() < 2 {
            return None;
        }
        let u16s: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&u| u != 0)
            .collect();
        Some(String::from_utf16_lossy(&u16s))
    }

    pub fn get_string_a(&self, tag: u32) -> Option<String> {
        let bytes = self.properties.get(&tag)?;
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        Some(String::from_utf8_lossy(&bytes[..end]).to_string())
    }

    pub fn get_string_any(&self, prop_id: u16) -> Option<String> {
        let tag_w = ((prop_id as u32) << 16) | 0x001F;
        let tag_a = ((prop_id as u32) << 16) | 0x001E;
        self.get_string_w(tag_w).or_else(|| self.get_string_a(tag_a))
    }
}

/// Table Context (`0x7C`) parser and serializer for attachment & recipient tables.
#[derive(Debug, Clone, Default)]
pub struct TableContext {
    pub rows: Vec<TableContextRow>,
}

impl TableContext {
    /// Parses either compact `0x7C` TC (written by `PstWriter`) or MS-PST `HNHDR`/`0x7C` TC.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() >= 4 && bytes[0] == 0x7C && bytes[1] == 0x01 {
            return Self::parse_compact_tc(bytes);
        }
        if bytes.len() >= 12 && bytes[2] == 0xEC && bytes[3] == 0x7C {
            return Self::parse_hn_tc(bytes);
        }
        None
    }

    /// Compact `0x7C` TC wire format:
    /// `[0x7C, 0x01, row_count: u16 LE]`, then each row: `[row_id: u32, prop_count: u16]`,
    /// then `prop_count` x `[tag: u32, len: u32, bytes[len]]`.
    pub fn serialize_compact(&self) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        buf.write_u8(0x7C)?;
        buf.write_u8(0x01)?;
        buf.write_u16::<LittleEndian>(self.rows.len() as u16)?;
        for row in &self.rows {
            buf.write_u32::<LittleEndian>(row.row_id)?;
            buf.write_u16::<LittleEndian>(row.properties.len() as u16)?;
            let mut tags: Vec<u32> = row.properties.keys().copied().collect();
            tags.sort_unstable();
            for tag in tags {
                let val = &row.properties[&tag];
                buf.write_u32::<LittleEndian>(tag)?;
                buf.write_u32::<LittleEndian>(val.len() as u32)?;
                buf.write_all(val)?;
            }
        }
        Ok(buf)
    }

    fn parse_compact_tc(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 4 {
            return None;
        }
        let row_count = u16::from_le_bytes([bytes[2], bytes[3]]) as usize;
        let mut cur = 4usize;
        let mut rows = Vec::with_capacity(row_count.min(4096));
        for _ in 0..row_count {
            if cur + 6 > bytes.len() {
                break;
            }
            let row_id = u32::from_le_bytes(bytes[cur..cur + 4].try_into().ok()?);
            let prop_count = u16::from_le_bytes(bytes[cur + 4..cur + 6].try_into().ok()?) as usize;
            cur += 6;
            let mut properties = HashMap::new();
            for _ in 0..prop_count {
                if cur + 8 > bytes.len() {
                    break;
                }
                let tag = u32::from_le_bytes(bytes[cur..cur + 4].try_into().ok()?);
                let len = u32::from_le_bytes(bytes[cur + 4..cur + 8].try_into().ok()?) as usize;
                cur += 8;
                if cur + len > bytes.len() {
                    break;
                }
                properties.insert(tag, bytes[cur..cur + len].to_vec());
                cur += len;
            }
            rows.push(TableContextRow { row_id, properties });
        }
        Some(Self { rows })
    }

    fn parse_hn_tc(bytes: &[u8]) -> Option<Self> {
        let heap = HnPage::parse(bytes)?;
        let hid_user_root = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
        let tcinfo = heap.get_hid(hid_user_root)?;
        if tcinfo.len() < 22 || tcinfo[0] != 0x7C {
            return None;
        }
        let c_cols = tcinfo[1] as usize;
        let rgib_tce = [
            u16::from_le_bytes([tcinfo[2], tcinfo[3]]) as usize,
            u16::from_le_bytes([tcinfo[4], tcinfo[5]]) as usize,
            u16::from_le_bytes([tcinfo[6], tcinfo[7]]) as usize,
            u16::from_le_bytes([tcinfo[8], tcinfo[9]]) as usize,
        ];
        let hid_row_index = u32::from_le_bytes(tcinfo[10..14].try_into().ok()?);
        let hnid_rows = u32::from_le_bytes(tcinfo[14..18].try_into().ok()?);

        // Column descriptors start at tcinfo[22..], 8 bytes each: tag(u32), ibData(u16), cbData(u8), iBit(u8)
        let mut cols = Vec::with_capacity(c_cols);
        for i in 0..c_cols {
            let off = 22 + i * 8;
            if off + 8 > tcinfo.len() {
                break;
            }
            let tag = u32::from_le_bytes(tcinfo[off..off + 4].try_into().ok()?);
            let ib_data = u16::from_le_bytes([tcinfo[off + 4], tcinfo[off + 5]]) as usize;
            let cb_data = tcinfo[off + 6] as usize;
            cols.push((tag, ib_data, cb_data));
        }

        let row_stride = rgib_tce[3];
        if row_stride == 0 {
            return Some(Self::default());
        }

        let rows_buf = if (hnid_rows & 0x1F) == 0 && hnid_rows != 0 {
            heap.get_hid(hnid_rows).unwrap_or(&[])
        } else {
            &[]
        };

        // Optional row ID map from BTH
        let mut row_ids = Vec::new();
        if hid_row_index != 0
            && let Some(bth_hdr) = heap.get_hid(hid_row_index)
            && bth_hdr.len() >= 8
            && bth_hdr[0] == 0xB5
        {
            let b_idx_levels = bth_hdr[3];
            let hid_root = u32::from_le_bytes(bth_hdr[4..8].try_into().unwrap_or([0; 4]));
            let mut recs = Vec::new();
            collect_bth_records(&heap, hid_root, b_idx_levels, 8, &mut recs, 0);
            for r in recs {
                if r.len() >= 8 {
                    let dw_row_id = u32::from_le_bytes(r[0..4].try_into().unwrap_or([0; 4]));
                    let dw_row_idx = u32::from_le_bytes(r[4..8].try_into().unwrap_or([0; 4]));
                    row_ids.push((dw_row_idx as usize, dw_row_id));
                }
            }
        }

        let mut rows = Vec::new();
        for (row_idx, row_bytes) in rows_buf.chunks_exact(row_stride).enumerate() {
            let mut properties = HashMap::new();
            for &(tag, ib_data, cb_data) in &cols {
                if ib_data + cb_data > row_bytes.len() {
                    continue;
                }
                let prop_type = (tag & 0xFFFF) as u16;
                let slice = &row_bytes[ib_data..ib_data + cb_data];
                if matches!(prop_type, 0x001F | 0x001E | 0x0102 | 0x101F | 0x1102) && cb_data == 4 {
                    let hnid = u32::from_le_bytes(slice.try_into().unwrap_or([0; 4]));
                    if hnid != 0
                        && (hnid & 0x1F) == 0
                        && let Some(val) = heap.get_hid(hnid)
                    {
                        let mut v = val.to_vec();
                        trim_prop_bytes(prop_type, &mut v);
                        properties.insert(tag, v);
                    }
                } else {
                    properties.insert(tag, slice.to_vec());
                }
            }
            let row_id = row_ids
                .iter()
                .find(|(idx, _)| *idx == row_idx)
                .map(|(_, id)| *id)
                .unwrap_or(row_idx as u32);
            rows.push(TableContextRow { row_id, properties });
        }

        Some(Self { rows })
    }
}

pub struct PropertyContextEntry {
    pub tag: u32,
    pub data: Vec<u8>,
}

pub struct PropertyContext {
    pub entries: Vec<PropertyContextEntry>,
}

impl PropertyContext {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn add_string_w(&mut self, tag: u32, val: &str) {
        let mut u16_bytes = Vec::new();
        for u in val.encode_utf16() {
            u16_bytes.extend_from_slice(&u.to_le_bytes());
        }
        u16_bytes.extend_from_slice(&[0, 0]); // null terminate
        self.entries.push(PropertyContextEntry { tag, data: u16_bytes });
    }

    pub fn add_string_a(&mut self, tag: u32, val: &str) {
        let mut bytes = val.as_bytes().to_vec();
        bytes.push(0);
        self.entries.push(PropertyContextEntry { tag, data: bytes });
    }

    pub fn add_u32(&mut self, tag: u32, val: u32) {
        self.entries.push(PropertyContextEntry {
            tag,
            data: val.to_le_bytes().to_vec(),
        });
    }

    pub fn add_u64(&mut self, tag: u32, val: u64) {
        self.entries.push(PropertyContextEntry {
            tag,
            data: val.to_le_bytes().to_vec(),
        });
    }

    pub fn add_systime(&mut self, tag: u32, dt: DateTime<Utc>) {
        self.add_u64(tag, datetime_to_filetime(dt));
    }

    pub fn add_bool(&mut self, tag: u32, val: bool) {
        let b = if val { 1u8 } else { 0u8 };
        self.entries.push(PropertyContextEntry { tag, data: vec![b] });
    }

    pub fn add_binary(&mut self, tag: u32, bytes: &[u8]) {
        self.entries.push(PropertyContextEntry {
            tag,
            data: bytes.to_vec(),
        });
    }

    pub fn add_mv_binary(&mut self, tag: u32, items: &[Vec<u8>]) {
        let count = items.len() as u32;
        let header_size = 4 + items.len() * 4;
        let mut payload = Vec::with_capacity(header_size + items.iter().map(|i| i.len()).sum::<usize>());
        payload.extend_from_slice(&count.to_le_bytes());
        let mut cur_off = header_size as u32;
        for item in items {
            payload.extend_from_slice(&cur_off.to_le_bytes());
            cur_off += item.len() as u32;
        }
        for item in items {
            payload.extend_from_slice(item);
        }
        self.entries.push(PropertyContextEntry { tag, data: payload });
    }

    /// Serializes the property context into an MS-PST 0xBC BTH/HN table.
    pub fn serialize(&self) -> io::Result<Vec<u8>> {
        const HEADER: usize = 12;
        const B5: usize = 8;
        let entries_start = HEADER + B5;
        let entries_bytes = self.entries.len() * 8;
        let heap_start = entries_start + entries_bytes;

        let mut heap = Vec::new();
        let mut encoded: Vec<(u32, [u8; 4])> = Vec::with_capacity(self.entries.len());
        let mut heap_ranges: Vec<(usize, usize)> = Vec::new();

        for entry in &self.entries {
            let prop_type = (entry.tag & 0xFFFF) as u16;
            let inline = matches!(prop_type, 0x0003 | 0x000B | 0x0002) && entry.data.len() <= 4;
            let mut value = [0u8; 4];
            if inline {
                value[..entry.data.len()].copy_from_slice(&entry.data);
            } else {
                let start = heap_start + heap.len();
                let reference = ((3 + heap_ranges.len()) * 32) as u32;
                value = reference.to_le_bytes();
                heap.extend_from_slice(&entry.data);
                heap_ranges.push((start, start + entry.data.len()));
            }
            encoded.push((entry.tag, value));
        }

        let index_offset = heap_start + heap.len();
        if index_offset > u16::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "property context exceeds 64 KiB HNHDR limit",
            ));
        }
        let mut buf = Vec::with_capacity(index_offset + 10);

        // BTH / HNHDR header
        buf.write_u16::<LittleEndian>(index_offset as u16)?;
        buf.write_u8(0xEC)?;
        buf.write_u8(0xBC)?;
        buf.write_u32::<LittleEndian>(0x20)?; // reference to b5 header (index 0)
        buf.write_u32::<LittleEndian>(0)?; // padding

        // b5 table header
        buf.write_u8(0xB5)?;
        buf.write_u8(0x02)?; // cbKey
        buf.write_u8(0x06)?; // cbValue
        buf.write_u8(0x00)?; // level
        buf.write_u32::<LittleEndian>(0x40)?; // reference to record entries (index 1)

        // Property descriptors
        for (tag, value) in encoded {
            buf.write_u16::<LittleEndian>((tag >> 16) as u16)?;
            buf.write_u16::<LittleEndian>((tag & 0xFFFF) as u16)?;
            buf.write_all(&value)?;
        }
        buf.extend_from_slice(&heap);
        debug_assert_eq!(buf.len(), index_offset);

        // HNPAGEMAP / BTH index: N values require N+1 offsets.
        let mut offsets: Vec<u16> = vec![HEADER as u16, entries_start as u16];
        if heap_ranges.is_empty() {
            offsets.push(index_offset as u16);
        } else {
            offsets.push(heap_ranges[0].0 as u16);
            for (_, end) in &heap_ranges {
                offsets.push(*end as u16);
            }
        }
        buf.write_u16::<LittleEndian>((offsets.len() - 1) as u16)?;
        buf.write_u16::<LittleEndian>(0)?; // cFree
        for offset in offsets {
            buf.write_u16::<LittleEndian>(offset)?;
        }
        Ok(buf)
    }
}

impl Default for PropertyContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subitem_table_7c_has_valid_structure() {
        let data = build_subitem_table_7c(&[0x8004, 0x8024]);
        assert_eq!(data[2], 0xEC);
        assert_eq!(data[3], 0x7C);
        assert_eq!(u32::from_le_bytes(data[4..8].try_into().unwrap()), 0x40);
        let index_offset = u16::from_le_bytes([data[0], data[1]]) as usize;
        let n = u16::from_le_bytes([data[index_offset], data[index_offset + 1]]) as usize;
        let offsets: Vec<usize> = (0..=n)
            .map(|i| {
                u16::from_le_bytes(
                    data[index_offset + 4 + i * 2..index_offset + 6 + i * 2]
                        .try_into()
                        .unwrap(),
                ) as usize
            })
            .collect();
        assert_eq!(offsets[0], 12);
        assert_eq!(offsets[1], 20);
        let record_ids: Vec<u32> = (0..2)
            .map(|i| u32::from_le_bytes(data[offsets[2] + i * 8..offsets[2] + i * 8 + 4].try_into().unwrap()))
            .collect();
        let value_ids: Vec<u32> = (0..2)
            .map(|i| u32::from_le_bytes(data[offsets[3] + i * 4..offsets[3] + i * 4 + 4].try_into().unwrap()))
            .collect();
        assert_eq!(record_ids, vec![0x8004, 0x8024]);
        assert_eq!(value_ids, record_ids);
    }

    #[test]
    fn bth_property_context_roundtrips_multiple_binaries() {
        let mut pc = PropertyContext::new();
        pc.add_string_w(PR_SUBJECT_W, "hello");
        pc.add_u32(PR_IMPORTANCE, 2);
        pc.add_binary(0x10130102, b"<p>html</p>");
        pc.add_binary(PR_MAIL2RUST_PRESERVED, br#"{"raw_mime":"abc"}"#);
        let bytes = pc.serialize().unwrap();
        assert_eq!(bytes[2], 0xEC);
        assert_eq!(bytes[3], 0xBC);
        let parsed = ParsedPropertyContext::parse(&bytes).expect("parse");
        assert_eq!(parsed.get_string_w(PR_SUBJECT_W).as_deref(), Some("hello"));
        assert_eq!(parsed.get_u32(PR_IMPORTANCE), Some(2));
        assert_eq!(parsed.get_binary(0x10130102).unwrap(), b"<p>html</p>");
        assert_eq!(
            parsed.get_binary(PR_MAIL2RUST_PRESERVED).unwrap(),
            br#"{"raw_mime":"abc"}"#
        );
    }
}
