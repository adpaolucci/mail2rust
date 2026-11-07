use crate::models::{EmailAddress, EmailMessage, Importance, MapiProperty, MessageFlags};
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use std::io;
use tracing::debug;

pub struct IcalReader;

impl IcalReader {
    pub fn parse(bytes: &[u8], folder_path: Vec<String>) -> io::Result<Vec<EmailMessage>> {
        let content = String::from_utf8_lossy(bytes);
        let mut messages = Vec::new();
        let mut in_event = false;
        let mut current_lines = Vec::new();

        // Handle line unfolding (lines beginning with space or tab continue previous line)
        let mut unfolded: Vec<String> = Vec::new();
        for line in content.lines() {
            if line.starts_with(' ') || line.starts_with('\t') {
                if let Some(last) = unfolded.last_mut() {
                    last.push_str(line.trim_start());
                } else {
                    unfolded.push(line.trim_start().to_string());
                }
            } else {
                unfolded.push(line.to_string());
            }
        }

        for line in unfolded {
            let trimmed = line.trim();
            if trimmed.eq_ignore_ascii_case("BEGIN:VEVENT") {
                in_event = true;
                current_lines.clear();
            } else if trimmed.eq_ignore_ascii_case("END:VEVENT") {
                if in_event {
                    if let Some(msg) = Self::build_event(&current_lines, &folder_path) {
                        messages.push(msg);
                    }
                    in_event = false;
                }
            } else if in_event {
                current_lines.push(trimmed.to_string());
            }
        }

        debug!(count = messages.len(), "Parsed iCalendar events");
        Ok(messages)
    }

    fn build_event(lines: &[String], folder_path: &[String]) -> Option<EmailMessage> {
        let mut summary = String::new();
        let mut description = String::new();
        let mut location = String::new();
        let mut dtstart = None;
        let mut dtend = None;
        let mut organizer = String::new();
        let mut attendees = Vec::new();
        let mut uid = String::new();

        for line in lines {
            let (key, val) = match line.find(':') {
                Some(idx) => (line[..idx].to_uppercase(), line[idx + 1..].trim()),
                None => continue,
            };

            let base_key = key.split(';').next().unwrap_or(&key);

            match base_key {
                "SUMMARY" => summary = val.to_string(),
                "DESCRIPTION" => description = val.replace("\\n", "\n").replace("\\,", ",").to_string(),
                "LOCATION" => location = val.replace("\\,", ",").to_string(),
                "DTSTART" => dtstart = Self::parse_ical_date(val),
                "DTEND" => dtend = Self::parse_ical_date(val),
                "ORGANIZER" => {
                    organizer = val
                        .trim_start_matches("mailto:")
                        .trim_start_matches("MAILTO:")
                        .to_string();
                }
                "ATTENDEE" => {
                    attendees.push(
                        val.trim_start_matches("mailto:")
                            .trim_start_matches("MAILTO:")
                            .to_string(),
                    );
                }
                "UID" => uid = val.to_string(),
                _ => {}
            }
        }

        if summary.is_empty() && description.is_empty() {
            return None;
        }

        let event_title = if !summary.is_empty() {
            summary
        } else {
            "Calendar Appointment".to_string()
        };

        let start_time = dtstart.unwrap_or_else(Utc::now);
        let end_str = dtend
            .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| "N/A".to_string());

        let mut body = String::new();
        body.push_str(&format!("Event: {}\n", event_title));
        body.push_str(&format!("Start: {}\n", start_time.format("%Y-%m-%d %H:%M UTC")));
        body.push_str(&format!("End:   {}\n", end_str));
        if !location.is_empty() {
            body.push_str(&format!("Location: {}\n", location));
        }
        if !organizer.is_empty() {
            body.push_str(&format!("Organizer: {}\n", organizer));
        }
        if !attendees.is_empty() {
            body.push_str(&format!("Attendees: {}\n", attendees.join(", ")));
        }
        if !description.is_empty() {
            body.push_str(&format!("\nDescription:\n{}\n", description));
        }

        let folder = if folder_path.is_empty() || folder_path == ["Import"] {
            vec!["Calendar".to_string()]
        } else {
            folder_path.to_vec()
        };

