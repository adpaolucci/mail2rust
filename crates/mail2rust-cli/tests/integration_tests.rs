use chrono::Utc;
use mail2rust_core::{Attachment, EmailAddress, EmailMessage, FormatSniffer, Importance, MailboxFormat, MessageFlags};
use mail2rust_eml::{EmlExporter, EmlLayout};
use mail2rust_filter::{SplitConfig, VolumeSplitter};
use mail2rust_forensics::ForensicEngine;
use mail2rust_mbox::MboxFileExporter;
use mail2rust_pst::{PstValidator, PstWriter};
use mail2rust_sync::CheckpointManager;
use std::fs::File;
use tempfile::tempdir;

fn create_sample_message(id: &str, thread_id: &str, subject: &str, body: &str) -> EmailMessage {
    let mut msg = EmailMessage {
        id: id.to_string(),
        message_id: format!("<{}@test.corp.local>", id),
        conversation_id: thread_id.to_string(),
        folder_path: vec!["Inbox".to_string(), "Project Titan".to_string()],
        subject: subject.to_string(),
        from: EmailAddress::new("Dr. Evelyn Vance", "evelyn@titan.corp.local"),
        to: vec![EmailAddress::new("Anthony Paolucci", "adpaolucci@corp.local")],
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
        let (mut socket, _) = listener.accept().expect("Failed to accept");
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
    assert!(pst_reader.is_password_protected());
    assert!(pst_reader.verify_password("SecretPass123!"));
    assert!(!pst_reader.verify_password("WrongPassword"));

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
