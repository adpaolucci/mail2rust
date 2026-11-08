use chrono::Datelike;
use mail2rust_core::EmailMessage;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tracing::debug;

pub mod deliverables;
pub use deliverables::{HtmlIndexGenerator, HtmlIndexRow, ManifestRecord, ManifestWriter};

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
    html_index_title: Option<String>,
    html_rows: Vec<HtmlIndexRow>,
    manifest_writer: Option<ManifestWriter<File>>,
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
            html_index_title: None,
            html_rows: Vec::new(),
            manifest_writer: None,
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

    pub fn with_html_index(mut self, title: impl Into<String>) -> Self {
        self.html_index_title = Some(title.into());
        self
    }

    pub fn with_manifest_csv(mut self, path: impl AsRef<Path>) -> io::Result<Self> {
        let writer = ManifestWriter::create_file(path.as_ref())?;
        self.manifest_writer = Some(writer);
        Ok(self)
    }

    pub fn export_message(&mut self, msg: &EmailMessage) -> io::Result<PathBuf> {
        self.export_message_annotated(msg, "", "")
    }

    pub fn export_message_annotated(
        &mut self,
        msg: &EmailMessage,
        matched_keywords: &str,
        source_container: &str,
    ) -> io::Result<PathBuf> {
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

        let rel_path = final_path.strip_prefix(&self.output_dir).unwrap_or(&final_path);
        let rel_str = rel_path.to_string_lossy().replace('\\', "/");

        if self.html_index_title.is_some() {
            self.html_rows.push(HtmlIndexRow {
                filename: rel_str.clone(),
                subject: mail2rust_core::decode_rfc2047(&msg.subject),
                from: msg.from.to_string(),
                date: msg.date.format("%Y-%m-%d %H:%M:%S").to_string(),
                size_bytes: msg.size_bytes.max(msg.raw_mime.len()),
                matched_keywords: matched_keywords.to_string(),
            });
        }

        if let Some(ref mut writer) = self.manifest_writer {
            let to_str = msg.to.iter().map(|a| a.to_string()).collect::<Vec<_>>().join("; ");
            writer.write_record(&ManifestRecord {
                message_id: msg.message_id.clone(),
                date: msg.date.to_rfc3339(),
                from: msg.from.to_string(),
                to: to_str,
                subject: mail2rust_core::decode_rfc2047(&msg.subject),
                size_bytes: msg.size_bytes.max(msg.raw_mime.len()),
                matched_keywords: matched_keywords.to_string(),
                source_container: source_container.to_string(),
                sha256_hash: msg.sha256.clone(),
                eml_filename: rel_str,
            })?;
        }

        debug!(path = ?final_path, "Exported message to EML");
        Ok(final_path)
    }

    pub fn finalize(&mut self) -> io::Result<()> {
        if let Some(ref title) = self.html_index_title {
            let idx_path = self.output_dir.join("index.html");
            HtmlIndexGenerator::write_index_file(&idx_path, title, &self.html_rows)?;
        }
        if let Some(ref mut writer) = self.manifest_writer {
            writer.flush()?;
        }
        Ok(())
    }

    pub fn sanitize_segment(name: &str) -> String {
        let invalid = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
        let mut clean: String = name
            .chars()
            .map(|c| if invalid.contains(&c) || c.is_control() { '_' } else { c })
            .collect();
        clean = clean.trim().trim_matches('.').to_string();
        if clean.is_empty() || clean == "." || clean == ".." {
            clean = "Folder".to_string();
        }
        // Protect against Windows DOS reserved device names
        let upper = clean.to_ascii_uppercase();
        let stem = upper.split('.').next().unwrap_or("");
        if matches!(
            stem,
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        ) {
            clean = format!("_{}", clean);
        }
        clean
    }

    fn generate_filename(msg: &EmailMessage) -> String {
        let date_prefix = msg.date.format("%Y%m%d_%H%M%S").to_string();
        let decoded_subj = mail2rust_core::decode_rfc2047(&msg.subject);
        let safe_subj = Self::sanitize_segment(&decoded_subj);
        let truncated: String = safe_subj.chars().take(50).collect();
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

    #[test]
    fn test_eml_sanitize_segment_and_device_names() {
        assert_eq!(EmlExporter::sanitize_segment(".."), "Folder");
        assert_eq!(EmlExporter::sanitize_segment("."), "Folder");
        assert_eq!(EmlExporter::sanitize_segment(""), "Folder");
        assert_eq!(EmlExporter::sanitize_segment("..."), "Folder");
        assert_eq!(EmlExporter::sanitize_segment("CON"), "_CON");
        assert_eq!(EmlExporter::sanitize_segment("prn.txt"), "_prn.txt");
        assert_eq!(EmlExporter::sanitize_segment("NUL"), "_NUL");
        assert_eq!(EmlExporter::sanitize_segment("Normal Folder"), "Normal Folder");
        assert_eq!(
            EmlExporter::sanitize_segment("Folder/With\\Slashes:And*Chars?"),
            "Folder_With_Slashes_And_Chars_"
        );
    }

    #[test]
    fn test_eml_generate_filename_multibyte_utf8() {
        let dir = tempdir().unwrap();
        let mut exporter = EmlExporter::new(dir.path(), EmlLayout::Flat).with_naming(EmlNaming::Subject);
        // Multibyte string that would have previously panicked if sliced at byte 60:
        let msg = make_test_msg(
            "こんにちは世界！これは非常に長い件名のメールです。🦀🚀✨マルチバイトテスト",
            &["Inbox"],
        );
        let path = exporter.export_message(&msg).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_eml_export_with_html_index_and_manifest() {
        let dir = tempdir().unwrap();
        let manifest_path = dir.path().join("test_manifest.csv");
        let mut exporter = EmlExporter::new(dir.path(), EmlLayout::Flat)
            .with_html_index("Discovery Production Review")
            .with_manifest_csv(&manifest_path)
            .unwrap();

        let msg1 = make_test_msg("Critical Exhibit A", &["Inbox"]);
        let msg2 = make_test_msg("=?UTF-8?Q?RE:_100_Corporate_Blvd?=", &["Archive"]);

        exporter
            .export_message_annotated(&msg1, "EXHIBIT_A", "inbox.mbox")
            .unwrap();
        exporter
            .export_message_annotated(&msg2, "CORPORATE", "archive.mbox")
            .unwrap();
        exporter.finalize().unwrap();

        let index_html_path = dir.path().join("index.html");
        assert!(index_html_path.exists());
        let html_content = fs::read_to_string(&index_html_path).unwrap();
        assert!(html_content.contains("Discovery Production Review"));
        assert!(html_content.contains("Critical Exhibit A"));
        assert!(html_content.contains("RE: 100 Corporate Blvd"));
        assert!(html_content.contains("EXHIBIT_A"));
        assert!(html_content.contains("CORPORATE"));

        assert!(manifest_path.exists());
        let manifest_content = fs::read_to_string(&manifest_path).unwrap();
        assert!(manifest_content.contains("Critical Exhibit A"));
        assert!(manifest_content.contains("RE: 100 Corporate Blvd"));
        assert!(manifest_content.contains("EXHIBIT_A"));
        assert!(manifest_content.contains("CORPORATE"));
    }
}
