use flate2::write::GzEncoder;
use flate2::Compression;
use mail2rust_core::EmailMessage;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;
use tracing::{debug, info};

pub struct MboxWriter<W: Write> {
    writer: W,
    count: usize,
}

impl<W: Write> MboxWriter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer, count: 0 }
    }

    pub fn write_message(&mut self, msg: &EmailMessage) -> io::Result<()> {
        let sender = if msg.from.email.is_empty() {
            "MAILER-DAEMON@mail2rust.local"
        } else {
            &msg.from.email
        };

        let asctime = msg.date.format("%a %b %d %H:%M:%S %Y").to_string();
        writeln!(self.writer, "From {} {}", sender, asctime)?;

        if !msg.raw_mime.is_empty() {
            let s = String::from_utf8_lossy(&msg.raw_mime);
            Self::write_escaped_mime(&mut self.writer, &s)?;
        } else {
            writeln!(self.writer, "From: {} <{}>", msg.from.name, msg.from.email)?;
            for to in &msg.to {
                writeln!(self.writer, "To: {} <{}>", to.name, to.email)?;
            }
            writeln!(self.writer, "Subject: {}", msg.subject)?;
            writeln!(self.writer, "Date: {}", msg.date.to_rfc2822())?;
            writeln!(self.writer, "Message-ID: {}", msg.message_id)?;

            // Status headers
            let mut status = String::new();
            if msg.flags.read {
                status.push('R');
                status.push('O');
            }
            if !status.is_empty() {
                writeln!(self.writer, "Status: {}", status)?;
            }
            if msg.flags.flagged {
                writeln!(self.writer, "X-Status: F")?;
            }

            writeln!(self.writer, "Content-Type: text/plain; charset=utf-8")?;
            writeln!(self.writer)?;
            Self::write_escaped_mime(&mut self.writer, &msg.body_text)?;
        }

        writeln!(self.writer)?;
        self.count += 1;
        debug!(count = self.count, id = %msg.id, "Wrote message to MBOX");
        Ok(())
    }

    fn write_escaped_mime<O: Write>(out: &mut O, text: &str) -> io::Result<()> {
        for line in text.lines() {
            if line.starts_with("From ") || line.starts_with(">From ") {
                write!(out, ">")?;
            }
            writeln!(out, "{}", line)?;
        }
        Ok(())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

pub struct MboxFileExporter;

impl MboxFileExporter {
    pub fn export_to_file(messages: &[EmailMessage], file_path: &Path, compress_gzip: bool) -> io::Result<usize> {
        Self::export_to_file_options(messages, file_path, compress_gzip, false)
    }

    pub fn export_to_file_options(
        messages: &[EmailMessage],
        file_path: &Path,
        compress_gzip: bool,
        append: bool,
    ) -> io::Result<usize> {
        info!(path = ?file_path, compress = compress_gzip, append = append, items = messages.len(), "Writing MBOX file");
        let file = if append && !compress_gzip {
            std::fs::OpenOptions::new().create(true).append(true).open(file_path)?
        } else {
            File::create(file_path)?
        };
        let buf = BufWriter::new(file);

        if compress_gzip {
            let mut gz = GzEncoder::new(buf, Compression::default());
            let mut writer = MboxWriter::new(&mut gz);
            for m in messages {
                writer.write_message(m)?;
            }
            writer.flush()?;
            gz.finish()?;
        } else {
            let mut writer = MboxWriter::new(buf);
            for m in messages {
                writer.write_message(m)?;
            }
            writer.flush()?;
        }

        Ok(messages.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use mail2rust_core::{EmailAddress, EmailMessage, Importance, MessageFlags};
    use std::io::Cursor;

    #[test]
    fn test_mbox_escaping() {
        let mut buffer = Cursor::new(Vec::new());
        let mut writer = MboxWriter::new(&mut buffer);

        let msg = EmailMessage {
            id: "msg_1".to_string(),
            message_id: "<id1@host>".to_string(),
            conversation_id: "conv_1".to_string(),
            folder_path: vec!["Inbox".to_string()],
            subject: "Mbox Escape Test".to_string(),
            from: EmailAddress::new("Tester", "test@domain.com"),
            to: vec![],
            cc: vec![],
            bcc: vec![],
            date: Utc::now(),
            size_bytes: 50,
            body_text: "Line 1\nFrom someone\n>From quote\nLine 4".to_string(),
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

        writer.write_message(&msg).unwrap();
        let output = String::from_utf8(buffer.into_inner()).unwrap();
        assert!(output.contains(">From someone"));
        assert!(output.contains(">>From quote"));
    }
}
