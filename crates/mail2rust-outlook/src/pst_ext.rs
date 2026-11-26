use crate::cfb_ole2::MsgDocument;
use crate::error::{OutlookError, OutlookResult};
use crate::nk2::{EntryIdKind, decode_entry_id};
use chrono::{DateTime, Utc};
use mail2rust_core::{EmailMessage, FormatSniffer, MailboxFormat, MapiProperty};
use mail2rust_pst::{
    NID_MESSAGE_STORE, NID_NAME_TO_ID_MAP, NID_ROOT_FOLDER, PR_BODY_W, PR_BUSINESS_TELEPHONE_NUMBER_W,
    PR_COMPANY_NAME_W, PR_CONTACT_EMAIL1_W, PR_CONTACT_EMAIL2_W, PR_CONTACT_EMAIL3_W, PR_DEPARTMENT_NAME_W,
    PR_DISPLAY_NAME_W, PR_DISTLIST_ONEOFF_MEMBERS, PR_EMAIL_ADDRESS_W, PR_HOME_TELEPHONE_NUMBER_W,
    PR_LAST_MODIFICATION_TIME, PR_MESSAGE_CLASS_W, PR_MOBILE_TELEPHONE_NUMBER_W, PR_OFFICE_LOCATION_W,
    PR_SMTP_ADDRESS_W, PR_SUBJECT_W, PR_TITLE_W, ParsedPropertyContext, PropertyContext, PstReader, PstValidator,
    PstWriter, ltp::PropertyContextEntry,
};
use std::collections::HashSet;
use std::fs::{self, File};
use std::path::Path;
use walkdir::WalkDir;

// PSETID_Address GUID in little-endian MS-OXPROPS byte order: {00062004-0000-0000-C000-000000000046}
pub const PSETID_ADDRESS_GUID: [u8; 16] = [
    0x04, 0x20, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
];

#[derive(Debug, Clone)]
pub struct PstContactRaw {
    pub message_class: String,
    pub display_name: String,
    pub email1: String,
    pub email2: String,
    pub email3: String,
    pub company: String,
    pub job_title: String,
    pub department: String,
    pub phone: String,
    pub mobile_phone: String,
    pub office_location: String,
    pub notes: String,
    pub modified_time: Option<DateTime<Utc>>,
    pub members: Vec<(String, String)>,
}

/// Loads `EmailMessage` items from any supported file or directory (`.pst`, `.ost`, `.msg`, `.mbox`, `.eml`, Maildir, etc.).
pub fn load_messages_from_source(source: &Path) -> OutlookResult<Vec<EmailMessage>> {
    if !source.exists() {
        return Err(OutlookError::NotFound(format!(
            "Source path does not exist: {}",
            source.display()
        )));
    }
    if source.is_file() {
        return load_messages_from_file(source);
    }

    // Directory walk: check if Maildir or MH first via FormatSniffer
    let dir_fmt = FormatSniffer::sniff_path(source);
    if dir_fmt == MailboxFormat::Maildir || dir_fmt == MailboxFormat::Mh {
        return mail2rust_core::ingest_path(source).map_err(|e| OutlookError::ParseError(e.to_string()));
    }

    let mut all = Vec::new();
    for entry in WalkDir::new(source).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(
            ext.as_str(),
            "pst" | "ost" | "msg" | "eml" | "emlx" | "mbox" | "mbx" | "olm"
        ) && let Ok(mut msgs) = load_messages_from_file(p)
        {
            all.append(&mut msgs);
        }
    }
    Ok(all)
}

fn load_messages_from_file(path: &Path) -> OutlookResult<Vec<EmailMessage>> {
    let fmt = FormatSniffer::sniff_path(path);
    match fmt {
        MailboxFormat::Pst | MailboxFormat::Ost => {
            PstReader::read_path(path).map_err(|e| OutlookError::ParseError(format!("{}: {e}", path.display())))
        }
        MailboxFormat::Msg => {
            let bytes = fs::read(path)?;
            if let Ok(doc) = MsgDocument::parse_bytes(&bytes) {
                return Ok(vec![doc.to_email_message(vec!["MSG".to_string()])]);
            }
            mail2rust_core::ingest_path(path).map_err(|e| OutlookError::ParseError(format!("{}: {e}", path.display())))
        }
        _ => {
            mail2rust_core::ingest_path(path).map_err(|e| OutlookError::ParseError(format!("{}: {e}", path.display())))
        }
    }
}

