use crate::ltp::{
    MAX_PROPERTY_CONTEXT_BYTES, PR_ATTACH_CONTENT_ID_W, PR_ATTACH_DATA_BIN, PR_ATTACH_FILENAME_W, PR_ATTACH_FLAGS,
    PR_ATTACH_LONG_FILENAME_W, PR_ATTACH_METHOD, PR_ATTACH_MIME_TAG_W, PR_ATTACH_NUM, PR_ATTACH_SIZE, PR_BODY_HTML_W,
    PR_BODY_W, PR_CLIENT_SUBMIT_TIME, PR_CONTENT_COUNT, PR_CONTENT_UNREAD, PR_DISPLAY_BCC_W, PR_DISPLAY_CC_W,
    PR_DISPLAY_NAME_W, PR_DISPLAY_TO_W, PR_EMAIL_ADDRESS_W, PR_HASATTACH, PR_IMPORTANCE, PR_INTERNET_MESSAGE_ID_W,
    PR_MAIL2RUST_PRESERVED_COUNT, PR_MESSAGE_CLASS_W, PR_MESSAGE_DELIVERY_TIME, PR_MESSAGE_FLAGS, PR_PST_PASSWORD,
    PR_RECIPIENT_TYPE, PR_SENDER_EMAIL_ADDRESS_W, PR_SENDER_NAME_W, PR_SMTP_ADDRESS_W, PR_SUBFOLDERS, PR_SUBJECT_W,
    PropertyContext, TableContext, TableContextRow, build_subitem_table_7c, mail2rust_chunk_tag,
};
use crate::ndb::{
    AMAP_START_OFFSET, DATA_START_OFFSET, NID_ATTACHMENT_TABLE, NID_MESSAGE_STORE, NID_RECIPIENT_TABLE,
    NID_ROOT_FOLDER, NID_TYPE_ATTACHMENT, PMAP_START_OFFSET, PST_CLIENT_MAGIC, PST_CLIENT_VER, PST_HEADER_SIZE,
    PST_PAGE_DATA_SIZE, PST_PAGE_SIZE, PST_UNICODE_VER, PTYPE_AMAP, PTYPE_BBT, PTYPE_NBT, PTYPE_PMAP, PageTrailer,
    PstEncryption, calculate_partial_header_crc, compute_pst_password_crc, cyclic_crypt, permute_bytes, weak_crc32,
};
use crate::preserve::PreservedMessage;
use mail2rust_core::EmailMessage;
use std::collections::HashMap;
use std::io::{self, Seek, SeekFrom, Write};
use tracing::{debug, info};

pub struct PstFolderRecord {
    pub nid: u32,
    pub name: String,
    pub parent_nid: u32,
    pub message_count: u32,
}

pub struct PstWriter<W: Write + Seek> {
    writer: W,
    current_offset: u64,
    alloc_nid_counter: u32,
    alloc_bid_counter: u64,
    alloc_page_bid_counter: u64,
    nbt_entries: Vec<(u32, u64, u64, u32)>, // (NID, BID, BidSub, ParentNID)
    bbt_entries: Vec<(u64, u64, u32)>,      // (BID, FileOffset, ByteLength)
    folder_map: HashMap<String, u32>,
    folder_path_nids: HashMap<Vec<String>, u32>,
    folder_children: HashMap<u32, Vec<u32>>,
    folder_messages: HashMap<u32, Vec<u32>>,
    encryption: PstEncryption,
    password: Option<String>,
}

impl<W: Write + Seek> PstWriter<W> {
    pub fn new(writer: W, use_compression: bool) -> io::Result<Self> {
        let enc = if use_compression {
            PstEncryption::Permute
        } else {
            PstEncryption::None
        };
        Self::with_encryption(writer, enc, None)
    }

    pub fn with_encryption(mut writer: W, encryption: PstEncryption, password: Option<String>) -> io::Result<Self> {
        info!(
            encryption = ?encryption,
            has_password = password.is_some(),
            "Initializing MS-PST 64-bit Unicode Personal Folders container"
        );
        // Reserve initial space up to DATA_START_OFFSET (0x4800 = 18,432 bytes)
        // for HEADER, AMap (0x4400..0x4600), and PMap (0x4600..0x4800)
        let init_buf = vec![0u8; DATA_START_OFFSET as usize];
        writer.seek(SeekFrom::Start(0))?;
        writer.write_all(&init_buf)?;

        Ok(Self {
            writer,
            current_offset: DATA_START_OFFSET,
            alloc_nid_counter: 0x8004, // type bits 0x04 (normal node), matching genuine PSTs
            alloc_bid_counter: 0x0080,
            alloc_page_bid_counter: 0x0042,
            nbt_entries: Vec::new(),
            bbt_entries: Vec::new(),
            folder_map: HashMap::new(),
            folder_path_nids: HashMap::new(),
            folder_children: HashMap::new(),
            folder_messages: HashMap::new(),
            encryption,
            password,
        })
    }