        let raw_bytes = body.as_bytes();
        let (sha256, md5) = EmailMessage::compute_hashes(raw_bytes);
        let msg_id = if !uid.is_empty() {
            format!("<{}>", uid)
        } else {
            format!("<ical-{}@mail2rust.local>", &sha256[..12])
        };

        let organizer_addr = if !organizer.is_empty() {
            EmailAddress::new("Organizer", organizer)
        } else {
            EmailAddress::new("Calendar Service", "calendar@local")
        };

        let mapi_properties = vec![
            MapiProperty {
                tag: 0x001A001F, // PR_MESSAGE_CLASS_W
                name: "PR_MESSAGE_CLASS_W".to_string(),
                prop_type: "PT_UNICODE".to_string(),
                value: "IPM.Appointment".to_string(),
            },
            MapiProperty {
                tag: 0x0037001F, // PR_SUBJECT_W
                name: "PR_SUBJECT_W".to_string(),
                prop_type: "PT_UNICODE".to_string(),
                value: event_title.clone(),
            },
            MapiProperty {
                tag: 0x1000001F, // PR_BODY_W
                name: "PR_BODY_W".to_string(),
                prop_type: "PT_UNICODE".to_string(),
                value: body.clone(),
            },
        ];

        Some(EmailMessage {
            id: format!("event_{}", &sha256[..8]),
            message_id: msg_id.clone(),
            conversation_id: format!("event_{}", &sha256[..8]),
            folder_path: folder,
            subject: event_title,
            from: organizer_addr,
            to: attendees.into_iter().map(|a| EmailAddress::new(&a, &a)).collect(),
            cc: vec![],
            bcc: vec![],
            date: start_time,
            size_bytes: raw_bytes.len(),
            body_text: body,
            body_html: None,
            raw_mime: Vec::new(),
            attachments: Vec::new(),
            mapi_properties,
            flags: MessageFlags::default(),
            importance: Importance::Normal,
            sha256,
            md5,
            bates_number: None,
            legal_hold: false,
            source_format: "iCalendar (ICS)".to_string(),
        })
    }

    fn parse_ical_date(raw: &str) -> Option<DateTime<Utc>> {
        let clean = raw.trim();
        // 1. UTC format: 20261107T143000Z
        if let Ok(naive) = NaiveDateTime::parse_from_str(clean, "%Y%m%dT%H%M%SZ") {
            return Some(DateTime::from_naive_utc_and_offset(naive, Utc));
        }
        // 2. Local/Floating format: 20261107T143000
        if let Ok(naive) = NaiveDateTime::parse_from_str(clean, "%Y%m%dT%H%M%S") {
            return Some(DateTime::from_naive_utc_and_offset(naive, Utc));
        }
        // 3. Date only format: 20261107
        if clean.len() == 8 && clean.chars().all(|c| c.is_ascii_digit()) {
            let y: i32 = clean[0..4].parse().ok()?;
            let m: u32 = clean[4..6].parse().ok()?;
            let d: u32 = clean[6..8].parse().ok()?;
            return Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).single();
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ical_parsing() {
        let ics_data = b"BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
PRODID:-//Mail2Rust//NONSGML Calendar//EN\r\n\
BEGIN:VEVENT\r\n\
UID:meet-102@titan.corp\r\n\
DTSTART:20261107T140000Z\r\n\
DTEND:20261107T150000Z\r\n\
SUMMARY:Project Titan Engineering Review\r\n\
LOCATION:Conference Room Delta\r\n\
ORGANIZER:mailto:lead@titan.corp\r\n\
ATTENDEE:mailto:elena.vance@titan.corp\r\n\
DESCRIPTION:Quarterly architecture sync and Rust MS-PST verification signoff.\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";

        let events = IcalReader::parse(ics_data, vec!["Calendar".to_string()]).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].subject, "Project Titan Engineering Review");
        assert_eq!(events[0].message_id, "<meet-102@titan.corp>");
        assert!(events[0].body_text.contains("Conference Room Delta"));
        assert_eq!(events[0].date.format("%Y-%m-%d").to_string(), "2026-11-07");
        assert!(events[0].mapi_properties.iter().any(|p| p.value == "IPM.Appointment"));
    }
}