/// Extracts `IPM.Contact` and `IPM.DistList` items from a PST/OST file, resolving `NID_NAME_TO_ID_MAP`.
pub fn extract_pst_contacts(path: &Path) -> OutlookResult<Vec<PstContactRaw>> {
    let file = File::open(path)?;
    let mut reader = PstReader::open(file).map_err(|e| OutlookError::ParseError(e.to_string()))?;
    let name_map = reader.read_name_to_id_map();
    let nodes = reader.nbt_entries().to_vec();

    let email1_id = name_map.resolve_lid(&PSETID_ADDRESS_GUID, 0x8083).unwrap_or(0x8083);
    let email2_id = name_map.resolve_lid(&PSETID_ADDRESS_GUID, 0x8093).unwrap_or(0x8093);
    let email3_id = name_map.resolve_lid(&PSETID_ADDRESS_GUID, 0x80A3).unwrap_or(0x80A3);
    let dl_oneoff_id = name_map.resolve_lid(&PSETID_ADDRESS_GUID, 0x8054).unwrap_or(0x8054);

    let mut contacts = Vec::new();
    for node in nodes {
        let Some(pc) = reader.read_property_context(node.bid_data, node.bid_sub) else {
            continue;
        };
        let msg_class = pc.get_string_any((PR_MESSAGE_CLASS_W >> 16) as u16).unwrap_or_default();
        let upper = msg_class.to_ascii_uppercase();
        if !upper.starts_with("IPM.CONTACT") && !upper.starts_with("IPM.DISTLIST") {
            continue;
        }
        contacts.push(decode_contact_pc(
            &pc,
            &msg_class,
            email1_id,
            email2_id,
            email3_id,
            dl_oneoff_id,
        ));
    }
    Ok(contacts)
}

fn decode_contact_pc(
    pc: &ParsedPropertyContext,
    msg_class: &str,
    email1_id: u16,
    email2_id: u16,
    email3_id: u16,
    dl_oneoff_id: u16,
) -> PstContactRaw {
    let display_name = pc
        .get_string_any((PR_DISPLAY_NAME_W >> 16) as u16)
        .or_else(|| pc.get_string_any((PR_SUBJECT_W >> 16) as u16))
        .unwrap_or_default();

    let email1 = pc
        .get_string_any(email1_id)
        .or_else(|| pc.get_string_any((PR_CONTACT_EMAIL1_W >> 16) as u16))
        .or_else(|| pc.get_string_any((PR_SMTP_ADDRESS_W >> 16) as u16))
        .or_else(|| pc.get_string_any((PR_EMAIL_ADDRESS_W >> 16) as u16))
        .unwrap_or_default();
    let email2 = pc
        .get_string_any(email2_id)
        .or_else(|| pc.get_string_any((PR_CONTACT_EMAIL2_W >> 16) as u16))
        .unwrap_or_default();
    let email3 = pc
        .get_string_any(email3_id)
        .or_else(|| pc.get_string_any((PR_CONTACT_EMAIL3_W >> 16) as u16))
        .unwrap_or_default();

    let company = pc.get_string_any((PR_COMPANY_NAME_W >> 16) as u16).unwrap_or_default();
    let job_title = pc.get_string_any((PR_TITLE_W >> 16) as u16).unwrap_or_default();
    let department = pc
        .get_string_any((PR_DEPARTMENT_NAME_W >> 16) as u16)
        .unwrap_or_default();
    let phone = pc
        .get_string_any((PR_BUSINESS_TELEPHONE_NUMBER_W >> 16) as u16)
        .or_else(|| pc.get_string_any((PR_HOME_TELEPHONE_NUMBER_W >> 16) as u16))
        .unwrap_or_default();
    let mobile_phone = pc
        .get_string_any((PR_MOBILE_TELEPHONE_NUMBER_W >> 16) as u16)
        .unwrap_or_default();
    let office_location = pc
        .get_string_any((PR_OFFICE_LOCATION_W >> 16) as u16)
        .unwrap_or_default();
    let notes = pc.get_string_any((PR_BODY_W >> 16) as u16).unwrap_or_default();
    let modified_time = pc.get_systime(PR_LAST_MODIFICATION_TIME);

    let dl_tag = (u32::from(dl_oneoff_id) << 16) | 0x1102;
    let mut members = Vec::new();
    if let Some(blobs) = pc
        .get_mv_binary(dl_tag)
        .or_else(|| pc.get_mv_binary(PR_DISTLIST_ONEOFF_MEMBERS))
    {
        for blob in blobs {
            match decode_entry_id(&blob) {
                EntryIdKind::OneOff {
                    display_name, email, ..
                } => members.push((display_name, email)),
                EntryIdKind::ExchangeAb { distinguished_name } => {
                    members.push((distinguished_name.clone(), distinguished_name));
                }
                EntryIdKind::Unknown => {}
            }
        }
    }

    PstContactRaw {
        message_class: msg_class.to_string(),
        display_name,
        email1,
        email2,
        email3,
        company,
        job_title,
        department,
        phone,
        mobile_phone,
        office_location,
        notes,
        modified_time,
        members,
    }
}

