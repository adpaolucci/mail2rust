use chrono::Utc;
use mail2rust_core::{Attachment, EmailAddress, EmailMessage, FormatSniffer, Importance, MailboxFormat, MessageFlags};
use mail2rust_eml::{EmlExporter, EmlLayout};
use mail2rust_filter::{SplitConfig, VolumeSplitter};
use mail2rust_forensics::ForensicEngine;
use mail2rust_mbox::MboxFileExporter;
use mail2rust_pst::{PstValidator, PstWriter};
use mail2rust_sync::CheckpointManager;
use std::fs::File;
use std::path::PathBuf;
use tempfile::tempdir;

fn create_sample_message(id: &str, thread_id: &str, subject: &str, body: &str) -> EmailMessage {
    let mut msg = EmailMessage {
        id: id.to_string(),
        message_id: format!("<{}@test.corp.local>", id),
        conversation_id: thread_id.to_string(),
        folder_path: vec!["Inbox".to_string(), "Project Titan".to_string()],
        subject: subject.to_string(),
        from: EmailAddress::new("Dr. Evelyn Vance", "evelyn@titan.corp.local"),
        to: vec![EmailAddress::new("Alex Wright", "alex.wright@corp.local")],
        cc: vec![EmailAddress::new("Audit Board", "audit@titan.corp.local")],
        bcc: vec![],
        date: Utc::now(),
        size_bytes: body.len() + 200,
        body_text: body.to_string(),
        body_html: Some(format!("<p>{}</p>", body)),
        raw_mime: vec![],
        attachments: vec![Attachment::new(
            "spec_v3.pdf".to_string(),
            "application/pdf".to_string(),
            b"%PDF-1.4 sample content".to_vec(),
            None,
        )],
        mapi_properties: vec![],
        flags: MessageFlags {
            read: true,
            flagged: false,
            replied: false,
            forwarded: false,
            deleted: false,
            draft: false,
        },
        importance: Importance::High,
        sha256: String::new(),
        md5: String::new(),
        bates_number: None,
        legal_hold: false,
        source_format: "IntegrationTest".to_string(),
    };

    let (sha, md5) = EmailMessage::compute_hashes(body.as_bytes());
    msg.sha256 = sha;
    msg.md5 = md5;
    msg
}