    pub fn alloc_page_bid(&mut self) -> u64 {
        let bid = self.alloc_page_bid_counter;
        self.alloc_page_bid_counter += 4;
        bid
    }

    fn write_padded_ndb_block(&mut self, bid: u64, payload: &[u8]) -> io::Result<u64> {
        let offset = self.current_offset;
        let len = payload.len() as u32;

        // A Unicode NDB block is padded to a 64-byte boundary and carries a
        // 16-byte footer (data size u16, sig u16, checksum u32, back pointer u64)
        // at its end, which conforming readers like libpff validate against BBT.
        const FOOTER_SIZE: usize = 16;
        let mut block_size = (len as usize).div_ceil(64) * 64;
        if block_size - (len as usize) < FOOTER_SIZE {
            block_size += 64;
        }
        let mut block = vec![0u8; block_size];
        block[..len as usize].copy_from_slice(payload);
        let footer = block_size - FOOTER_SIZE;
        block[footer..footer + 2].copy_from_slice(&(len as u16).to_le_bytes());

        self.writer.seek(SeekFrom::Start(offset))?;
        self.writer.write_all(&block)?;
        self.current_offset = offset + block_size as u64;

        self.bbt_entries.push((bid, offset, len));
        Ok(bid)
    }

    pub fn write_block(&mut self, mut data: Vec<u8>) -> io::Result<u64> {
        let bid = self.alloc_bid_counter;
        self.alloc_bid_counter += 4; // BIDs increment by 4 in Unicode NDB

        match self.encryption {
            PstEncryption::None => {}
            PstEncryption::Permute => permute_bytes(&mut data),
            PstEncryption::Cyclic => cyclic_crypt(&mut data, bid),
        }

        self.write_padded_ndb_block(bid, &data)
    }

    fn write_internal_block(&mut self, data: Vec<u8>) -> io::Result<u64> {
        // Internal NDB blocks (XBLOCK/XXBLOCK/SLBLOCK/SIBLOCK) set bit 1 (0x02) and are not encrypted
        let bid = self.alloc_bid_counter | 0x02;
        self.alloc_bid_counter += 4;
        self.write_padded_ndb_block(bid, &data)
    }

    /// Writes `data` as a single block if <= 8192 bytes, or as an MS-PST `XBLOCK` (`0x01/0x01`) tree if larger.
    pub fn write_stream_or_block(&mut self, data: Vec<u8>) -> io::Result<u64> {
        if data.len() <= 8192 {
            return self.write_block(data);
        }
        let total_len = data.len() as u32;
        let mut child_bids = Vec::new();
        for chunk in data.chunks(8192) {
            child_bids.push(self.write_block(chunk.to_vec())?);
        }
        let mut xblock = Vec::with_capacity(8 + child_bids.len() * 8);
        xblock.push(0x01); // bType = XBLOCK
        xblock.push(0x01); // cLevel = 1
        xblock.extend_from_slice(&(child_bids.len() as u16).to_le_bytes());
        xblock.extend_from_slice(&total_len.to_le_bytes());
        for bid in child_bids {
            xblock.extend_from_slice(&bid.to_le_bytes());
        }
        self.write_internal_block(xblock)
    }

    fn add_folder_with_parent_nid(&mut self, name: &str, parent_nid: u32) -> io::Result<u32> {
        let nid = self.alloc_nid_counter;
        self.alloc_nid_counter += 0x20;

        let mut pc = PropertyContext::new();
        pc.add_string_w(PR_DISPLAY_NAME_W, name);
        pc.add_bool(PR_SUBFOLDERS, false);
        pc.add_u32(PR_CONTENT_COUNT, 0);
        pc.add_u32(PR_CONTENT_UNREAD, 0);

        let data = pc.serialize()?;
        let bid = self.write_block(data)?;

        self.nbt_entries.push((nid, bid, 0, parent_nid));
        self.folder_children.entry(parent_nid).or_default().push(nid);
        self.folder_map.entry(name.to_string()).or_insert(nid);
        debug!(nid = nid, name = name, parent_nid = parent_nid, "Added folder to PST");
        Ok(nid)
    }

    pub fn add_folder(&mut self, name: &str, parent_path: Option<&str>) -> io::Result<u32> {
        let parent_nid = if let Some(p) = parent_path {
            *self.folder_map.get(p).unwrap_or(&NID_ROOT_FOLDER)
        } else {
            NID_ROOT_FOLDER
        };
        let nid = self.add_folder_with_parent_nid(name, parent_nid)?;
        self.folder_map.insert(name.to_string(), nid);
        Ok(nid)
    }

