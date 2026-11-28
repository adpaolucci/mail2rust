#![forbid(unsafe_code)]

pub mod address_book;
pub mod attach_view;
pub mod cfb_ole2;
pub mod error;
pub mod fixtures;
pub mod nk2;
pub mod oab;
pub mod office_ins;
pub mod pst_ext;
pub mod regf;
pub mod stat_view;
pub mod time;

use guap_mcp::McpServerRegistry;
use std::sync::Arc;

pub use address_book::{
    ContactRecord, ContactsTableProvider, OutlookContactsCommand, collect_contacts, export_contacts_vcf,
};
pub use attach_view::{
    AttachTableProvider, AttachmentFilter, AttachmentRecord, ExtractionResult, OutlookAttachCommand,
    OutlookAttachExtractCommand, OutlookAttachStripCommand, collect_attachments, extract_attachments_to_dir,
    strip_attachments_from_source,
};
pub use cfb_ole2::{CfbContainer, MsgAttachment, MsgDocument, build_msg_cfb_bytes, strip_msg_cfb_bytes};
pub use error::{OutlookError, OutlookResult, ScanWarning};
pub use nk2::{
    EntryIdKind, NK2_MAGIC, Nk2Archive, Nk2EditCommand, Nk2Entry, Nk2InspectCommand, Nk2PropValue, Nk2RawProperty,
    Nk2TableProvider, decode_entry_id, encode_exchange_ab_entry_id, encode_oneoff_entry_id,
};
pub use oab::{OAB_COMPRESSED_V4_MAGIC, OabEntry, build_uncompressed_oab_v4, parse_oab_bytes, parse_oab_file};
#[cfg(windows)]
pub use office_ins::LiveWindowsRegistry;
pub use office_ins::{
    HiveRoot, MockRegistryBackend, OfficeAddinRecord, OfficeAddinsListCommand, OfficeAddinsTableProvider,
    OfficeAddinsToggleCommand, OfficeScanReport, OfflineRegfBackend, RegistryBackend, check_pe_authenticode_bytes,
    check_pe_authenticode_present, default_appdata_microsoft_dir, scan_office_addins, toggle_office_addin,
};
pub use pst_ext::{PstContactRaw, extract_pst_contacts, load_messages_from_source, strip_pst_to_file};
pub use regf::{RegKeyNode, RegValue, RegfHive, build_synthetic_regf_hive};
pub use stat_view::{
    OutlookStatsCommand, StatRecord, StatsGroupBy, StatsOptions, StatsTableProvider, aggregate_messages_stats,
    compute_mailbox_stats,
};

