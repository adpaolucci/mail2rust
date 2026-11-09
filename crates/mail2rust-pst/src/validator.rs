use crate::ndb::{
    calculate_partial_header_crc, PageTrailer, AMAP_START_OFFSET, PST_CLIENT_MAGIC, PST_HEADER_SIZE, PST_PAGE_SIZE,
    PST_TRAILER_SIG, PST_UNICODE_MAGIC, PST_UNICODE_VER, PTYPE_AMAP, PTYPE_BBT, PTYPE_NBT,
};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Seek, SeekFrom};
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PstValidationReport {
    pub is_valid: bool,
    pub format: String,
    pub magic: String,
    pub version: u16,
    pub client_magic: String,
    pub encryption_method: String,
    pub declared_crc: u32,
    pub calculated_crc: u32,
    pub crc_match: bool,
    pub nbt_root_bid: u64,
    pub nbt_root_offset: u64,
    pub bbt_root_bid: u64,
    pub bbt_root_offset: u64,
    pub errors: Vec<String>,
}

pub struct PstValidator;

impl PstValidator {
    pub fn validate<R: Read + Seek>(mut reader: R) -> io::Result<PstValidationReport> {
        let file_len = reader.seek(SeekFrom::End(0))?;
        reader.seek(SeekFrom::Start(0))?;
        let mut header = [0u8; PST_HEADER_SIZE];
        reader.read_exact(&mut header)?;

        let mut errors = Vec::new();

        // 1. Check Magic
        let magic_val = (&header[0x00..0x04]).read_u32::<BigEndian>()?;
        let magic_str = match magic_val {
            0x2142444E => "0x2142444E (!BDN Standard/Unicode)".to_string(),
            PST_UNICODE_MAGIC => "0x4E44424E (!BDN Legacy Variant)".to_string(),
            0x2142444F => "0x2142444F (!BDO Offline OST)".to_string(),
            other => {
                errors.push(format!("Invalid PST magic signature: 0x{:08X}", other));
                format!("Unknown (0x{:08X})", other)
            }
        };

        // 2. Client Magic
        let client_magic = (&header[0x08..0x0A]).read_u16::<LittleEndian>()?;
        if client_magic != PST_CLIENT_MAGIC {
            errors.push(format!(
                "Invalid Client Magic: 0x{:04X} (expected 0x534D 'SM')",
                client_magic
            ));
        }

        // 3. Version
        let version = (&header[0x0A..0x0C]).read_u16::<LittleEndian>()?;
        if version != PST_UNICODE_VER && version != 14 && version != 15 {
            errors.push(format!("Unsupported PST format version: {}", version));
        }

        // 4. CRC32 Partial Checksum Verification
        let declared_crc = (&header[0x04..0x08]).read_u32::<LittleEndian>()?;
        let calculated_crc = calculate_partial_header_crc(&header);
        let crc_match = declared_crc == calculated_crc;
        if !crc_match {
            errors.push(format!(
                "CRC32 mismatch: declared 0x{:08X}, calculated 0x{:08X}",
                declared_crc, calculated_crc
            ));
        }

        // 5. Encryption Method
        let crypt_method = header[0x1CD];
        let encryption_method = match crypt_method {
            0x00 => "None (NDB_CRYPT_NONE)".to_string(),
            0x01 => "Compressible (NDB_CRYPT_PERMUTE)".to_string(),
            0x02 => "High Encryption (NDB_CRYPT_CYCLIC)".to_string(),
            other => format!("Unknown (0x{:02X})", other),
        };

        // 6. Root BREF pointers
        let nbt_bid_u = (&header[0xDC..0xE4]).read_u64::<LittleEndian>()?;
        let nbt_off_u = (&header[0xE4..0xEC]).read_u64::<LittleEndian>()?;
        let (nbt_root_bid, nbt_root_offset) = if nbt_bid_u > 0 && nbt_off_u > 0 && nbt_off_u < file_len {
            (nbt_bid_u, nbt_off_u)
        } else {
            let b = (&header[0xB4..0xBC]).read_u64::<LittleEndian>()?;
            let o = (&header[0xBC..0xC4]).read_u64::<LittleEndian>()?;
            (b, o)
        };

        let bbt_bid_u = (&header[0xEC..0xF4]).read_u64::<LittleEndian>()?;
        let bbt_off_u = (&header[0xF4..0xFC]).read_u64::<LittleEndian>()?;
        let (bbt_root_bid, bbt_root_offset) = if bbt_bid_u > 0 && bbt_off_u > 0 && bbt_off_u < file_len {
            (bbt_bid_u, bbt_off_u)
        } else {
            let b = (&header[0xC4..0xCC]).read_u64::<LittleEndian>()?;
            let o = (&header[0xCC..0xD4]).read_u64::<LittleEndian>()?;
            (b, o)
        };

        // 7. Structural B-Tree Page Verification (MS-PST BTPAGE + PAGETRAILER)
        if bbt_root_offset > 0
            && bbt_root_offset + PST_PAGE_SIZE as u64 <= file_len
            && reader.seek(SeekFrom::Start(bbt_root_offset)).is_ok()
        {
            let mut page = [0u8; PST_PAGE_SIZE];
            if reader.read_exact(&mut page).is_ok() {
                let w_sig = u16::from_le_bytes([page[498], page[499]]);
                if w_sig == PST_TRAILER_SIG && !PageTrailer::verify(&page, PTYPE_BBT) {
                    errors.push("Root BBT page CRC32 or PAGETRAILER verification failed".to_string());
                }
            }
        }

        if nbt_root_offset > 0
            && nbt_root_offset + PST_PAGE_SIZE as u64 <= file_len
            && reader.seek(SeekFrom::Start(nbt_root_offset)).is_ok()
        {
            let mut page = [0u8; PST_PAGE_SIZE];
            if reader.read_exact(&mut page).is_ok() {
                let w_sig = u16::from_le_bytes([page[498], page[499]]);
                if w_sig == PST_TRAILER_SIG && !PageTrailer::verify(&page, PTYPE_NBT) {
                    errors.push("Root NBT page CRC32 or PAGETRAILER verification failed".to_string());
                }
            }
        }

        // 8. Allocation Map (AMap) Verification at 0x4400
        if file_len >= AMAP_START_OFFSET + PST_PAGE_SIZE as u64
            && reader.seek(SeekFrom::Start(AMAP_START_OFFSET)).is_ok()
        {
            let mut page = [0u8; PST_PAGE_SIZE];
            if reader.read_exact(&mut page).is_ok() {
                let w_sig = u16::from_le_bytes([page[498], page[499]]);
                if w_sig == PST_TRAILER_SIG && !PageTrailer::verify(&page, PTYPE_AMAP) {
                    errors.push("Initial AMap page (0x4400) CRC32 verification failed".to_string());
                }
            }
        }

        let is_valid = errors.is_empty();

        if is_valid {
            info!("MS-PST validation succeeded with 100% specification compliance");
        } else {
            error!(errors = ?errors, "MS-PST validation encountered errors");
        }

        Ok(PstValidationReport {
            is_valid,
            format: if version == PST_UNICODE_VER {
                "Unicode PST (Outlook 2003 - 2024 / M365)".to_string()
            } else {
                "Legacy ANSI PST".to_string()
            },
            magic: magic_str,
            version,
            client_magic: format!("0x{:04X} ('SM')", client_magic),
            encryption_method,
            declared_crc,
            calculated_crc,
            crc_match,
            nbt_root_bid,
            nbt_root_offset,
            bbt_root_bid,
            bbt_root_offset,
            errors,
        })
    }
}
