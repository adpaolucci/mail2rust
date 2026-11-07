use chrono::Datelike;
use mail2rust_core::EmailMessage;
use std::collections::{HashMap, HashSet};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EmlNaming {
    #[default]
    Subject,
    Index,
}

pub struct EmlExporter {
    layout: EmlLayout,
    naming: EmlNaming,
    max_per_dir: usize,
    output_dir: PathBuf,
    seen_paths: HashSet<PathBuf>,
    dir_counts: HashMap<PathBuf, usize>,
    global_index: usize,
}

impl EmlExporter {
    pub fn new(output_dir: impl Into<PathBuf>, layout: EmlLayout) -> Self {
        Self {
            output_dir: output_dir.into(),
            layout,
            naming: EmlNaming::Subject,
            max_per_dir: 0,
            seen_paths: HashSet::new(),
            dir_counts: HashMap::new(),
            global_index: 0,
        }
    }

    pub fn with_naming(mut self, naming: EmlNaming) -> Self {
        self.naming = naming;
        self
    }

    pub fn with_max_per_dir(mut self, max: usize) -> Self {
        self.max_per_dir = max;
        self
    }

    pub fn export_message(&mut self, msg: &EmailMessage) -> io::Result<PathBuf> {
        self.global_index += 1;

        let base_rel_dir = match self.layout {
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

        // Determine directory part if max_per_dir is configured
        let target_dir = if let Some(max) = std::num::NonZeroUsize::new(self.max_per_dir) {
            let count = self.dir_counts.entry(base_rel_dir.clone()).or_insert(0);
            let part_num = *count / max;
            *count += 1;
            if part_num > 0 {
                self.output_dir
                    .join(&base_rel_dir)
                    .join(format!("__part{}", part_num + 1))
            } else {
                self.output_dir.join(&base_rel_dir)
            }
        } else {
            self.output_dir.join(base_rel_dir)
        };

        fs::create_dir_all(&target_dir)?;

        let base_name = match self.naming {
            EmlNaming::Index => format!("{:06}.eml", self.global_index),
            EmlNaming::Subject => Self::generate_filename(msg),
        };

        let mut final_path = target_dir.join(&base_name);
        let mut counter = 1;

        while self.seen_paths.contains(&final_path) || final_path.exists() {
            let stem = Path::new(&base_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("email");
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
        let mut clean: String = name
            .chars()
            .map(|c| if invalid.contains(&c) { '_' } else { c })
            .collect();
        clean = clean.trim().to_string();
        if clean.is_empty() {
            clean = "Folder".to_string();
        }
        clean
    }

    fn generate_filename(msg: &EmailMessage) -> String {
        let date_prefix = msg.date.format("%Y%m%d_%H%M%S").to_string();
        let safe_subj = Self::sanitize_segment(&msg.subject);
        let truncated = if safe_subj.len() > 60 {
            &safe_subj[..60]
        } else {
            &safe_subj
        };
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

    fn make_test_msg(subject: &str, folder: &[&str]) -> EmailMessage {
        EmailMessage {
            id: "msg_1".to_string(),
            message_id: "<test@example.com>".to_string(),
            conversation_id: "conv_1".to_string(),
            folder_path: folder.iter().map(|s| s.to_string()).collect(),
            subject: subject.to_string(),
            from: EmailAddress::new("Alice", "alice@test.com"),
            to: vec![EmailAddress::new("Bob", "bob@test.com")],
            cc: vec![],
            bcc: vec![],
            date: Utc::now(),
            size_bytes: 50,
            body_text: "Test body text".to_string(),
            body_html: None,
            raw_mime: b"From: Alice <alice@test.com>\r\nSubject: Test\r\n\r\nTest body".to_vec(),
            attachments: vec![],
            mapi_properties: vec![],
            flags: MessageFlags::default(),
            importance: Importance::Normal,
            sha256: "sha_dummy".to_string(),
            md5: "md5_dummy".to_string(),
            bates_number: None,
            legal_hold: false,
            source_format: "test".to_string(),
        }
    }

    #[test]
    fn test_eml_export_naming_index() {
        let dir = tempdir().unwrap();
        let mut exporter = EmlExporter::new(dir.path(), EmlLayout::Flat).with_naming(EmlNaming::Index);

        let msg = make_test_msg("Hello World", &["Inbox"]);
        let path = exporter.export_message(&msg).unwrap();
        assert!(path.file_name().unwrap().to_str().unwrap().contains("000001.eml"));
    }

    #[test]
    fn test_eml_export_max_per_dir() {
        let dir = tempdir().unwrap();
        let mut exporter = EmlExporter::new(dir.path(), EmlLayout::Flat).with_max_per_dir(2);

        let m1 = make_test_msg("M1", &["Inbox"]);
        let m2 = make_test_msg("M2", &["Inbox"]);
        let m3 = make_test_msg("M3", &["Inbox"]);

        let p1 = exporter.export_message(&m1).unwrap();
        let p2 = exporter.export_message(&m2).unwrap();
        let p3 = exporter.export_message(&m3).unwrap();

        assert_eq!(p1.parent(), p2.parent());
        assert!(p3.to_str().unwrap().contains("__part2"));
    }
}