fn accept_with_timeout(listener: &std::net::TcpListener, timeout: std::time::Duration) -> std::net::TcpStream {
    listener.set_nonblocking(true).expect("set_nonblocking failed");
    let start = std::time::Instant::now();
    loop {
        match listener.accept() {
            Ok((socket, _)) => {
                socket.set_nonblocking(false).expect("revert nonblocking failed");
                let _ = socket.set_read_timeout(Some(std::time::Duration::from_secs(10)));
                let _ = socket.set_write_timeout(Some(std::time::Duration::from_secs(10)));
                return socket;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if start.elapsed() > timeout {
                    panic!("Timeout waiting for incoming connection on mock TCP listener");
                }
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            Err(e) => panic!("Accept error: {}", e),
        }
    }
}

#[test]
fn test_end_to_end_conversion_lifecycle() {
    let temp = tempdir().expect("Failed to create tempdir");
    let base_path = temp.path();

    // 1. Create a suite of threaded email messages
    let msg1 = create_sample_message(
        "001",
        "conv_titan_01",
        "Project Titan Architecture Spec",
        "Initial blueprint for pure-Rust PST migration.",
    );
    let msg2 = create_sample_message(
        "002",
        "conv_titan_01",
        "Re: Project Titan Architecture Spec",
        "Approved by the review board. Proceed with execution.",
    );
    let msg3 = create_sample_message(
        "003",
        "conv_secops_99",
        "Security Review & Audit Notice",
        "Cryptographic chain of custody verified.",
    );

    let original_messages = vec![msg1.clone(), msg2.clone(), msg3.clone()];

    // 2. Export to MBOX
    let mbox_file = base_path.join("archive.mbox");
    let count = MboxFileExporter::export_to_file(&original_messages, &mbox_file, false).expect("Mbox export failed");
    assert_eq!(count, 3);
    assert_eq!(FormatSniffer::sniff_path(&mbox_file), MailboxFormat::Mbox);

    // 3. Export to EML folder hierarchy
    let eml_dir = base_path.join("eml_export");
    let mut eml_exporter = EmlExporter::new(&eml_dir, EmlLayout::Folder);
    for m in &original_messages {
        let p = eml_exporter.export_message(m).expect("EML export failed");
        assert!(p.exists());
    }

    // 4. Generate Pure-Rust Unicode PST File
    let pst_file = base_path.join("output.pst");
    {
        let file = File::create(&pst_file).expect("Failed to create PST file");
        let mut writer = PstWriter::new(file, true).expect("Failed to create PstWriter");
        let folder_nid = writer.add_folder("Project Titan", None).expect("Failed to add folder");
        for m in &original_messages {
            writer.add_message(m, folder_nid).expect("Failed to add message to PST");
        }
        writer.finalize().expect("Failed to finalize PST");
    }

    // 5. Validate the Generated PST Structure
    assert_eq!(FormatSniffer::sniff_path(&pst_file), MailboxFormat::Pst);
    let pst_handle = File::open(&pst_file).expect("Failed to open generated PST");
    let report = PstValidator::validate(pst_handle).expect("Validation execution failed");
    assert!(report.is_valid, "PST must be 100% valid: {:?}", report.errors);
    assert!(report.crc_match, "PST CRC-32 header check must match");
    assert_eq!(report.version, 23);
    assert_eq!(report.encryption_method, "Compressible (NDB_CRYPT_PERMUTE)");

    // 6. Test Bates Stamping & Forensic Manifest
    let mut stamped_messages = original_messages.clone();
    let bates_range = ForensicEngine::apply_bates_stamping(&mut stamped_messages, "TITAN-LEGAL", 5000, 7);
    assert_eq!(bates_range.start, "TITAN-LEGAL-0005000");
    assert_eq!(bates_range.end, "TITAN-LEGAL-0005002");
    assert_eq!(stamped_messages[0].bates_number.as_deref(), Some("TITAN-LEGAL-0005000"));

    let manifest_csv = base_path.join("audit_manifest.csv");
    let manifest_json = base_path.join("audit_manifest.json");
    ForensicEngine::export_manifest_csv(&stamped_messages, &manifest_csv).expect("CSV export failed");
    let manifest = ForensicEngine::generate_manifest(
        &stamped_messages,
        "CASE-TITAN-2026",
        "Lead Examiner",
        "archive.mbox",
        "output.pst",
        bates_range,
    );
    ForensicEngine::export_manifest_json(&manifest, &manifest_json).expect("JSON manifest export failed");
    assert!(manifest_csv.exists());
    assert!(manifest_json.exists());
    assert_eq!(manifest.total_items, 3);
    assert!(!manifest.root_sha256.is_empty());

    // 7. Test Multi-Volume Splitting with Conversation Thread Clustering
    let split_cfg = SplitConfig {
        max_volume_bytes: 500, // small boundary forces split
        cluster_threads: true,
        file_prefix: "TitanSplit".to_string(),
        target_ext: "pst".to_string(),
    };
    let volumes = VolumeSplitter::split(&original_messages, &split_cfg);
    assert!(volumes.len() >= 2, "Should split into multiple volumes");
    // Ensure msg1 and msg2 (conv_titan_01) stay in the same volume together
    let vol_with_msg1 = volumes
        .iter()
        .find(|v| v.messages.iter().any(|m| m.id == "001"))
        .unwrap();
    assert!(
        vol_with_msg1.messages.iter().any(|m| m.id == "002"),
        "Threaded replies must stay together in the same volume"
    );

    // 8. Test Delta Sync & Checkpointing
    let mut checkpoint = CheckpointManager::new_checkpoint("source_repo", "target_pst");
    CheckpointManager::commit_run(
        &mut checkpoint,
        &[original_messages[0].clone(), original_messages[1].clone()],
    );
    assert_eq!(checkpoint.total_messages_migrated, 2);

    let (delta, already_done) = CheckpointManager::calculate_delta(&original_messages, &checkpoint);
    assert_eq!(already_done, 2);
    assert_eq!(delta.len(), 1);
    assert_eq!(delta[0].id, "003");

    // 9. Test Universal Ingestion round-trip via mail2rust_core::ingest_path
    let ingested_mbox = mail2rust_core::ingest_path(&mbox_file).expect("Failed to ingest MBOX");
    assert_eq!(ingested_mbox.len(), 3);
    assert_eq!(ingested_mbox[0].subject, "Project Titan Architecture Spec");

    let ingested_eml_tree = mail2rust_core::ingest_path(&eml_dir).expect("Failed to ingest EML directory");
    assert_eq!(ingested_eml_tree.len(), 3);
}

#[test]
fn test_cli_subcommands_end_to_end() {
    use std::process::Command;

    let temp = tempdir().expect("Failed to create tempdir");
    let base = temp.path();

    // 1. Prepare sample MBOX file
    let mbox_path = base.join("source.mbox");
    let sample_emails = vec![
        create_sample_message("c01", "conv_1", "Quarterly Report", "Financial Q3 numbers."),
        create_sample_message("c02", "conv_1", "Re: Quarterly Report", "Approved without changes."),
        create_sample_message("c03", "conv_2", "Spam / Ads", "Buy now for 50% discount."),
    ];
    MboxFileExporter::export_to_file(&sample_emails, &mbox_path, false).expect("Failed to export source mbox");

    let bin_path = env!("CARGO_BIN_EXE_mail2rust");

    // Test: mail2rust sniff
    let sniff_out = Command::new(bin_path)
        .args(["sniff", mbox_path.to_str().unwrap()])
        .output()
        .expect("sniff failed to execute");
    assert!(sniff_out.status.success());
    let sniff_stdout = String::from_utf8_lossy(&sniff_out.stdout);
    assert!(sniff_stdout.contains("Standard MBOX Archive"));

    // Test: mail2rust convert (MBOX -> PST)
    let out_pst = base.join("converted.pst");
    let conv_out = Command::new(bin_path)
        .args([
            "convert",
            mbox_path.to_str().unwrap(),
            out_pst.to_str().unwrap(),
            "--pst-root",
            "RootArchive",
        ])
        .output()
        .expect("convert failed to execute");
    assert!(conv_out.status.success());
    assert!(out_pst.exists());

    // Test: mail2rust validate-pst
    let val_out = Command::new(bin_path)
        .args(["validate-pst", out_pst.to_str().unwrap()])
        .output()
        .expect("validate-pst failed to execute");
    assert!(val_out.status.success());
    let val_stdout = String::from_utf8_lossy(&val_out.stdout);
    assert!(val_stdout.contains("Specification Valid: true"));
    assert!(val_stdout.contains("CRC Match: true"));

    // Test: mail2rust --dry-run convert
    let dry_out = Command::new(bin_path)
        .args([
            "--dry-run",
            "convert",
            mbox_path.to_str().unwrap(),
            out_pst.to_str().unwrap(),
        ])
        .output()
        .expect("dry-run failed to execute");
    assert!(dry_out.status.success());
    let dry_stdout = String::from_utf8_lossy(&dry_out.stdout);
    assert!(dry_stdout.contains("PRE-FLIGHT DRY-RUN AUDIT REPORT"));
    assert!(dry_stdout.contains("Total Messages:        3"));

    // Test: mail2rust audit
    let audit_csv = base.join("audit.csv");
    let audit_json = base.join("audit.json");
    let audit_out = Command::new(bin_path)
        .args([
            "audit",
            mbox_path.to_str().unwrap(),
            "--output-csv",
            audit_csv.to_str().unwrap(),
            "--output-json",
            audit_json.to_str().unwrap(),
            "--bates-prefix",
            "CASE-LEGAL",
            "--bates-start",
            "100",
        ])
        .output()
        .expect("audit failed to execute");
    assert!(audit_out.status.success());
    assert!(audit_csv.exists());
    assert!(audit_json.exists());
    let audit_stdout = String::from_utf8_lossy(&audit_out.stdout);
    assert!(audit_stdout.contains("Root SHA-256 Digest:"));

    // Test: mail2rust eml with max_per_dir
    let eml_dir = base.join("eml_split");
    let eml_out = Command::new(bin_path)
        .args([
            "eml",
            mbox_path.to_str().unwrap(),
            eml_dir.to_str().unwrap(),
            "--naming",
            "index",
            "--max-per-dir",
            "1",
        ])
        .output()
        .expect("eml failed to execute");
    assert!(eml_out.status.success());
    assert!(eml_dir.exists());

    // Test: mail2rust convert with filtering (--exclude-folder, --dedup)
    let filtered_pst = base.join("filtered.pst");
    let filt_out = Command::new(bin_path)
        .args([
            "--dedup",
            "sha256",
            "convert",
            mbox_path.to_str().unwrap(),
            filtered_pst.to_str().unwrap(),
        ])
        .output()
        .expect("filtered convert failed to execute");
    assert!(filt_out.status.success());
    assert!(filtered_pst.exists());

    // Test: mail2rust imap direct ingestion into PST
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind mock IMAP listener");
    let local_port = listener.local_addr().expect("Failed to get port").port();

    let server_handle = std::thread::spawn(move || {
        use std::io::{BufRead, BufReader, Write};
        let mut socket = accept_with_timeout(&listener, std::time::Duration::from_secs(15));
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        socket
            .set_write_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(socket.try_clone().expect("Failed to clone"));

        socket.write_all(b"* OK Mock CLI IMAP Server Ready\r\n").unwrap();
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap() > 0 {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                line.clear();
                continue;
            }
            let tag = parts[0];
            let cmd = parts.get(1).map(|c| c.to_uppercase()).unwrap_or_default();
            match cmd.as_str() {
                "LOGIN" => {
                    socket
                        .write_all(format!("{} OK LOGIN completed\r\n", tag).as_bytes())
                        .unwrap();
                }
                "LIST" => {
                    socket
                        .write_all(b"* LIST (\\HasNoChildren) \"/\" \"INBOX\"\r\n")
                        .unwrap();
                    socket
                        .write_all(format!("{} OK LIST completed\r\n", tag).as_bytes())
                        .unwrap();
                }
                "SELECT" => {
                    socket.write_all(b"* 1 EXISTS\r\n").unwrap();
                    socket.write_all(b"* OK [UIDVALIDITY 100] UIDs valid\r\n").unwrap();
                    socket
                        .write_all(format!("{} OK [READ-WRITE] SELECT completed\r\n", tag).as_bytes())
                        .unwrap();
                }
                "UID" => {
                    let sub_cmd = parts.get(2).map(|c| c.to_uppercase()).unwrap_or_default();
                    if sub_cmd == "SEARCH" {
                        socket.write_all(b"* SEARCH 77\r\n").unwrap();
                        socket
                            .write_all(format!("{} OK SEARCH completed\r\n", tag).as_bytes())
                            .unwrap();
                    } else if sub_cmd == "FETCH" {
                        let raw_msg = "From: server@example.com\r\nTo: client@example.com\r\nSubject: Direct IMAP Ingestion\r\nDate: Sat, 07 Nov 2026 12:00:00 +0000\r\n\r\nLive stream direct to PST test!";
                        let resp = format!(
                            "* 1 FETCH (UID 77 FLAGS (\\Seen) INTERNALDATE \"07-Nov-2026 12:00:00 +0000\" BODY[] {{{}}}\r\n{})\r\n{} OK FETCH completed\r\n",
                            raw_msg.len(),
                            raw_msg,
                            tag
                        );
                        socket.write_all(resp.as_bytes()).unwrap();
                    }
                }
                "LOGOUT" => {
                    socket.write_all(b"* BYE Logging out\r\n").unwrap();
                    socket
                        .write_all(format!("{} OK LOGOUT completed\r\n", tag).as_bytes())
                        .unwrap();
                    break;
                }
                _ => {
                    socket
                        .write_all(format!("{} BAD Unsupported\r\n", tag).as_bytes())
                        .unwrap();
                }
            }
            line.clear();
        }
    });

    let imap_pst = base.join("imap_exported.pst");
    let imap_out = Command::new(bin_path)
        .args([
            "imap",
            "--host",
            "127.0.0.1",
            "--port",
            &local_port.to_string(),
            "--username",
            "testuser@domain.com",
            "--password",
            "secretpass",
            "--no-tls",
            "--output",
            imap_pst.to_str().unwrap(),
        ])
        .output()
        .expect("imap command failed to execute");

    server_handle.join().expect("Mock IMAP server thread panicked");
    assert!(imap_out.status.success());
    assert!(imap_pst.exists());

    let reader = File::open(&imap_pst).expect("Failed to open imap_pst");
    let val = PstValidator::validate(reader).expect("PST validation failed on IMAP output");
    assert!(val.is_valid);

    let messages = mail2rust_pst::PstReader::read_path(&imap_pst).expect("Failed to read messages from PST");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].subject, "Direct IMAP Ingestion");

    // Test: mail2rust pst with cyclic encryption and password protection
    let cyclic_pst = base.join("cyclic_encrypted.pst");
    let cyclic_out = Command::new(bin_path)
        .args([
            "pst",
            mbox_path.to_str().unwrap(),
            cyclic_pst.to_str().unwrap(),
            "--encryption",
            "cyclic",
            "--password",
            "PrivilegedLegal2026",
        ])
        .output()
        .expect("PST cyclic export failed to execute");
    assert!(cyclic_out.status.success());
    assert!(cyclic_pst.exists());

    // Test: validate-pst with password verification
    let val_out = Command::new(bin_path)
        .args([
            "validate-pst",
            cyclic_pst.to_str().unwrap(),
            "--password",
            "PrivilegedLegal2026",
        ])
        .output()
        .expect("Validate PST failed to execute");
    assert!(val_out.status.success());
    let val_stdout = String::from_utf8_lossy(&val_out.stdout);
    assert!(val_stdout.contains("High Encryption (NDB_CRYPT_CYCLIC)"));
    assert!(val_stdout.contains("Password Protected: true"));
    assert!(val_stdout.contains("SUCCESS (MATCH)"));

    // Verify reading messages directly from the cyclic PST
    let read_msgs = mail2rust_pst::PstReader::read_path(&cyclic_pst).expect("Failed to read cyclic PST");
    assert_eq!(read_msgs.len(), 3);
    assert!(read_msgs.iter().any(|m| m.subject.contains("Quarterly Report")));
}

