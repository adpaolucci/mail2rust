use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailboxFormat {
    Pst,
    Ost,
    Msg,
    Mbox,
    Maildir,
    Eml,
    Emlx,
    Olm,
    Dbx,
    Nsf,
    Tbb,
    Mbs,
    Mh,
    Mmdf,
    Babyl,
    Zip,
    Gz,
    Unknown,
}

impl MailboxFormat {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Pst => "Outlook Personal Folders (PST)",
            Self::Ost => "Outlook Offline Storage (OST)",
            Self::Msg => "Outlook Item (MSG)",
            Self::Mbox => "Standard MBOX Archive",
            Self::Maildir => "Maildir Mailbox Tree",
            Self::Eml => "RFC 5322 EML Message",
            Self::Emlx => "Apple Mail EMLX",
            Self::Olm => "Outlook for Mac Archive (OLM)",
            Self::Dbx => "Outlook Express Database (DBX)",
            Self::Nsf => "Lotus Notes Storage Facility (NSF)",
            Self::Tbb => "The Bat! Message Base (TBB)",
            Self::Mbs => "Opera Mail (MBS)",
            Self::Mh => "MH / Sylpheed / Claws Mail",
            Self::Mmdf => "MMDF Mailbox",
            Self::Babyl => "Emacs Babyl",
            Self::Zip => "Compressed Archive (ZIP / Takeout)",
            Self::Gz => "Gzip Compressed Stream (GZ)",
            Self::Unknown => "Unknown / Unrecognized Format",
        }
    }
}

pub struct FormatSniffer;

impl FormatSniffer {
    pub fn sniff_path(path: &Path) -> MailboxFormat {
        if path.is_dir() {
            // Check for Maildir
            if path.join("cur").is_dir() && path.join("new").is_dir() {
                return MailboxFormat::Maildir;
            }
            // Check for MH
            if path.join(".mh_sequences").is_file() {
                return MailboxFormat::Mh;
            }
            return MailboxFormat::Eml;
        }

        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => return Self::from_extension(&extension),
        };

        Self::sniff_bytes(&bytes, &extension)
    }

    pub fn sniff_bytes(bytes: &[u8], ext_hint: &str) -> MailboxFormat {
        if bytes.len() >= 2 {
            // GZ magic 0x1F 0x8B
            if bytes[0] == 0x1F && bytes[1] == 0x8B {
                return MailboxFormat::Gz;
            }
        }

        if bytes.len() >= 4 {
            // PST / OST
            if bytes.len() >= 4 && (&bytes[0..4] == b"!BDN" || bytes[0..4] == [0x4E, 0x44, 0x42, 0x4E]) {
                return MailboxFormat::Pst;
            }
            if bytes.len() >= 4 && &bytes[0..4] == b"!BDO" {
                return MailboxFormat::Ost;
            }
            // DBX
            if bytes[0..4] == [0xCF, 0xAD, 0x12, 0xFE] {
                return MailboxFormat::Dbx;
            }
            // NSF
            if bytes[0..4] == [0x1A, 0x00, 0x00, 0x04] {
                return MailboxFormat::Nsf;
            }
            // MBS
            if &bytes[0..4] == b"MBS2" {
                return MailboxFormat::Mbs;
            }
            // MMDF
            if bytes[0..4] == [0x01, 0x01, 0x01, 0x01] {
                return MailboxFormat::Mmdf;
            }
            // ZIP magic PK\x03\x04
            if bytes[0..4] == [0x50, 0x4B, 0x03, 0x04] {
                if ext_hint == "olm" {
                    return MailboxFormat::Olm;
                }
                return MailboxFormat::Zip;
            }
        }

        if bytes.len() >= 8 {
            // MSG (CFB)
            if bytes[0..8] == [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1] {
                return MailboxFormat::Msg;
            }
        }

        if bytes.len() >= 7 && &bytes[0..7] == b"BABYL O" {
            return MailboxFormat::Babyl;
        }

        if bytes.len() >= 5 && &bytes[0..5] == b"From " {
            return MailboxFormat::Mbox;
        }

        // Check for EMLX: starts with ascii digits followed by newline
        if let Some(pos) = bytes.iter().take(10).position(|&b| b == b'\n') {
            if let Ok(s) = std::str::from_utf8(&bytes[..pos]) {
                if s.trim().chars().all(|c| c.is_ascii_digit()) {
                    return MailboxFormat::Emlx;
                }
            }
        }

        Self::from_extension(ext_hint)
    }

    fn from_extension(ext: &str) -> MailboxFormat {
        match ext {
            "pst" => MailboxFormat::Pst,
            "ost" => MailboxFormat::Ost,
            "msg" => MailboxFormat::Msg,
            "mbox" => MailboxFormat::Mbox,
            "eml" => MailboxFormat::Eml,
            "emlx" => MailboxFormat::Emlx,
            "olm" => MailboxFormat::Olm,
            "dbx" => MailboxFormat::Dbx,
            "nsf" => MailboxFormat::Nsf,
            "tbb" => MailboxFormat::Tbb,
            "mbs" => MailboxFormat::Mbs,
            "zip" => MailboxFormat::Zip,
            "gz" | "tgz" => MailboxFormat::Gz,
            _ => MailboxFormat::Unknown,
        }
    }
}
