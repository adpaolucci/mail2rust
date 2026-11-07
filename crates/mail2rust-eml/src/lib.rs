use chrono::Datelike;
use mail2rust_core::EmailMessage;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tracing::debug;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EmlLayout {
    #[default]
    Folder,
    Year,
    Month,
    Flat,
}

pub struct EmlExporter {
    layout: EmlLayout,
    output_dir: PathBuf,
    seen_paths: HashSet<PathBuf>,
}

impl EmlExporter {
    pub fn new(output_dir: impl Into<PathBuf>, layout: EmlLayout) -> Self {
        Self {
            output_dir: output_dir.into(),
            layout,
            seen_paths: HashSet::new(),
        }
    }

    pub fn export_message(&mut self, msg: &EmailMessage) -> io::Result<PathBuf> {
        let rel_dir = match self.layout {
            EmlLayout::Folder => {
                let mut dir = PathBuf::new();
                for seg in &msg.folder_path {
                    dir.push(Self::sanitize_segment(seg));
                }
                dir
            }
            EmlLayout::Year => PathBuf::from(format!("{}", msg.date.year())),
            EmlLayout::Month => PathBuf::from(format!("{}/{:02}", msg.date.year(), msg.date.month())),
            EmlLayout::Flat => PathBuf::new(),
        };

        let target_dir = self.output_dir.join(rel_dir);
        fs::create_dir_all(&target_dir)?;

        let base_name = Self::generate_filename(msg);
        let mut final_path = target_dir.join(&base_name);
        let mut counter = 1;

        while self.seen_paths.contains(&final_path) || final_path.exists() {
            let stem = Path::new(&base_name).file_stem().and_then(|s| s.to_str()).unwrap_or("email");
            let new_name = format!("{} ({}).eml", stem, counter);
            final_path = target_dir.join(new_name);
            counter += 1;
        }

        self.seen_paths.insert(final_path.clone());

        let mut file = File::create(&final_path)?;
        if !msg.raw_mime.is_empty() {
            file.write_all(&msg.raw_mime)?;
        } else {
            Self::write_synthetic_mime(&mut file, msg)?;
        }

        debug!(path = ?final_path, "Exported message to EML");
        Ok(final_path)
    }

    pub fn sanitize_segment(name: &str) -> String {
        let invalid = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
        let mut clean: String = name.chars().map(|c| if invalid.contains(&c) { '_' } else { c }).collect();
        clean = clean.trim().to_string();
        if clean.is_empty() {
            clean = "Folder".to_string();
        }
        clean
    }

    fn generate_filename(msg: &EmailMessage) -> String {
        let date_prefix = msg.date.format("%Y%m%d_%H%M%S").to_string();
        let safe_subj = Self::sanitize_segment(&msg.subject);
        let truncated = if safe_subj.len() > 60 { &safe_subj[..60] } else { &safe_subj };
        format!("{}_{}.eml", date_prefix, truncated)
    }

    fn write_synthetic_mime<W: Write>(writer: &mut W, msg: &EmailMessage) -> io::Result<()> {
        writeln!(writer, "From: {} <{}>", msg.from.name, msg.from.email)?;
        for to in &msg.to {
            writeln!(writer, "To: {} <{}>", to.name, to.email)?;
        }
        writeln!(writer, "Subject: {}", msg.subject)?;
        writeln!(writer, "Date: {}", msg.date.to_rfc2822())?;
        writeln!(writer, "Message-ID: {}", msg.message_id)?;
        writeln!(writer, "MIME-Version: 1.0")?;
        writeln!(writer, "Content-Type: text/plain; charset=utf-8")?;
        writeln!(writer)?;
        write!(writer, "{}", msg.body_text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use mail2rust_core::{EmailAddress, EmailMessage, Importance, MessageFlags};
    use tempfile::tempdir;

    #[test]
    fn test_eml_export_folder_layout() {
        let temp_dir = tempdir().unwrap();
        let mut exporter = EmlExporter::new(temp_dir.path(), EmlLayout::Folder);

        let msg = EmailMessage {
            id: "msg_1".to_string(),
            message_id: "<id1@host>".to_string(),
            conversation_id: "conv_1".to_string(),
            folder_path: vec!["Archive".to_string(), "2026".to_string()],
            subject: "Quarterly Audit".to_string(),
            from: EmailAddress::new("Alice", "alice@example.com"),
            to: vec![EmailAddress::new("Bob", "bob@example.com")],
            cc: Vec::new(),
            bcc: Vec::new(),
            date: Utc::now(),
            size_bytes: 50,
            body_text: "Audit notes here.".to_string(),
            body_html: None,
            raw_mime: Vec::new(),
            attachments: Vec::new(),
            mapi_properties: Vec::new(),
            flags: MessageFlags::default(),
            importance: Importance::Normal,
            sha256: "sha".to_string(),
            md5: "md5".to_string(),
            bates_number: None,
            legal_hold: false,
            source_format: "test".to_string(),
        };

        let exported = exporter.export_message(&msg).unwrap();
        assert!(exported.exists());
        assert!(exported.to_str().unwrap().contains("Archive"));
        assert!(exported.to_str().unwrap().contains("2026"));
    }
}