#[test]
fn test_validate_all_sample_files() {
    let samples_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("samples");
    assert!(samples_dir.exists(), "samples directory must exist");

    // 1. Standard Unicode PST Validation
    let p_unicode = samples_dir.join("pst").join("sample_unicode.pst");
    assert!(p_unicode.exists());
    let r_unicode = File::open(&p_unicode).expect("open unicode pst");
    let val_unicode = PstValidator::validate(r_unicode).expect("validate unicode pst");
    assert!(val_unicode.is_valid);
    assert!(val_unicode.crc_match);
    let msgs_unicode = mail2rust_pst::PstReader::read_path(&p_unicode).expect("read unicode pst");
    assert_eq!(msgs_unicode.len(), 3);

    // 2. Encrypted PST with Password Protection
    let p_cyclic = samples_dir.join("pst").join("sample_encrypted_password.pst");
    assert!(p_cyclic.exists());
    let r_cyclic = File::open(&p_cyclic).expect("open cyclic pst");
    let val_cyclic = PstValidator::validate(r_cyclic).expect("validate cyclic pst");
    assert!(val_cyclic.is_valid);
    assert!(val_cyclic.crc_match);
    assert!(val_cyclic.encryption_method.contains("High Encryption"));

    let f_cyclic = File::open(&p_cyclic).expect("open cyclic pst for password check");
    let mut pst_reader = mail2rust_pst::PstReader::open(f_cyclic).expect("open cyclic pst reader");
    let valid_pw = String::from_utf8(vec![83, 101, 99, 114, 101, 116, 80, 97, 115, 115, 49, 50, 51, 33]).unwrap();
    let invalid_pw = String::from_utf8(vec![87, 114, 111, 110, 103, 80, 97, 115, 115]).unwrap();
    assert!(pst_reader.is_password_protected());
    assert!(pst_reader.verify_password(&valid_pw));
    assert!(!pst_reader.verify_password(&invalid_pw));

    let msgs_cyclic = mail2rust_pst::PstReader::read_path(&p_cyclic).expect("read cyclic pst");
    assert_eq!(msgs_cyclic.len(), 1);

    // 3. Multi-Format Sniffing, Universal Ingestion, and PST Re-export
    let test_cases = vec![
        (
            samples_dir.join("mbox").join("sample_mailbox.mbox"),
            MailboxFormat::Mbox,
            3,
        ),
        (
            samples_dir.join("mbox").join("sample_mailbox.mbox.gz"),
            MailboxFormat::Gz,
            3,
        ),
        (
            samples_dir.join("eml").join("standalone_message.eml"),
            MailboxFormat::Eml,
            1,
        ),
        (samples_dir.join("eml").join("thread"), MailboxFormat::Eml, 3),
        (
            samples_dir.join("emlx").join("sample_apple_mail.emlx"),
            MailboxFormat::Emlx,
            1,
        ),
        (samples_dir.join("maildir"), MailboxFormat::Maildir, 2),
        (
            samples_dir.join("contacts").join("sample_contacts.vcf"),
            MailboxFormat::Vcard,
            2,
        ),
        (
            samples_dir.join("calendar").join("sample_calendar.ics"),
            MailboxFormat::Icalendar,
            1,
        ),
        (
            samples_dir.join("msg").join("sample_outlook.msg"),
            MailboxFormat::Msg,
            1,
        ),
        (
            samples_dir.join("olm").join("sample_mac_archive.olm"),
            MailboxFormat::Olm,
            1,
        ),
        (
            samples_dir.join("takeout").join("sample_google_takeout.zip"),
            MailboxFormat::Zip,
            3,
        ),
        (samples_dir.join("mh"), MailboxFormat::Mh, 2),
        (samples_dir.join("mmdf").join("sample_mmdf.txt"), MailboxFormat::Mmdf, 2),
        (
            samples_dir.join("babyl").join("sample_babyl.txt"),
            MailboxFormat::Babyl,
            1,
        ),
    ];

    let temp = tempdir().expect("tempdir");
    for (path, expected_format, expected_count) in test_cases {
        assert!(path.exists(), "Path {:?} must exist", path);
        let sniffed = FormatSniffer::sniff_path(&path);
        assert_eq!(sniffed, expected_format, "Sniff mismatch for {:?}", path);

        let msgs = mail2rust_core::ingest_path(&path).unwrap_or_else(|e| panic!("Failed to ingest {:?}: {}", path, e));
        assert_eq!(msgs.len(), expected_count, "Message count mismatch for {:?}", path);

        // Convert ingested messages to a temporary PST and validate specification compliance
        let out_pst_path = temp
            .path()
            .join(format!("{}.pst", sniffed.name().replace([' ', '/', '(', ')'], "_")));
        let mut out_file = File::create(&out_pst_path).expect("create test pst");
        let mut writer = PstWriter::new(&mut out_file, true).expect("create pst writer");
        let folder_id = writer.add_folder("SampleFolder", None).expect("add folder");
        for m in &msgs {
            writer.add_message(m, folder_id).expect("add message");
        }
        writer.finalize().expect("finalize pst");

        let r = File::open(&out_pst_path).expect("open generated pst");
        let val = PstValidator::validate(r).expect("validate generated pst");
        assert!(val.is_valid);
        assert!(val.crc_match);
    }

    // 4. Forensics Manifest and Sync Checkpoint Validation
    let manifest_path = samples_dir.join("forensics").join("sample_manifest.json");
    assert!(manifest_path.exists());
    let manifest_content = std::fs::read_to_string(&manifest_path).expect("read manifest");
    assert!(manifest_content.contains("CASE-2026-SEC-TITAN"));
    assert!(manifest_content.contains("EVIDENCE-2026-0001000"));

    let checkpoint_path = samples_dir.join("sync").join("sample_checkpoint.json");
    assert!(checkpoint_path.exists());
    let checkpoint = CheckpointManager::load(&checkpoint_path).expect("load checkpoint");
    assert_eq!(checkpoint.migrated_message_ids.len(), 3);
}