    /// Ensures every segment in `segments` exists as nested folders (optionally under `root_parent`),
    /// returning the leaf folder NID. Defaults to `Inbox` when `segments` has no non-empty parts.
    pub fn ensure_folder_path(&mut self, segments: &[String], root_parent: Option<&str>) -> io::Result<u32> {
        let mut current_path = Vec::new();
        let mut parent_nid = if let Some(root) = root_parent.map(str::trim).filter(|s| !s.is_empty()) {
            current_path.push(root.to_string());
            if let Some(&nid) = self.folder_path_nids.get(&current_path) {
                nid
            } else if let Some(&nid) = self.folder_map.get(root) {
                self.folder_path_nids.insert(current_path.clone(), nid);
                nid
            } else {
                let nid = self.add_folder_with_parent_nid(root, NID_ROOT_FOLDER)?;
                self.folder_path_nids.insert(current_path.clone(), nid);
                nid
            }
        } else {
            NID_ROOT_FOLDER
        };

        let non_empty: Vec<&str> = segments.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        let effective: Vec<&str> = if non_empty.is_empty() { vec!["Inbox"] } else { non_empty };

        for seg in effective {
            current_path.push(seg.to_string());
            if let Some(&nid) = self.folder_path_nids.get(&current_path) {
                parent_nid = nid;
            } else {
                let nid = self.add_folder_with_parent_nid(seg, parent_nid)?;
                self.folder_path_nids.insert(current_path.clone(), nid);
                parent_nid = nid;
            }
        }

        Ok(parent_nid)
    }

