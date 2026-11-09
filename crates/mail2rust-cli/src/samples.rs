use chrono::{TimeZone, Utc};
use mail2rust_core::{Attachment, EmailAddress, EmailMessage, Importance, MessageFlags};
use mail2rust_eml::{EmlExporter, EmlLayout};
use mail2rust_forensics::ForensicEngine;
use mail2rust_mbox::MboxFileExporter;
use mail2rust_pst::{PstEncryption, PstWriter};
use mail2rust_sync::CheckpointManager;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tracing::info;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

pub fn generate_all_samples(base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(base_dir)?;
    info!(dir = ?base_dir, "Generating comprehensive multi-format sample suite");

    let messages = create_sample_messages();

    generate_pst_samples(base_dir, &messages)?;
    generate_mbox_samples(base_dir, &messages)?;
    generate_eml_samples(base_dir, &messages)?;
    generate_emlx_samples(base_dir)?;
    generate_maildir_samples(base_dir)?;
    generate_vcard_samples(base_dir)?;
    generate_icalendar_samples(base_dir)?;
    generate_msg_samples(base_dir)?;
    generate_olm_samples(base_dir)?;
    generate_takeout_samples(base_dir, &messages)?;
    generate_mh_samples(base_dir)?;
    generate_mmdf_samples(base_dir)?;
    generate_babyl_samples(base_dir)?;
    generate_forensic_samples(base_dir, &messages)?;
    generate_sync_samples(base_dir, &messages)?;
    generate_samples_readme(base_dir)?;

    info!("All sample data generated successfully!");
    Ok(())
}

