use byteorder::{LittleEndian, WriteBytesExt};
use crc32fast::Hasher;
use std::io::{self, Write};

pub const PST_UNICODE_MAGIC: u32 = 0x4E44424E; // "!BDN"
pub const PST_CLIENT_MAGIC: u16 = 0x534D; // "SM"
pub const PST_UNICODE_VER: u16 = 23; // Outlook 2003 - 2024 / M365
pub const PST_CLIENT_VER: u16 = 19;
pub const PST_HEADER_SIZE: usize = 512;

pub const NID_MESSAGE_STORE: u32 = 0x21;
pub const NID_NAME_TO_ID_MAP: u32 = 0x61;
pub const NID_ROOT_FOLDER: u32 = 0x122;

/// Official Microsoft [MS-PST] 5.1 Permutative Encoding Table mpbbR
pub static PERMUTE_TABLE: [u8; 256] = [
    65, 54, 19, 98, 168, 33, 110, 187, 244, 22, 204, 4, 127, 100, 232, 93, 30, 242, 203, 42, 116, 197, 94, 53, 210,
    149, 71, 158, 150, 45, 154, 136, 76, 125, 132, 63, 219, 172, 49, 182, 72, 95, 246, 196, 216, 57, 139, 231, 35, 59,
    56, 142, 200, 193, 223, 37, 177, 32, 165, 70, 96, 78, 156, 251, 170, 211, 86, 81, 69, 124, 85, 0, 7, 201, 43, 157,
    133, 155, 9, 160, 143, 173, 179, 15, 99, 171, 137, 75, 215, 167, 21, 90, 113, 102, 66, 191, 38, 74, 107, 152, 250,
    234, 119, 83, 178, 112, 5, 44, 253, 89, 58, 134, 126, 206, 6, 235, 130, 120, 87, 199, 141, 67, 175, 180, 28, 212,
    91, 205, 226, 233, 39, 79, 195, 8, 114, 128, 207, 176, 239, 245, 40, 109, 190, 48, 77, 52, 146, 213, 14, 60, 34,
    50, 229, 228, 249, 159, 194, 209, 10, 129, 18, 225, 238, 145, 131, 118, 227, 151, 230, 97, 138, 23, 121, 164, 183,
    220, 144, 122, 92, 140, 2, 166, 202, 105, 222, 80, 26, 17, 147, 185, 82, 135, 88, 252, 237, 29, 55, 73, 27, 106,
    224, 41, 51, 153, 189, 108, 217, 148, 243, 64, 84, 111, 240, 198, 115, 184, 214, 62, 101, 24, 68, 31, 221, 103, 16,
    241, 12, 25, 236, 174, 3, 161, 20, 123, 169, 11, 255, 248, 163, 192, 162, 1, 247, 46, 188, 36, 104, 117, 13, 254,
    186, 47, 181, 208, 218, 61,
];

pub fn permute_bytes(data: &mut [u8]) {
    for b in data.iter_mut() {
        *b = PERMUTE_TABLE[*b as usize];
    }
}

pub static DEPERMUTE_TABLE: std::sync::LazyLock<[u8; 256]> = std::sync::LazyLock::new(|| {
    let mut table = [0u8; 256];
    for (i, &b) in PERMUTE_TABLE.iter().enumerate() {
        table[b as usize] = i as u8;
    }
    table
});

pub fn depermute_bytes(data: &mut [u8]) {
    let table = &*DEPERMUTE_TABLE;
    for b in data.iter_mut() {
        *b = table[*b as usize];
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Bref {
    pub bid: u64,
    pub ib: u64,
}

impl Bref {
    pub fn write_le<W: Write>(&self, mut writer: W) -> io::Result<()> {
        writer.write_u64::<LittleEndian>(self.bid)?;
        writer.write_u64::<LittleEndian>(self.ib)?;
        Ok(())
    }
}

pub fn calculate_partial_header_crc(header: &[u8; PST_HEADER_SIZE]) -> u32 {
    let mut hasher = Hasher::new();
    // In MS-PST, dwCRCPartial is the CRC32 of 471 bytes starting from offset 0x08 to 0x1DF
    hasher.update(&header[0x08..0x1DF]);
    hasher.finalize()
}
