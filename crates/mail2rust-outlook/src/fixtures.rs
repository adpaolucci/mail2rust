use crate::cfb_ole2::build_msg_cfb_bytes;
use crate::error::OutlookResult;
use crate::nk2::{
    Nk2Archive, Nk2Entry, Nk2PropValue, Nk2RawProperty, decode_entry_id, encode_exchange_ab_entry_id,
    encode_oneoff_entry_id,
};
use crate::oab::{OabEntry, build_uncompressed_oab_v4};
use crate::regf::{RegKeyNode, RegValue, build_synthetic_regf_hive};
use crate::time::datetime_to_filetime;
use chrono::{TimeZone, Utc};
use mail2rust_core::{Attachment, EmailAddress, EmailMessage, Importance, MapiProperty, MessageFlags};
use mail2rust_pst::{
    MSGFLAG_FROMME, PR_BODY_W, PR_BUSINESS_TELEPHONE_NUMBER_W, PR_COMPANY_NAME_W, PR_CONTACT_EMAIL1_W,
    PR_CONTACT_EMAIL2_W, PR_DEPARTMENT_NAME_W, PR_DISPLAY_NAME_W, PR_DISTLIST_ONEOFF_MEMBERS,
    PR_LAST_MODIFICATION_TIME, PR_MESSAGE_CLASS_W, PR_MESSAGE_FLAGS, PR_MOBILE_TELEPHONE_NUMBER_W, PR_SUBJECT_W,
    PR_TITLE_W, PropertyContext, PstWriter,
};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

/// Generates a full suite of synthetic Outlook & Office fixtures in `base_dir` for testing and demos.
pub struct OutlookFixturePaths {
    pub nk2_path: PathBuf,
    pub autocomplete_dat_path: PathBuf,
    pub pst_path: PathBuf,
    pub msg_path: PathBuf,
    pub mbox_path: PathBuf,
    pub eml_path: PathBuf,
    pub oab_path: PathBuf,
    pub ntuser_dat_path: PathBuf,
    pub software_hive_path: PathBuf,
    pub appdata_microsoft_dir: PathBuf,
    pub signed_dll_path: PathBuf,
    pub unsigned_dll_path: PathBuf,
}

pub fn generate_all_outlook_fixtures(base_dir: &Path) -> OutlookResult<OutlookFixturePaths> {
    fs::create_dir_all(base_dir)?;

    let nk2_path = base_dir.join("Outlook.nk2");
    let nk2_bytes = build_synthetic_nk2_bytes(false);
    fs::write(&nk2_path, nk2_bytes)?;

    let autocomplete_dat_path = base_dir.join("Stream_Autocomplete_0_0123456789ABCDEF0123456789ABCDEF.dat");
    fs::write(&autocomplete_dat_path, build_synthetic_nk2_bytes(true))?;

    let pst_path = base_dir.join("outlook_suite.pst");
    build_synthetic_outlook_pst(&pst_path)?;

    let msg_path = base_dir.join("sample_with_attachments.msg");
    fs::write(&msg_path, build_synthetic_msg_cfb())?;

    let mbox_path = base_dir.join("outlook_stats.mbox");
    let eml_path = base_dir.join("quarterly_report.eml");
    build_synthetic_mbox_and_eml(&mbox_path, &eml_path)?;

    let oab_path = base_dir.join("udetails.oab");
    fs::write(&oab_path, build_synthetic_oab_v4())?;

    let signed_dll_path = base_dir.join("TitanSignedAddin.dll");
    let unsigned_dll_path = base_dir.join("ContosoUnsigned.dll");
    fs::write(&signed_dll_path, build_synthetic_pe_dll(true))?;
    fs::write(&unsigned_dll_path, build_synthetic_pe_dll(false))?;

    let ntuser_dat_path = base_dir.join("NTUSER.DAT");
    fs::write(&ntuser_dat_path, build_synthetic_ntuser_hive(&signed_dll_path))?;

    let software_hive_path = base_dir.join("SOFTWARE");
    fs::write(&software_hive_path, build_synthetic_software_hive(&unsigned_dll_path))?;

    let appdata_microsoft_dir = base_dir.join("AppDataMicrosoft");
    let word_startup = appdata_microsoft_dir.join("Word").join("STARTUP");
    let outlook_dir = appdata_microsoft_dir.join("Outlook");
    fs::create_dir_all(&word_startup)?;
    fs::create_dir_all(&outlook_dir)?;
    fs::write(word_startup.join("corp_macros.dotm"), b"PK\x03\x04SYNTHETIC_DOTM")?;
    fs::write(outlook_dir.join("VbaProject.OTM"), b"\xD0\xCF\x11\xE0SYNTHETIC_OTM")?;

    Ok(OutlookFixturePaths {
        nk2_path,
        autocomplete_dat_path,
        pst_path,
        msg_path,
        mbox_path,
        eml_path,
        oab_path,
        ntuser_dat_path,
        software_hive_path,
        appdata_microsoft_dir,
        signed_dll_path,
        unsigned_dll_path,
    })
}