#[test]
fn test_cli_imap_sync_server_to_server() {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::process::Command;
    use std::thread;

    // 1. Source Mock IMAP Server
    let src_listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind src mock listener");
    let src_addr = src_listener.local_addr().expect("Failed to get src local addr");

    let src_handle = thread::spawn(move || {
        let mut socket = accept_with_timeout(&src_listener, std::time::Duration::from_secs(15));
        let mut reader = BufReader::new(socket.try_clone().expect("Failed to clone socket"));

        socket.write_all(b"* OK Mock Source IMAP Ready\r\n").unwrap();

        let mut line = String::new();
        while reader.read_line(&mut line).unwrap() > 0 {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                line.clear();
                continue;
            }
            let tag = parts[0];
            let cmd = parts.get(1).map(|c| c.to_uppercase()).unwrap_or_default();

            match cmd.as_str() {
                "LOGIN" => {
                    socket
                        .write_all(format!("{} OK LOGIN completed\r\n", tag).as_bytes())
                        .unwrap();
                }
                "LIST" => {
                    socket
                        .write_all(b"* LIST (\\HasNoChildren) \"/\" \"INBOX\"\r\n")
                        .unwrap();
                    socket
                        .write_all(format!("{} OK LIST completed\r\n", tag).as_bytes())
                        .unwrap();
                }
                "SELECT" => {
                    socket.write_all(b"* 1 EXISTS\r\n").unwrap();
                    socket.write_all(b"* 1 RECENT\r\n").unwrap();
                    socket.write_all(b"* OK [UIDVALIDITY 4321] UIDs valid\r\n").unwrap();
                    socket
                        .write_all(format!("{} OK [READ-WRITE] SELECT completed\r\n", tag).as_bytes())
                        .unwrap();
                }
                "UID" => {
                    let sub_cmd = parts.get(2).map(|c| c.to_uppercase()).unwrap_or_default();
                    if sub_cmd == "SEARCH" {
                        socket.write_all(b"* SEARCH 99\r\n").unwrap();
                        socket
                            .write_all(format!("{} OK SEARCH completed\r\n", tag).as_bytes())
                            .unwrap();
                    } else if sub_cmd == "FETCH" {
                        let raw_msg = "From: ceo@alpha.corp\r\nTo: cto@beta.corp\r\nSubject: Server-to-Server Zero Disk Sync\r\nDate: Sat, 07 Nov 2026 15:00:00 +0000\r\n\r\nDirect socket streaming across servers!";
                        let resp = format!(
                            "* 1 FETCH (UID 99 FLAGS (\\Seen \\Flagged) INTERNALDATE \"07-Nov-2026 15:00:00 +0000\" ENVELOPE (\"Sat, 07 Nov 2026 15:00:00 +0000\" \"Server-to-Server Zero Disk Sync\" ((NIL NIL \"ceo\" \"alpha.corp\")) NIL NIL NIL NIL NIL NIL NIL) BODY[] {{{}}}\r\n{})\r\n{} OK FETCH completed\r\n",
                            raw_msg.len(),
                            raw_msg,
                            tag
                        );
                        socket.write_all(resp.as_bytes()).unwrap();
                    }
                }
                "LOGOUT" => {
                    socket.write_all(b"* BYE Source logging out\r\n").unwrap();
                    socket
                        .write_all(format!("{} OK LOGOUT completed\r\n", tag).as_bytes())
                        .unwrap();
                    break;
                }
                _ => {
                    socket
                        .write_all(format!("{} BAD Unsupported\r\n", tag).as_bytes())
                        .unwrap();
                }
            }
            line.clear();
        }
    });

    // 2. Destination Mock IMAP Server
    let dst_listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind dst mock listener");
    let dst_addr = dst_listener.local_addr().expect("Failed to get dst local addr");

    let dst_handle = thread::spawn(move || {
        let mut socket = accept_with_timeout(&dst_listener, std::time::Duration::from_secs(15));
        let mut reader = BufReader::new(socket.try_clone().expect("Failed to clone socket"));

        socket.write_all(b"* OK Mock Destination IMAP Ready\r\n").unwrap();

        let mut line = String::new();
        while reader.read_line(&mut line).unwrap() > 0 {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                line.clear();
                continue;
            }
            let tag = parts[0];
            let cmd = parts.get(1).map(|c| c.to_uppercase()).unwrap_or_default();

            match cmd.as_str() {
                "LOGIN" => {
                    socket
                        .write_all(format!("{} OK LOGIN completed\r\n", tag).as_bytes())
                        .unwrap();
                }
                "LIST" => {
                    socket
                        .write_all(b"* LIST (\\HasNoChildren) \"/\" \"INBOX\"\r\n")
                        .unwrap();
                    socket
                        .write_all(format!("{} OK LIST completed\r\n", tag).as_bytes())
                        .unwrap();
                }
                "CREATE" => {
                    socket
                        .write_all(format!("{} OK CREATE completed\r\n", tag).as_bytes())
                        .unwrap();
                }
                "APPEND" => {
                    let literal_size: usize = line
                        .find('{')
                        .and_then(|start| {
                            line[start + 1..]
                                .find('}')
                                .map(|end| line[start + 1..start + 1 + end].parse().unwrap_or(0))
                        })
                        .unwrap_or(0);

                    socket.write_all(b"+ Ready for literal data\r\n").unwrap();

                    let mut literal_buf = vec![0u8; literal_size];
                    std::io::Read::read_exact(&mut reader, &mut literal_buf).unwrap();

                    socket
                        .write_all(format!("{} OK [APPENDUID 1 7777] APPEND completed\r\n", tag).as_bytes())
                        .unwrap();
                }
                "LOGOUT" => {
                    socket.write_all(b"* BYE Destination logging out\r\n").unwrap();
                    socket
                        .write_all(format!("{} OK LOGOUT completed\r\n", tag).as_bytes())
                        .unwrap();
                    break;
                }
                _ => {
                    socket
                        .write_all(format!("{} BAD Unsupported\r\n", tag).as_bytes())
                        .unwrap();
                }
            }
            line.clear();
        }
    });

    let bin_path = env!("CARGO_BIN_EXE_mail2rust");

    let sync_out = Command::new(bin_path)
        .args([
            "imap-sync",
            "--src-host",
            "127.0.0.1",
            "--src-port",
            &src_addr.port().to_string(),
            "--src-user",
            "source_admin@alpha.corp",
            "--src-pass",
            "SecretPassAlpha",
            "--no-src-tls",
            "--dst-host",
            "127.0.0.1",
            "--dst-port",
            &dst_addr.port().to_string(),
            "--dst-user",
            "dest_admin@beta.corp",
            "--dst-pass",
            "SecretPassBeta",
            "--no-dst-tls",
        ])
        .output()
        .expect("imap-sync command failed to execute");

    src_handle.join().expect("Source mock thread panicked");
    dst_handle.join().expect("Destination mock thread panicked");

    assert!(sync_out.status.success());
    let stdout = String::from_utf8_lossy(&sync_out.stdout);
    assert!(stdout.contains("IMAP SERVER-TO-SERVER DIRECT MIGRATION"));
    assert!(stdout.contains("Storage Mode:  Direct Socket-to-Socket Streaming (Zero Local Disk Saving)"));
    assert!(stdout.contains("Disk Footprint:        0 bytes (No local files saved)"));
    assert!(stdout.contains("Status:                SUCCESS"));
    assert!(stdout.contains("Messages Transferred:  1"));
}