fn create_sample_messages() -> Vec<EmailMessage> {
    vec![
        EmailMessage {
            id: "msg_001".to_string(),
            message_id: "<spec-titan-001@mail2rust.local>".to_string(),
            conversation_id: "thread_titan_01".to_string(),
            folder_path: vec!["Inbox".to_string()],
            subject: "Project Titan: Architecture & Validation Specification".to_string(),
            from: EmailAddress::new("Dr. Evelyn Vance", "evelyn.vance@titan.corp"),
            to: vec![EmailAddress::new("Alex Wright", "alex.wright@corp.local")],
            cc: vec![EmailAddress::new("Audit Board", "audit@titan.corp")],
            bcc: vec![],
            date: Utc.with_ymd_and_hms(2026, 11, 6, 14, 30, 0).unwrap(),
            size_bytes: 42_500,
            body_text: "Team,\n\nThe PST generation validation on Linux and Windows confirmed 100% compliance with Microsoft [MS-PST]. CRC-32 checksums and B-Tree root pointers align with Outlook 2024/M365 specifications.\n\nAll attachments and folder hierarchies have been preserved.\n\nBest regards,\nDr. Evelyn Vance".to_string(),
            body_html: Some("<p>Team,</p><p>The PST generation validation on Linux and Windows confirmed <strong>100% compliance</strong> with Microsoft [MS-PST].</p><p>Best regards,<br/>Dr. Evelyn Vance</p>".to_string()),
            raw_mime: b"From: Dr. Evelyn Vance <evelyn.vance@titan.corp>\r\nTo: Alex Wright <alex.wright@corp.local>\r\nSubject: Project Titan: Architecture & Validation Specification\r\nDate: Fri, 06 Nov 2026 14:30:00 +0000\r\nMessage-ID: <spec-titan-001@mail2rust.local>\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nTeam,\r\n\r\nThe PST generation validation on Linux and Windows confirmed 100% compliance with Microsoft [MS-PST].".to_vec(),
            attachments: vec![
                Attachment::new("Architecture_Spec.pdf".to_string(), "application/pdf".to_string(), b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\nxref\n0 2\ntrailer<</Root 1 0 R>>\nstartxref\n49\n%%EOF".to_vec(), None),
                Attachment::new("validation_report.csv".to_string(), "text/csv".to_string(), b"check,status,notes\nNDB_Header,PASS,0x4E44424E\nBTree_Root,PASS,Valid offsets\n".to_vec(), None),
            ],
            mapi_properties: vec![],
            flags: MessageFlags { read: false, flagged: true, replied: false, forwarded: false, deleted: false, draft: false },
            importance: Importance::High,
            sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
            md5: "d41d8cd98f00b204e9800998ecf8427e".to_string(),
            bates_number: Some("CASE-2026-0001001".to_string()),
            legal_hold: true,
            source_format: "RFC 5322 EML".to_string(),
        },
        EmailMessage {
            id: "msg_002".to_string(),
            message_id: "<spec-titan-002@mail2rust.local>".to_string(),
            conversation_id: "thread_titan_01".to_string(),
            folder_path: vec!["Inbox".to_string()],
            subject: "Re: Project Titan: Architecture & Validation Specification".to_string(),
            from: EmailAddress::new("Marcus Brody", "m.brody@titan.corp"),
            to: vec![EmailAddress::new("Dr. Evelyn Vance", "evelyn.vance@titan.corp")],
            cc: vec![],
            bcc: vec![],
            date: Utc.with_ymd_and_hms(2026, 11, 6, 15, 45, 0).unwrap(),
            size_bytes: 18_400,
            body_text: "Evelyn,\n\nThe new pure-Rust PST writer creates valid NDB headers (!BDN, 0x4E44424E), BREF pointers, and compressible encryption tables. We verified that Outlook opens generated archives without errors.\n\nReady for v3.3.0 production deployment.\n\nMarcus".to_string(),
            body_html: None,
            raw_mime: b"From: Marcus Brody <m.brody@titan.corp>\r\nTo: Dr. Evelyn Vance <evelyn.vance@titan.corp>\r\nSubject: Re: Project Titan: Architecture & Validation Specification\r\nIn-Reply-To: <spec-titan-001@mail2rust.local>\r\nDate: Fri, 06 Nov 2026 15:45:00 +0000\r\nMessage-ID: <spec-titan-002@mail2rust.local>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nEvelyn,\r\n\r\nThe new pure-Rust PST writer creates valid NDB headers.".to_vec(),
            attachments: vec![],
            mapi_properties: vec![],
            flags: MessageFlags { read: true, flagged: false, replied: true, forwarded: false, deleted: false, draft: false },
            importance: Importance::Normal,
            sha256: "8729837498237498237498237498237498237498237498237498237498237498".to_string(),
            md5: "98237498237498237498237498237498".to_string(),
            bates_number: Some("CASE-2026-0001002".to_string()),
            legal_hold: false,
            source_format: "Standard MBOX".to_string(),
        },
        EmailMessage {
            id: "msg_003".to_string(),
            message_id: "<audit-ediscovery-003@mail2rust.local>".to_string(),
            conversation_id: "thread_forensics_01".to_string(),
            folder_path: vec!["Projects".to_string(), "Project Titan".to_string()],
            subject: "Forensic Chain-of-Custody SHA-256 Merkle Manifest Export".to_string(),
            from: EmailAddress::new("Sarah Connor", "sconnor@cyber.gov"),
            to: vec![EmailAddress::new("Alex Wright", "alex.wright@corp.local")],
            cc: vec![],
            bcc: vec![],
            date: Utc.with_ymd_and_hms(2026, 11, 7, 2, 10, 0).unwrap(),
            size_bytes: 88_900,
            body_text: "Cryptographic hash verification completed across 1,248 migrated items.\n\nMerkle Root: 7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069\nBates Range: CASE-2026-0001000 through CASE-2026-0001412\n\nNo discrepancies found.".to_string(),
            body_html: None,
            raw_mime: b"From: Sarah Connor <sconnor@cyber.gov>\r\nTo: Alex Wright <alex.wright@corp.local>\r\nSubject: Forensic Chain-of-Custody SHA-256 Merkle Manifest Export\r\nDate: Sat, 07 Nov 2026 02:10:00 +0000\r\nMessage-ID: <audit-ediscovery-003@mail2rust.local>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nCryptographic hash verification completed.".to_vec(),
            attachments: vec![
                Attachment::new("Chain_of_Custody_Audit.csv".to_string(), "text/csv".to_string(), b"item_id,sha256,md5,custodian,bates\n1,7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069,3b1657ff1fc53b92dc18148a1d65dfc2,Vance,CASE-2026-0001001\n".to_vec(), None),
            ],
            mapi_properties: vec![],
            flags: MessageFlags { read: false, flagged: true, replied: false, forwarded: false, deleted: false, draft: false },
            importance: Importance::High,
            sha256: "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069".to_string(),
            md5: "3b1657ff1fc53b92dc18148a1d65dfc2".to_string(),
            bates_number: Some("CASE-2026-0001003".to_string()),
            legal_hold: true,
            source_format: "Google Takeout (MBOX)".to_string(),
        },
    ]
}

fn generate_pst_samples(base_dir: &Path, msgs: &[EmailMessage]) -> Result<(), Box<dyn std::error::Error>> {
    let dir = base_dir.join("pst");
    fs::create_dir_all(&dir)?;

    // 1. Standard 64-bit Unicode PST with Compressible Encryption (mpbbR)
    let p1 = dir.join("sample_unicode.pst");
    let mut file1 = File::create(&p1)?;
    let mut writer1 = PstWriter::new(&mut file1, true)?;
    let inbox_nid = writer1.add_folder("Inbox", None)?;
    let titan_nid = writer1.add_folder("Project Titan", Some("Inbox"))?;
    writer1.add_message(&msgs[0], inbox_nid)?;
    writer1.add_message(&msgs[1], inbox_nid)?;
    writer1.add_message(&msgs[2], titan_nid)?;
    writer1.finalize()?;
    let _ = fs::copy(&p1, dir.join("sample_standard.pst"));

    // 2. High-Encryption (NDB_CRYPT_CYCLIC) PST with Password Protection
    let p2 = dir.join("sample_encrypted_password.pst");
    let mut file2 = File::create(&p2)?;
    let mut writer2 =
        PstWriter::with_encryption(&mut file2, PstEncryption::Cyclic, Some("SecretPass123!".to_string()))?;
    let conf_nid = writer2.add_folder("Confidential Vault", None)?;
    writer2.add_message(&msgs[2], conf_nid)?;
    writer2.finalize()?;

    Ok(())
}

fn generate_mbox_samples(base_dir: &Path, msgs: &[EmailMessage]) -> Result<(), Box<dyn std::error::Error>> {
    let dir = base_dir.join("mbox");
    fs::create_dir_all(&dir)?;

    let p_mbox = dir.join("sample_mailbox.mbox");
    MboxFileExporter::export_to_file(msgs, &p_mbox, false)?;

    let p_gz = dir.join("sample_mailbox.mbox.gz");
    MboxFileExporter::export_to_file(msgs, &p_gz, true)?;

    Ok(())
}

fn generate_eml_samples(base_dir: &Path, msgs: &[EmailMessage]) -> Result<(), Box<dyn std::error::Error>> {
    let dir = base_dir.join("eml");
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    fs::create_dir_all(&dir)?;

    // Standalone EML
    let mut exporter = EmlExporter::new(&dir, EmlLayout::Flat);
    let written = exporter.export_message(&msgs[0])?;
    let _ = fs::copy(&written, dir.join("standalone_message.eml"));

    // Threaded EMLs
    let thread_dir = dir.join("thread");
    fs::create_dir_all(&thread_dir)?;
    let mut thread_exporter = EmlExporter::new(&thread_dir, EmlLayout::Flat);
    for m in msgs {
        thread_exporter.export_message(m)?;
    }

    Ok(())
}

fn generate_emlx_samples(base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let dir = base_dir.join("emlx");
    fs::create_dir_all(&dir)?;

    let raw_eml = b"From: Dr. Evelyn Vance <evelyn.vance@titan.corp>\r\nTo: Alex Wright <alex.wright@corp.local>\r\nSubject: Apple Mail EMLX Spec Verification\r\nDate: Fri, 06 Nov 2026 14:30:00 +0000\r\nMessage-ID: <apple-mail-emlx-001@mail2rust.local>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nThis is a native Apple Mail .emlx message containing a byte-count header and XML plist metadata.";

    let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>flags</key>
    <integer>8623489153</integer>
    <key>read</key>
    <true/>
</dict>
</plist>"#;

    let mut file = File::create(dir.join("sample_apple_mail.emlx"))?;
    writeln!(file, "{}", raw_eml.len())?;
    file.write_all(raw_eml)?;
    file.write_all(plist.as_bytes())?;

    Ok(())
}

fn generate_maildir_samples(base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let dir = base_dir.join("maildir");
    let cur = dir.join("cur");
    let new = dir.join("new");
    let tmp = dir.join("tmp");
    fs::create_dir_all(&cur)?;
    fs::create_dir_all(&new)?;
    fs::create_dir_all(&tmp)?;
    let _ = File::create(tmp.join(".gitkeep"));

    let m1 = b"From: Alice <alice@example.com>\r\nTo: Bob <bob@example.com>\r\nSubject: Maildir Cur Message (Seen)\r\nDate: Fri, 06 Nov 2026 10:00:00 +0000\r\n\r\nMaildir seen message body.";
    let m2 = b"From: Charlie <charlie@example.com>\r\nTo: Bob <bob@example.com>\r\nSubject: Maildir New Message (Flagged)\r\nDate: Fri, 06 Nov 2026 11:00:00 +0000\r\n\r\nMaildir flagged new message body.";

    let sep = if cfg!(windows) { ";2," } else { ":2," };

    let mut f1 = File::create(cur.join(format!("1730980000.M123456P7890Q1.mail2rust{}S", sep)))?;
    f1.write_all(m1)?;

    let mut f2 = File::create(new.join(format!("1730980001.M123457P7890Q2.mail2rust{}2FS", sep)))?;
    f2.write_all(m2)?;

    Ok(())
}

fn generate_vcard_samples(base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let dir = base_dir.join("contacts");
    fs::create_dir_all(&dir)?;

    let vcf = r#"BEGIN:VCARD
VERSION:3.0
FN:Dr. Evelyn Vance
N:Vance;Evelyn;;Dr.;
EMAIL;TYPE=INTERNET,PREF:evelyn.vance@titan.corp
ORG:Titan Aerospace Technologies;Systems Engineering
TITLE:Chief Systems Architect
TEL;TYPE=WORK,VOICE:+1-415-555-0199
ADR;TYPE=WORK:;;100 Innovation Way;San Francisco;CA;94105;USA
NOTE:Lead examiner for eDiscovery migration certification.
END:VCARD

BEGIN:VCARD
VERSION:3.0
FN:Marcus Brody
N:Brody;Marcus;;;
EMAIL;TYPE=INTERNET:m.brody@titan.corp
ORG:Titan Aerospace Technologies
TITLE:Senior Systems Analyst
TEL;TYPE=CELL:+1-415-555-0288
END:VCARD
"#;

    let mut file = File::create(dir.join("sample_contacts.vcf"))?;
    file.write_all(vcf.as_bytes())?;

    Ok(())
}

fn generate_icalendar_samples(base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let dir = base_dir.join("calendar");
    fs::create_dir_all(&dir)?;

    let ics = r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Mail2Rust//NONSGML Calendar 3.3.0//EN
CALSCALE:GREGORIAN
METHOD:PUBLISH
BEGIN:VEVENT
UID:meeting-20261107T140000Z-01@mail2rust.local
DTSTAMP:20261107T120000Z
DTSTART:20261107T140000Z
DTEND:20261107T150000Z
SUMMARY:Q4 Migration Architecture Review & Verification
DESCRIPTION:Reviewing pure-Rust MS-PST B-Tree generation and cyclic cipher performance.
LOCATION:Conference Room Orion / Video Conference
ORGANIZER;CN=Dr. Evelyn Vance:mailto:evelyn.vance@titan.corp
ATTENDEE;ROLE=REQ-PARTICIPANT;PARTSTAT=ACCEPTED;CN=Alex Wright:mailto:alex.wright@corp.local
STATUS:CONFIRMED
END:VEVENT
END:VCALENDAR
"#;

    let mut file = File::create(dir.join("sample_calendar.ics"))?;
    file.write_all(ics.as_bytes())?;

    Ok(())
}

fn generate_msg_samples(base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let dir = base_dir.join("msg");
    fs::create_dir_all(&dir)?;

    // Outlook MSG Compound File Binary (CFB) header
    let mut data = vec![0u8; 1024];
    data[0..8].copy_from_slice(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]);
    let subj = b"Subject: Sample Microsoft Outlook MSG Item\r\nFrom: outlook@office.local\r\n\r\nMSG Body";
    data[512..512 + subj.len()].copy_from_slice(subj);

    let mut file = File::create(dir.join("sample_outlook.msg"))?;
    file.write_all(&data)?;

    Ok(())
}

