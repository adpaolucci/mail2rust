use crate::ltp::{
    PropertyContext, PR_BODY_HTML_W, PR_BODY_W, PR_CONTENT_COUNT, PR_CONTENT_UNREAD, PR_DISPLAY_NAME_W, PR_HASATTACH,
    PR_IMPORTANCE, PR_INTERNET_MESSAGE_ID_W, PR_MESSAGE_CLASS_W, PR_MESSAGE_FLAGS, PR_PST_PASSWORD,
    PR_SENDER_EMAIL_ADDRESS_W, PR_SENDER_NAME_W, PR_SUBFOLDERS, PR_SUBJECT_W,
};
use crate::ndb::{
    calculate_partial_header_crc, compute_pst_password_crc, cyclic_crypt, permute_bytes, PageTrailer, PstEncryption,
    AMAP_START_OFFSET, DATA_START_OFFSET, NID_MESSAGE_STORE, NID_ROOT_FOLDER, PMAP_START_OFFSET, PST_CLIENT_MAGIC,
    PST_CLIENT_VER, PST_HEADER_SIZE, PST_PAGE_DATA_SIZE, PST_PAGE_SIZE, PST_UNICODE_VER, PTYPE_AMAP, PTYPE_BBT,
    PTYPE_NBT, PTYPE_PMAP,
};
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
    nbt_entries: Vec<(u32, u64, u32)>, // (NID, BID, ParentNID)
    bbt_entries: Vec<(u64, u64, u32)>, // (BID, FileOffset, ByteLength)
    folder_map: HashMap<String, u32>,
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
        // for HEADER (0x0000..0x0200), AMap (0x4400..0x4600), and PMap (0x4600..0x4800)
        let init_buf = vec![0u8; DATA_START_OFFSET as usize];
        writer.seek(SeekFrom::Start(0))?;
        writer.write_all(&init_buf)?;

        Ok(Self {
            writer,
            current_offset: DATA_START_OFFSET,
            alloc_nid_counter: 0x8000,
            alloc_bid_counter: 0x0080,
            alloc_page_bid_counter: 0x0042,
            nbt_entries: Vec::new(),
            bbt_entries: Vec::new(),
            folder_map: HashMap::new(),
            encryption,
            password,
        })
    }

    pub fn alloc_page_bid(&mut self) -> u64 {
        let bid = self.alloc_page_bid_counter;
        self.alloc_page_bid_counter += 4;
        bid
    }

    pub fn write_block(&mut self, mut data: Vec<u8>) -> io::Result<u64> {
        let bid = self.alloc_bid_counter;
        self.alloc_bid_counter += 4; // BIDs increment by 4 in Unicode NDB

        match self.encryption {
            PstEncryption::None => {}
            PstEncryption::Permute => permute_bytes(&mut data),
            PstEncryption::Cyclic => cyclic_crypt(&mut data, bid),
        }

        let offset = self.current_offset;
        let len = data.len() as u32;

        self.writer.seek(SeekFrom::Start(offset))?;
        self.writer.write_all(&data)?;

        // Padding to 64-byte boundary
        let pad = (64 - (len % 64)) % 64;
        if pad > 0 {
            let pad_bytes = vec![0u8; pad as usize];
            self.writer.write_all(&pad_bytes)?;
        }
        self.current_offset = offset + len as u64 + pad as u64;

        self.bbt_entries.push((bid, offset, len));
        Ok(bid)
    }

    pub fn add_folder(&mut self, name: &str, parent_path: Option<&str>) -> io::Result<u32> {
        let nid = self.alloc_nid_counter;
        self.alloc_nid_counter += 1;

        let parent_nid = if let Some(p) = parent_path {
            *self.folder_map.get(p).unwrap_or(&NID_ROOT_FOLDER)
        } else {
            NID_ROOT_FOLDER
        };

        let mut pc = PropertyContext::new();
        pc.add_string_w(PR_DISPLAY_NAME_W, name);
        pc.add_bool(PR_SUBFOLDERS, false);
        pc.add_u32(PR_CONTENT_COUNT, 0);
        pc.add_u32(PR_CONTENT_UNREAD, 0);

        let data = pc.serialize()?;
        let bid = self.write_block(data)?;

        self.nbt_entries.push((nid, bid, parent_nid));
        self.folder_map.insert(name.to_string(), nid);
        debug!(nid = nid, name = name, "Added folder to PST");
        Ok(nid)
    }

    pub fn add_message(&mut self, msg: &EmailMessage, folder_nid: u32) -> io::Result<u32> {
        let nid = self.alloc_nid_counter;
        self.alloc_nid_counter += 1;

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
        pc.add_u32(PR_MESSAGE_FLAGS, if msg.flags.read { 1 } else { 0 });
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

        let data = pc.serialize()?;
        let bid = self.write_block(data)?;

        self.nbt_entries.push((nid, bid, folder_nid));
        debug!(nid = nid, subject = %msg.subject, "Injected message into PST");
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
        self.nbt_entries.push((NID_MESSAGE_STORE, store_bid, 0));

        // 2. Ensure Root Folder (NID_ROOT_FOLDER = 0x122) exists
        if !self.nbt_entries.iter().any(|(nid, _, _)| *nid == NID_ROOT_FOLDER) {
            let mut root_pc = PropertyContext::new();
            root_pc.add_string_w(PR_DISPLAY_NAME_W, "Top of Personal Folders");
            root_pc.add_bool(PR_SUBFOLDERS, true);
            root_pc.add_u32(PR_CONTENT_COUNT, 0);
            root_pc.add_u32(PR_CONTENT_UNREAD, 0);
            let root_data = root_pc.serialize()?;
            let root_bid = self.write_block(root_data)?;
            self.nbt_entries.push((NID_ROOT_FOLDER, root_bid, 0));
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

            for (idx, (nid, bid, parent_nid)) in chunk.iter().enumerate() {
                let base = idx * 32;
                page[base..base + 8].copy_from_slice(&(*nid as u64).to_le_bytes());
                page[base + 8..base + 16].copy_from_slice(&bid.to_le_bytes());
                page[base + 16..base + 24].copy_from_slice(&0u64.to_le_bytes()); // bidSub
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

        // Legacy compatibility BREF pointers at 0xB4 & 0xC4
        header[0xB4..0xBC].copy_from_slice(&nbt_root_bid.to_le_bytes());
        header[0xBC..0xC4].copy_from_slice(&nbt_root_offset.to_le_bytes());
        header[0xC4..0xCC].copy_from_slice(&bbt_root_bid.to_le_bytes());
        header[0xCC..0xD4].copy_from_slice(&bbt_root_offset.to_le_bytes());

        // Standard MS-PST Unicode ROOT structure at offset 0xB8:
        // - 0xBC..0xC4: ibFileEof (file length)
        // - 0xC4..0xCC: ibAMapLast (0x4400)
        // - 0xDC..0xE4: BREFNBT.bid
        // - 0xE4..0xEC: BREFNBT.ib
        // - 0xEC..0xF4: BREFBBT.bid
        // - 0xF4..0xFC: BREFBBT.ib
        // - 0xFC: fAMapValid (0x01)
        header[0xBC..0xC4].copy_from_slice(&file_len.to_le_bytes());
        header[0xC4..0xCC].copy_from_slice(&AMAP_START_OFFSET.to_le_bytes());
        header[0xDC..0xE4].copy_from_slice(&nbt_root_bid.to_le_bytes());
        header[0xE4..0xEC].copy_from_slice(&nbt_root_offset.to_le_bytes());
        header[0xEC..0xF4].copy_from_slice(&bbt_root_bid.to_le_bytes());
        header[0xF4..0xFC].copy_from_slice(&bbt_root_offset.to_le_bytes());
        header[0xFC] = 0x01; // fAMapValid = 1 (valid)

        // bSentinel & bCryptMethod (both ANSI 0x1CD and Unicode 0x201 locations)
        header[0x1CC] = 0x80;
        header[0x1CD] = self.encryption.as_u8();

        // Calculate dwCRCPartial (over bytes 0x08..0x1DF) and write at 0x04
        let crc = calculate_partial_header_crc(&header);
        header[0x04..0x08].copy_from_slice(&crc.to_le_bytes());

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