#[test]
fn test_cli_merge_multi_format_to_pst_and_mbox() {
    let cargo_bin = env!("CARGO_BIN_EXE_mail2rust");
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
    let samples_dir = workspace_root.join("samples");

    let s1 = samples_dir.join("takeout").join("sample_google_takeout.zip");
    let s2 = samples_dir.join("maildir");
    let s3 = samples_dir.join("emlx").join("sample_apple_mail.emlx");
    let s4 = samples_dir.join("contacts").join("sample_contacts.vcf");
    let s5 = samples_dir.join("calendar").join("sample_calendar.ics");

    assert!(s1.exists());
    assert!(s2.exists());
    assert!(s3.exists());
    assert!(s4.exists());
    assert!(s5.exists());

    let temp = tempdir().expect("tempdir");
    let merged_pst = temp.path().join("merged_all.pst");

    // 1. Merge into a unified PST with 'combine' strategy
    let merge_out = std::process::Command::new(cargo_bin)
        .arg("merge")
        .arg(&s1)
        .arg(&s2)
        .arg(&s3)
        .arg(&s4)
        .arg(&s5)
        .arg("-o")
        .arg(&merged_pst)
        .arg("--folder-strategy")
        .arg("combine")
        .arg("--dedup")
        .arg("message-id")
        .output()
        .expect("failed to execute merge command");

    assert!(
        merge_out.status.success(),
        "Merge command failed: {:?}",
        String::from_utf8_lossy(&merge_out.stderr)
    );
    let stdout = String::from_utf8_lossy(&merge_out.stdout);
    assert!(stdout.contains("MAILBOX MERGE & CONSOLIDATION SUMMARY"));
    assert!(stdout.contains("Total Ingested Sources:      5"));
    assert!(stdout.contains("Per-Source Ingestion Breakdown"));
    assert!(stdout.contains("Merged output successfully saved to:"));

    assert!(merged_pst.exists());
    let r = File::open(&merged_pst).expect("open merged pst");
    let val = PstValidator::validate(r).expect("validate merged pst");
    assert!(val.is_valid);
    assert!(val.crc_match);

    let read_msgs = mail2rust_pst::PstReader::read_path(&merged_pst).expect("read merged pst");
    // s1 (3) + s2 (2) + s3 (1) + s4 (2) + s5 (1) = 9 messages
    assert_eq!(read_msgs.len(), 9);

    // 2. Merge into MBOX with 'prefix' strategy and dry-run check
    let dry_run_out = std::process::Command::new(cargo_bin)
        .arg("merge")
        .arg(&s1)
        .arg(&s2)
        .arg("-o")
        .arg(temp.path().join("dummy.mbox"))
        .arg("--dry-run")
        .output()
        .expect("failed dry run merge");
    assert!(dry_run_out.status.success());
    let dry_stdout = String::from_utf8_lossy(&dry_run_out.stdout);
    assert!(dry_stdout.contains("Dry-run mode active: no target files written."));
    assert!(!temp.path().join("dummy.mbox").exists());
}

#[test]
fn test_cli_oauth_and_app_passwords() {
    let cargo_bin = env!("CARGO_BIN_EXE_mail2rust");

    // 1. Verify app-passwords command for all providers
    let out = std::process::Command::new(cargo_bin)
        .arg("app-passwords")
        .arg("all")
        .output()
        .expect("failed to run app-passwords");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("PROVIDER APP PASSWORDS SETUP GUIDE"));
    assert!(stdout.contains("GOOGLE GMAIL & GOOGLE WORKSPACE"));
    assert!(stdout.contains("MICROSOFT OUTLOOK.COM & OFFICE 365"));
    assert!(stdout.contains("YAHOO MAIL & AOL"));
    assert!(stdout.contains("PROTON MAIL & PROTON MAIL BRIDGE"));
    assert!(stdout.contains("APPLE ICLOUD MAIL"));
    assert!(stdout.contains("FASTMAIL"));
    assert!(stdout.contains("ZOHO MAIL"));
    assert!(stdout.contains("myaccount.google.com/apppasswords"));
    assert!(stdout.contains("account.live.com/proofs/manage/additional"));
    assert!(stdout.contains("login.yahoo.com/account/security"));
    assert!(stdout.contains("appleid.apple.com"));
    assert!(stdout.contains("fastmail.com"));
    assert!(stdout.contains("accounts.zoho.com"));
    assert!(stdout.contains("1143"));

    // 2. Verify targeted provider guide
    let out_google = std::process::Command::new(cargo_bin)
        .arg("app-passwords")
        .arg("google")
        .output()
        .expect("failed to run app-passwords google");
    assert!(out_google.status.success());
    let google_stdout = String::from_utf8_lossy(&out_google.stdout);
    assert!(google_stdout.contains("GOOGLE GMAIL & GOOGLE WORKSPACE"));
    assert!(!google_stdout.contains("YAHOO MAIL & AOL"));

    let out_proton = std::process::Command::new(cargo_bin)
        .arg("app-passwords")
        .arg("proton")
        .output()
        .expect("failed to run app-passwords proton");
    assert!(out_proton.status.success());
    let proton_stdout = String::from_utf8_lossy(&out_proton.stdout);
    assert!(proton_stdout.contains("PROTON MAIL & PROTON MAIL BRIDGE"));
    assert!(proton_stdout.contains("proton.me/mail/bridge"));

    let out_apple = std::process::Command::new(cargo_bin)
        .arg("app-passwords")
        .arg("apple")
        .output()
        .expect("failed to run app-passwords apple");
    assert!(out_apple.status.success());
    let apple_stdout = String::from_utf8_lossy(&out_apple.stdout);
    assert!(apple_stdout.contains("APPLE ICLOUD MAIL"));
    assert!(apple_stdout.contains("appleid.apple.com"));

    let out_fastmail = std::process::Command::new(cargo_bin)
        .arg("app-passwords")
        .arg("fastmail")
        .output()
        .expect("failed to run app-passwords fastmail");
    assert!(out_fastmail.status.success());
    let fastmail_stdout = String::from_utf8_lossy(&out_fastmail.stdout);
    assert!(fastmail_stdout.contains("FASTMAIL"));
    assert!(fastmail_stdout.contains("Settings > Password & Security"));

    let out_zoho = std::process::Command::new(cargo_bin)
        .arg("app-passwords")
        .arg("zoho")
        .output()
        .expect("failed to run app-passwords zoho");
    assert!(out_zoho.status.success());
    let zoho_stdout = String::from_utf8_lossy(&out_zoho.stdout);
    assert!(zoho_stdout.contains("ZOHO MAIL"));
    assert!(zoho_stdout.contains("accounts.zoho.com"));

    // 3. Verify IMAP CLI help exposes auth-method and oauth-token
    let imap_help = std::process::Command::new(cargo_bin)
        .arg("imap")
        .arg("--help")
        .output()
        .expect("failed to run imap --help");
    assert!(imap_help.status.success());
    let imap_stdout = String::from_utf8_lossy(&imap_help.stdout);
    assert!(imap_stdout.contains("--auth-method"));
    assert!(imap_stdout.contains("--oauth-token"));

    // 4. Verify IMAP sync CLI help exposes source and destination oauth parameters
    let sync_help = std::process::Command::new(cargo_bin)
        .arg("imap-sync")
        .arg("--help")
        .output()
        .expect("failed to run imap-sync --help");
    assert!(sync_help.status.success());
    let sync_stdout = String::from_utf8_lossy(&sync_help.stdout);
    assert!(sync_stdout.contains("--src-auth-method"));
    assert!(sync_stdout.contains("--src-oauth-token"));
    assert!(sync_stdout.contains("--dst-auth-method"));
    assert!(sync_stdout.contains("--dst-oauth-token"));

    // 5. Verify OAuth login help exposes provider, client-id, client-secret
    let oauth_help = std::process::Command::new(cargo_bin)
        .arg("oauth-login")
        .arg("--help")
        .output()
        .expect("failed to run oauth-login --help");
    assert!(oauth_help.status.success());
    let oauth_stdout = String::from_utf8_lossy(&oauth_help.stdout);
    assert!(oauth_stdout.contains("--provider"));
    assert!(oauth_stdout.contains("--client-id"));
    assert!(oauth_stdout.contains("--client-secret"));
}

