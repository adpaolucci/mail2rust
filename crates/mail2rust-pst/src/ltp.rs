use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::collections::HashMap;
use std::io::{self, Write};

// Common MAPI property tags
pub const PR_ACKNOWLEDGEMENT_MODE: u32 = 0x00010003;
pub const PR_IMPORTANCE: u32 = 0x00170003;
pub const PR_MESSAGE_CLASS_W: u32 = 0x001A001F;
pub const PR_SUBJECT_W: u32 = 0x0037001F;
pub const PR_CLIENT_SUBMIT_TIME: u32 = 0x00390040;
pub const PR_SENDER_NAME_W: u32 = 0x0C1A001F;
pub const PR_SENDER_EMAIL_ADDRESS_W: u32 = 0x0C1F001F;
pub const PR_MESSAGE_DELIVERY_TIME: u32 = 0x0E060040;
pub const PR_MESSAGE_FLAGS: u32 = 0x0E070003;
pub const PR_MESSAGE_SIZE: u32 = 0x0E080003;
pub const PR_HASATTACH: u32 = 0x0E1B000B;
pub const PR_BODY_W: u32 = 0x1000001F;
pub const PR_BODY_HTML_W: u32 = 0x10130102;
pub const PR_INTERNET_MESSAGE_ID_W: u32 = 0x1035001F;
pub const PR_DISPLAY_NAME_W: u32 = 0x3001001F;
pub const PR_CONTENT_COUNT: u32 = 0x36020003;
pub const PR_CONTENT_UNREAD: u32 = 0x36030003;
pub const PR_SUBFOLDERS: u32 = 0x360A000B;

pub const MSGFLAG_READ: u32 = 0x00000001;
pub const MSGFLAG_UNSENT: u32 = 0x00000008;

#[derive(Debug, Clone, Default)]
pub struct ParsedPropertyContext {
    pub properties: HashMap<u32, Vec<u8>>,
}

impl ParsedPropertyContext {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 6 || bytes[0] != 0xBC {
            return None;
        }

        let entries_count = (&bytes[4..6]).read_u16::<LittleEndian>().ok()? as usize;
        let header_end = 6 + entries_count * 8;
        if bytes.len() < header_end {
            return None;
        }

        let mut properties = HashMap::new();

        for i in 0..entries_count {
            let offset = 6 + i * 8;
            let prop_id = (&bytes[offset..offset + 2]).read_u16::<LittleEndian>().ok()?;
            let prop_type = (&bytes[offset + 2..offset + 4]).read_u16::<LittleEndian>().ok()?;
            let tag = ((prop_id as u32) << 16) | (prop_type as u32);
            let val_or_offset = (&bytes[offset + 4..offset + 8]).read_u32::<LittleEndian>().ok()?;

            if prop_type == 0x0003 || prop_type == 0x000B || prop_type == 0x0002 {
                properties.insert(tag, val_or_offset.to_le_bytes().to_vec());
            } else {
                let data_off = val_or_offset as usize;
                if data_off < bytes.len() {
                    if prop_type == 0x001F {
                        // UTF-16LE string: find double null
                        let mut end = data_off;
                        while end + 1 < bytes.len() && (bytes[end] != 0 || bytes[end + 1] != 0) {
                            end += 2;
                        }
                        properties.insert(tag, bytes[data_off..end].to_vec());
                    } else if prop_type == 0x001E {
                        // ANSI string: find single null
                        let mut end = data_off;
                        while end < bytes.len() && bytes[end] != 0 {
                            end += 1;
                        }
                        properties.insert(tag, bytes[data_off..end].to_vec());
                    } else {
                        // Binary payload
                        properties.insert(tag, bytes[data_off..].to_vec());
                    }
                }
            }
        }

        Some(Self { properties })
    }

    #[allow(clippy::chunks_exact_to_as_chunks)]
    pub fn get_string_w(&self, tag: u32) -> Option<String> {
        let bytes = self.properties.get(&tag)?;
        if bytes.len() < 2 {
            return None;
        }
        let u16s: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        Some(String::from_utf16_lossy(&u16s))
    }

    pub fn get_string_a(&self, tag: u32) -> Option<String> {
        let bytes = self.properties.get(&tag)?;
        Some(String::from_utf8_lossy(bytes).to_string())
    }

    pub fn get_u32(&self, tag: u32) -> Option<u32> {
        let bytes = self.properties.get(&tag)?;
        if bytes.len() >= 4 {
            Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        } else {
            None
        }
    }

    pub fn get_bool(&self, tag: u32) -> Option<bool> {
        let bytes = self.properties.get(&tag)?;
        bytes.first().map(|b| *b != 0)
    }

    pub fn get_binary(&self, tag: u32) -> Option<Vec<u8>> {
        self.properties.get(&tag).cloned()
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

    pub fn add_u32(&mut self, tag: u32, val: u32) {
        self.entries.push(PropertyContextEntry {
            tag,
            data: val.to_le_bytes().to_vec(),
        });
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

    /// Serializes PC into a Property Context Heap Record conforming to MS-PST LTP spec
    pub fn serialize(&self) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        // BTH header
        buf.write_u8(0xBC)?; // bType = 0xBC (Property Context)
        buf.write_u8(0x02)?; // cbKey = 2 (tag ID)
        buf.write_u8(0x06)?; // cbEnt = 6 (property value descriptor)
        buf.write_u8(0x00)?; // bIdxLevels = 0

        // Write entries count
        buf.write_u16::<LittleEndian>(self.entries.len() as u16)?;

        // Data heap offset tracking
        let data_heap_offset = 6 + (self.entries.len() * 8);
        let mut data_pool = Vec::new();

        for entry in &self.entries {
            // Write Property ID (high 16 bits of tag)
            let prop_id = (entry.tag >> 16) as u16;
            let prop_type = (entry.tag & 0xFFFF) as u16;
            buf.write_u16::<LittleEndian>(prop_id)?;
            buf.write_u16::<LittleEndian>(prop_type)?;

            if entry.data.len() <= 4 {
                // Inlined data
                let mut inlined = [0u8; 4];
                inlined[..entry.data.len()].copy_from_slice(&entry.data);
                buf.write_all(&inlined)?;
            } else {
                // Offset into heap
                let offset = (data_heap_offset + data_pool.len()) as u32;
                buf.write_u32::<LittleEndian>(offset)?;
                data_pool.extend_from_slice(&entry.data);
            }
        }

        buf.extend_from_slice(&data_pool);
        Ok(buf)
    }
}

impl Default for PropertyContext {
    fn default() -> Self {
        Self::new()
    }
}