    pub fn add_message(&mut self, msg: &EmailMessage, folder_nid: u32) -> io::Result<u32> {
        let nid = self.alloc_nid_counter;
        self.alloc_nid_counter += 0x20;

        let mut pc = PropertyContext::new();
        let msg_class = msg
            .mapi_properties
            .iter()
            .find(|p| p.tag == PR_MESSAGE_CLASS_W)
            .map(|p| p.value.as_str())
            .unwrap_or("IPM.Note");
        pc.add_string_w(PR_MESSAGE_CLASS_W, msg_class);
        pc.add_string_w(PR_SUBJECT_W, &msg.subject);
        pc.add_string_w(PR_BODY_W, &msg.body_text);
        if let Some(ref html) = msg.body_html {
            pc.add_binary(PR_BODY_HTML_W, html.as_bytes());
        }
        pc.add_string_w(PR_SENDER_NAME_W, &msg.from.name);
        pc.add_string_w(PR_SENDER_EMAIL_ADDRESS_W, &msg.from.email);
        if !msg.message_id.is_empty() {
            pc.add_string_w(PR_INTERNET_MESSAGE_ID_W, &msg.message_id);
        }
        let mut flags_val = if msg.flags.read { 1u32 } else { 0u32 };
        if msg.flags.replied {
            flags_val |= 0x04;
        }
        if msg.flags.draft {
            flags_val |= 0x08;
        }
        if let Some(flag_prop) = msg.mapi_properties.iter().find(|p| p.tag == PR_MESSAGE_FLAGS)
            && let Ok(parsed_flags) = flag_prop.value.parse::<u32>()
        {
            flags_val |= parsed_flags;
        }
        pc.add_u32(PR_MESSAGE_FLAGS, flags_val);
        pc.add_systime(PR_MESSAGE_DELIVERY_TIME, msg.date);
        pc.add_systime(PR_CLIENT_SUBMIT_TIME, msg.date);
        if !msg.attachments.is_empty() {
            pc.add_bool(PR_HASATTACH, true);
        }
        pc.add_u32(
            PR_IMPORTANCE,
            match msg.importance {
                mail2rust_core::Importance::High => 2,
                mail2rust_core::Importance::Low => 0,
                mail2rust_core::Importance::Normal => 1,
            },
        );
        for prop in &msg.mapi_properties {
            if matches!(prop.tag, PR_MESSAGE_CLASS_W | PR_MESSAGE_FLAGS) {
                continue;
            }
            let ptype = (prop.tag & 0xFFFF) as u16;
            match ptype {
                0x001F => pc.add_string_w(prop.tag, &prop.value),
                0x001E => pc.add_string_a(prop.tag, &prop.value),
                0x0003 => {
                    if let Ok(v) = prop.value.parse::<u32>() {
                        pc.add_u32(prop.tag, v);
                    }
                }
                0x000B => pc.add_bool(prop.tag, prop.value == "true" || prop.value == "1"),
                _ => {}
            }
        }

        let join_addresses = |addrs: &[mail2rust_core::EmailAddress]| {
            addrs.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
        };
        if !msg.to.is_empty() {
            pc.add_string_w(PR_DISPLAY_TO_W, &join_addresses(&msg.to));
        }
        if !msg.cc.is_empty() {
            pc.add_string_w(PR_DISPLAY_CC_W, &join_addresses(&msg.cc));
        }
        if !msg.bcc.is_empty() {
            pc.add_string_w(PR_DISPLAY_BCC_W, &join_addresses(&msg.bcc));
        }

        // Lossless preservation blob split into 4 KiB chunks. If large attachment
        // payloads push the JSON blob past the single-PC limit, defer their raw
        // bytes to the SLBLOCK + XBLOCK attachment subnodes written below.
        let mut preserved = PreservedMessage::from_message(msg);
        let mut blob = serde_json::to_vec(&preserved).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        const CHUNK: usize = 4096;
        let exceeds_pc_budget = |len: usize| {
            let count = len.div_ceil(CHUNK);
            len + count * 12 > MAX_PROPERTY_CONTEXT_BYTES
        };
        if exceeds_pc_budget(blob.len()) && !preserved.attachments.is_empty() {
            for att in &mut preserved.attachments {
                att.data.clear();
            }
            blob = serde_json::to_vec(&preserved).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        }
        if exceeds_pc_budget(blob.len()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "message metadata exceeds single PST property context limit",
            ));
        }
        let chunks: Vec<&[u8]> = blob.chunks(CHUNK).collect();
        pc.add_u32(PR_MAIL2RUST_PRESERVED_COUNT, chunks.len() as u32);
        for (i, chunk) in chunks.iter().enumerate() {
            pc.add_binary(mail2rust_chunk_tag(i as u32), chunk);
        }

        let data = pc.serialize()?;
        let bid = self.write_stream_or_block(data)?;

        // Build optional subnodes (Recipient Table + Attachment Table + Attachment PCs)
        let mut subnodes: Vec<(u32, u64, u64)> = Vec::new();
        if !msg.to.is_empty() || !msg.cc.is_empty() || !msg.bcc.is_empty() {
            let mut tc = TableContext::default();
            let mut row_idx = 0u32;
            for (rtype, list) in [(1u32, &msg.to), (2u32, &msg.cc), (3u32, &msg.bcc)] {
                for addr in list {
                    let mut props = HashMap::new();
                    props.insert(PR_RECIPIENT_TYPE, rtype.to_le_bytes().to_vec());
                    let name_w: Vec<u8> = addr
                        .name
                        .encode_utf16()
                        .chain([0])
                        .flat_map(|u| u.to_le_bytes())
                        .collect();
                    let email_w: Vec<u8> = addr
                        .email
                        .encode_utf16()
                        .chain([0])
                        .flat_map(|u| u.to_le_bytes())
                        .collect();
                    props.insert(PR_DISPLAY_NAME_W, name_w);
                    props.insert(PR_EMAIL_ADDRESS_W, email_w.clone());
                    props.insert(PR_SMTP_ADDRESS_W, email_w);
                    tc.rows.push(TableContextRow {
                        row_id: row_idx,
                        properties: props,
                    });
                    row_idx += 1;
                }
            }
            let tc_bytes = tc.serialize_compact()?;
            let tc_bid = self.write_stream_or_block(tc_bytes)?;
            subnodes.push((NID_RECIPIENT_TABLE, tc_bid, 0));
        }

        if !msg.attachments.is_empty() {
            let mut att_tc = TableContext::default();
            for (idx, att) in msg.attachments.iter().enumerate() {
                let att_num = idx as u32;
                let att_nid = ((att_num + 1) << 5) | (NID_TYPE_ATTACHMENT as u32);
                let mut att_pc = PropertyContext::new();
                att_pc.add_u32(PR_ATTACH_NUM, att_num);
                att_pc.add_u32(PR_ATTACH_METHOD, 1); // ATTACH_BY_VALUE
                att_pc.add_u32(PR_ATTACH_SIZE, att.data.len() as u32);
                att_pc.add_string_w(PR_ATTACH_FILENAME_W, &att.filename);
                att_pc.add_string_w(PR_ATTACH_LONG_FILENAME_W, &att.filename);
                att_pc.add_string_w(PR_ATTACH_MIME_TAG_W, &att.mime_type);
                if let Some(ref cid) = att.content_id {
                    att_pc.add_string_w(PR_ATTACH_CONTENT_ID_W, cid);
                    att_pc.add_u32(PR_ATTACH_FLAGS, 0x04);
                } else {
                    att_pc.add_u32(PR_ATTACH_FLAGS, 0);
                }
                att_pc.add_binary(PR_ATTACH_DATA_BIN, &att.data);
                let att_bytes = att_pc.serialize()?;
                let att_bid = self.write_stream_or_block(att_bytes)?;
                subnodes.push((att_nid, att_bid, 0));

                let mut row_props = HashMap::new();
                row_props.insert(PR_ATTACH_NUM, att_num.to_le_bytes().to_vec());
                row_props.insert(PR_ATTACH_SIZE, (att.data.len() as u32).to_le_bytes().to_vec());
                let fname_w: Vec<u8> = att
                    .filename
                    .encode_utf16()
                    .chain([0])
                    .flat_map(|u| u.to_le_bytes())
                    .collect();
                row_props.insert(PR_ATTACH_LONG_FILENAME_W, fname_w);
                att_tc.rows.push(TableContextRow {
                    row_id: att_num,
                    properties: row_props,
                });
            }
            let att_tc_bytes = att_tc.serialize_compact()?;
            let att_tc_bid = self.write_stream_or_block(att_tc_bytes)?;
            subnodes.push((NID_ATTACHMENT_TABLE, att_tc_bid, 0));
        }

        let bid_sub = if subnodes.is_empty() {
            0
        } else {
            subnodes.sort_by_key(|s| s.0);
            let mut slblock = Vec::with_capacity(8 + subnodes.len() * 24);
            slblock.push(0x02); // bType = SLBLOCK
            slblock.push(0x00); // cLevel = 0
            slblock.extend_from_slice(&(subnodes.len() as u16).to_le_bytes());
            slblock.extend_from_slice(&0u32.to_le_bytes());
            for (snid, sbid, ssub) in subnodes {
                slblock.extend_from_slice(&(snid as u64).to_le_bytes());
                slblock.extend_from_slice(&sbid.to_le_bytes());
                slblock.extend_from_slice(&ssub.to_le_bytes());
            }
            self.write_internal_block(slblock)?
        };

        self.nbt_entries.push((nid, bid, bid_sub, folder_nid));
        self.folder_messages.entry(folder_nid).or_default().push(nid);
        debug!(nid = nid, subject = %msg.subject, "Injected message into PST");
        Ok(nid)
    }

    /// Writes a custom `PropertyContext` node (such as `IPM.Contact` or `IPM.DistList`) into `folder_nid`.
    pub fn add_property_node(&mut self, pc: &PropertyContext, folder_nid: u32) -> io::Result<u32> {
        let nid = self.alloc_nid_counter;
        self.alloc_nid_counter += 0x20;
        let data = pc.serialize()?;
        let bid = self.write_stream_or_block(data)?;
        self.nbt_entries.push((nid, bid, 0, folder_nid));
        Ok(nid)
    }

    pub fn finalize(&mut self) -> io::Result<()> {
        info!("Finalizing PST file and constructing B-Trees (NBT/BBT)");

        // 1. Create Message Store node (NID_MESSAGE_STORE = 0x21) with optional password CRC
        let mut store_pc = PropertyContext::new();
        store_pc.add_string_w(PR_DISPLAY_NAME_W, "Personal Folders");
        if let Some(ref pwd) = self.password {
            let pwd_crc = compute_pst_password_crc(pwd);
            store_pc.add_u32(PR_PST_PASSWORD, pwd_crc);
        }
        let store_data = store_pc.serialize()?;
        let store_bid = self.write_block(store_data)?;
        self.nbt_entries.push((NID_MESSAGE_STORE, store_bid, 0, 0));

        // 2. Ensure Root Folder (NID_ROOT_FOLDER = 0x122) exists
        if !self.nbt_entries.iter().any(|(nid, _, _, _)| *nid == NID_ROOT_FOLDER) {
            let mut root_pc = PropertyContext::new();
            root_pc.add_string_w(PR_DISPLAY_NAME_W, "Top of Personal Folders");
            root_pc.add_bool(PR_SUBFOLDERS, true);
            root_pc.add_u32(PR_CONTENT_COUNT, 0);
            root_pc.add_u32(PR_CONTENT_UNREAD, 0);
            let root_data = root_pc.serialize()?;
            let root_bid = self.write_block(root_data)?;
            // libpff identifies the root folder by an NBT parent identifier that points to itself.
            self.nbt_entries.push((NID_ROOT_FOLDER, root_bid, 0, NID_ROOT_FOLDER));
        }

        // 2b. Emit 0x7C hierarchy and contents tables (NID + 11 / NID + 12) for conforming readers
        let children = self.folder_children.clone();
        let messages = self.folder_messages.clone();
        for (folder_nid, child_ids) in children {
            let data = build_subitem_table_7c(&child_ids);
            let bid = self.write_block(data)?;
            self.nbt_entries.push((folder_nid + 11, bid, 0, folder_nid));
        }
        for (folder_nid, msg_ids) in messages {
            let data = build_subitem_table_7c(&msg_ids);
            let bid = self.write_block(data)?;
            self.nbt_entries.push((folder_nid + 12, bid, 0, folder_nid));
        }

        // Align offset to 512-byte page boundary
        let pad_nbt = (PST_PAGE_SIZE as u64 - (self.current_offset % PST_PAGE_SIZE as u64)) % PST_PAGE_SIZE as u64;
        if pad_nbt > 0 {
            self.writer.seek(SeekFrom::Start(self.current_offset))?;
            self.writer.write_all(&vec![0u8; pad_nbt as usize])?;
            self.current_offset += pad_nbt;
        }

        // 3. Construct Node B-Tree (NBT) using 512-byte BTPAGE structures
        self.nbt_entries.sort_by_key(|e| e.0);
        let nbt_entries = self.nbt_entries.clone();
        let mut current_nbt_pages: Vec<(u64, u64, u64)> = Vec::new(); // (key, page_bid, page_offset)

        for chunk in nbt_entries.chunks(15) {
            let page_bid = self.alloc_page_bid();
            let page_offset = self.current_offset;
            let mut page = vec![0u8; PST_PAGE_SIZE];

            for (idx, (nid, bid, bid_sub, parent_nid)) in chunk.iter().enumerate() {
                let base = idx * 32;
                page[base..base + 8].copy_from_slice(&(*nid as u64).to_le_bytes());
                page[base + 8..base + 16].copy_from_slice(&bid.to_le_bytes());
                page[base + 16..base + 24].copy_from_slice(&bid_sub.to_le_bytes());
                page[base + 24..base + 28].copy_from_slice(&parent_nid.to_le_bytes());
                page[base + 28..base + 32].copy_from_slice(&0u32.to_le_bytes());
                // dwPadding
            }

            page[488] = chunk.len() as u8; // cEnt
            page[489] = 15; // cEntMax
            page[490] = 32; // cbEnt
            page[491] = 0; // cLevel = 0 (leaf)

            let trailer = PageTrailer::new(PTYPE_NBT, page_bid, &page[0..PST_PAGE_DATA_SIZE]);
            trailer.write_to_slice(&mut page[PST_PAGE_DATA_SIZE..PST_PAGE_SIZE]);

            self.writer.seek(SeekFrom::Start(page_offset))?;
            self.writer.write_all(&page)?;
            self.current_offset += PST_PAGE_SIZE as u64;

            current_nbt_pages.push((chunk[0].0 as u64, page_bid, page_offset));
        }

        let mut nbt_level = 1u8;
        while current_nbt_pages.len() > 1 {
            let mut next_level_pages: Vec<(u64, u64, u64)> = Vec::new();
            for chunk in current_nbt_pages.chunks(20) {
                let page_bid = self.alloc_page_bid();
                let page_offset = self.current_offset;
                let mut page = vec![0u8; PST_PAGE_SIZE];

                for (idx, (key, child_bid, child_offset)) in chunk.iter().enumerate() {
                    let base = idx * 24;
                    page[base..base + 8].copy_from_slice(&key.to_le_bytes());
                    page[base + 8..base + 16].copy_from_slice(&child_bid.to_le_bytes());
                    page[base + 16..base + 24].copy_from_slice(&child_offset.to_le_bytes());
                }

                page[488] = chunk.len() as u8;
                page[489] = 20;
                page[490] = 24;
                page[491] = nbt_level;

                let trailer = PageTrailer::new(PTYPE_NBT, page_bid, &page[0..PST_PAGE_DATA_SIZE]);
                trailer.write_to_slice(&mut page[PST_PAGE_DATA_SIZE..PST_PAGE_SIZE]);

                self.writer.seek(SeekFrom::Start(page_offset))?;
                self.writer.write_all(&page)?;
                self.current_offset += PST_PAGE_SIZE as u64;

                next_level_pages.push((chunk[0].0, page_bid, page_offset));
            }
            current_nbt_pages = next_level_pages;
            nbt_level += 1;
        }

        let (nbt_root_bid, nbt_root_offset) = if let Some(first) = current_nbt_pages.first() {
            (first.1, first.2)
        } else {
            (0, 0)
        };

        // Align offset to 512-byte page boundary for BBT
        let pad_bbt = (PST_PAGE_SIZE as u64 - (self.current_offset % PST_PAGE_SIZE as u64)) % PST_PAGE_SIZE as u64;
        if pad_bbt > 0 {
            self.writer.seek(SeekFrom::Start(self.current_offset))?;
            self.writer.write_all(&vec![0u8; pad_bbt as usize])?;
            self.current_offset += pad_bbt;
        }

        // 4. Construct Block B-Tree (BBT) using 512-byte BTPAGE structures
        self.bbt_entries.sort_by_key(|e| e.0);
        let bbt_entries = self.bbt_entries.clone();
        let mut current_bbt_pages: Vec<(u64, u64, u64)> = Vec::new();

        for chunk in bbt_entries.chunks(20) {
            let page_bid = self.alloc_page_bid();
            let page_offset = self.current_offset;
            let mut page = vec![0u8; PST_PAGE_SIZE];

            for (idx, (bid, off, len)) in chunk.iter().enumerate() {
                let base = idx * 24;
                page[base..base + 8].copy_from_slice(&bid.to_le_bytes());
                page[base + 8..base + 16].copy_from_slice(&off.to_le_bytes());
                page[base + 16..base + 18].copy_from_slice(&(*len as u16).to_le_bytes());
                page[base + 18..base + 20].copy_from_slice(&1u16.to_le_bytes()); // cRef = 1
                page[base + 20..base + 24].copy_from_slice(&0u32.to_le_bytes());
                // dwPadding = 0
            }

            page[488] = chunk.len() as u8;
            page[489] = 20;
            page[490] = 24;
            page[491] = 0;

            let trailer = PageTrailer::new(PTYPE_BBT, page_bid, &page[0..PST_PAGE_DATA_SIZE]);
            trailer.write_to_slice(&mut page[PST_PAGE_DATA_SIZE..PST_PAGE_SIZE]);

            self.writer.seek(SeekFrom::Start(page_offset))?;
            self.writer.write_all(&page)?;
            self.current_offset += PST_PAGE_SIZE as u64;

            current_bbt_pages.push((chunk[0].0, page_bid, page_offset));
        }

        let mut bbt_level = 1u8;
        while current_bbt_pages.len() > 1 {
            let mut next_level_pages: Vec<(u64, u64, u64)> = Vec::new();
            for chunk in current_bbt_pages.chunks(20) {
                let page_bid = self.alloc_page_bid();
                let page_offset = self.current_offset;
                let mut page = vec![0u8; PST_PAGE_SIZE];

                for (idx, (key, child_bid, child_offset)) in chunk.iter().enumerate() {
                    let base = idx * 24;
                    page[base..base + 8].copy_from_slice(&key.to_le_bytes());
                    page[base + 8..base + 16].copy_from_slice(&child_bid.to_le_bytes());
                    page[base + 16..base + 24].copy_from_slice(&child_offset.to_le_bytes());
                }

                page[488] = chunk.len() as u8;
                page[489] = 20;
                page[490] = 24;
                page[491] = bbt_level;

                let trailer = PageTrailer::new(PTYPE_BBT, page_bid, &page[0..PST_PAGE_DATA_SIZE]);
                trailer.write_to_slice(&mut page[PST_PAGE_DATA_SIZE..PST_PAGE_SIZE]);

                self.writer.seek(SeekFrom::Start(page_offset))?;
                self.writer.write_all(&page)?;
                self.current_offset += PST_PAGE_SIZE as u64;

                next_level_pages.push((chunk[0].0, page_bid, page_offset));
            }
            current_bbt_pages = next_level_pages;
            bbt_level += 1;
        }

        let (bbt_root_bid, bbt_root_offset) = if let Some(first) = current_bbt_pages.first() {
            (first.1, first.2)
        } else {
            (0, 0)
        };

        let file_len = self.current_offset;

        // 5. Write AMap Page at 0x4400
        let mut amap_page = vec![0u8; PST_PAGE_SIZE];
        let blocks_alloc = (file_len / 64) as usize;
        let full_amap_bytes = (blocks_alloc / 8).min(PST_PAGE_DATA_SIZE);
        for b in &mut amap_page[0..full_amap_bytes] {
            *b = 0xFF;
        }
        if full_amap_bytes < PST_PAGE_DATA_SIZE {
            let rem_bits = blocks_alloc % 8;
            if rem_bits > 0 {
                amap_page[full_amap_bytes] = (1u8 << rem_bits) - 1;
            }
        }
        let amap_trailer = PageTrailer::new(PTYPE_AMAP, 0x0002, &amap_page[0..PST_PAGE_DATA_SIZE]);
        amap_trailer.write_to_slice(&mut amap_page[PST_PAGE_DATA_SIZE..PST_PAGE_SIZE]);
        self.writer.seek(SeekFrom::Start(AMAP_START_OFFSET))?;
        self.writer.write_all(&amap_page)?;

        // 6. Write PMap Page at 0x4600
        let mut pmap_page = vec![0u8; PST_PAGE_SIZE];
        let pages_alloc = (file_len / PST_PAGE_SIZE as u64) as usize;
        let full_pmap_bytes = (pages_alloc / 8).min(PST_PAGE_DATA_SIZE);
        for b in &mut pmap_page[0..full_pmap_bytes] {
            *b = 0xFF;
        }
        if full_pmap_bytes < PST_PAGE_DATA_SIZE {
            let rem_bits = pages_alloc % 8;
            if rem_bits > 0 {
                pmap_page[full_pmap_bytes] = (1u8 << rem_bits) - 1;
            }
        }
        let pmap_trailer = PageTrailer::new(PTYPE_PMAP, 0x0006, &pmap_page[0..PST_PAGE_DATA_SIZE]);
        pmap_trailer.write_to_slice(&mut pmap_page[PST_PAGE_DATA_SIZE..PST_PAGE_SIZE]);
        self.writer.seek(SeekFrom::Start(PMAP_START_OFFSET))?;
        self.writer.write_all(&pmap_page)?;

        // 7. Assemble and write the 512-byte MS-PST Header at offset 0
        let mut header = [0u8; PST_HEADER_SIZE];

        // dwMagic: "!BDN" = [0x21, 0x42, 0x44, 0x4E]
        header[0x00..0x04].copy_from_slice(&[0x21, 0x42, 0x44, 0x4E]);

        // wMagicClient: "SM" = 0x534D
        header[0x08..0x0A].copy_from_slice(&PST_CLIENT_MAGIC.to_le_bytes());

        // wVer: 23 (Unicode PST)
        header[0x0A..0x0C].copy_from_slice(&PST_UNICODE_VER.to_le_bytes());

        // wVerClient: 19
        header[0x0C..0x0E].copy_from_slice(&PST_CLIENT_VER.to_le_bytes());

        // Platform create & access: 1 (Windows)
        header[0x0E] = 0x01;
        header[0x0F] = 0x01;

        // bidNextB: next allocation block ID
        header[0x18..0x20].copy_from_slice(&self.alloc_bid_counter.to_le_bytes());

        // bidNextP: next allocation page ID
        header[0x20..0x28].copy_from_slice(&self.alloc_page_bid_counter.to_le_bytes());

        // dwUnique
        header[0x28..0x2C].copy_from_slice(&1u32.to_le_bytes());

        // rgnid[0]: allocation NID high-water mark
        header[0x2C..0x30].copy_from_slice(&self.alloc_nid_counter.to_le_bytes());

        // MS-PST 64-bit Unicode ROOT structure at 0xB4.
        header[0xB4..0xB8].copy_from_slice(&0u32.to_le_bytes()); // cOrphans
        header[0xB8..0xC0].copy_from_slice(&file_len.to_le_bytes()); // ibFileEof
        header[0xC0..0xC8].copy_from_slice(&AMAP_START_OFFSET.to_le_bytes()); // ibAMapLast
        // cbAMapFree / cbPMapFree (claim 0 free bytes; all allocated pages are tracked)
        header[0xC8..0xD0].copy_from_slice(&0u64.to_le_bytes());
        header[0xD0..0xD8].copy_from_slice(&0u64.to_le_bytes());
        header[0xD8..0xE0].copy_from_slice(&nbt_root_bid.to_le_bytes()); // bRefNBT
        header[0xE0..0xE8].copy_from_slice(&nbt_root_offset.to_le_bytes()); // ibNBT
        header[0xE8..0xF0].copy_from_slice(&bbt_root_bid.to_le_bytes()); // bRefBBT
        header[0xF0..0xF8].copy_from_slice(&bbt_root_offset.to_le_bytes()); // ibBBT
        header[0xF8] = 0x02; // fAMapValid = VALID_AMAP2
        header[0xF9] = 0x00; // bARVec
        header[0xFA..0xFC].copy_from_slice(&0u16.to_le_bytes()); // cARVec
        header[0xFC..0x100].copy_from_slice(&0u32.to_le_bytes()); // dwAlign

        // bSentinel & bCryptMethod at the Unicode positions.
        header[0x200] = 0x80;
        header[0x201] = self.encryption.as_u8();
        // bidNextB at 0x204
        header[0x204..0x20C].copy_from_slice(&self.alloc_bid_counter.to_le_bytes());

        // dwCRCPartial (weak CRC over bytes 0x08..0x1DF) written at 0x04
        let crc = calculate_partial_header_crc(&header);
        header[0x04..0x08].copy_from_slice(&crc.to_le_bytes());

        // dwCRCFull at 0x20C: weak CRC over bytes 0x08..0x20C (516 bytes).
        let full_crc = weak_crc32(&header[0x08..0x20C]);
        header[0x20C..0x210].copy_from_slice(&full_crc.to_le_bytes());

        // Write header to start of file
        self.writer.seek(SeekFrom::Start(0))?;
        self.writer.write_all(&header)?;
        self.writer.flush()?;

        info!(
            crc = format!("0x{:08X}", crc),
            encryption = ?self.encryption,
            nodes = self.nbt_entries.len(),
            blocks = self.bbt_entries.len(),
            nbt_root_bid = nbt_root_bid,
            nbt_root_offset = nbt_root_offset,
            bbt_root_bid = bbt_root_bid,
            bbt_root_offset = bbt_root_offset,
            file_len = file_len,
            "MS-PST 512-byte BTPAGE & PAGETRAILER construction finalized successfully"
        );
        Ok(())
    }
}