#[test]
fn test_thunderbird_profile_and_msf_exclusion() {
    let cargo_bin = env!("CARGO_BIN_EXE_mail2rust");
    let temp = tempdir().expect("tempdir");
    let base = temp.path();

    // Replicate real Mozilla Thunderbird profile tree:
    // Mail/Local Folders/
    //   Inbox (raw MBOX, 2 messages, no extension)
    //   Inbox.msf (Thunderbird Mork index cache, should be skipped)
    //   Projects.sbd/
    //     Titan (raw MBOX, 1 message, no extension)
    //     Titan.msf (index cache, should be skipped)
    let local_folders = base.join("Mail").join("Local Folders");
    let projects_sbd = local_folders.join("Projects.sbd");
    std::fs::create_dir_all(&projects_sbd).expect("create_dir_all");

    let mbox_inbox_data = b"From alice@example.com Sat Nov  7 12:00:00 2026\r\n\
From: Alice <alice@example.com>\r\n\
To: Bob <bob@example.com>\r\n\
Subject: Thunderbird Message 1\r\n\
Date: Sat, 07 Nov 2026 12:00:00 +0000\r\n\
Message-ID: <tb-msg-01@domain.local>\r\n\
\r\n\
First Thunderbird local message.\r\n\
\r\n\
From charlie@example.com Sat Nov  7 12:05:00 2026\r\n\
From: Charlie <charlie@example.com>\r\n\
To: Bob <bob@example.com>\r\n\
Subject: Thunderbird Message 2\r\n\
Date: Sat, 07 Nov 2026 12:05:00 +0000\r\n\
Message-ID: <tb-msg-02@domain.local>\r\n\
\r\n\
Second Thunderbird local message.\r\n";

    let mbox_titan_data = b"From dev@example.com Sat Nov  7 12:10:00 2026\r\n\
From: Dev Lead <dev@example.com>\r\n\
To: Team <team@example.com>\r\n\
Subject: Project Titan Update\r\n\
Date: Sat, 07 Nov 2026 12:10:00 +0000\r\n\
Message-ID: <tb-msg-03@domain.local>\r\n\
\r\n\
Subfolder message inside Projects.sbd container.\r\n";

    let mock_msf_data = b"// <!-- <mdb:mork:z v=\"1.4\"/> -->\n\
< <(a=c)> // (f=iso-8859-1)\n\
  (B8=totalMessages)(B9=numUnseenMessages) >\n\
{1:^80 (flags=0) (B8=2) (B9=0)}\n";

    std::fs::write(local_folders.join("Inbox"), mbox_inbox_data).expect("write inbox mbox");
    std::fs::write(local_folders.join("Inbox.msf"), mock_msf_data).expect("write inbox msf");
    std::fs::write(projects_sbd.join("Titan"), mbox_titan_data).expect("write titan mbox");
    std::fs::write(projects_sbd.join("Titan.msf"), mock_msf_data).expect("write titan msf");

    let out_pst = base.join("thunderbird_migrated.pst");

    // Convert the entire Thunderbird Local Folders hierarchy to Outlook PST
    let convert_out = std::process::Command::new(cargo_bin)
        .arg("convert")
        .arg(&local_folders)
        .arg(&out_pst)
        .output()
        .expect("convert thunderbird command");

    assert!(
        convert_out.status.success(),
        "Conversion failed: {:?}",
        String::from_utf8_lossy(&convert_out.stderr)
    );

    // Validate the resulting PST container
    assert!(out_pst.exists());
    let r = std::fs::File::open(&out_pst).expect("open converted pst");
    let val = PstValidator::validate(r).expect("validate pst");
    assert!(val.is_valid);
    assert!(val.crc_match);

    // Read messages and verify:
    // 1. Exactly 3 messages ingested (not 2, and not 4 with .msf garbage)
    let read_msgs = mail2rust_pst::PstReader::read_path(&out_pst).expect("read pst");
    assert_eq!(read_msgs.len(), 3);

    // 2. Verify subfolder path was normalized from 'Projects.sbd' to 'Projects'
    let titan_msg = read_msgs
        .iter()
        .find(|m| m.subject == "Project Titan Update")
        .expect("titan msg");
    assert!(
        titan_msg.folder_path.contains(&"Projects".to_string()),
        "Expected folder path to contain normalized 'Projects', got {:?}",
        titan_msg.folder_path
    );
    assert!(
        !titan_msg.folder_path.iter().any(|f| f.ends_with(".sbd")),
        "Folder path should not contain raw .sbd extension: {:?}",
        titan_msg.folder_path
    );
}

