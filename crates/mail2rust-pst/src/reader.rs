use crate::ltp::{
    ParsedPropertyContext, PR_BODY_HTML_W, PR_BODY_W, PR_DISPLAY_NAME_W, PR_IMPORTANCE, PR_INTERNET_MESSAGE_ID_W,
    PR_MESSAGE_CLASS_W, PR_MESSAGE_FLAGS, PR_PST_PASSWORD, PR_SENDER_EMAIL_ADDRESS_W, PR_SENDER_NAME_W, PR_SUBJECT_W,
};
use crate::ndb::{
    compute_pst_password_crc, cyclic_crypt, depermute_bytes, PstEncryption, NID_MESSAGE_STORE, NID_ROOT_FOLDER,
    PST_HEADER_SIZE, PST_PAGE_DATA_SIZE, PST_PAGE_SIZE, PST_TRAILER_SIG, PTYPE_BBT, PTYPE_NBT,
};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use chrono::Utc;
use mail2rust_core::{EmailAddress, EmailMessage, Importance, MessageFlags};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use tracing::{debug, info};

pub struct PstReader<R: Read + Seek> {
    reader: R,
    format_name: String,
    crypt_method: u8,
    bbt_map: HashMap<u64, (u64, u32)>, // BID -> (Offset, Length)
    nbt_nodes: Vec<(u32, u64, u32)>,   // (NID, BID, ParentNID)
}