/// Returns `true` when `msg_class` represents a mail item rather than a Contact, DistList, Appointment, Task, etc.
pub fn is_mail_message_class(msg_class: &str) -> bool {
    let upper = msg_class.trim().to_ascii_uppercase();
    if upper.is_empty() {
        return true;
    }
    !(upper.starts_with("IPM.CONTACT")
        || upper.starts_with("IPM.DISTLIST")
        || upper.starts_with("IPM.APPOINTMENT")
        || upper.starts_with("IPM.TASK")
        || upper.starts_with("IPM.STICKYNOTE")
        || upper.starts_with("IPM.ACTIVITY")
        || upper.starts_with("IPM.SCHEDULE.")
        || upper.starts_with("IPM.CONFIGURATION")
        || upper.starts_with("IPC."))
}

/// Returns `true` if `msg` is an email item (not a non-mail PST node such as `IPM.Contact` or `IPM.DistList`).
pub fn is_mail_message(msg: &EmailMessage) -> bool {
    let Some(mc) = msg
        .mapi_properties
        .iter()
        .find(|p| p.tag == PR_MESSAGE_CLASS_W || (p.tag >> 16) == (PR_MESSAGE_CLASS_W >> 16))
        .map(|p| p.value.as_str())
    else {
        return true;
    };
    is_mail_message_class(mc)
}

fn parsed_pc_to_property_context(parsed: &ParsedPropertyContext) -> PropertyContext {
    let mut pc = PropertyContext::new();
    let mut tags: Vec<u32> = parsed.properties.keys().copied().collect();
    tags.sort_unstable();
    for tag in tags {
        let mut data = parsed.properties[&tag].clone();
        let prop_type = (tag & 0xFFFF) as u16;
        match prop_type {
            0x001F => {
                if !data.len().is_multiple_of(2) {
                    data.push(0);
                }
                if !data.ends_with(&[0, 0]) {
                    data.extend_from_slice(&[0, 0]);
                }
            }
            0x001E => {
                if !data.ends_with(&[0]) {
                    data.push(0);
                }
            }
            0x0002 | 0x0003 | 0x000B if data.len() < 4 => {
                data.resize(4, 0);
            }
            _ => {}
        }
        pc.entries.push(PropertyContextEntry { tag, data });
    }
    pc
}