pub fn build_synthetic_nk2_bytes(is_stream_autocomplete: bool) -> Vec<u8> {
    let mut e1 = Nk2Entry::new_smtp("Dr. Evelyn Vance", "evelyn.vance@titan.corp", 450);
    // Include an unknown Outlook internal property (0x6000_0003) to verify preservation
    e1.raw_props.push(Nk2RawProperty {
        prop_type: 0x0003,
        prop_id: 0x6000,
        reserved: 0,
        value: Nk2PropValue::Inline(0x0000_0001),
    });

    let ex_dn = "/o=TitanCorp/ou=Exchange Administrative Group/cn=Recipients/cn=mbrody";
    let ex_entry_id = encode_exchange_ab_entry_id(ex_dn);
    let mut e2 = Nk2Entry {
        index: 1,
        display_name: "Marcus Brody".to_string(),
        email_address: ex_dn.to_string(),
        smtp_address: "m.brody@titan.corp".to_string(),
        address_type: "EX".to_string(),
        dropdown_name: "Marcus Brody (EX)".to_string(),
        weight: 275,
        send_rich_info: Some(true),
        modified_time: Some(Utc.with_ymd_and_hms(2026, 10, 14, 9, 15, 0).unwrap()),
        entry_id: ex_entry_id.clone(),
        entry_id_kind: decode_entry_id(&ex_entry_id),
        search_key: format!("EX:{ex_dn}\0").into_bytes(),
        raw_props: Vec::new(),
    };
    e2.sync_raw_props();

    let e3 = Nk2Entry::new_smtp("Sarah Connor", "sconnor@cyber.gov", 120);

    let archive = Nk2Archive {
        meta_major: if is_stream_autocomplete { 0x0C } else { 0x0A },
        meta_minor: 0x01,
        entries: vec![e1, e2, e3],
        footer: vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x01, 0x00, 0x00, 0x00],
    };
    archive.serialize()
}

