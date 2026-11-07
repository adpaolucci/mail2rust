use crate::ltp::{
    PropertyContext, PR_BODY_HTML_W, PR_BODY_W, PR_CONTENT_COUNT, PR_CONTENT_UNREAD, PR_DISPLAY_NAME_W, PR_HASATTACH,
    PR_IMPORTANCE, PR_INTERNET_MESSAGE_ID_W, PR_MESSAGE_CLASS_W, PR_MESSAGE_FLAGS, PR_SENDER_EMAIL_ADDRESS_W,
    PR_SENDER_NAME_W, PR_SUBFOLDERS, PR_SUBJECT_W,
};
use crate::ndb::{
    calculate_partial_header_crc, permute_bytes, NID_ROOT_FOLDER, PST_CLIENT_MAGIC, PST_CLIENT_VER, PST_HEADER_SIZE,
    PST_UNICODE_MAGIC, PST_UNICODE_VER,
};
use byteorder::{LittleEndian, WriteBytesExt};
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
    nbt_entries: Vec<(u32, u64, u32)>, // (NID, BID, ParentNID)
    bbt_entries: Vec<(u64, u64, u32)>, // (BID, FileOffset, ByteLength)
    folder_map: HashMap<String, u32>,
    use_compression: bool,
}

impl<W: Write + Seek> PstWriter<W> {
    pub fn new(mut writer: W, use_compression: bool) -> io::Result<Self> {
        info!("Initializing MS-PST 64-bit Unicode Personal Folders container");
        // Reserve 512 bytes for header
        let zero_hdr = [0u8; PST_HEADER_SIZE];
        writer.write_all(&zero_hdr)?;

        Ok(Self {
            writer,
            current_offset: PST_HEADER_SIZE as u64,
            alloc_nid_counter: 0x8000,
            alloc_bid_counter: 0x0080,
            nbt_entries: Vec::new(),
            bbt_entries: Vec::new(),
            folder_map: HashMap::new(),
            use_compression,
        })
    }

    pub fn write_block(&mut self, mut data: Vec<u8>) -> io::Result<u64> {
        let bid = self.alloc_bid_counter;
        self.alloc_bid_counter += 4; // BIDs increment by 4 in Unicode NDB

        if self.use_compression {
            permute_bytes(&mut data);
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

        // 1. Write NBT (Node B-Tree root block)
        let mut nbt_buf = Vec::new();
        for (nid, bid, parent_nid) in &self.nbt_entries {
            nbt_buf.write_u32::<LittleEndian>(*nid)?;
            nbt_buf.write_u64::<LittleEndian>(*bid)?;
            nbt_buf.write_u32::<LittleEndian>(*parent_nid)?;
        }
        let nbt_bid = self.write_block(nbt_buf)?;
        let nbt_offset = self.bbt_entries.iter().find(|(b, _, _)| *b == nbt_bid).unwrap().1;

        // 2. Write BBT (Block B-Tree root block)
        let mut bbt_buf = Vec::new();
        for (bid, off, len) in &self.bbt_entries {
            bbt_buf.write_u64::<LittleEndian>(*bid)?;
            bbt_buf.write_u64::<LittleEndian>(*off)?;
            bbt_buf.write_u32::<LittleEndian>(*len)?;
            bbt_buf.write_u32::<LittleEndian>(1)?; // ref count
        }
        let bbt_bid = self.write_block(bbt_buf)?;
        let bbt_offset = self.bbt_entries.iter().find(|(b, _, _)| *b == bbt_bid).unwrap().1;

        // 3. Assemble and write the 512-byte MS-PST Header
        let mut header = [0u8; PST_HEADER_SIZE];

        // dwMagic: "!BDN" = 0x4E44424E
        header[0x00..0x04].copy_from_slice(&PST_UNICODE_MAGIC.to_be_bytes());

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
        header[0x20..0x28].copy_from_slice(&0x00000040u64.to_le_bytes());

        // dwUnique
        header[0x28..0x2C].copy_from_slice(&1u32.to_le_bytes());

        // Root BREF structure at 0xB4:
        // NBT BREF: bid, offset
        header[0xB4..0xBC].copy_from_slice(&nbt_bid.to_le_bytes());
        header[0xBC..0xC4].copy_from_slice(&nbt_offset.to_le_bytes());

        // BBT BREF: bid, offset
        header[0xC4..0xCC].copy_from_slice(&bbt_bid.to_le_bytes());
        header[0xCC..0xD4].copy_from_slice(&bbt_offset.to_le_bytes());

        // bCryptMethod: 0x01 for Compressible (NDB_CRYPT_PERMUTE), 0x00 for None
        header[0x1CD] = if self.use_compression { 0x01 } else { 0x00 };

        // Calculate dwCRCPartial (over bytes 0x08..0x1DF) and write at 0x04
        let crc = calculate_partial_header_crc(&header);
        header[0x04..0x08].copy_from_slice(&crc.to_le_bytes());

        // Write header to start of file
        self.writer.seek(SeekFrom::Start(0))?;
        self.writer.write_all(&header)?;
        self.writer.flush()?;

        info!(
            crc = format!("0x{:08X}", crc),
            nodes = self.nbt_entries.len(),
            blocks = self.bbt_entries.len(),
            "MS-PST construction finalized successfully"
        );
        Ok(())
    }
}