/// Safely rewrites a PST container without attachments matching `strip_ids` (or all attachments if `strip_ids` is empty).
pub fn strip_pst_to_file(
    src_path: &Path,
    dest_path: &Path,
    strip_filenames_lower: &HashSet<String>,
) -> OutlookResult<usize> {
    let file = File::open(src_path)?;
    let mut reader = PstReader::open(file).map_err(|e| OutlookError::ParseError(e.to_string()))?;
    if reader.is_password_protected() {
        return Err(OutlookError::UnsupportedFeature(
            "Cannot rewrite password-protected PST without decrypting credentials".to_string(),
        ));
    }
    let encryption = reader.get_encryption();
    let folder_paths = reader.read_folder_paths();
    let nbt_nodes = reader.nbt_entries().to_vec();
    let mut preserved_non_mail: Vec<(Vec<String>, PropertyContext)> = Vec::new();
    for node in &nbt_nodes {
        if node.nid == NID_MESSAGE_STORE || node.nid == NID_NAME_TO_ID_MAP || node.nid == NID_ROOT_FOLDER {
            continue;
        }
        if let Some(pc) = reader.read_property_context(node.bid_data, node.bid_sub)
            && let Some(mc) = pc.get_string_any((PR_MESSAGE_CLASS_W >> 16) as u16)
            && !is_mail_message_class(&mc)
        {
            let folder = folder_paths
                .get(&node.nid_parent)
                .cloned()
                .unwrap_or_else(|| vec!["Contacts".to_string()]);
            preserved_non_mail.push((folder, parsed_pc_to_property_context(&pc)));
        }
    }

    let mut messages: Vec<EmailMessage> = reader
        .read_all_messages()
        .map_err(|e| OutlookError::ParseError(e.to_string()))?
        .into_iter()
        .filter(is_mail_message)
        .collect();

    let mut stripped_count = 0usize;
    for msg in &mut messages {
        let mut kept = Vec::with_capacity(msg.attachments.len());
        for att in std::mem::take(&mut msg.attachments) {
            if strip_filenames_lower.is_empty() || strip_filenames_lower.contains(&att.filename.to_ascii_lowercase()) {
                msg.body_text.push_str(&format!(
                    "\n[Attachment stripped by mail2rust: {} sha256={}]",
                    att.filename, att.sha256
                ));
                stripped_count += 1;
            } else {
                kept.push(att);
            }
        }
        msg.attachments = kept;
    }

    if let Some(parent) = dest_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let tmp_path = dest_path.with_extension("pst.tmp");
    {
        let out_file = File::create(&tmp_path)?;
        let mut writer = PstWriter::with_encryption(out_file, encryption, None)
            .map_err(|e| OutlookError::Io(std::io::Error::other(e)))?;
        for msg in &messages {
            let folder_nid = writer
                .ensure_folder_path(&msg.folder_path, None)
                .map_err(|e| OutlookError::Io(std::io::Error::other(e)))?;
            writer
                .add_message(msg, folder_nid)
                .map_err(|e| OutlookError::Io(std::io::Error::other(e)))?;
        }
        for (folder_path, pc) in &preserved_non_mail {
            let folder_nid = writer
                .ensure_folder_path(folder_path, None)
                .map_err(|e| OutlookError::Io(std::io::Error::other(e)))?;
            writer
                .add_property_node(pc, folder_nid)
                .map_err(|e| OutlookError::Io(std::io::Error::other(e)))?;
        }
        writer
            .finalize()
            .map_err(|e| OutlookError::Io(std::io::Error::other(e)))?;
    }

    // Validate rewritten PST before rename
    {
        let vf = File::open(&tmp_path)?;
        let report = PstValidator::validate(vf).map_err(|e| OutlookError::CorruptStructure(e.to_string()))?;
        if !report.is_valid {
            let _ = fs::remove_file(&tmp_path);
            return Err(OutlookError::CorruptStructure(format!(
                "Stripped PST validation failed: {:?}",
                report.errors
            )));
        }
    }

    if src_path == dest_path && dest_path.exists() {
        let bak_path = dest_path.with_extension("pst.bak");
        fs::copy(dest_path, bak_path)?;
    }
    if dest_path.exists() {
        fs::remove_file(dest_path)?;
    }
    fs::rename(&tmp_path, dest_path)?;

    Ok(stripped_count)
}

pub fn mapi_prop(tag: u32, name: &str, prop_type: &str, value: impl Into<String>) -> MapiProperty {
    MapiProperty {
        tag,
        name: name.to_string(),
        prop_type: prop_type.to_string(),
        value: value.into(),
    }
}