pub fn build_synthetic_outlook_pst(path: &Path) -> OutlookResult<()> {
    let file = File::create(path)?;
    let mut writer = PstWriter::new(file, true).map_err(|e| crate::OutlookError::Io(std::io::Error::other(e)))?;

    let inbox_nid = writer
        .add_folder("Inbox", None)
        .map_err(|e| crate::OutlookError::Io(std::io::Error::other(e)))?;
    let sent_nid = writer
        .add_folder("Sent Items", None)
        .map_err(|e| crate::OutlookError::Io(std::io::Error::other(e)))?;
    let contacts_nid = writer
        .add_folder("Contacts", None)
        .map_err(|e| crate::OutlookError::Io(std::io::Error::other(e)))?;

    let msg1 = EmailMessage {
        id: "pst_m1".to_string(),
        message_id: "<titan-01@titan.corp>".to_string(),
        conversation_id: "conv_1".to_string(),
        folder_path: vec!["Inbox".to_string()],
        subject: "Q3 Architecture Specification & Budget".to_string(),
        from: EmailAddress::new("Dr. Evelyn Vance", "evelyn.vance@titan.corp"),
        to: vec![EmailAddress::new("Alex Wright", "alex.wright@corp.local")],
        cc: vec![EmailAddress::new("Audit", "audit@titan.corp")],
        bcc: vec![],
        date: Utc.with_ymd_and_hms(2026, 9, 10, 10, 0, 0).unwrap(),
        size_bytes: 24_000,
        body_text: "Please review the attached specification PDF and CSV ledger.".to_string(),
        body_html: None,
        raw_mime: vec![],
        attachments: vec![
            Attachment::new(
                "Architecture_Spec.pdf".to_string(),
                "application/pdf".to_string(),
                b"%PDF-1.4\n%Titan\n%%EOF".to_vec(),
                None,
            ),
            Attachment::new(
                "logo.png".to_string(),
                "image/png".to_string(),
                vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
                Some("cid:logo@titan.corp".to_string()),
            ),
        ],
        mapi_properties: vec![],
        flags: MessageFlags {
            read: true,
            ..Default::default()
        },
        importance: Importance::High,
        sha256: "s1".to_string(),
        md5: "m1".to_string(),
        bates_number: None,
        legal_hold: false,
        source_format: "PST".to_string(),
    };

    let msg2 = EmailMessage {
        id: "pst_m2".to_string(),
        message_id: "<titan-02@corp.local>".to_string(),
        conversation_id: "conv_1".to_string(),
        folder_path: vec!["Sent Items".to_string()],
        subject: "Re: Q3 Architecture Specification & Budget".to_string(),
        from: EmailAddress::new("Alex Wright", "alex.wright@corp.local"),
        to: vec![EmailAddress::new("Dr. Evelyn Vance", "evelyn.vance@titan.corp")],
        cc: vec![],
        bcc: vec![],
        date: Utc.with_ymd_and_hms(2026, 10, 5, 16, 45, 0).unwrap(),
        size_bytes: 12_000,
        body_text: "Approved! Attached is the signed confirmation CSV.".to_string(),
        body_html: None,
        raw_mime: vec![],
        attachments: vec![Attachment::new(
            "approval_metrics.csv".to_string(),
            "text/csv".to_string(),
            b"item,status\nspec,approved\n".to_vec(),
            None,
        )],
        mapi_properties: vec![MapiProperty {
            tag: PR_MESSAGE_FLAGS,
            name: "PR_MESSAGE_FLAGS".to_string(),
            prop_type: "PT_LONG".to_string(),
            value: (1 | MSGFLAG_FROMME).to_string(),
        }],
        flags: MessageFlags {
            read: true,
            ..Default::default()
        },
        importance: Importance::Normal,
        sha256: "s2".to_string(),
        md5: "m2".to_string(),
        bates_number: None,
        legal_hold: false,
        source_format: "PST".to_string(),
    };

    writer
        .add_message(&msg1, inbox_nid)
        .map_err(|e| crate::OutlookError::Io(std::io::Error::other(e)))?;
    writer
        .add_message(&msg2, sent_nid)
        .map_err(|e| crate::OutlookError::Io(std::io::Error::other(e)))?;

    // IPM.Contact
    let mut contact_pc = PropertyContext::new();
    contact_pc.add_string_w(PR_MESSAGE_CLASS_W, "IPM.Contact");
    contact_pc.add_string_w(PR_SUBJECT_W, "Dr. Evelyn Vance");
    contact_pc.add_string_w(PR_DISPLAY_NAME_W, "Dr. Evelyn Vance");
    contact_pc.add_string_w(PR_CONTACT_EMAIL1_W, "evelyn.vance@titan.corp");
    contact_pc.add_string_w(PR_CONTACT_EMAIL2_W, "evance@personal.org");
    contact_pc.add_string_w(PR_COMPANY_NAME_W, "Titan Aerospace");
    contact_pc.add_string_w(PR_TITLE_W, "Chief Systems Architect");
    contact_pc.add_string_w(PR_DEPARTMENT_NAME_W, "Systems Engineering");
    contact_pc.add_string_w(PR_BUSINESS_TELEPHONE_NUMBER_W, "+1-415-555-0199");
    contact_pc.add_string_w(PR_MOBILE_TELEPHONE_NUMBER_W, "+1-415-555-0100");
    contact_pc.add_string_w(PR_BODY_W, "Primary engineering contact");
    contact_pc.add_systime(
        PR_LAST_MODIFICATION_TIME,
        Utc.with_ymd_and_hms(2026, 10, 12, 8, 0, 0).unwrap(),
    );
    writer
        .add_property_node(&contact_pc, contacts_nid)
        .map_err(|e| crate::OutlookError::Io(std::io::Error::other(e)))?;

    // IPM.DistList with DistListOneOffMembers (One-Off EntryIDs)
    let mut dl_pc = PropertyContext::new();
    dl_pc.add_string_w(PR_MESSAGE_CLASS_W, "IPM.DistList");
    dl_pc.add_string_w(PR_SUBJECT_W, "Architecture Review Board");
    dl_pc.add_string_w(PR_DISPLAY_NAME_W, "Architecture Review Board");
    dl_pc.add_mv_binary(
        PR_DISTLIST_ONEOFF_MEMBERS,
        &[
            encode_oneoff_entry_id("Dr. Evelyn Vance", "SMTP", "evelyn.vance@titan.corp"),
            encode_oneoff_entry_id("Marcus Brody", "SMTP", "m.brody@titan.corp"),
        ],
    );
    writer
        .add_property_node(&dl_pc, contacts_nid)
        .map_err(|e| crate::OutlookError::Io(std::io::Error::other(e)))?;

    writer
        .finalize()
        .map_err(|e| crate::OutlookError::Io(std::io::Error::other(e)))?;
    Ok(())
}

