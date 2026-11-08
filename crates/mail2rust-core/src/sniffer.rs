use std::fs::File;
use std::io::Read;
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
    Vcard,
    Icalendar,
    Zip,
    Gz,
    ThunderbirdProfile,
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
            Self::Vcard => "vCard Address Book (VCF)",
            Self::Icalendar => "iCalendar Events & Meetings (ICS)",
            Self::Zip => "Compressed Archive (ZIP / Takeout)",
            Self::Gz => "Gzip Compressed Stream (GZ)",
            Self::ThunderbirdProfile => "Mozilla Thunderbird Profile / Mail Tree",
            Self::Unknown => "Unknown / Unrecognized Format",
        }
    }
}

pub struct FormatSniffer;

impl FormatSniffer {
    pub fn sniff_path(path: &Path) -> MailboxFormat {
        if path.is_dir() {
            // Check for Thunderbird Profile / Mail Tree
            if path.join("profiles.ini").is_file()
                || path.join("prefs.js").is_file()
                || path.join("ImapMail").is_dir()
                || path.join("Mail").is_dir()
            {
                return MailboxFormat::ThunderbirdProfile;
            }

            // Check if folder contains .sbd directories or .msf files
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                        if name.ends_with(".sbd") || name == "ImapMail" || name == "Mail" {
                            return MailboxFormat::ThunderbirdProfile;
                        }
                    } else if p.is_file() {
                        let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                        if ext == "msf" {
                            return MailboxFormat::ThunderbirdProfile;
                        }
                    }
                }
            }

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

        // Skip Thunderbird index & filter metadata files
        if extension == "msf" || extension == "dat" {
            return MailboxFormat::Unknown;
        }

        // Check for extensionless Thunderbird MBOX
        if extension.is_empty() {
            if path.with_extension("msf").is_file() {
                return MailboxFormat::Mbox;
            }
            if let Some(parent) = path.parent() {
                let pname = parent.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if pname.ends_with(".sbd") || pname == "ImapMail" || pname == "Mail" {
                    return MailboxFormat::Mbox;
                }
            }
        }

        // Bounded header read (max 8192 bytes) to avoid OOM on multi-gigabyte mailboxes
        let mut f = match File::open(path) {
            Ok(f) => f,
            Err(_) => return Self::from_extension(&extension),
        };

        let mut bytes = vec![0u8; 8192];
        let n = f.read(&mut bytes).unwrap_or(0);
        bytes.truncate(n);

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

        // vCard (BEGIN:VCARD)
        if bytes.len() >= 11 {
            let prefix = String::from_utf8_lossy(&bytes[..50.min(bytes.len())]).to_uppercase();
            if prefix.contains("BEGIN:VCARD") {
                return MailboxFormat::Vcard;
            }
            if prefix.contains("BEGIN:VCALENDAR") {
                return MailboxFormat::Icalendar;
            }
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
            "vcf" | "vcard" => MailboxFormat::Vcard,
            "ics" | "ical" | "ifb" => MailboxFormat::Icalendar,
            "zip" => MailboxFormat::Zip,
            "gz" | "tgz" => MailboxFormat::Gz,
            _ => MailboxFormat::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thunderbird_sniffer() {
        let temp_dir = std::env::temp_dir().join(format!("tb_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let imap_dir = temp_dir.join("ImapMail");
        let _ = std::fs::create_dir_all(&imap_dir);

        assert_eq!(FormatSniffer::sniff_path(&temp_dir), MailboxFormat::ThunderbirdProfile);

        let inbox_file = imap_dir.join("INBOX");
        let msf_file = imap_dir.join("INBOX.msf");
        let _ = std::fs::write(
            &inbox_file,
            b"From mailer-daemon Mon Nov 8 00:00:00 2026\r\nSubject: Hi\r\n\r\nBody",
        );
        let _ = std::fs::write(&msf_file, b"// <!-- <mdb:mork:z v=\"1.4\"/> -->");

        assert_eq!(FormatSniffer::sniff_path(&inbox_file), MailboxFormat::Mbox);
        assert_eq!(FormatSniffer::sniff_path(&msf_file), MailboxFormat::Unknown);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