#[test]
fn test_cli_jmap_ingestion_and_help() {
    let cargo_bin = env!("CARGO_BIN_EXE_mail2rust");

    // 1. Verify JMAP CLI help output
    let jmap_help = std::process::Command::new(cargo_bin)
        .arg("jmap")
        .arg("--help")
        .output()
        .expect("jmap help");
    assert!(jmap_help.status.success());
    let jmap_stdout = String::from_utf8_lossy(&jmap_help.stdout);
    assert!(jmap_stdout.contains("--session-url"));
    assert!(jmap_stdout.contains("--token"));
    assert!(jmap_stdout.contains("--folders"));
    assert!(jmap_stdout.contains("--checkpoint"));

    // 2. Start mock JMAP server and run CLI ingestion to PST
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let port = listener.local_addr().unwrap().port();

    std::thread::spawn(move || {
        use std::io::{BufRead, Read, Write};

        // 1. Session request
        let mut socket = accept_with_timeout(&listener, std::time::Duration::from_secs(15));
        let mut reader = std::io::BufReader::new(socket.try_clone().unwrap());
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap() > 0 {
            if line == "\r\n" || line == "\n" {
                break;
            }
            line.clear();
        }

        let session_json = serde_json::json!({
            "apiUrl": format!("http://127.0.0.1:{}/jmap/api", port),
            "downloadUrl": format!("http://127.0.0.1:{}/jmap/download/{{accountId}}/{{blobId}}/{{name}}", port),
            "primaryAccounts": {
                "urn:ietf:params:jmap:mail": "acc_fastmail_test"
            },
            "accounts": {
                "acc_fastmail_test": {
                    "name": "tester@fastmail.com",
                    "isPersonal": true
                }
            }
        });

        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            session_json.to_string().len(),
            session_json
        );
        socket.write_all(resp.as_bytes()).unwrap();
        socket.flush().unwrap();
        drop(socket);

        // 2. Mailbox/get request
        let mut socket2 = accept_with_timeout(&listener, std::time::Duration::from_secs(15));
        let mut reader2 = std::io::BufReader::new(socket2.try_clone().unwrap());
        line.clear();
        let mut content_len = 0;
        while reader2.read_line(&mut line).unwrap() > 0 {
            if line.to_lowercase().starts_with("content-length:") {
                content_len = line.split(':').nth(1).unwrap().trim().parse().unwrap_or(0);
            }
            if line == "\r\n" || line == "\n" {
                break;
            }
            line.clear();
        }
        let mut body_bytes = vec![0u8; content_len];
        reader2.read_exact(&mut body_bytes).unwrap();

        let mailbox_resp = serde_json::json!({
            "methodResponses": [
                [
                    "Mailbox/get",
                    {
                        "accountId": "acc_fastmail_test",
                        "state": "state_1",
                        "list": [
                            {
                                "id": "mb_inbox",
                                "name": "Inbox",
                                "totalEmails": 1,
                                "unreadEmails": 0
                            }
                        ]
                    },
                    "m0"
                ]
            ]
        });
        let resp2 = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            mailbox_resp.to_string().len(),
            mailbox_resp
        );
        socket2.write_all(resp2.as_bytes()).unwrap();
        socket2.flush().unwrap();
        drop(socket2);

        // 3. Email/query request
        let mut socket3 = accept_with_timeout(&listener, std::time::Duration::from_secs(15));
        let mut reader3 = std::io::BufReader::new(socket3.try_clone().unwrap());
        line.clear();
        content_len = 0;
        while reader3.read_line(&mut line).unwrap() > 0 {
            if line.to_lowercase().starts_with("content-length:") {
                content_len = line.split(':').nth(1).unwrap().trim().parse().unwrap_or(0);
            }
            if line == "\r\n" || line == "\n" {
                break;
            }
            line.clear();
        }
        let mut body_bytes3 = vec![0u8; content_len];
        reader3.read_exact(&mut body_bytes3).unwrap();

        let query_resp = serde_json::json!({
            "methodResponses": [
                [
                    "Email/query",
                    {
                        "accountId": "acc_fastmail_test",
                        "ids": ["jmap_cli_msg_01"],
                        "total": 1
                    },
                    "q0"
                ]
            ]
        });
        let resp3 = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            query_resp.to_string().len(),
            query_resp
        );
        socket3.write_all(resp3.as_bytes()).unwrap();
        socket3.flush().unwrap();
        drop(socket3);

        // 4. Email/get request
        let mut socket4 = accept_with_timeout(&listener, std::time::Duration::from_secs(15));
        let mut reader4 = std::io::BufReader::new(socket4.try_clone().unwrap());
        line.clear();
        content_len = 0;
        while reader4.read_line(&mut line).unwrap() > 0 {
            if line.to_lowercase().starts_with("content-length:") {
                content_len = line.split(':').nth(1).unwrap().trim().parse().unwrap_or(0);
            }
            if line == "\r\n" || line == "\n" {
                break;
            }
            line.clear();
        }
        let mut body_bytes4 = vec![0u8; content_len];
        reader4.read_exact(&mut body_bytes4).unwrap();

        let get_resp = serde_json::json!({
            "methodResponses": [
                [
                    "Email/get",
                    {
                        "accountId": "acc_fastmail_test",
                        "list": [
                            {
                                "id": "jmap_cli_msg_01",
                                "blobId": "blob_cli_01",
                                "threadId": "thread_fastmail_cli",
                                "mailboxIds": {
                                    "mb_inbox": true
                                },
                                "keywords": {
                                    "$seen": true
                                },
                                "size": 320,
                                "receivedAt": "2026-11-07T18:00:00Z",
                                "messageId": ["<fastmail-cli-01@fastmail.com>"],
                                "subject": "Fastmail JMAP CLI Ingestion Test",
                                "from": [
                                    {
                                        "name": "Fastmail Ops",
                                        "email": "ops@fastmail.com"
                                    }
                                ],
                                "to": [
                                    {
                                        "name": "User",
                                        "email": "tester@fastmail.com"
                                    }
                                ],
                                "textBody": [
                                    {
                                        "partId": "txt_1",
                                        "size": 35,
                                        "type": "text/plain"
                                    }
                                ],
                                "bodyValues": {
                                    "txt_1": {
                                        "value": "JMAP HTTP JSON Sync Verified!"
                                    }
                                }
                            }
                        ]
                    },
                    "e0"
                ]
            ]
        });
        let resp4 = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            get_resp.to_string().len(),
            get_resp
        );
        socket4.write_all(resp4.as_bytes()).unwrap();
        socket4.flush().unwrap();
        drop(socket4);
    });

    let temp = tempdir().expect("tempdir");
    let out_pst = temp.path().join("jmap_export.pst");
    let session_url = format!("http://127.0.0.1:{}/jmap/session", port);

    let jmap_cmd = std::process::Command::new(cargo_bin)
        .arg("jmap")
        .arg("--session-url")
        .arg(&session_url)
        .arg("--token")
        .arg("fmu1-mock-bearer-token")
        .arg("-o")
        .arg(&out_pst)
        .output()
        .expect("execute jmap command");

    assert!(
        jmap_cmd.status.success(),
        "JMAP CLI failed: {:?}",
        String::from_utf8_lossy(&jmap_cmd.stderr)
    );
    assert!(out_pst.exists(), "Output PST should exist");

    let read_msgs = mail2rust_pst::PstReader::read_path(&out_pst).expect("read pst");
    assert_eq!(read_msgs.len(), 1);
    assert_eq!(read_msgs[0].subject, "Fastmail JMAP CLI Ingestion Test");
    assert_eq!(read_msgs[0].from.email, "ops@fastmail.com");
    assert_eq!(read_msgs[0].body_text, "JMAP HTTP JSON Sync Verified!");
    assert_eq!(read_msgs[0].source_format, "Microsoft Outlook PST (Unicode)");
}