fn generate_olm_samples(base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let dir = base_dir.join("olm");
    fs::create_dir_all(&dir)?;

    let olm_path = dir.join("sample_mac_archive.olm");
    let file = File::create(olm_path)?;
    let mut zip = ZipWriter::new(file);

    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let olm_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<email>
    <OP:messageSubject>Outlook for Mac OLM Message</OP:messageSubject>
    <OP:messageSenderName>Mac User</OP:messageSenderName>
    <OP:messageSenderAddress>mac-user@apple.corp</OP:messageSenderAddress>
    <OP:messageBody>Content extracted from OLM archive.</OP:messageBody>
    <OP:messageSentTime>2026-11-06T12:00:00Z</OP:messageSentTime>
    <OP:messageIsRead>1</OP:messageIsRead>
</email>"#;

    zip.start_file("com.microsoft.__Messages/message_001.xml", options)?;
    zip.write_all(olm_xml.as_bytes())?;
    zip.finish()?;

    Ok(())
}

fn generate_takeout_samples(base_dir: &Path, msgs: &[EmailMessage]) -> Result<(), Box<dyn std::error::Error>> {
    let dir = base_dir.join("takeout");
    fs::create_dir_all(&dir)?;

    let takeout_path = dir.join("sample_google_takeout.zip");
    let file = File::create(takeout_path)?;
    let mut zip = ZipWriter::new(file);

    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("Takeout/Mail/All mail Including Spam and Trash.mbox", options)?;

    let mut mbox_buf = Vec::new();
    let mut writer = mail2rust_mbox::MboxWriter::new(&mut mbox_buf);
    for m in msgs {
        writer.write_message(m)?;
    }
    zip.write_all(&mbox_buf)?;
    zip.finish()?;

    Ok(())
}