pub fn build_synthetic_msg_cfb() -> Vec<u8> {
    let msg = EmailMessage {
        id: "msg_cfb_01".to_string(),
        message_id: "<cfb-msg-01@titan.corp>".to_string(),
        conversation_id: "conv_cfb".to_string(),
        folder_path: vec!["Inbox".to_string()],
        subject: "CFB OLE2 Synthetic MSG with Attachments".to_string(),
        from: EmailAddress::new("Marcus Brody", "m.brody@titan.corp"),
        to: vec![EmailAddress::new("Alex Wright", "alex.wright@corp.local")],
        cc: vec![EmailAddress::new("Legal", "legal@titan.corp")],
        bcc: vec![],
        date: Utc.with_ymd_and_hms(2026, 11, 2, 13, 20, 0).unwrap(),
        size_bytes: 1024,
        body_text: "Hello Alex,\n\nAttached is the CFB contract PDF.\n".to_string(),
        body_html: Some("<p>Hello Alex</p>".to_string()),
        raw_mime: vec![],
        attachments: vec![
            Attachment::new(
                "Contract_Q4.pdf".to_string(),
                "application/pdf".to_string(),
                b"%PDF-1.7\nContract Payload Bytes\n%%EOF".to_vec(),
                None,
            ),
            Attachment::new(
                "badge.png".to_string(),
                "image/png".to_string(),
                vec![0x89, 0x50, 0x4E, 0x47],
                Some("badge@titan.corp".to_string()),
            ),
        ],
        mapi_properties: vec![],
        flags: MessageFlags {
            read: true,
            ..Default::default()
        },
        importance: Importance::High,
        sha256: String::new(),
        md5: String::new(),
        bates_number: None,
        legal_hold: false,
        source_format: "MSG".to_string(),
    };
    build_msg_cfb_bytes(&msg)
}

pub fn build_synthetic_mbox_and_eml(mbox_path: &Path, eml_path: &Path) -> OutlookResult<()> {
    let eml = concat!(
        "From: Dr. Evelyn Vance <evelyn.vance@titan.corp>\r\n",
        "To: Alex Wright <alex.wright@corp.local>\r\n",
        "Subject: Q4 Engineering Report\r\n",
        "Date: Mon, 02 Nov 2026 09:00:00 +0000\r\n",
        "Message-ID: <q4-rep@titan.corp>\r\n",
        "MIME-Version: 1.0\r\n",
        "Content-Type: multipart/mixed; boundary=\"B_TITAN_01\"\r\n\r\n",
        "--B_TITAN_01\r\n",
        "Content-Type: text/plain; charset=\"utf-8\"\r\n\r\n",
        "Report attached.\r\n",
        "--B_TITAN_01\r\n",
        "Content-Type: text/csv; name=\"metrics.csv\"\r\n",
        "Content-Disposition: attachment; filename=\"metrics.csv\"\r\n",
        "Content-Transfer-Encoding: base64\r\n\r\n",
        "a2V5LHZhbHVlCnBhc3MsMTAwCg==\r\n",
        "--B_TITAN_01--\r\n"
    );
    fs::write(eml_path, eml.as_bytes())?;

    let mbox = concat!(
        "From evelyn.vance@titan.corp Tue Oct 06 10:00:00 2026\n",
        "From: Dr. Evelyn Vance <evelyn.vance@titan.corp>\n",
        "To: Alex Wright <alex.wright@corp.local>\n",
        "Subject: October Check-in\n",
        "Date: Tue, 06 Oct 2026 10:00:00 +0000\n",
        "Message-ID: <oct-01@titan.corp>\n\n",
        "Status check.\n",
        "From sconnor@cyber.gov Mon Nov 02 11:00:00 2026\n",
        "From: Sarah Connor <sconnor@cyber.gov>\n",
        "To: Alex Wright <alex.wright@corp.local>\n",
        "Subject: November Security Briefing\n",
        "Date: Mon, 02 Nov 2026 11:00:00 +0000\n",
        "Message-ID: <nov-01@cyber.gov>\n\n",
        "Briefing ready.\n"
    );
    fs::write(mbox_path, mbox.as_bytes())?;
    Ok(())
}

