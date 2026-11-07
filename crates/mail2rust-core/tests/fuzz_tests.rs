use mail2rust_core::{BabylReader, EmlReader, EmlxReader, FormatSniffer, MboxReader, MmdfReader, MsgReader};
use std::io::Cursor;
use tempfile::tempdir;

struct FuzzRng {
    state: u64,
}

impl FuzzRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0xDEADBEEFCAFEBABE } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_usize(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next_u64() as usize) % bound
        }
    }

    fn next_u8(&mut self) -> u8 {
        (self.next_u64() & 0xFF) as u8
    }

    fn mutate(&mut self, data: &mut Vec<u8>) {
        if data.is_empty() {
            data.push(self.next_u8());
            return;
        }

        let op = self.next_usize(6);
        match op {
            0 => {
                let idx = self.next_usize(data.len());
                let bit = 1 << self.next_usize(8);
                data[idx] ^= bit;
            }
            1 => {
                let idx = self.next_usize(data.len());
                data[idx] = self.next_u8();
            }
            2 => {
                let idx = self.next_usize(data.len() + 1);
                let val = self.next_u8();
                data.insert(idx, val);
            }
            3 => {
                if data.len() > 1 {
                    let idx = self.next_usize(data.len());
                    data.remove(idx);
                }
            }
            4 => {
                let tokens: [&[u8]; 10] = [
                    b"\r\n\r\n",
                    b"\n\n",
                    b"\0",
                    b"From ",
                    b"boundary=\"--\"",
                    b"=?UTF-8?B?AAA=?=",
                    b"=?ISO-8859-1?Q?=20?=",
                    b"<key>flags</key><integer>1</integer>",
                    b"!BDN",
                    b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1",
                ];
                let tok = tokens[self.next_usize(tokens.len())];
                let idx = self.next_usize(data.len() + 1);
                for (offset, &b) in tok.iter().enumerate() {
                    data.insert(idx + offset, b);
                }
            }
            5 => {
                let new_len = self.next_usize(data.len());
                data.truncate(new_len);
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn test_fuzz_rfc5322_eml_parser() {
    let mut rng = FuzzRng::new(42);
    let seed_data = b"From: Alice <alice@example.com>\r\n\
To: Bob <bob@example.com>\r\n\
Subject: =?UTF-8?B?VGVzdA==?=\r\n\
Date: Fri, 07 Nov 2026 12:00:00 +0000\r\n\
Message-ID: <fuzz-01@domain.local>\r\n\
Content-Type: multipart/mixed; boundary=\"boundary_123\"\r\n\
\r\n\
--boundary_123\r\n\
Content-Type: text/plain; charset=\"utf-8\"\r\n\
Content-Transfer-Encoding: quoted-printable\r\n\
\r\n\
Fuzzing =20 payload\r\n\
--boundary_123\r\n\
Content-Type: application/octet-stream\r\n\
Content-Disposition: attachment; filename=\"payload.bin\"\r\n\
Content-Transfer-Encoding: base64\r\n\
\r\n\
QUJDREVGR0g=\r\n\
--boundary_123--\r\n";

    let mut corpus = seed_data.to_vec();

    for i in 0..10_000 {
        rng.mutate(&mut corpus);
        // Ensure parser never panics on corrupted or mutated input
        let _ = EmlReader::parse(&corpus, vec!["FuzzFolder".to_string()]);

        // Periodic reset to seed so mutations explore diverse neighborhoods
        if i % 100 == 0 {
            corpus = seed_data.to_vec();
            for _ in 0..(i % 20) {
                rng.mutate(&mut corpus);
            }
        }
    }
}

#[test]
fn test_fuzz_mbox_streaming_reader() {
    let mut rng = FuzzRng::new(1337);
    let seed_mbox = b"From sender@corp.com Fri Nov 07 10:00:00 2026\r\n\
From: sender@corp.com\r\n\
To: recipient@corp.com\r\n\
Subject: First Mbox Message\r\n\
\r\n\
Body content with >From escaping.\r\n\
\r\n\
From sender2@corp.com Fri Nov 07 11:00:00 2026\r\n\
From: sender2@corp.com\r\n\
Subject: Second Mbox Message\r\n\
\r\n\
Second body content.\r\n";

    let mut corpus = seed_mbox.to_vec();

    for i in 0..10_000 {
        rng.mutate(&mut corpus);
        let cursor = Cursor::new(&corpus);
        let _ = MboxReader::read_all(cursor, vec!["MboxFuzz".to_string()]);

        if i % 100 == 0 {
            corpus = seed_mbox.to_vec();
            for _ in 0..(i % 20) {
                rng.mutate(&mut corpus);
            }
        }
    }
}

#[test]
fn test_fuzz_apple_emlx_reader() {
    let mut rng = FuzzRng::new(999);
    let seed_emlx = b"142\n\
From: user@apple.com\r\n\
Subject: Apple EMLX Test\r\n\
\r\n\
This is an Apple Mail message.\r\n\
<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n\
<dict>\n\
    <key>flags</key>\n\
    <integer>8623489129</integer>\n\
</dict>\n\
</plist>\n";

    let mut corpus = seed_emlx.to_vec();

    for i in 0..10_000 {
        rng.mutate(&mut corpus);
        let _ = EmlxReader::parse(&corpus, vec!["EmlxFolder".to_string()]);

        if i % 100 == 0 {
            corpus = seed_emlx.to_vec();
        }
    }
}

#[test]
fn test_fuzz_msg_cfb_scanner() {
    let mut rng = FuzzRng::new(2026);
    let mut seed_msg = Vec::from(MsgReader::CFB_MAGIC);
    seed_msg.extend_from_slice(&[0u8; 512]);
    // Inject UTF-16LE PR_SUBJECT marker ("0037")
    seed_msg.extend_from_slice(&"0037".encode_utf16().flat_map(|u| u.to_le_bytes()).collect::<Vec<_>>());
    seed_msg.extend_from_slice(&[0u8; 64]);
    seed_msg.extend_from_slice(
        &"Fuzzed Subject"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect::<Vec<_>>(),
    );

    let mut corpus = seed_msg.clone();

    for i in 0..10_000 {
        rng.mutate(&mut corpus);
        let _ = MsgReader::parse(&corpus, vec!["MsgFuzz".to_string()]);

        if i % 100 == 0 {
            corpus = seed_msg.clone();
        }
    }
}

#[test]
fn test_fuzz_babyl_and_mmdf_readers() {
    let mut rng = FuzzRng::new(777);
    let seed_babyl = b"BABYL OPTIONS:\nVersion: 5\n\x1F\n1, unseen,,\n*** EOOH ***\nFrom: bob@babyl.org\nSubject: Babyl Test\n\nBabyl Body\n\x1F";
    let seed_mmdf = b"\x01\x01\x01\x01\nFrom: mmdf@unix.org\nSubject: MMDF Test\n\nMMDF Body\n\x01\x01\x01\x01\n";

    let mut corpus_babyl = seed_babyl.to_vec();
    let mut corpus_mmdf = seed_mmdf.to_vec();

    for _ in 0..5_000 {
        rng.mutate(&mut corpus_babyl);
        let _ = BabylReader::parse(Cursor::new(&corpus_babyl), vec!["Babyl".to_string()]);

        rng.mutate(&mut corpus_mmdf);
        let _ = MmdfReader::parse(Cursor::new(&corpus_mmdf), vec!["MMDF".to_string()]);
    }
}

#[test]
fn test_fuzz_pst_validator_and_ndb() {
    let mut rng = FuzzRng::new(555);
    // Generate valid PST header baseline
    let mut header = vec![0u8; 512];
    header[0..4].copy_from_slice(&0x4E44424Eu32.to_be_bytes()); // !BDN
    header[8..10].copy_from_slice(&0x534Du16.to_le_bytes()); // SM
    header[10..12].copy_from_slice(&23u16.to_le_bytes()); // Unicode
    header[12..14].copy_from_slice(&19u16.to_le_bytes());
    header[0x1CD] = 1; // Compressible

    let mut corpus = header.clone();

    for i in 0..10_000 {
        rng.mutate(&mut corpus);
        let cursor = Cursor::new(&corpus);
        let _ = mail2rust_pst::PstValidator::validate(cursor);

        if i % 100 == 0 {
            corpus = header.clone();
        }
    }
}

#[test]
fn test_fuzz_format_sniffer() {
    let temp = tempdir().expect("tempdir failed");
    let mut rng = FuzzRng::new(31415);
    let test_file = temp.path().join("fuzz_target.bin");

    for _ in 0..5_000 {
        let len = rng.next_usize(2048);
        let mut buf = vec![0u8; len];
        for b in &mut buf {
            *b = rng.next_u8();
        }
        std::fs::write(&test_file, &buf).expect("write failed");
        let format = FormatSniffer::sniff_path(&test_file);
        assert!(!format.name().is_empty());
    }
}

#[test]
fn test_fuzz_filter_parsers() {
    let mut rng = FuzzRng::new(888);
    let base_strings = [
        "2026-11-07",
        "2026-11-07T12:00:00Z",
        "Fri, 07 Nov 2026 12:00:00 +0000",
        "20GB",
        "500MB",
        "1024KB",
        "15g",
        "*Trash*;*Spam*",
        "Inbox/*",
        "",
        "INVALID_DATE_XYZ",
        "-9999999999",
        "0.0.0.0",
        "\0\0\0",
    ];

    for s in &base_strings {
        let mut bytes = s.as_bytes().to_vec();
        for _ in 0..1_000 {
            rng.mutate(&mut bytes);
            let candidate = String::from_utf8_lossy(&bytes);
            let _ = mail2rust_filter::parse_cli_datetime(&candidate, false);
            let _ = mail2rust_filter::parse_cli_datetime(&candidate, true);
            let _ = mail2rust_filter::parse_size_bytes(&candidate);
            let filter = mail2rust_filter::FolderFilter::new(vec![candidate.to_string()], Vec::new());
            let _ = filter.matches(&["Inbox".to_string(), "Sub".to_string()]);
        }
    }
}