impl<R: Read + Seek> PstReader<R> {
    pub fn open(mut reader: R) -> io::Result<Self> {
        reader.seek(SeekFrom::Start(0))?;
        let mut header = [0u8; PST_HEADER_SIZE];
        reader.read_exact(&mut header)?;

        // Magic check
        let magic_val = (&header[0x00..0x04]).read_u32::<BigEndian>()?;
        let version = (&header[0x0A..0x0C]).read_u16::<LittleEndian>()?;
        let format_name = if version >= 23 {
            "Microsoft Outlook PST (Unicode)".to_string()
        } else if magic_val == 0x2142444F {
            "Offline Exchange OST (!BDO)".to_string()
        } else {
            "Legacy ANSI PST".to_string()
        };

        let crypt_method = header[0x1CD];
        info!(format = %format_name, crypt = crypt_method, "Opened PST/OST container for reading");

        let file_len = reader.seek(SeekFrom::End(0))?;

        // Root BREF pointers (Unicode ROOT at 0xB8 or legacy fallback at 0xB4/0xC4)
        let nbt_bid_u = (&header[0xDC..0xE4]).read_u64::<LittleEndian>()?;
        let nbt_off_u = (&header[0xE4..0xEC]).read_u64::<LittleEndian>()?;
        let (nbt_bid, nbt_offset) = if nbt_bid_u > 0 && nbt_off_u > 0 && nbt_off_u < file_len {
            (nbt_bid_u, nbt_off_u)
        } else {
            let b = (&header[0xB4..0xBC]).read_u64::<LittleEndian>()?;
            let o = (&header[0xBC..0xC4]).read_u64::<LittleEndian>()?;
            (b, o)
        };

        let bbt_bid_u = (&header[0xEC..0xF4]).read_u64::<LittleEndian>()?;
        let bbt_off_u = (&header[0xF4..0xFC]).read_u64::<LittleEndian>()?;
        let (bbt_bid, bbt_offset) = if bbt_bid_u > 0 && bbt_off_u > 0 && bbt_off_u < file_len {
            (bbt_bid_u, bbt_off_u)
        } else {
            let b = (&header[0xC4..0xCC]).read_u64::<LittleEndian>()?;
            let o = (&header[0xCC..0xD4]).read_u64::<LittleEndian>()?;
            (b, o)
        };

        let mut bbt_map = HashMap::new();
        let mut nbt_nodes = Vec::new();

        // 1. Read BBT (Block B-Tree)
        let mut bbt_is_btpage = false;
        if bbt_offset > 0 && bbt_offset + PST_PAGE_SIZE as u64 <= file_len && reader.seek(SeekFrom::Start(bbt_offset)).is_ok() {
            let mut page = [0u8; PST_PAGE_SIZE];
            if reader.read_exact(&mut page).is_ok() {
                let w_sig = u16::from_le_bytes([page[498], page[499]]);
                if page[496] == PTYPE_BBT && w_sig == PST_TRAILER_SIG {
                    bbt_is_btpage = true;
                }
            }
        }

        if bbt_is_btpage {
            let _ = traverse_bbt(&mut reader, bbt_offset, &mut bbt_map);
        } else if bbt_offset > 0 && bbt_bid > 0 && bbt_offset < file_len && reader.seek(SeekFrom::Start(bbt_offset)).is_ok() {
            let to_read = (file_len - bbt_offset).min(100 * 1024 * 1024) as usize;
            let mut bbt_raw = Vec::with_capacity(to_read.min(10 * 1024 * 1024));
            if reader.by_ref().take(to_read as u64).read_to_end(&mut bbt_raw).is_ok() {
                if crypt_method == 0x01 {
                    depermute_bytes(&mut bbt_raw);
                } else if crypt_method == 0x02 {
                    cyclic_crypt(&mut bbt_raw, bbt_bid);
                }
                // Parse 24-byte BBT records: (bid: u64, offset: u64, len: u32, ref_count: u32)
                let mut cur = &bbt_raw[..];
                while cur.len() >= 24 {
                    if let (Ok(bid), Ok(off), Ok(len), Ok(_ref_count)) = (
                        cur.read_u64::<LittleEndian>(),
                        cur.read_u64::<LittleEndian>(),
                        cur.read_u32::<LittleEndian>(),
                        cur.read_u32::<LittleEndian>(),
                    ) {
                        if bid > 0 && off > 0 && len > 0 && len <= 100 * 1024 * 1024 {
                            bbt_map.insert(bid, (off, len));
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        // 2. Read NBT (Node B-Tree)
        let mut nbt_is_btpage = false;
        if nbt_offset > 0 && nbt_offset + PST_PAGE_SIZE as u64 <= file_len && reader.seek(SeekFrom::Start(nbt_offset)).is_ok() {
            let mut page = [0u8; PST_PAGE_SIZE];
            if reader.read_exact(&mut page).is_ok() {
                let w_sig = u16::from_le_bytes([page[498], page[499]]);
                if page[496] == PTYPE_NBT && w_sig == PST_TRAILER_SIG {
                    nbt_is_btpage = true;
                }
            }
        }

        if nbt_is_btpage {
            let _ = traverse_nbt(&mut reader, nbt_offset, &mut nbt_nodes);
        } else if nbt_offset > 0 && nbt_bid > 0 && nbt_offset < file_len && reader.seek(SeekFrom::Start(nbt_offset)).is_ok() {
            let to_read = if bbt_offset > nbt_offset {
                (bbt_offset - nbt_offset).min(100 * 1024 * 1024) as usize
            } else {
                (file_len - nbt_offset).min(100 * 1024 * 1024) as usize
            };
            let mut nbt_raw = Vec::with_capacity(to_read.min(10 * 1024 * 1024));
            if reader.by_ref().take(to_read as u64).read_to_end(&mut nbt_raw).is_ok() {
                if crypt_method == 0x01 {
                    depermute_bytes(&mut nbt_raw);
                } else if crypt_method == 0x02 {
                    cyclic_crypt(&mut nbt_raw, nbt_bid);
                }
                // Parse 16-byte NBT records: (nid: u32, bid: u64, parent_nid: u32)
                let mut cur = &nbt_raw[..];
                while cur.len() >= 16 {
                    if let (Ok(nid), Ok(bid), Ok(parent_nid)) = (
                        cur.read_u32::<LittleEndian>(),
                        cur.read_u64::<LittleEndian>(),
                        cur.read_u32::<LittleEndian>(),
                    ) {
                        if nid > 0 && bid > 0 && nid != 0x47474747 {
                            nbt_nodes.push((nid, bid, parent_nid));
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        debug!(
            blocks = bbt_map.len(),
            nodes = nbt_nodes.len(),
            "Parsed PST B-Tree headers"
        );

        Ok(Self {
            reader,
            format_name,
            crypt_method,
            bbt_map,
            nbt_nodes,
        })
    }

    pub fn read_all_messages(&mut self) -> io::Result<Vec<EmailMessage>> {
        let mut messages = Vec::new();
        let mut folder_names: HashMap<u32, String> = HashMap::new();
        folder_names.insert(NID_ROOT_FOLDER, "Top of Personal Folders".to_string());

        let nodes = self.nbt_nodes.clone();

        // Step 1: Pre-scan folder nodes
        for (nid, bid, _parent) in &nodes {
            if let Some(block) = self.read_block_by_bid(*bid) {
                if let Some(pc) = ParsedPropertyContext::parse(&block) {
                    if let Some(name) = pc
                        .get_string_w(PR_DISPLAY_NAME_W)
                        .or_else(|| pc.get_string_a(PR_DISPLAY_NAME_W))
                    {
                        folder_names.insert(*nid, name);
                    }
                }
            }
        }

        // Step 2: Read messages from NBT nodes
        for (nid, bid, parent_nid) in &nodes {
            if let Some(block) = self.read_block_by_bid(*bid) {
                if let Some(pc) = ParsedPropertyContext::parse(&block) {
                    // Check if this node is an email message or item
                    let subject = pc.get_string_w(PR_SUBJECT_W).or_else(|| pc.get_string_a(PR_SUBJECT_W));
                    let body = pc.get_string_w(PR_BODY_W).or_else(|| pc.get_string_a(PR_BODY_W));
                    let msg_class = pc
                        .get_string_w(PR_MESSAGE_CLASS_W)
                        .or_else(|| pc.get_string_a(PR_MESSAGE_CLASS_W));

                    if subject.is_some() || body.is_some() || msg_class.is_some() {
                        let folder_name = folder_names
                            .get(parent_nid)
                            .cloned()
                            .unwrap_or_else(|| "Inbox".to_string());
                        let msg = self.build_email_message(*nid, &pc, vec![folder_name]);
                        messages.push(msg);
                    }
                }
            }
        }

        // Step 3: Forensic Fallback Scanner
        // If NBT/BBT did not yield messages (e.g. damaged B-tree or orphaned OST),
        // scan block sectors for Property Contexts (0xBC)
        if messages.is_empty() {
            info!("No messages located via root B-trees; activating forensic block carving scanner");
            let carved = self.carve_messages_from_stream()?;
            info!(carved = carved.len(), "Forensic carving completed");
            messages = carved;
        }

        info!(
            count = messages.len(),
            format = %self.format_name,
            "Successfully extracted messages from PST/OST container"
        );
        Ok(messages)
    }

    pub fn get_encryption(&self) -> PstEncryption {
        PstEncryption::from_u8(self.crypt_method)
    }

    pub fn get_stored_password_crc(&mut self) -> Option<u32> {
        let store_node = self.nbt_nodes.iter().find(|(nid, _, _)| *nid == NID_MESSAGE_STORE)?;
        let block = self.read_block_by_bid(store_node.1)?;
        let pc = ParsedPropertyContext::parse(&block)?;
        pc.get_u32(PR_PST_PASSWORD)
    }

    pub fn is_password_protected(&mut self) -> bool {
        self.get_stored_password_crc().is_some()
    }

    pub fn verify_password(&mut self, password: &str) -> bool {
        match self.get_stored_password_crc() {
            Some(stored_crc) => stored_crc == compute_pst_password_crc(password),
            None => password.is_empty(),
        }
    }

    fn read_block_by_bid(&mut self, bid: u64) -> Option<Vec<u8>> {
        let (offset, len) = *self.bbt_map.get(&bid)?;
        self.reader.seek(SeekFrom::Start(offset)).ok()?;
        let mut buf = vec![0u8; len as usize];
        self.reader.read_exact(&mut buf).ok()?;

        if self.crypt_method == 0x01 {
            depermute_bytes(&mut buf);
        } else if self.crypt_method == 0x02 {
            cyclic_crypt(&mut buf, bid);
        }

        Some(buf)
    }

    fn carve_messages_from_stream(&mut self) -> io::Result<Vec<EmailMessage>> {
        let stream_len = self.reader.seek(SeekFrom::End(0))?;
        self.reader.seek(SeekFrom::Start(0))?;

        // Guard against unbounded memory allocation on massive containers
        const MAX_CARVE_BYTES: u64 = 256 * 1024 * 1024; // 256 MB threshold
        if stream_len > MAX_CARVE_BYTES {
            info!(
                size = stream_len,
                threshold = MAX_CARVE_BYTES,
                "Container size exceeds 256MB in-memory carving threshold; skipping deep block carving"
            );
            return Ok(Vec::new());
        }

        let mut data = Vec::with_capacity(stream_len as usize);
        self.reader.read_to_end(&mut data)?;

        let mut recovered = Vec::new();
        let mut offset = PST_HEADER_SIZE;

        while offset + 6 < data.len() {
            // Check for direct or permuted 0xBC (Property Context)
            let chunk = if offset + 4096 <= data.len() {
                data[offset..offset + 4096].to_vec()
            } else {
                data[offset..].to_vec()
            };

            // Test raw
            if let Some(pc) = ParsedPropertyContext::parse(&chunk) {
                if pc.get_string_w(PR_SUBJECT_W).is_some() || pc.get_string_w(PR_BODY_W).is_some() {
                    let msg = self.build_email_message(offset as u32, &pc, vec!["Recovered Items".to_string()]);
                    recovered.push(msg);
                    offset += 64;
                    continue;
                }
            }

            // Test depermuted
            if self.crypt_method == 0x01 || data[offset] == crate::ndb::MPBB[0xBC] {
                let mut dep = chunk.clone();
                depermute_bytes(&mut dep);
                if let Some(pc) = ParsedPropertyContext::parse(&dep) {
                    if pc.get_string_w(PR_SUBJECT_W).is_some() || pc.get_string_w(PR_BODY_W).is_some() {
                        let msg = self.build_email_message(offset as u32, &pc, vec!["Recovered Items".to_string()]);
                        recovered.push(msg);
                        offset += 64;
                        continue;
                    }
                }
            }

            offset += 64; // Blocks are 64-byte aligned in MS-PST
        }

        Ok(recovered)
    }

    fn build_email_message(&self, nid: u32, pc: &ParsedPropertyContext, folder: Vec<String>) -> EmailMessage {
        let subject = pc
            .get_string_w(PR_SUBJECT_W)
            .or_else(|| pc.get_string_a(PR_SUBJECT_W))
            .unwrap_or_else(|| "(No Subject)".to_string());

        let body_text = pc
            .get_string_w(PR_BODY_W)
            .or_else(|| pc.get_string_a(PR_BODY_W))
            .unwrap_or_default();

        let body_html = pc.get_binary(PR_BODY_HTML_W).and_then(|b| String::from_utf8(b).ok());

        let sender_name = pc
            .get_string_w(PR_SENDER_NAME_W)
            .or_else(|| pc.get_string_a(PR_SENDER_NAME_W))
            .unwrap_or_else(|| "Unknown Sender".to_string());

        let sender_email = pc
            .get_string_w(PR_SENDER_EMAIL_ADDRESS_W)
            .or_else(|| pc.get_string_a(PR_SENDER_EMAIL_ADDRESS_W))
            .unwrap_or_else(|| "unknown@pst.local".to_string());

        let message_id = pc
            .get_string_w(PR_INTERNET_MESSAGE_ID_W)
            .or_else(|| pc.get_string_a(PR_INTERNET_MESSAGE_ID_W))
            .unwrap_or_else(|| format!("<pst-{:08x}@mail2rust.local>", nid));

        let flags_val = pc.get_u32(PR_MESSAGE_FLAGS).unwrap_or(0);
        let read = (flags_val & 0x01) != 0;

        let imp_val = pc.get_u32(PR_IMPORTANCE).unwrap_or(1);
        let importance = match imp_val {
            2 => Importance::High,
            0 => Importance::Low,
            _ => Importance::Normal,
        };

        let raw_bytes = body_text.as_bytes();
        let (sha256, md5) = EmailMessage::compute_hashes(raw_bytes);
        let conversation_id = EmailMessage::generate_conversation_id(&subject, &message_id, None);

        EmailMessage {
            id: format!("pst_{:x}", nid),
            message_id,
            conversation_id,
            folder_path: folder,
            subject,
            from: EmailAddress::new(sender_name, sender_email),
            to: vec![],
            cc: vec![],
            bcc: vec![],
            date: Utc::now(),
            size_bytes: raw_bytes.len(),
            body_text,
            body_html,
            raw_mime: Vec::new(),
            attachments: Vec::new(),
            mapi_properties: Vec::new(),
            flags: MessageFlags {
                read,
                ..Default::default()
            },
            importance,
            sha256,
            md5,
            bates_number: None,
            legal_hold: false,
            source_format: self.format_name.clone(),
        }
    }
}

impl PstReader<File> {
    pub fn read_path(path: &Path) -> io::Result<Vec<EmailMessage>> {
        let file = File::open(path)?;
        let mut reader = PstReader::open(file)?;
        reader.read_all_messages()
    }
}

fn traverse_bbt<R: Read + Seek>(
    reader: &mut R,
    page_offset: u64,
    bbt_map: &mut HashMap<u64, (u64, u32)>,
) -> io::Result<()> {
    reader.seek(SeekFrom::Start(page_offset))?;
    let mut page = [0u8; PST_PAGE_SIZE];
    reader.read_exact(&mut page)?;

    let ptype = page[496];
    let w_sig = u16::from_le_bytes([page[498], page[499]]);
    if ptype != PTYPE_BBT || w_sig != PST_TRAILER_SIG {
        return Ok(());
    }

    let c_ent = page[488] as usize;
    let c_level = page[491];

    if c_level == 0 {
        // Leaf page: 24-byte BBTENTRY
        for i in 0..c_ent {
            let base = i * 24;
            if base + 24 <= PST_PAGE_DATA_SIZE {
                let bid = u64::from_le_bytes(page[base..base + 8].try_into().unwrap());
                let ib = u64::from_le_bytes(page[base + 8..base + 16].try_into().unwrap());
                let cb = u16::from_le_bytes(page[base + 16..base + 18].try_into().unwrap()) as u32;
                if bid > 0 && ib > 0 && cb > 0 {
                    bbt_map.insert(bid, (ib, cb));
                }
            }
        }
    } else {
        // Intermediate branch page: 24-byte BTENTRY
        let mut child_offsets = Vec::with_capacity(c_ent);
        for i in 0..c_ent {
            let base = i * 24;
            if base + 24 <= PST_PAGE_DATA_SIZE {
                let child_ib = u64::from_le_bytes(page[base + 16..base + 24].try_into().unwrap());
                if child_ib > 0 {
                    child_offsets.push(child_ib);
                }
            }
        }
        for child_ib in child_offsets {
            traverse_bbt(reader, child_ib, bbt_map)?;
        }
    }
    Ok(())
}

fn traverse_nbt<R: Read + Seek>(
    reader: &mut R,
    page_offset: u64,
    nbt_nodes: &mut Vec<(u32, u64, u32)>,
) -> io::Result<()> {
    reader.seek(SeekFrom::Start(page_offset))?;
    let mut page = [0u8; PST_PAGE_SIZE];
    reader.read_exact(&mut page)?;

    let ptype = page[496];
    let w_sig = u16::from_le_bytes([page[498], page[499]]);
    if ptype != PTYPE_NBT || w_sig != PST_TRAILER_SIG {
        return Ok(());
    }

    let c_ent = page[488] as usize;
    let c_level = page[491];

    if c_level == 0 {
        // Leaf page: 32-byte NBTENTRY
        for i in 0..c_ent {
            let base = i * 32;
            if base + 32 <= PST_PAGE_DATA_SIZE {
                let nid = u64::from_le_bytes(page[base..base + 8].try_into().unwrap()) as u32;
                let bid_data = u64::from_le_bytes(page[base + 8..base + 16].try_into().unwrap());
                let nid_parent = u32::from_le_bytes(page[base + 24..base + 28].try_into().unwrap());
                if nid > 0 && bid_data > 0 && nid != 0x47474747 {
                    nbt_nodes.push((nid, bid_data, nid_parent));
                }
            }
        }
    } else {
        // Intermediate branch page: 24-byte BTENTRY
        let mut child_offsets = Vec::with_capacity(c_ent);
        for i in 0..c_ent {
            let base = i * 24;
            if base + 24 <= PST_PAGE_DATA_SIZE {
                let child_ib = u64::from_le_bytes(page[base + 16..base + 24].try_into().unwrap());
                if child_ib > 0 {
                    child_offsets.push(child_ib);
                }
            }
        }
        for child_ib in child_offsets {
            traverse_nbt(reader, child_ib, nbt_nodes)?;
        }
    }
    Ok(())
}