pub fn build_synthetic_oab_v4() -> Vec<u8> {
    build_uncompressed_oab_v4(&[
        OabEntry {
            display_name: "Dr. Evelyn Vance".to_string(),
            email1: "evelyn.vance@titan.corp".to_string(),
            email2: String::new(),
            company: "Titan Aerospace".to_string(),
            job_title: "Chief Systems Architect".to_string(),
            department: "Engineering".to_string(),
            phone: "+1-415-555-0199".to_string(),
        },
        OabEntry {
            display_name: "Sarah Connor".to_string(),
            email1: "sconnor@cyber.gov".to_string(),
            email2: String::new(),
            company: "Cyber Defense Unit".to_string(),
            job_title: "Principal Auditor".to_string(),
            department: "Security".to_string(),
            phone: "+1-202-555-0144".to_string(),
        },
    ])
}

/// Builds a minimal PE32+ DLL image, optionally containing an `IMAGE_DIRECTORY_ENTRY_SECURITY` `WIN_CERTIFICATE`.
pub fn build_synthetic_pe_dll(signed: bool) -> Vec<u8> {
    let mut pe = vec![0u8; 512];
    pe[0..2].copy_from_slice(b"MZ");
    let e_lfanew = 0x80usize;
    pe[0x3C..0x40].copy_from_slice(&(e_lfanew as u32).to_le_bytes());
    pe[e_lfanew..e_lfanew + 4].copy_from_slice(b"PE\0\0");
    // Optional header at e_lfanew + 24
    let opt = e_lfanew + 24;
    pe[opt..opt + 2].copy_from_slice(&0x020Bu16.to_le_bytes()); // PE32+
    pe[opt + 108..opt + 112].copy_from_slice(&16u32.to_le_bytes()); // NumberOfRvaAndSizes
    if signed {
        let cert_off = 512u32;
        let cert_len = 32u32;
        let sec_dir = opt + 112 + 4 * 8;
        pe[sec_dir..sec_dir + 4].copy_from_slice(&cert_off.to_le_bytes());
        pe[sec_dir + 4..sec_dir + 8].copy_from_slice(&cert_len.to_le_bytes());
        // Append WIN_CERTIFICATE (dwLength: u32, wRevision: u16, wCertificateType: u16 = 0x0002)
        pe.extend_from_slice(&cert_len.to_le_bytes());
        pe.extend_from_slice(&0x0200u16.to_le_bytes());
        pe.extend_from_slice(&0x0002u16.to_le_bytes());
        pe.resize(512 + cert_len as usize, 0xAB);
    }
    pe
}

fn reg_key(name: &str) -> RegKeyNode {
    RegKeyNode {
        name: name.to_string(),
        last_written: Some(Utc::now()),
        values: BTreeMap::new(),
        subkeys: BTreeMap::new(),
    }
}

