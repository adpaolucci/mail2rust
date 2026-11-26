use crate::ltp::{
    PR_ATTACH_DATA_BIN, PR_ATTACH_NUM, PR_BODY_HTML_W, PR_BODY_W, PR_CLIENT_SUBMIT_TIME, PR_CREATION_TIME,
    PR_DISPLAY_NAME_W, PR_IMPORTANCE, PR_INTERNET_MESSAGE_ID_W, PR_MESSAGE_CLASS_W, PR_MESSAGE_DELIVERY_TIME,
    PR_MESSAGE_FLAGS, PR_PST_PASSWORD, PR_RECIPIENT_TYPE, PR_SENDER_EMAIL_ADDRESS_W, PR_SENDER_NAME_W, PR_SUBJECT_W,
    ParsedPropertyContext, TableContext,
};
use crate::ndb::{
    NID_ATTACHMENT_TABLE, NID_MESSAGE_STORE, NID_NAME_TO_ID_MAP, NID_RECIPIENT_TABLE, NID_ROOT_FOLDER,
    NID_TYPE_ATTACHMENT, PST_HEADER_SIZE, PST_PAGE_DATA_SIZE, PST_PAGE_SIZE, PST_TRAILER_SIG, PTYPE_BBT, PTYPE_NBT,
    PstEncryption, compute_pst_password_crc, cyclic_crypt, depermute_bytes,
};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use chrono::Utc;
use mail2rust_core::{Attachment, EmailAddress, EmailMessage, Importance, MapiProperty, MessageFlags};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use tracing::{debug, info};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NbtNode {
    pub nid: u32,
    pub bid_data: u64,
    pub bid_sub: u64,
    pub nid_parent: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubnodeEntry {
    pub nid: u32,
    pub bid_data: u64,
    pub bid_sub: u64,
}

#[derive(Debug, Clone, Default)]
pub struct NameToIdMap {
    /// Maps `(guid_bytes, lid_or_hash)` -> `prop_id` (0x8000..0xFFFF)
    pub lid_to_prop_id: HashMap<([u8; 16], u32), u16>,
}

impl NameToIdMap {
    pub fn resolve_lid(&self, guid: &[u8; 16], lid: u32) -> Option<u16> {
        self.lid_to_prop_id.get(&(*guid, lid)).copied()
    }
}

pub struct PstReader<R: Read + Seek> {
    reader: R,
    format_name: String,
    crypt_method: u8,
    bbt_map: HashMap<u64, (u64, u32)>, // BID -> (Offset, Length)
    nbt_nodes: Vec<NbtNode>,
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
        if bbt_offset > 0
            && bbt_offset + PST_PAGE_SIZE as u64 <= file_len
            && reader.seek(SeekFrom::Start(bbt_offset)).is_ok()
        {
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
        } else if bbt_offset > 0
            && bbt_bid > 0
            && bbt_offset < file_len
            && reader.seek(SeekFrom::Start(bbt_offset)).is_ok()
        {
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
        if nbt_offset > 0
            && nbt_offset + PST_PAGE_SIZE as u64 <= file_len
            && reader.seek(SeekFrom::Start(nbt_offset)).is_ok()
        {
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
        } else if nbt_offset > 0
            && nbt_bid > 0
            && nbt_offset < file_len
            && reader.seek(SeekFrom::Start(nbt_offset)).is_ok()
        {
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
                            nbt_nodes.push(NbtNode {
                                nid,
                                bid_data: bid,
                                bid_sub: 0,
                                nid_parent: parent_nid,
                            });
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

    pub fn nbt_entries(&self) -> &[NbtNode] {
        &self.nbt_nodes
    }

    pub fn format_name(&self) -> &str {
        &self.format_name
    }

    /// Reads and decrypts a single raw block by BID (skipping encryption for internal blocks `(bid & 0x02) != 0`).
    pub fn read_block(&mut self, bid: u64) -> Option<Vec<u8>> {
        self.read_block_by_bid(bid)
    }

    /// Reads a data stream by BID, transparently expanding MS-PST `XBLOCK` (`0x01/0x01`) and `XXBLOCK` (`0x01/0x02`) trees.
    pub fn read_data_stream(&mut self, bid: u64) -> Option<Vec<u8>> {
        let mut visited = HashSet::new();
        self.read_data_stream_inner(bid, &mut visited, 0)
    }

    fn read_data_stream_inner(&mut self, bid: u64, visited: &mut HashSet<u64>, depth: usize) -> Option<Vec<u8>> {
        if depth > 8 || bid == 0 || !visited.insert(bid) {
            return None;
        }
        let block = self.read_block_by_bid(bid)?;
        if (bid & 0x02) != 0 && block.len() >= 8 && block[0] == 0x01 && (block[1] == 0x01 || block[1] == 0x02) {
            let c_ent = u16::from_le_bytes([block[2], block[3]]) as usize;
            let lcb_total = u32::from_le_bytes(block[4..8].try_into().ok()?) as usize;
            let max_entries = ((block.len() - 8) / 8).min(c_ent);
            let mut out = Vec::with_capacity(lcb_total.min(64 * 1024 * 1024));
            for i in 0..max_entries {
                let off = 8 + i * 8;
                let child_bid = u64::from_le_bytes(block[off..off + 8].try_into().ok()?);
                if let Some(chunk) = self.read_data_stream_inner(child_bid, visited, depth + 1) {
                    out.extend_from_slice(&chunk);
                }
            }
            if lcb_total > 0 && out.len() > lcb_total {
                out.truncate(lcb_total);
            }
            return Some(out);
        }
        Some(block)
    }

    /// Traverses `SLBLOCK` (`0x02/0x00`) and `SIBLOCK` (`0x02/0x01`) subnode trees rooted at `bid_sub`.
    pub fn read_subnodes(&mut self, bid_sub: u64) -> Vec<SubnodeEntry> {
        let mut out = Vec::new();
        let mut visited = HashSet::new();
        self.read_subnodes_inner(bid_sub, &mut out, &mut visited, 0);
        out
    }

    fn read_subnodes_inner(
        &mut self,
        bid_sub: u64,
        out: &mut Vec<SubnodeEntry>,
        visited: &mut HashSet<u64>,
        depth: usize,
    ) {
        if depth > 8 || bid_sub == 0 || !visited.insert(bid_sub) {
            return;
        }
        let Some(block) = self.read_block_by_bid(bid_sub) else {
            return;
        };
        if block.len() < 8 || block[0] != 0x02 {
            return;
        }
        let c_level = block[1];
        let c_ent = u16::from_le_bytes([block[2], block[3]]) as usize;
        if c_level == 0 {
            // SLBLOCK: 8-byte header (bType, cLevel, cEnt, dwPadding), then 24-byte SLENTRY (nid: u64, bidData: u64, bidSub: u64)
            let max_ent = ((block.len() - 8) / 24).min(c_ent);
            for i in 0..max_ent {
                let base = 8 + i * 24;
                let nid = u64::from_le_bytes(block[base..base + 8].try_into().unwrap_or([0; 8])) as u32;
                let bid_data = u64::from_le_bytes(block[base + 8..base + 16].try_into().unwrap_or([0; 8]));
                let sub = u64::from_le_bytes(block[base + 16..base + 24].try_into().unwrap_or([0; 8]));
                if nid > 0 {
                    out.push(SubnodeEntry {
                        nid,
                        bid_data,
                        bid_sub: sub,
                    });
                }
            }
        } else {
            // SIBLOCK: 8-byte header, then 16-byte SIENTRY (nid: u64, bid: u64)
            let max_ent = ((block.len() - 8) / 16).min(c_ent);
            for i in 0..max_ent {
                let base = 8 + i * 16;
                let child_bid = u64::from_le_bytes(block[base + 8..base + 16].try_into().unwrap_or([0; 8]));
                self.read_subnodes_inner(child_bid, out, visited, depth + 1);
            }
        }
    }

    /// Reads a node's `ParsedPropertyContext`, resolving `XBLOCK` streams and subnode-backed HNIDs.
    pub fn read_property_context(&mut self, bid_data: u64, bid_sub: u64) -> Option<ParsedPropertyContext> {
        let block = self.read_data_stream(bid_data)?;
        if bid_sub == 0 {
            return ParsedPropertyContext::parse(&block);
        }
        let subnodes = self.read_subnodes(bid_sub);
        let mut sub_bytes: HashMap<u32, Vec<u8>> = HashMap::new();
        for sn in &subnodes {
            if let Some(data) = self.read_data_stream(sn.bid_data) {
                sub_bytes.insert(sn.nid, data);
            }
        }
        ParsedPropertyContext::parse_with_subnodes(&block, &|nid| sub_bytes.get(&nid).cloned())
    }

    /// Reads `NID_NAME_TO_ID_MAP (0x61)` to map Outlook named property LIDs (e.g., `PSETID_Address`) to MAPI property IDs.
    pub fn read_name_to_id_map(&mut self) -> NameToIdMap {
        let mut map = NameToIdMap::default();
        let Some(node) = self.nbt_nodes.iter().find(|n| n.nid == NID_NAME_TO_ID_MAP).copied() else {
            return map;
        };
        let Some(pc) = self.read_property_context(node.bid_data, node.bid_sub) else {
            return map;
        };
        let Some(guids_raw) = pc.get_binary(0x00020102) else {
            return map;
        };
        let Some(entries_raw) = pc.get_binary(0x00030102) else {
            return map;
        };

        let mut guids: Vec<[u8; 16]> = Vec::new();
        // Built-in GUID indices 0=None, 1=PS_MAPI, 2=PS_PUBLIC_STRINGS; streams start at index 3
        guids.push([0u8; 16]);
        guids.push([
            0x28, 0x03, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
        ]);
        guids.push([
            0x29, 0x03, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
        ]);
        for chunk in guids_raw.as_chunks::<16>().0 {
            guids.push(*chunk);
        }

        // NAMEID entries are 8 bytes: dwPropertyID (u32), wGuid (u16), wPropIdx (u16)
        for chunk in entries_raw.as_chunks::<8>().0 {
            let dw_prop_id = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let w_guid = u16::from_le_bytes([chunk[4], chunk[5]]);
            let w_prop_idx = u16::from_le_bytes([chunk[6], chunk[7]]);
            let is_string = (w_guid & 0x0001) != 0;
            let guid_idx = (w_guid >> 1) as usize;
            if !is_string && let Some(guid) = guids.get(guid_idx) {
                let prop_id = 0x8000u16.saturating_add(w_prop_idx);
                map.lid_to_prop_id.insert((*guid, dw_prop_id), prop_id);
            }
        }
        map
    }

    pub fn read_folder_paths(&mut self) -> HashMap<u32, Vec<String>> {
        let nodes = self.nbt_nodes.clone();
        let mut folder_names: HashMap<u32, (String, u32)> = HashMap::new();
        for node in &nodes {
            // Skip message store / root from path prefix, and skip actual messages
            if node.nid == NID_MESSAGE_STORE || node.nid == NID_NAME_TO_ID_MAP {
                continue;
            }
            if let Some(pc) = self.read_property_context(node.bid_data, node.bid_sub)
                && let Some(name) = pc
                    .get_string_w(PR_DISPLAY_NAME_W)
                    .or_else(|| pc.get_string_a(PR_DISPLAY_NAME_W))
            {
                let has_msg_fields = pc.get_string_w(PR_SUBJECT_W).is_some()
                    || pc.get_string_w(PR_MESSAGE_CLASS_W).is_some()
                    || pc.get_string_a(PR_MESSAGE_CLASS_W).is_some();
                if !has_msg_fields {
                    folder_names.insert(node.nid, (name, node.nid_parent));
                }
            }
        }

        let mut paths = HashMap::new();
        for &nid in folder_names.keys() {
            if nid == NID_ROOT_FOLDER {
                paths.insert(nid, vec!["Top of Personal Folders".to_string()]);
                continue;
            }
            let mut chain = Vec::new();
            let mut cur = nid;
            let mut guard = 0usize;
            while cur != 0 && cur != NID_ROOT_FOLDER && guard < 32 {
                if let Some((name, parent)) = folder_names.get(&cur) {
                    chain.push(name.clone());
                    if *parent == cur {
                        break;
                    }
                    cur = *parent;
                } else {
                    break;
                }
                guard += 1;
            }
            chain.reverse();
            if !chain.is_empty() {
                paths.insert(nid, chain);
            }
        }
        paths
    }

    pub fn read_all_messages(&mut self) -> io::Result<Vec<EmailMessage>> {
        let mut messages = Vec::new();
        let folder_paths = self.read_folder_paths();
        let nodes = self.nbt_nodes.clone();

        // Read messages from NBT nodes (with full subnode attachments, recipients, and timestamps)
        for node in &nodes {
            if node.nid == NID_MESSAGE_STORE || node.nid == NID_NAME_TO_ID_MAP || node.nid == NID_ROOT_FOLDER {
                continue;
            }
            if let Some(pc) = self.read_property_context(node.bid_data, node.bid_sub) {
                let subject = pc.get_string_w(PR_SUBJECT_W).or_else(|| pc.get_string_a(PR_SUBJECT_W));
                let body = pc.get_string_w(PR_BODY_W).or_else(|| pc.get_string_a(PR_BODY_W));
                let msg_class = pc
                    .get_string_w(PR_MESSAGE_CLASS_W)
                    .or_else(|| pc.get_string_a(PR_MESSAGE_CLASS_W));

                if subject.is_some() || body.is_some() || msg_class.is_some() {
                    let folder = folder_paths
                        .get(&node.nid_parent)
                        .cloned()
                        .unwrap_or_else(|| vec!["Inbox".to_string()]);
                    let (to, cc, bcc, attachments) = self.read_message_subnodes(node.bid_sub);
                    let mut msg = self.build_email_message(node.nid, &pc, folder);
                    msg.to = to;
                    msg.cc = cc;
                    msg.bcc = bcc;
                    msg.attachments = attachments;
                    messages.push(msg);
                }
            }
        }

        // Step 3: Forensic Fallback Scanner
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

    fn read_message_subnodes(
        &mut self,
        bid_sub: u64,
    ) -> (Vec<EmailAddress>, Vec<EmailAddress>, Vec<EmailAddress>, Vec<Attachment>) {
        let mut to = Vec::new();
        let mut cc = Vec::new();
        let mut bcc = Vec::new();
        let mut attachments = Vec::new();

        if bid_sub == 0 {
            return (to, cc, bcc, attachments);
        }

        let subnodes = self.read_subnodes(bid_sub);

        // 1. Recipient table (NID_RECIPIENT_TABLE = 0x0692)
        if let Some(recip_sn) = subnodes.iter().find(|s| s.nid == NID_RECIPIENT_TABLE)
            && let Some(raw) = self.read_data_stream(recip_sn.bid_data)
            && let Some(tc) = TableContext::parse(&raw)
        {
            for row in tc.rows {
                let rtype = row.get_u32(PR_RECIPIENT_TYPE).unwrap_or(1);
                let name = row.get_string_any(0x3001).unwrap_or_default();
                let email = row
                    .get_string_any(0x39FE)
                    .or_else(|| row.get_string_any(0x3003))
                    .unwrap_or_default();
                if email.is_empty() && name.is_empty() {
                    continue;
                }
                let addr = EmailAddress::new(
                    if name.is_empty() { email.clone() } else { name },
                    if email.is_empty() {
                        "unknown@pst.local".to_string()
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

        // 2. Attachment subnodes (NID type 0x0E) + optional NID_ATTACHMENT_TABLE (0x0671)
        let mut att_nums: HashSet<u32> = HashSet::new();
        if let Some(att_tbl_sn) = subnodes.iter().find(|s| s.nid == NID_ATTACHMENT_TABLE)
            && let Some(raw) = self.read_data_stream(att_tbl_sn.bid_data)
            && let Some(tc) = TableContext::parse(&raw)
        {
            for row in tc.rows {
                if let Some(num) = row.get_u32(PR_ATTACH_NUM) {
                    att_nums.insert(num);
                }
            }
        }

        for sn in &subnodes {
            if (sn.nid & 0x1F) == NID_TYPE_ATTACHMENT as u32
                && let Some(pc) = self.read_property_context(sn.bid_data, sn.bid_sub)
            {
                let filename = pc
                    .get_string_any(0x3707)
                    .or_else(|| pc.get_string_any(0x3704))
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| format!("attachment_{:x}.bin", sn.nid));
                let mime = pc
                    .get_string_any(0x370E)
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "application/octet-stream".to_string());
                let content_id = pc.get_string_any(0x3712).filter(|s| !s.is_empty());
                let data = pc.get_binary(PR_ATTACH_DATA_BIN).unwrap_or_default();
                attachments.push(Attachment::new(filename, mime, data, content_id));
            }
        }

        (to, cc, bcc, attachments)
    }

    pub fn get_encryption(&self) -> PstEncryption {
        PstEncryption::from_u8(self.crypt_method)
    }

    pub fn get_stored_password_crc(&mut self) -> Option<u32> {
        let store_node = self.nbt_nodes.iter().find(|n| n.nid == NID_MESSAGE_STORE)?;
        let block = self.read_block_by_bid(store_node.bid_data)?;
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
        let (offset, len) = self
            .bbt_map
            .get(&bid)
            .or_else(|| self.bbt_map.get(&(bid & !0x01)))
            .copied()?;
        self.reader.seek(SeekFrom::Start(offset)).ok()?;
        let mut buf = vec![0u8; len as usize];
        self.reader.read_exact(&mut buf).ok()?;

        // Internal NDB blocks (XBLOCK/XXBLOCK/SLBLOCK/SIBLOCK) have bit 1 (0x02) set and are unencrypted
        if (bid & 0x02) == 0 {
            if self.crypt_method == 0x01 {
                depermute_bytes(&mut buf);
            } else if self.crypt_method == 0x02 {
                cyclic_crypt(&mut buf, bid);
            }
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
            if let Some(pc) = ParsedPropertyContext::parse(&chunk)
                && (pc.get_string_w(PR_SUBJECT_W).is_some() || pc.get_string_w(PR_BODY_W).is_some())
            {
                let msg = self.build_email_message(offset as u32, &pc, vec!["Recovered Items".to_string()]);
                recovered.push(msg);
                offset += 64;
                continue;
            }

            // Test depermuted
            if self.crypt_method == 0x01 || data[offset] == crate::ndb::MPBB[0xBC] {
                let mut dep = chunk.clone();
                depermute_bytes(&mut dep);
                if let Some(pc) = ParsedPropertyContext::parse(&dep)
                    && (pc.get_string_w(PR_SUBJECT_W).is_some() || pc.get_string_w(PR_BODY_W).is_some())
                {
                    let msg = self.build_email_message(offset as u32, &pc, vec!["Recovered Items".to_string()]);
                    recovered.push(msg);
                    offset += 64;
                    continue;
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
        let replied = (flags_val & 0x04) != 0;
        let unsent = (flags_val & 0x08) != 0;

        let imp_val = pc.get_u32(PR_IMPORTANCE).unwrap_or(1);
        let importance = match imp_val {
            2 => Importance::High,
            0 => Importance::Low,
            _ => Importance::Normal,
        };

        let date = pc
            .get_systime(PR_MESSAGE_DELIVERY_TIME)
            .or_else(|| pc.get_systime(PR_CLIENT_SUBMIT_TIME))
            .or_else(|| pc.get_systime(PR_CREATION_TIME))
            .unwrap_or_else(Utc::now);

        let mut mapi_properties = Vec::new();
        if let Some(mc) = pc
            .get_string_w(PR_MESSAGE_CLASS_W)
            .or_else(|| pc.get_string_a(PR_MESSAGE_CLASS_W))
        {
            mapi_properties.push(MapiProperty {
                tag: PR_MESSAGE_CLASS_W,
                name: "PR_MESSAGE_CLASS_W".to_string(),
                prop_type: "PT_UNICODE".to_string(),
                value: mc,
            });
        }
        if flags_val != 0 {
            mapi_properties.push(MapiProperty {
                tag: PR_MESSAGE_FLAGS,
                name: "PR_MESSAGE_FLAGS".to_string(),
                prop_type: "PT_LONG".to_string(),
                value: flags_val.to_string(),
            });
        }

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
            date,
            size_bytes: raw_bytes.len(),
            body_text,
            body_html,
            raw_mime: Vec::new(),
            attachments: Vec::new(),
            mapi_properties,
            flags: MessageFlags {
                read,
                replied,
                draft: unsent,
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

fn traverse_nbt<R: Read + Seek>(reader: &mut R, page_offset: u64, nbt_nodes: &mut Vec<NbtNode>) -> io::Result<()> {
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
                let bid_sub = u64::from_le_bytes(page[base + 16..base + 24].try_into().unwrap());
                let nid_parent = u32::from_le_bytes(page[base + 24..base + 28].try_into().unwrap());
                if nid > 0 && bid_data > 0 && nid != 0x47474747 {
                    nbt_nodes.push(NbtNode {
                        nid,
                        bid_data,
                        bid_sub,
                        nid_parent,
                    });
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