/// Registers all Outlook & Office `ActionCommand`s (read-only + mutating) into a `guap_mcp::McpServerRegistry`.
pub fn register_outlook_mcp_commands(registry: &mut McpServerRegistry) {
    registry.register_action(Arc::new(Nk2InspectCommand));
    registry.register_action(Arc::new(Nk2EditCommand));
    registry.register_action(Arc::new(OutlookAttachCommand));
    registry.register_action(Arc::new(OutlookAttachExtractCommand));
    registry.register_action(Arc::new(OutlookAttachStripCommand));
    registry.register_action(Arc::new(OutlookStatsCommand));
    registry.register_action(Arc::new(OutlookContactsCommand));
    registry.register_action(Arc::new(OfficeAddinsListCommand));
    registry.register_action(Arc::new(OfficeAddinsToggleCommand));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{build_synthetic_nk2_bytes, generate_all_outlook_fixtures};
    use guap_export::{ExportFormat, ExportOptions, write_table_export};
    use guap_mcp::{McpServerRegistry, PolicyMode};
    use guap_table::{TableProvider, TableQuery};
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn nk2_and_stream_autocomplete_roundtrip_edit_merge_and_atomic_save() {
        let dir = tempdir().unwrap();
        let nk2_path = dir.path().join("Outlook.nk2");
        let dat_path = dir.path().join("Stream_Autocomplete_0_TEST.dat");
        std::fs::write(&nk2_path, build_synthetic_nk2_bytes(false)).unwrap();
        std::fs::write(&dat_path, build_synthetic_nk2_bytes(true)).unwrap();

        let mut arc = Nk2Archive::from_file(&nk2_path).unwrap();
        assert_eq!(arc.entries.len(), 3);
        assert_eq!(arc.entries[0].display_name, "Dr. Evelyn Vance");
        assert_eq!(arc.entries[0].email_address, "evelyn.vance@titan.corp");
        assert_eq!(arc.entries[1].address_type, "EX");
        assert!(matches!(arc.entries[1].entry_id_kind, EntryIdKind::ExchangeAb { .. }));
        // Verify unknown tag 0x6000_0003 preserved
        assert!(arc.entries[0].raw_props.iter().any(|p| p.tag() == 0x6000_0003));
        assert_eq!(arc.footer.len(), 12);

        // Add + update + remove + dedup + atomic save
        arc.add_entry(Nk2Entry::new_smtp("Duplicate Evelyn", "evelyn.vance@titan.corp", 999));
        assert_eq!(arc.entries.len(), 4);
        assert_eq!(arc.dedup(), 1);
        assert_eq!(arc.update_weight("sconnor@cyber.gov", 500), 1);
        assert_eq!(arc.remove_by_email("sconnor@cyber.gov"), 1);

        let dat_arc = Nk2Archive::from_file(&dat_path).unwrap();
        assert_eq!(dat_arc.meta_major, 0x0C);
        assert!(arc.merge(&dat_arc) >= 1);

        arc.save_atomic(&nk2_path, true).unwrap();
        assert!(nk2_path.with_extension("nk2.bak").exists());

        let reloaded = Nk2Archive::from_file(&nk2_path).unwrap();
        assert_eq!(reloaded.footer.len(), 12);
        assert!(reloaded.entries[0].raw_props.iter().any(|p| p.tag() == 0x6000_0003));

        // TableProvider + Export check
        let provider = Nk2TableProvider::from_archive(&reloaded);
        let data = provider.query(&TableQuery::default()).unwrap();
        assert_eq!(data.total_rows, reloaded.entries.len());
        let csv_out = dir.path().join("nk2.csv");
        let manifest = write_table_export(&data, &csv_out, ExportFormat::Csv, &ExportOptions::default()).unwrap();
        assert_eq!(manifest.artifact_count, 1);
        assert!(csv_out.exists());
    }

    #[test]
    fn outlook_attach_view_pst_msg_eml_extract_and_safe_strip() {
        let dir = tempdir().unwrap();
        let fx = generate_all_outlook_fixtures(dir.path()).unwrap();

        // 1. PST attachments (include_inline = true vs false)
        let all_pst = collect_attachments(
            &fx.pst_path,
            &AttachmentFilter {
                include_inline: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(all_pst.len(), 3);
        let non_inline_pst = collect_attachments(&fx.pst_path, &AttachmentFilter::default()).unwrap();
        assert_eq!(non_inline_pst.len(), 2);

        // 2. CFB .msg attachments
        let msg_atts = collect_attachments(
            &fx.msg_path,
            &AttachmentFilter {
                include_inline: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(msg_atts.len(), 2);
        assert!(
            msg_atts
                .iter()
                .any(|a| a.attachment_name == "Contract_Q4.pdf" && a.data.starts_with(b"%PDF-1.7"))
        );

        // 3. EML attachment
        let eml_atts = collect_attachments(&fx.eml_path, &AttachmentFilter::default()).unwrap();
        assert_eq!(eml_atts.len(), 1);
        assert_eq!(eml_atts[0].attachment_name, "metrics.csv");

        // 4. Bulk extract + custody manifest + collision-safe naming
        let extract_dir = dir.path().join("extracted");
        let mut dup_recs = all_pst.clone();
        dup_recs.push(all_pst[0].clone()); // trigger collision dedup
        let res = extract_attachments_to_dir(&dup_recs, &extract_dir).unwrap();
        assert_eq!(res.extracted_files.len(), 4);
        assert!(res.manifest_path.exists());
        assert!(!res.manifest.merkle_root_sha256.is_empty());

        // 5. Safe strip on PST & MSG (dry-run vs copy)
        let (_, dry_cnt) = strip_attachments_from_source(
            &fx.pst_path,
            None,
            &AttachmentFilter {
                extensions: vec!["pdf".to_string()],
                include_inline: true,
                ..Default::default()
            },
            false,
            true,
        )
        .unwrap();
        assert_eq!(dry_cnt, 1);

        let stripped_pst = dir.path().join("stripped.pst");
        let (out_p, removed) = strip_attachments_from_source(
            &fx.pst_path,
            Some(&stripped_pst),
            &AttachmentFilter {
                extensions: vec!["pdf".to_string()],
                include_inline: true,
                ..Default::default()
            },
            false,
            false,
        )
        .unwrap();
        assert_eq!(out_p, stripped_pst);
        assert_eq!(removed, 1);
        let remaining = collect_attachments(
            &stripped_pst,
            &AttachmentFilter {
                include_inline: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(remaining.len(), 2);

        let stripped_msg = dir.path().join("stripped.msg");
        let (_, msg_removed) = strip_attachments_from_source(
            &fx.msg_path,
            Some(&stripped_msg),
            &AttachmentFilter {
                extensions: vec!["pdf".to_string()],
                include_inline: true,
                ..Default::default()
            },
            false,
            false,
        )
        .unwrap();
        assert_eq!(msg_removed, 1);
        let rem_msg = collect_attachments(
            &stripped_msg,
            &AttachmentFilter {
                include_inline: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(rem_msg.len(), 1);
    }

    #[test]
    fn outlook_stat_view_aggregates_pst_and_mbox_by_correspondent_domain_folder_month() {
        let dir = tempdir().unwrap();
        let fx = generate_all_outlook_fixtures(dir.path()).unwrap();

        let by_corr = compute_mailbox_stats(
            &fx.pst_path,
            &StatsOptions {
                group_by: StatsGroupBy::Correspondent,
                ..Default::default()
            },
        )
        .unwrap();
        let evelyn = by_corr.iter().find(|r| r.email == "evelyn.vance@titan.corp").unwrap();
        assert_eq!(evelyn.received_count, 1);
        assert_eq!(evelyn.sent_count, 1);
        assert_eq!(evelyn.total_count, 2);

        let by_month = compute_mailbox_stats(
            &fx.mbox_path,
            &StatsOptions {
                group_by: StatsGroupBy::Month,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(by_month.iter().any(|r| r.correspondent == "2026-10"));
        assert!(by_month.iter().any(|r| r.correspondent == "2026-11"));

        let by_dom = compute_mailbox_stats(
            &fx.mbox_path,
            &StatsOptions {
                group_by: StatsGroupBy::Domain,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(by_dom.iter().any(|r| r.domain == "titan.corp"));
        assert!(by_dom.iter().any(|r| r.domain == "cyber.gov"));
    }

    #[test]
    fn outlook_address_book_view_pst_contacts_distlist_oab_nk2_and_vcard_roundtrip() {
        let dir = tempdir().unwrap();
        let fx = generate_all_outlook_fixtures(dir.path()).unwrap();

        let pst_contacts = collect_contacts(&fx.pst_path).unwrap();
        assert_eq!(pst_contacts.len(), 2);
        let person = pst_contacts.iter().find(|c| c.entry_type == "Contact").unwrap();
        assert_eq!(person.display_name, "Dr. Evelyn Vance");
        assert_eq!(person.email1, "evelyn.vance@titan.corp");
        assert_eq!(person.company, "Titan Aerospace");
        let dl = pst_contacts.iter().find(|c| c.entry_type == "DistList").unwrap();
        assert_eq!(dl.members.len(), 2);

        // vCard 3.0 export & re-parse roundtrip
        let vcf_text = export_contacts_vcf(&pst_contacts);
        let vcf_path = dir.path().join("exported.vcf");
        std::fs::write(&vcf_path, &vcf_text).unwrap();
        let vcf_contacts = collect_contacts(&vcf_path).unwrap();
        assert_eq!(vcf_contacts.len(), 2);

        // Uncompressed OAB v4
        let oab_contacts = collect_contacts(&fx.oab_path).unwrap();
        assert_eq!(oab_contacts.len(), 2);

        // LZX-compressed OAB returns graceful UnsupportedFeature
        let mut lzx_bytes = vec![0u8; 32];
        lzx_bytes[0..4].copy_from_slice(&OAB_COMPRESSED_V4_MAGIC.to_le_bytes());
        let lzx_err = parse_oab_bytes(&lzx_bytes).unwrap_err();
        assert!(matches!(lzx_err, OutlookError::UnsupportedFeature(_)));

        // NK2 -> Contacts
        let nk2_contacts = collect_contacts(&fx.nk2_path).unwrap();
        assert_eq!(nk2_contacts.len(), 3);
    }

    #[test]
    fn office_ins_offline_regf_pe_signature_appdata_and_mock_access_denied() {
        let dir = tempdir().unwrap();
        let fx = generate_all_outlook_fixtures(dir.path()).unwrap();

        // 1. Offline regf hives + AppData + PE signature check
        let backend =
            OfflineRegfBackend::from_hive_paths(&[fx.ntuser_dat_path.clone(), fx.software_hive_path.clone()]).unwrap();
        let report = scan_office_addins(&backend, Some(&fx.appdata_microsoft_dir));
        assert!(report.warnings.is_empty());

        let titan = report
            .addins
            .iter()
            .find(|a| a.prog_id == "Titan.OutlookAddin")
            .unwrap();
        assert_eq!(titan.office_app, "Outlook");
        assert_eq!(titan.load_behavior, 3);
        assert!(titan.enabled);
        assert!(titan.signed, "Signed PE DLL must be detected");

        let contoso = report.addins.iter().find(|a| a.prog_id == "Contoso.WordWll").unwrap();
        assert_eq!(contoso.office_app, "Word");
        assert!(!contoso.enabled);
        assert!(!contoso.signed, "Unsigned PE DLL must be false");

        assert!(report.addins.iter().any(|a| a.addin_name == "corp_macros.dotm"));
        assert!(report.addins.iter().any(|a| a.addin_name == "VbaProject.OTM"));

        // 2. MockRegistryBackend: graceful AccessDenied on HKLM + toggle behavior
        let mock = MockRegistryBackend::default();
        let mut hkcu_vals = BTreeMap::new();
        hkcu_vals.insert(
            "FriendlyName".to_string(),
            RegValue::String("Mock Outlook Addin".to_string()),
        );
        hkcu_vals.insert("LoadBehavior".to_string(), RegValue::Dword(3));
        mock.insert_key(
            HiveRoot::Hkcu,
            "Software\\Microsoft\\Office\\Outlook\\Addins\\Mock.Addin",
            hkcu_vals,
        );
        mock.deny_prefix(HiveRoot::Hklm, "Software\\Microsoft\\Office");

        let mock_report = scan_office_addins(&mock, None);
        assert_eq!(mock_report.addins.len(), 1);
        assert!(
            !mock_report.warnings.is_empty(),
            "HKLM AccessDenied must produce non-fatal warning"
        );

        // Dry-run toggle then live toggle on mock
        let (_, old_v, new_v) = toggle_office_addin(&mock, "Mock.Addin", None, false, true).unwrap();
        assert_eq!((old_v, new_v), (3, 0));
        toggle_office_addin(&mock, "Mock.Addin", None, false, false).unwrap();
        let after = scan_office_addins(&mock, None);
        assert_eq!(after.addins[0].load_behavior, 0);
        assert!(!after.addins[0].enabled);
    }

    #[test]
    fn mcp_policy_gate_enforces_32003_on_mutating_outlook_and_office_commands() {
        let dir = tempdir().unwrap();
        let fx = generate_all_outlook_fixtures(dir.path()).unwrap();

        let mut ro_reg = McpServerRegistry::new(PolicyMode::ReadOnlyOnly);
        register_outlook_mcp_commands(&mut ro_reg);

        // ReadOnly tools work under ReadOnlyOnly
        let nk2_res = ro_reg
            .dispatch(
                "mail2rust_nk2_inspect",
                &json!({ "path": fx.nk2_path.to_string_lossy() }),
            )
            .unwrap();
        assert_eq!(nk2_res["total_rows"], 3);

        // Mutating tools return -32003 under ReadOnlyOnly
        let err_ro = ro_reg
            .dispatch(
                "mail2rust_nk2_edit",
                &json!({ "path": fx.nk2_path.to_string_lossy(), "dedup": true, "confirm_destructive": true }),
            )
            .unwrap_err();
        assert_eq!(err_ro.code, -32003);

        let err_toggle = ro_reg
            .dispatch(
                "mail2rust_office_addins_toggle",
                &json!({ "prog_id": "Titan.OutlookAddin", "enable": false, "confirm_destructive": true }),
            )
            .unwrap_err();
        assert_eq!(err_toggle.code, -32003);

        // Under AllowMutating without confirm_destructive=true -> still -32003
        let mut rw_reg = McpServerRegistry::new(PolicyMode::AllowMutating);
        register_outlook_mcp_commands(&mut rw_reg);
        let err_no_confirm = rw_reg
            .dispatch(
                "mail2rust_nk2_edit",
                &json!({ "path": fx.nk2_path.to_string_lossy(), "dedup": true }),
            )
            .unwrap_err();
        assert_eq!(err_no_confirm.code, -32003);

        // With AllowMutating + confirm_destructive=true + dry_run=true -> succeeds
        let ok_edit = rw_reg
            .dispatch(
                "mail2rust_nk2_edit",
                &json!({
                    "path": fx.nk2_path.to_string_lossy(),
                    "dedup": true,
                    "dry_run": true,
                    "confirm_destructive": true
                }),
            )
            .unwrap();
        assert_eq!(ok_edit["status"], "dry_run");

        // mail2rust_outlook_attach_extract is ActionRiskLevel::Mutating (writes files to disk)
        let blocked_extract_dir = dir.path().join("blocked_extract");
        let err_extract_ro = ro_reg
            .dispatch(
                "mail2rust_outlook_attach_extract",
                &json!({
                    "source": fx.pst_path.to_string_lossy(),
                    "extract_dir": blocked_extract_dir.to_string_lossy(),
                    "confirm_destructive": true
                }),
            )
            .unwrap_err();
        assert_eq!(err_extract_ro.code, -32003);
        assert!(!blocked_extract_dir.exists());

        let err_extract_nc = rw_reg
            .dispatch(
                "mail2rust_outlook_attach_extract",
                &json!({
                    "source": fx.pst_path.to_string_lossy(),
                    "extract_dir": blocked_extract_dir.to_string_lossy()
                }),
            )
            .unwrap_err();
        assert_eq!(err_extract_nc.code, -32003);
        assert!(!blocked_extract_dir.exists());

        // Dry-run preview under AllowMutating succeeds without confirm_destructive and writes nothing
        let dry_extract = rw_reg
            .dispatch(
                "mail2rust_outlook_attach_extract",
                &json!({
                    "source": fx.pst_path.to_string_lossy(),
                    "extract_dir": blocked_extract_dir.to_string_lossy(),
                    "dry_run": true
                }),
            )
            .unwrap();
        assert_eq!(dry_extract["status"], "dry_run");
        assert!(!blocked_extract_dir.exists());

        // DryRunOnly rejects non-dry-run (-32003) and permits dry_run=true preview
        let mut dry_reg = McpServerRegistry::new(PolicyMode::DryRunOnly);
        register_outlook_mcp_commands(&mut dry_reg);
        let err_dry_policy = dry_reg
            .dispatch(
                "mail2rust_outlook_attach_extract",
                &json!({
                    "source": fx.pst_path.to_string_lossy(),
                    "extract_dir": blocked_extract_dir.to_string_lossy(),
                    "confirm_destructive": true
                }),
            )
            .unwrap_err();
        assert_eq!(err_dry_policy.code, -32003);
        let ok_dry_policy = dry_reg
            .dispatch(
                "mail2rust_outlook_attach_extract",
                &json!({
                    "source": fx.pst_path.to_string_lossy(),
                    "extract_dir": blocked_extract_dir.to_string_lossy(),
                    "dry_run": true
                }),
            )
            .unwrap();
        assert_eq!(ok_dry_policy["status"], "dry_run");
        assert!(!blocked_extract_dir.exists());

        // Confirmed AllowMutating extracts attachments + custody_manifest.json
        let ok_extract_dir = dir.path().join("allowed_extract");
        let ok_extract = rw_reg
            .dispatch(
                "mail2rust_outlook_attach_extract",
                &json!({
                    "source": fx.pst_path.to_string_lossy(),
                    "extract_dir": ok_extract_dir.to_string_lossy(),
                    "confirm_destructive": true
                }),
            )
            .unwrap();
        assert_eq!(ok_extract["status"], "extracted");
        assert!(ok_extract_dir.join("custody_manifest.json").is_file());
    }

    #[test]
    fn attach_extract_deduplicates_collisions_three_identical_and_extensionless() {
        let dir = tempdir().unwrap();
        let out_dir = dir.path().join("extracted");

        let make_rec = |row_id: u64, name: &str, payload: &[u8]| AttachmentRecord {
            row_id,
            message_id: format!("<m{row_id}@test.local>"),
            message_subject: "Collision Test".to_string(),
            from: "sender@test.local".to_string(),
            received_time: chrono::Utc::now(),
            folder: "Inbox".to_string(),
            attachment_name: name.to_string(),
            extension: Path::new(name)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string(),
            mime_type: "application/octet-stream".to_string(),
            size_bytes: payload.len(),
            sha256: guap_export::sha256_hex(payload),
            content_id: None,
            is_inline: false,
            data: payload.to_vec(),
        };

        // PAYLOAD_B_DUP_DIFFERENT has sha256 prefix e5c7b3eb...
        let payload_b = b"PAYLOAD_B_DUP_DIFFERENT";
        let b_pfx = &guap_export::sha256_hex(payload_b)[..8];
        let precollide_name = format!("dup_{b_pfx}.txt");

        let recs = vec![
            // (a) Pre-colliding name + two dup.txt (one matching pre-colliding hash prefix)
            make_rec(0, &precollide_name, b"PRECOLLIDE_DIFFERENT_CONTENT"),
            make_rec(1, "dup.txt", b"PAYLOAD_A_DUP"),
            make_rec(2, "dup.txt", payload_b),
            // (b) 3+ identical-name & identical-SHA-256 attachments
            make_rec(3, "logo.png", b"IDENTICAL_LOGO_BYTES"),
            make_rec(4, "logo.png", b"IDENTICAL_LOGO_BYTES"),
            make_rec(5, "logo.png", b"IDENTICAL_LOGO_BYTES"),
            make_rec(6, "logo.png", b"IDENTICAL_LOGO_BYTES"),
            // (c) Extensionless attachments (no .bin appended)
            make_rec(7, "README", b"EXTLESS_PAYLOAD_1"),
            make_rec(8, "README", b"EXTLESS_PAYLOAD_1"),
            make_rec(9, "README", b"EXTLESS_PAYLOAD_1"),
        ];

        let res = extract_attachments_to_dir(&recs, &out_dir).unwrap();
        assert_eq!(res.extracted_files.len(), recs.len());
        assert_eq!(res.manifest.leaves.len(), recs.len());

        // Zero overwrites: all paths unique
        let unique_paths: std::collections::HashSet<_> = res
            .extracted_files
            .iter()
            .map(|p| p.to_string_lossy().to_ascii_lowercase())
            .collect();
        assert_eq!(unique_paths.len(), recs.len());

        // Extensionless filenames preserved without .bin
        for p in &res.extracted_files[7..10] {
            let fname = p.file_name().unwrap().to_str().unwrap();
            assert!(fname.starts_with("README"));
            assert!(!fname.ends_with(".bin"), "unexpected .bin on {fname}");
        }

        // 100% SHA-256 match between custody_manifest.json and disk files + original precollide intact
        let manifest_json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&res.manifest_path).unwrap()).unwrap();
        let leaves = manifest_json["leaves"].as_array().unwrap();
        assert_eq!(leaves.len(), recs.len());
        for (i, leaf) in leaves.iter().enumerate() {
            let p = Path::new(leaf["path"].as_str().unwrap());
            let disk_bytes = std::fs::read(p).unwrap();
            assert_eq!(guap_export::sha256_hex(&disk_bytes), leaf["sha256"].as_str().unwrap());
            assert_eq!(disk_bytes, recs[i].data);
        }
    }

    #[test]
    fn oab_partial_fields_roundtrip_without_msb_lsb_aliasing() {
        let entries = vec![
            OabEntry {
                display_name: "Alice".to_string(),
                email1: "a@b.com".to_string(),
                email2: String::new(),
                company: String::new(),
                job_title: String::new(),
                department: String::new(),
                phone: "555-1234".to_string(),
            },
            OabEntry {
                display_name: "Bob".to_string(),
                email1: "bob@acme.org".to_string(),
                email2: String::new(),
                company: "Acme Corp".to_string(),
                job_title: String::new(),
                department: "Ops".to_string(),
                phone: String::new(),
            },
        ];

        let raw = build_uncompressed_oab_v4(&entries);
        let parsed = parse_oab_bytes(&raw).unwrap();
        assert_eq!(parsed.len(), 2);

        assert_eq!(parsed[0].display_name, "Alice");
        assert_eq!(parsed[0].email1, "a@b.com");
        assert_eq!(parsed[0].company, "");
        assert_eq!(parsed[0].job_title, "");
        assert_eq!(parsed[0].department, "");
        assert_eq!(parsed[0].phone, "555-1234");

        assert_eq!(parsed[1].display_name, "Bob");
        assert_eq!(parsed[1].email1, "bob@acme.org");
        assert_eq!(parsed[1].company, "Acme Corp");
        assert_eq!(parsed[1].job_title, "");
        assert_eq!(parsed[1].department, "Ops");
        assert_eq!(parsed[1].phone, "");
    }

    #[test]
    fn stat_view_excludes_non_mail_and_domain_counts_all_sent_recipients() {
        let dir = tempdir().unwrap();
        let pst_path = dir.path().join("outlook_suite.pst");
        crate::fixtures::build_synthetic_outlook_pst(&pst_path).unwrap();

        // (a) Mixed PST excludes IPM.Contact & IPM.DistList (unknown@pst.local & Contacts folder absent)
        for group_by in [StatsGroupBy::Correspondent, StatsGroupBy::Domain, StatsGroupBy::Folder] {
            let rows = compute_mailbox_stats(
                &pst_path,
                &StatsOptions {
                    group_by,
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(
                !rows.iter().any(|r| r.email.contains("unknown@pst.local")),
                "unexpected unknown@pst.local in {group_by:?}: {rows:?}"
            );
            assert!(
                !rows.iter().any(|r| r.correspondent == "Contacts"),
                "unexpected Contacts folder in {group_by:?}: {rows:?}"
            );
        }

        // (b) StatsGroupBy::Domain counts all recipients (alpha.com & gamma.net) on multi-recipient sent email
        let sent_msg = mail2rust_core::EmailMessage {
            id: "sent_multi".to_string(),
            message_id: "<multi@corp.local>".to_string(),
            conversation_id: "c_multi".to_string(),
            folder_path: vec!["Sent Items".to_string()],
            subject: "Multi-recipient".to_string(),
            from: mail2rust_core::EmailAddress::new("Sender", "sender@corp.local"),
            to: vec![
                mail2rust_core::EmailAddress::new("Alice", "alice@alpha.com"),
                mail2rust_core::EmailAddress::new("Carol", "carol@gamma.net"),
                mail2rust_core::EmailAddress::new("Dave", "dave@alpha.com"),
            ],
            cc: vec![],
            bcc: vec![],
            date: chrono::Utc::now(),
            size_bytes: 128,
            body_text: "Hello".to_string(),
            body_html: None,
            raw_mime: vec![],
            attachments: vec![],
            mapi_properties: vec![mail2rust_core::MapiProperty {
                tag: mail2rust_pst::PR_MESSAGE_FLAGS,
                name: "PR_MESSAGE_FLAGS".to_string(),
                prop_type: "PT_LONG".to_string(),
                value: mail2rust_pst::MSGFLAG_FROMME.to_string(),
            }],
            flags: Default::default(),
            importance: mail2rust_core::Importance::Normal,
            sha256: String::new(),
            md5: String::new(),
            bates_number: None,
            legal_hold: false,
            source_format: "PST".to_string(),
        };

        let dom_stats = aggregate_messages_stats(
            &[sent_msg],
            &StatsOptions {
                group_by: StatsGroupBy::Domain,
                ..Default::default()
            },
        );
        let alpha = dom_stats.iter().find(|r| r.domain == "alpha.com").unwrap();
        let gamma = dom_stats.iter().find(|r| r.domain == "gamma.net").unwrap();
        assert_eq!(alpha.sent_count, 1);
        assert_eq!(gamma.sent_count, 1);
    }

    #[test]
    fn strip_pst_preserves_ipm_contact_and_distlist_fields() {
        let dir = tempdir().unwrap();
        let src_pst = dir.path().join("src.pst");
        let stripped_pst = dir.path().join("stripped.pst");
        crate::fixtures::build_synthetic_outlook_pst(&src_pst).unwrap();

        let (out_path, removed) = strip_attachments_from_source(
            &src_pst,
            Some(&stripped_pst),
            &AttachmentFilter {
                extensions: vec!["pdf".to_string()],
                include_inline: true,
                ..Default::default()
            },
            false,
            false,
        )
        .unwrap();
        assert_eq!(out_path, stripped_pst);
        assert_eq!(removed, 1);

        let raw_contacts = extract_pst_contacts(&stripped_pst).unwrap();
        assert_eq!(raw_contacts.len(), 2);

        let person = raw_contacts
            .iter()
            .find(|c| c.message_class.eq_ignore_ascii_case("IPM.Contact"))
            .unwrap();
        assert_eq!(person.display_name, "Dr. Evelyn Vance");
        assert_eq!(person.email1, "evelyn.vance@titan.corp");
        assert_eq!(person.email2, "evance@personal.org");
        assert_eq!(person.company, "Titan Aerospace");
        assert_eq!(person.job_title, "Chief Systems Architect");
        assert_eq!(person.department, "Systems Engineering");
        assert_eq!(person.phone, "+1-415-555-0199");
        assert_eq!(person.mobile_phone, "+1-415-555-0100");
        assert_eq!(person.notes, "Primary engineering contact");
        assert!(person.modified_time.is_some());

        let dl = raw_contacts
            .iter()
            .find(|c| c.message_class.eq_ignore_ascii_case("IPM.DistList"))
            .unwrap();
        assert_eq!(dl.display_name, "Architecture Review Board");
        assert_eq!(dl.members.len(), 2);
    }

    #[test]
    fn office_ins_offline_isolation_multi_hive_pe_rva_and_nk2_merge_bak() {
        let dir = tempdir().unwrap();
        let fx = generate_all_outlook_fixtures(dir.path()).unwrap();

        // 1. Offline regf scan with appdata=None does NOT fall back to host AppData
        let prov = OfficeAddinsTableProvider::scan_from_hives(
            &[fx.ntuser_dat_path.clone(), fx.software_hive_path.clone()],
            None,
        )
        .unwrap();
        let data = prov.query(&TableQuery::default()).unwrap();
        assert!(
            !data
                .rows
                .iter()
                .any(|r| r.detail_json.as_ref().and_then(|d| d["hive_location"].as_str()) == Some("AppData")),
            "Offline regf scan must not inject host AppData add-ins"
        );

        // 2. Multiple user hives do not overwrite each other
        let mut second_user_root = RegKeyNode {
            name: "$$$PROTO.HIV".to_string(),
            last_written: Some(chrono::Utc::now()),
            values: BTreeMap::new(),
            subkeys: BTreeMap::new(),
        };
        let mut addin2 = RegKeyNode {
            name: "SecondUser.Addin".to_string(),
            last_written: Some(chrono::Utc::now()),
            values: BTreeMap::new(),
            subkeys: BTreeMap::new(),
        };
        addin2.values.insert(
            "FriendlyName".to_string(),
            RegValue::String("Second User Addin".to_string()),
        );
        addin2.values.insert("LoadBehavior".to_string(), RegValue::Dword(3));
        let mut addins_k = RegKeyNode {
            name: "Addins".to_string(),
            ..Default::default()
        };
        addins_k.subkeys.insert("SecondUser.Addin".to_string(), addin2);
        let mut ol_k = RegKeyNode {
            name: "Outlook".to_string(),
            ..Default::default()
        };
        ol_k.subkeys.insert("Addins".to_string(), addins_k);
        let mut off_k = RegKeyNode {
            name: "Office".to_string(),
            ..Default::default()
        };
        off_k.subkeys.insert("Outlook".to_string(), ol_k);
        let mut ms_k = RegKeyNode {
            name: "Microsoft".to_string(),
            ..Default::default()
        };
        ms_k.subkeys.insert("Office".to_string(), off_k);
        let mut sw_k = RegKeyNode {
            name: "Software".to_string(),
            ..Default::default()
        };
        sw_k.subkeys.insert("Microsoft".to_string(), ms_k);
        second_user_root.subkeys.insert("Software".to_string(), sw_k);

        let second_ntuser = dir.path().join("Second_NTUSER.DAT");
        std::fs::write(&second_ntuser, build_synthetic_regf_hive(&second_user_root)).unwrap();
        let multi_backend = OfflineRegfBackend::from_hive_paths(&[fx.ntuser_dat_path.clone(), second_ntuser]).unwrap();
        let multi_rep = scan_office_addins(&multi_backend, None);
        assert!(multi_rep.addins.iter().any(|a| a.prog_id == "Titan.OutlookAddin"));
        assert!(multi_rep.addins.iter().any(|a| a.prog_id == "SecondUser.Addin"));

        // 3. PE NumberOfRvaAndSizes <= 4 must reject crafted security directory bytes
        let mut crafted_pe = crate::fixtures::build_synthetic_pe_dll(true);
        assert!(check_pe_authenticode_bytes(&crafted_pe));
        let opt_off = 0x80 + 24;
        crafted_pe[opt_off + 108..opt_off + 112].copy_from_slice(&0u32.to_le_bytes());
        assert!(!check_pe_authenticode_bytes(&crafted_pe));

        // 4. NK2 merge dedupes EX vs SMTP by smtp_address, and save_atomic appends .bak to .dat
        let mut base = Nk2Archive::from_file(&fx.nk2_path).unwrap(); // contains EX entry with smtp m.brody@titan.corp
        let other = Nk2Archive {
            meta_major: 0x0C,
            meta_minor: 0x01,
            entries: vec![Nk2Entry::new_smtp("Marcus Brody SMTP", "m.brody@titan.corp", 100)],
            footer: vec![],
        };
        assert_eq!(base.merge(&other), 0);

        let dat_path = dir.path().join("Stream_Autocomplete_0.dat");
        std::fs::write(&dat_path, build_synthetic_nk2_bytes(true)).unwrap();
        base.save_atomic(&dat_path, true).unwrap();
        assert!(dir.path().join("Stream_Autocomplete_0.dat.bak").exists());
        assert!(!dir.path().join("Stream_Autocomplete_0.nk2.bak").exists());
    }
}