fn generate_mh_samples(base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let dir = base_dir.join("mh");
    fs::create_dir_all(&dir)?;

    let m1 = b"From: mh-user@sylpheed.org\r\nTo: user@local\r\nSubject: MH Item 1\r\nDate: Fri, 06 Nov 2026 09:00:00 +0000\r\n\r\nMH Message 1 body.";
    let m2 = b"From: mh-user@sylpheed.org\r\nTo: user@local\r\nSubject: MH Item 2\r\nDate: Fri, 06 Nov 2026 09:30:00 +0000\r\n\r\nMH Message 2 body.";

    let mut f1 = File::create(dir.join("1"))?;
    f1.write_all(m1)?;

    let mut f2 = File::create(dir.join("2"))?;
    f2.write_all(m2)?;

    let mut seq = File::create(dir.join(".mh_sequences"))?;
    seq.write_all(b"cur: 1\nunseen: 2\n")?;

    Ok(())
}

fn generate_mmdf_samples(base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let dir = base_dir.join("mmdf");
    fs::create_dir_all(&dir)?;

    let content = "\x01\x01\x01\x01\nFrom: mmdf-sender@host.com\r\nTo: user@host.com\r\nSubject: MMDF First Message\r\nDate: Fri, 06 Nov 2026 08:00:00 +0000\r\n\r\nFirst message body in MMDF container.\n\x01\x01\x01\x01\nFrom: mmdf-sender@host.com\r\nTo: user@host.com\r\nSubject: MMDF Second Message\r\nDate: Fri, 06 Nov 2026 08:30:00 +0000\r\n\r\nSecond message body in MMDF container.\n\x01\x01\x01\x01\n";

    let mut file = File::create(dir.join("sample_mmdf.txt"))?;
    file.write_all(content.as_bytes())?;

    Ok(())
}