pub fn build_synthetic_ntuser_hive(signed_dll: &Path) -> Vec<u8> {
    let clsid = "{11111111-2222-3333-4444-555555555555}";

    let mut titan_addin = reg_key("Titan.OutlookAddin");
    titan_addin.values.insert(
        "FriendlyName".to_string(),
        RegValue::String("Titan Compliance Add-in".to_string()),
    );
    titan_addin.values.insert(
        "Description".to_string(),
        RegValue::String("Signed Outlook Archiving".to_string()),
    );
    titan_addin
        .values
        .insert("LoadBehavior".to_string(), RegValue::Dword(3));

    let mut addins = reg_key("Addins");
    addins.subkeys.insert("Titan.OutlookAddin".to_string(), titan_addin);

    let mut outlook = reg_key("Outlook");
    outlook.subkeys.insert("Addins".to_string(), addins);

    let mut office = reg_key("Office");
    office.subkeys.insert("Outlook".to_string(), outlook);

    let mut ms = reg_key("Microsoft");
    ms.subkeys.insert("Office".to_string(), office);

    // Software\Classes\Titan.OutlookAddin\CLSID & CLSID\{...}\InprocServer32
    let mut clsid_ref = reg_key("CLSID");
    clsid_ref
        .values
        .insert(String::new(), RegValue::String(clsid.to_string()));
    let mut progid_cls = reg_key("Titan.OutlookAddin");
    progid_cls.subkeys.insert("CLSID".to_string(), clsid_ref);

    let mut inproc = reg_key("InprocServer32");
    inproc
        .values
        .insert(String::new(), RegValue::String(signed_dll.display().to_string()));
    let mut guid_key = reg_key(clsid);
    guid_key.subkeys.insert("InprocServer32".to_string(), inproc);
    let mut clsid_root = reg_key("CLSID");
    clsid_root.subkeys.insert(clsid.to_string(), guid_key);

    let mut classes = reg_key("Classes");
    classes.subkeys.insert("Titan.OutlookAddin".to_string(), progid_cls);
    classes.subkeys.insert("CLSID".to_string(), clsid_root);

    let mut software = reg_key("Software");
    software.subkeys.insert("Microsoft".to_string(), ms);
    software.subkeys.insert("Classes".to_string(), classes);

    let mut root = reg_key("$$$PROTO.HIV");
    root.subkeys.insert("Software".to_string(), software);
    // Unused helper sanity check
    let _ = datetime_to_filetime(Utc::now());
    build_synthetic_regf_hive(&root)
}

pub fn build_synthetic_software_hive(unsigned_dll: &Path) -> Vec<u8> {
    let clsid = "{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}";

    let mut word_addin = reg_key("Contoso.WordWll");
    word_addin.values.insert(
        "FriendlyName".to_string(),
        RegValue::String("Contoso Word Helper".to_string()),
    );
    word_addin.values.insert(
        "Description".to_string(),
        RegValue::String("Unsigned 32-bit Word Add-in".to_string()),
    );
    word_addin.values.insert("LoadBehavior".to_string(), RegValue::Dword(0));

    let mut word_addins = reg_key("Addins");
    word_addins.subkeys.insert("Contoso.WordWll".to_string(), word_addin);
    let mut word = reg_key("Word");
    word.subkeys.insert("Addins".to_string(), word_addins);
    let mut wow_office = reg_key("Office");
    wow_office.subkeys.insert("Word".to_string(), word);
    let mut wow_ms = reg_key("Microsoft");
    wow_ms.subkeys.insert("Office".to_string(), wow_office);

    let mut wow64 = reg_key("WOW6432Node");
    wow64.subkeys.insert("Microsoft".to_string(), wow_ms);

    let mut clsid_ref = reg_key("CLSID");
    clsid_ref
        .values
        .insert(String::new(), RegValue::String(clsid.to_string()));
    let mut progid_cls = reg_key("Contoso.WordWll");
    progid_cls.subkeys.insert("CLSID".to_string(), clsid_ref);

    let mut inproc = reg_key("InprocServer32");
    inproc
        .values
        .insert(String::new(), RegValue::String(unsigned_dll.display().to_string()));
    let mut guid_key = reg_key(clsid);
    guid_key.subkeys.insert("InprocServer32".to_string(), inproc);
    let mut clsid_root = reg_key("CLSID");
    clsid_root.subkeys.insert(clsid.to_string(), guid_key);

    let mut classes = reg_key("Classes");
    classes.subkeys.insert("Contoso.WordWll".to_string(), progid_cls);
    classes.subkeys.insert("CLSID".to_string(), clsid_root);

    let mut root = reg_key("SOFTWARE");
    root.subkeys.insert("WOW6432Node".to_string(), wow64);
    root.subkeys.insert("Classes".to_string(), classes);
    build_synthetic_regf_hive(&root)
}