#[test]
fn test_cli_checkpoint_resumption_and_atomic_save() {
    let cargo_bin = env!("CARGO_BIN_EXE_mail2rust");
    let temp = tempdir().expect("tempdir");

    // 1. Create a sample MBOX with 4 messages
    let mbox_path = temp.path().join("source.mbox");
    let msg1 = create_sample_message("m1", "t1", "Sub 1", "Body 1");
    let msg2 = create_sample_message("m2", "t1", "Sub 2", "Body 2");
    let msg3 = create_sample_message("m3", "t2", "Sub 3", "Body 3");
    let msg4 = create_sample_message("m4", "t2", "Sub 4", "Body 4");
    MboxFileExporter::export_to_file(
        &[msg1.clone(), msg2.clone(), msg3.clone(), msg4.clone()],
        &mbox_path,
        false,
    )
    .expect("export mbox");

    // 2. Pre-populate a checkpoint simulating msg1 and msg2 were previously migrated
    let chk_path = temp.path().join("checkpoint.json");
    let mut chk = CheckpointManager::new_checkpoint("source.mbox", "dest");
    CheckpointManager::commit_run(&mut chk, &[msg1, msg2]);
    assert_eq!(chk.total_messages_migrated, 2);
    CheckpointManager::save_atomic(&chk, &chk_path).expect("save atomic checkpoint");

    // 3. Run convert with --resume checkpoint.json into an EML folder
    let eml_dir = temp.path().join("eml_out");
    let output = std::process::Command::new(cargo_bin)
        .arg("convert")
        .arg(&mbox_path)
        .arg(&eml_dir)
        .arg("--resume")
        .arg(&chk_path)
        .output()
        .expect("execute convert with resume");

    assert!(
        output.status.success(),
        "Convert failed: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify only remaining 2 messages were exported
    let mut exported_emls = 0;
    for entry in walkdir::WalkDir::new(&eml_dir) {
        let entry = entry.unwrap();
        if entry.path().extension().and_then(|s| s.to_str()) == Some("eml") {
            exported_emls += 1;
        }
    }
    assert_eq!(exported_emls, 2, "Expected exactly 2 newly converted EML files");

    // Verify checkpoint was atomically updated to 4 messages total
    let updated_chk = CheckpointManager::load(&chk_path).expect("load updated checkpoint");
    assert_eq!(
        updated_chk.total_messages_migrated, 4,
        "Expected checkpoint to have all 4 messages"
    );

    // 4. Test JSONL streaming checkpoint with mail2rust-mcp convert tool
    let jsonl_path = temp.path().join("checkpoint.jsonl");
    let dest_pst = temp.path().join("mcp_out.pst");

    let conv_res = mail2rust_mcp::call_tool(
        "mail2rust_convert",
        &serde_json::json!({
            "source": mbox_path.to_string_lossy(),
            "destination": dest_pst.to_string_lossy(),
            "target_format": "pst",
            "checkpoint_file": jsonl_path.to_string_lossy(),
            "resume": false
        }),
    )
    .expect("mcp convert call");

    assert_eq!(conv_res.get("status").and_then(|v| v.as_str()), Some("success"));
    assert_eq!(conv_res.get("messages_written").and_then(|v| v.as_u64()), Some(4));
    assert!(jsonl_path.exists(), "JSONL checkpoint should exist");

    // Resuming immediately with same JSONL should find 0 delta messages
    let dest_pst2 = temp.path().join("mcp_out2.pst");
    let resume_res = mail2rust_mcp::call_tool(
        "mail2rust_convert",
        &serde_json::json!({
            "source": mbox_path.to_string_lossy(),
            "destination": dest_pst2.to_string_lossy(),
            "target_format": "pst",
            "checkpoint_file": jsonl_path.to_string_lossy(),
            "resume": true
        }),
    )
    .expect("mcp resume call");

    assert_eq!(resume_res.get("already_migrated").and_then(|v| v.as_u64()), Some(4));
    assert_eq!(resume_res.get("messages_written").and_then(|v| v.as_u64()), Some(0));
}

#[test]
fn test_ediscovery_search_and_html_index_manifest_deliverables() {
    use std::fs;
    use std::process::Command;

    let temp = tempdir().expect("Failed to create tempdir");
    let source_dir = temp.path().join("Thunderbird").join("ImapMail");
    fs::create_dir_all(&source_dir).unwrap();

    // 1. Create Thunderbird extensionless MBOX + .msf
    let inbox_mbox = source_dir.join("INBOX");
    let inbox_msf = source_dir.join("INBOX.msf");
    fs::write(&inbox_msf, b"// <!-- <mdb:mork:z v=\"1.4\"/> -->").unwrap();

    let mbox_content = concat!(
        "From mailer-daemon@corp.local Mon Nov 8 10:00:00 2026\r\n",
        "Message-ID: <msg001@discovery.local>\r\n",
        "Date: Mon, 8 Nov 2026 10:00:00 +0000\r\n",
        "From: Alice <alice@corp.local>\r\n",
        "To: Bob <bob@corp.local>\r\n",
        "Subject: =?UTF-8?Q?RE:_100_Corporate_Blvd_Inspection?=\r\n",
        "\r\n",
        "Site review at 100 Corporate Boulevard went smoothly.\r\n",
        "\r\n",
        "From mailer-daemon@corp.local Mon Nov 8 11:00:00 2026\r\n",
        "Message-ID: <msg002@discovery.local>\r\n",
        "Date: Mon, 8 Nov 2026 11:00:00 +0000\r\n",
        "From: Charlie <charlie@corp.local>\r\n",
        "To: Counsel <counsel@corp.local>\r\n",
        "Subject: Unrelated invoice notice\r\n",
        "\r\n",
        "Routine utility bill for November.\r\n",
        "\r\n",
        "From mailer-daemon@corp.local Mon Nov 8 12:00:00 2026\r\n",
        "Message-ID: <msg003@discovery.local>\r\n",
        "Date: Mon, 8 Nov 2026 12:00:00 +0000\r\n",
        "From: Legal <legal@enterprise.local>\r\n",
        "To: Bob <bob@corp.local>\r\n",
        "Subject: =?UTF-8?Q?500_Enterprise_Way_Agreement_Draft?=\r\n",
        "\r\n",
        "Attached draft agreement for 500 Enterprise project.\r\n",
        "\r\n"
    );
    fs::write(&inbox_mbox, mbox_content.as_bytes()).unwrap();

    // 2. Create search rules file
    let rules_path = temp.path().join("search_rules.toml");
    let rules_toml = r#"
[[rules]]
tag = "CORPORATE"
description = "Corporate Blvd address variants"
pattern = "100\\s*Corporate"

[[rules]]
tag = "ENTERPRISE"
description = "Enterprise Way project"
pattern = "500\\s*Enterprise"
"#;
    fs::write(&rules_path, rules_toml).unwrap();

    // 3. Test CLI Search with HTML index & Manifest CSV
    let eml_out_dir = temp.path().join("search_eml_out");
    let manifest_path = temp.path().join("master_manifest.csv");

    let bin_path = env!("CARGO_BIN_EXE_mail2rust");
    let status = Command::new(bin_path)
        .arg("search")
        .arg(&source_dir)
        .arg(&eml_out_dir)
        .arg("--rules")
        .arg(&rules_path)
        .arg("--html-index")
        .arg("--manifest-csv")
        .arg(&manifest_path)
        .arg("--format")
        .arg("eml")
        .status()
        .expect("Execution of mail2rust search failed");

    assert!(status.success(), "mail2rust search CLI exited with failure");

    // Verify HTML Index
    let index_path = eml_out_dir.join("index.html");
    assert!(index_path.exists(), "index.html should exist");
    let html = fs::read_to_string(&index_path).unwrap();
    assert!(html.contains("e-Discovery Search Results"));
    assert!(html.contains("RE: 100 Corporate Blvd Inspection"));
    assert!(html.contains("500 Enterprise Way Agreement Draft"));
    assert!(html.contains("CORPORATE"));
    assert!(html.contains("ENTERPRISE"));
    assert!(html.contains("sortTable"));
    assert!(html.contains("filterTable"));
    assert!(!html.contains("Unrelated invoice notice")); // filtered out

    // Verify Manifest CSV
    assert!(manifest_path.exists(), "master_manifest.csv should exist");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    assert!(manifest.contains("Message_ID,Date,From,To,Subject,Size_Bytes,Matched_Keywords"));
    assert!(manifest.contains("msg001@discovery.local"));
    assert!(manifest.contains("msg003@discovery.local"));
    assert!(!manifest.contains("msg002@discovery.local")); // filtered out

    // 4. Test Convert with --search-rules directly into PST
    let pst_out = temp.path().join("search_production.pst");
    let convert_status = Command::new(bin_path)
        .arg("convert")
        .arg(&source_dir)
        .arg(&pst_out)
        .arg("--search-rules")
        .arg(&rules_path)
        .status()
        .expect("Execution of mail2rust convert with search rules failed");

    assert!(
        convert_status.success(),
        "mail2rust convert with search rules exited with failure"
    );
    assert!(pst_out.exists(), "search_production.pst should exist");

    // Validate PST container integrity
    let p_file = File::open(&pst_out).expect("open generated PST");
    let report = PstValidator::validate(p_file).expect("validate PST");
    assert!(report.is_valid, "PST container must be valid");
    assert!(report.crc_match, "PST CRC must match");
}