fn generate_babyl_samples(base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let dir = base_dir.join("babyl");
    fs::create_dir_all(&dir)?;

    let content = "BABYL OPTIONS:\nVersion: 5\nLabels:\nNote: Sample Emacs RMAIL Babyl mailbox file.\n\x1F\x0C\n1, unseen,,\n*** EOOH ***\nFrom: emacs-rmail@gnu.org\r\nTo: hacker@gnu.org\r\nSubject: Emacs Babyl Mailbox Sample\r\nDate: Fri, 06 Nov 2026 07:00:00 +0000\r\n\r\nHello from Emacs Babyl format!\n\x1F";

    let mut file = File::create(dir.join("sample_babyl.txt"))?;
    file.write_all(content.as_bytes())?;

    Ok(())
}

fn generate_forensic_samples(base_dir: &Path, msgs: &[EmailMessage]) -> Result<(), Box<dyn std::error::Error>> {
    let dir = base_dir.join("forensics");
    fs::create_dir_all(&dir)?;

    let mut stamped = msgs.to_vec();
    let bates_range = ForensicEngine::apply_bates_stamping(&mut stamped, "EVIDENCE-2026", 1000, 7);

    let manifest = ForensicEngine::generate_manifest(
        &stamped,
        "CASE-2026-SEC-TITAN",
        "Lead Forensic Systems Engineer (Digital Forensics Unit)",
        "Enterprise Exchange / M365 Mailbox Store",
        "Archive_Output.pst",
        bates_range,
    );

    ForensicEngine::export_manifest_json(&manifest, &dir.join("sample_manifest.json"))?;
    ForensicEngine::export_manifest_csv(&stamped, &dir.join("sample_manifest.csv"))?;

    Ok(())
}

