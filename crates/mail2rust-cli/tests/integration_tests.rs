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
}