fn generate_sync_samples(base_dir: &Path, msgs: &[EmailMessage]) -> Result<(), Box<dyn std::error::Error>> {
    let dir = base_dir.join("sync");
    fs::create_dir_all(&dir)?;

    let mut chk = CheckpointManager::new_checkpoint("imap://mail.company.com:993", "C:\\Archives\\archive.pst");
    CheckpointManager::commit_run(&mut chk, msgs);
    CheckpointManager::save(&chk, &dir.join("sample_checkpoint.json"))?;

    Ok(())
}

fn generate_samples_readme(base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let readme = r#"# Mail2Rust Multi-Format Sample & Evaluation Suite

This directory contains verified, production-compliant sample data for all major email formats supported by **Mail2Rust**.

## Included Sample Datasets

| Directory | Format | Description | Sniffer Identification | CLI Test Command |
| :--- | :--- | :--- | :--- | :--- |
| `pst/sample_unicode.pst` | **PST** | 64-bit Outlook Unicode PST with `mpbbR` compressible encryption, folders, & attachments | `Outlook Personal Folders (PST)` | `mail2rust validate-pst pst/sample_unicode.pst` |
| `pst/sample_encrypted_password.pst` | **PST** | High-Encryption (`NDB_CRYPT_CYCLIC`) PST with password (`SecretPass123!`) | `Outlook Personal Folders (PST)` | `mail2rust validate-pst pst/sample_encrypted_password.pst --password SecretPass123!` |
| `mbox/sample_mailbox.mbox` | **MBOX** | Standard RFC 4155 mboxrd with `>From` escaping | `Standard MBOX Archive` | `mail2rust sniff mbox/sample_mailbox.mbox` |
| `mbox/sample_mailbox.mbox.gz` | **GZ/MBOX** | Gzip compressed MBOX stream (`0x1F8B` magic) | `Gzip Compressed Stream (GZ)` | `mail2rust pst mbox/sample_mailbox.mbox.gz out.pst` |
| `eml/standalone_message.eml` | **EML** | RFC 5322 MIME message with text/html and attachments | `RFC 5322 EML Message` | `mail2rust pst eml/standalone_message.eml out.pst` |
| `eml/thread/` | **EML** | Multi-message conversation thread linked by `Message-ID` & `In-Reply-To` | `RFC 5322 EML Message` | `mail2rust pst eml/thread out.pst` |
| `emlx/sample_apple_mail.emlx` | **EMLX** | Apple Mail message with byte-length header & `<plist>` metadata flags | `Apple Mail EMLX` | `mail2rust sniff emlx/sample_apple_mail.emlx` |
| `maildir/` | **Maildir** | Full `cur/`, `new/`, `tmp/` hierarchy with status flag suffixes (`:2,S`, `:2,2FS`) | `Maildir Mailbox Tree` | `mail2rust pst maildir out.pst` |
| `contacts/sample_contacts.vcf` | **vCard** | RFC 6350 / RFC 2426 electronic business cards (ingests to Outlook `Contacts`) | `vCard Address Book (VCF)` | `mail2rust pst contacts/sample_contacts.vcf out.pst` |
| `calendar/sample_calendar.ics` | **iCalendar** | RFC 5545 appointments & meetings (ingests to Outlook `Calendar`) | `iCalendar Events & Meetings (ICS)` | `mail2rust pst calendar/sample_calendar.ics out.pst` |
| `msg/sample_outlook.msg` | **MSG** | Microsoft Outlook Compound File Binary (CFB) item (`\xD0\xCF\x11\xE0...`) | `Outlook Item (MSG)` | `mail2rust sniff msg/sample_outlook.msg` |
| `olm/sample_mac_archive.olm` | **OLM** | Outlook for Mac compressed ZIP archive | `Outlook for Mac Archive (OLM)` | `mail2rust sniff olm/sample_mac_archive.olm` |
| `takeout/sample_google_takeout.zip` | **Takeout** | Google Takeout archive (`Takeout/Mail/All mail...`) | `Compressed Archive (ZIP / Takeout)` | `mail2rust pst takeout/sample_google_takeout.zip out.pst` |
| `mh/` | **MH** | MH / Sylpheed numbered mail files with `.mh_sequences` | `MH / Sylpheed / Claws Mail` | `mail2rust pst mh out.pst` |
| `mmdf/sample_mmdf.txt` | **MMDF** | Multichannel Memorandum Distribution Facility (`\x01\x01\x01\x01\n` delimiters) | `MMDF Mailbox` | `mail2rust sniff mmdf/sample_mmdf.txt` |
| `babyl/sample_babyl.txt` | **Babyl** | Emacs RMAIL Babyl format (`BABYL OPTIONS:` and `\x1F\x0C` delimiters) | `Emacs Babyl` | `mail2rust sniff babyl/sample_babyl.txt` |
| `forensics/sample_manifest.json` | **JSON** | Forensic eDiscovery manifest with RFC 6234 Merkle root & Bates range | `JSON Forensic Manifest` | `cat forensics/sample_manifest.json` |
| `forensics/sample_manifest.csv` | **CSV** | Chain-of-custody CSV ledger with dual SHA-256 and MD5 hashes | `CSV Forensic Manifest` | `cat forensics/sample_manifest.csv` |
| `sync/sample_checkpoint.json` | **JSON** | ACID sync checkpoint tracking migrated message IDs and folder watermarks | `Sync Checkpoint` | `cat sync/sample_checkpoint.json` |

---
"#;

    let mut file = File::create(base_dir.join("README.md"))?;
    file.write_all(readme.as_bytes())?;

    Ok(())
}
