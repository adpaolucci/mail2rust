use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlIndexRow {
    pub filename: String,
    pub subject: String,
    pub from: String,
    pub date: String,
    pub size_bytes: usize,
    pub matched_keywords: String,
}

pub struct HtmlIndexGenerator;

impl HtmlIndexGenerator {
    pub fn generate_html(title: &str, rows: &[HtmlIndexRow]) -> String {
        let mut html = String::with_capacity(rows.len() * 256 + 2048);
        html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"UTF-8\">\n");
        html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
        html.push_str(&format!("<title>{}</title>\n", escape_html(title)));
        html.push_str("<style>\n");
        html.push_str("body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; font-size: 13px; margin: 20px; background-color: #fdfdfd; color: #222; }\n");
        html.push_str(".header-panel { display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px; flex-wrap: wrap; gap: 10px; }\n");
        html.push_str("h1 { font-size: 18px; margin: 0; color: #1a365d; }\n");
        html.push_str(".meta-summary { font-size: 13px; color: #4a5568; }\n");
        html.push_str(".search-box { padding: 6px 12px; border: 1px solid #cbd5e0; border-radius: 4px; font-size: 13px; width: 280px; outline: none; }\n");
        html.push_str(".search-box:focus { border-color: #3182ce; box-shadow: 0 0 0 1px #3182ce; }\n");
        html.push_str(".table-container { max-height: calc(100vh - 120px); overflow-y: auto; border: 1px solid #e2e8f0; border-radius: 6px; box-shadow: 0 1px 3px rgba(0,0,0,0.05); background: #fff; }\n");
        html.push_str("table { border-collapse: collapse; width: 100%; font-size: 12.5px; }\n");
        html.push_str("th { background-color: #edf2f7; color: #2d3748; position: sticky; top: 0; z-index: 10; padding: 8px 10px; text-align: left; border-bottom: 2px solid #cbd5e0; cursor: pointer; user-select: none; }\n");
        html.push_str("th:hover { background-color: #e2e8f0; }\n");
        html.push_str("td { padding: 6px 10px; border-bottom: 1px solid #edf2f7; vertical-align: top; }\n");
        html.push_str("tr:nth-child(even) { background-color: #f7fafc; }\n");
        html.push_str("tr:hover { background-color: #ebf8ff; }\n");
        html.push_str("a.eml-link { text-decoration: none; color: #2b6cb0; font-weight: 500; }\n");
        html.push_str("a.eml-link:hover { text-decoration: underline; color: #1a4971; }\n");
        html.push_str(".tag { display: inline-block; background-color: #e2e8f0; color: #2d3748; padding: 2px 6px; border-radius: 3px; font-size: 11px; margin-right: 4px; margin-bottom: 2px; }\n");
        html.push_str(".size-col { text-align: right; white-space: nowrap; color: #718096; }\n");
        html.push_str(".date-col { white-space: nowrap; color: #4a5568; }\n");
        html.push_str(
            ".from-col { max-width: 250px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }\n",
        );
        html.push_str("</style>\n</head>\n<body>\n");

        html.push_str("<div class=\"header-panel\">\n");
        html.push_str(&format!("  <div>\n    <h1>{}</h1>\n    <div class=\"meta-summary\">Total Items: <strong id=\"itemCount\">{}</strong></div>\n  </div>\n", escape_html(title), rows.len()));
        html.push_str("  <input type=\"text\" id=\"filterInput\" class=\"search-box\" placeholder=\"Filter subjects, senders, keywords...\" onkeyup=\"filterTable()\">\n");
        html.push_str("</div>\n\n");

        html.push_str("<div class=\"table-container\">\n");
        html.push_str("<table id=\"msgTable\">\n");
        html.push_str("<thead><tr>\n");
        html.push_str("  <th onclick=\"sortTable(0)\">Subject &#x25B4;&#x25BE;</th>\n");
        html.push_str("  <th onclick=\"sortTable(1)\">From &#x25B4;&#x25BE;</th>\n");
        html.push_str("  <th onclick=\"sortTable(2)\">Date &#x25B4;&#x25BE;</th>\n");
        html.push_str("  <th onclick=\"sortTable(3)\">Matched Keywords &#x25B4;&#x25BE;</th>\n");
        html.push_str("  <th onclick=\"sortTable(4)\" style=\"text-align:right\">Size &#x25B4;&#x25BE;</th>\n");
        html.push_str("</tr></thead>\n<tbody>\n");

        for row in rows {
            let size_str = format_size(row.size_bytes);
            let subj_display = if row.subject.trim().is_empty() {
                "(No Subject)"
            } else {
                &row.subject
            };

            let tags_html = if row.matched_keywords.is_empty() {
                String::new()
            } else {
                row.matched_keywords
                    .split(';')
                    .map(|t| format!("<span class=\"tag\">{}</span>", escape_html(t.trim())))
                    .collect::<Vec<_>>()
                    .join("")
            };

            html.push_str("<tr>\n");
            html.push_str(&format!(
                "  <td><a class=\"eml-link\" href=\"./{}\">{}</a></td>\n",
                escape_html(&row.filename),
                escape_html(subj_display)
            ));
            html.push_str(&format!(
                "  <td class=\"from-col\" title=\"{}\">{}</td>\n",
                escape_html(&row.from),
                escape_html(&row.from)
            ));
            html.push_str(&format!("  <td class=\"date-col\">{}</td>\n", escape_html(&row.date)));
            html.push_str(&format!("  <td>{}</td>\n", tags_html));
            html.push_str(&format!(
                "  <td class=\"size-col\" data-bytes=\"{}\">{}</td>\n",
                row.size_bytes, size_str
            ));
            html.push_str("</tr>\n");
        }

        html.push_str("</tbody>\n</table>\n</div>\n\n");

        // Client-side instant filter & column sorting script (zero dependencies, fully offline)
        html.push_str("<script>\n");
        html.push_str("function filterTable() {\n");
        html.push_str("  var input = document.getElementById('filterInput').value.toLowerCase();\n");
        html.push_str("  var rows = document.getElementById('msgTable').getElementsByTagName('tbody')[0].getElementsByTagName('tr');\n");
        html.push_str("  var visible = 0;\n");
        html.push_str("  for (var i = 0; i < rows.length; i++) {\n");
        html.push_str("    var text = rows[i].textContent.toLowerCase();\n");
        html.push_str("    if (text.indexOf(input) > -1) { rows[i].style.display = ''; visible++; } else { rows[i].style.display = 'none'; }\n");
        html.push_str("  }\n");
        html.push_str("  document.getElementById('itemCount').textContent = visible;\n");
        html.push_str("}\n\n");
        html.push_str("function sortTable(n) {\n");
        html.push_str("  var table = document.getElementById('msgTable');\n");
        html.push_str("  var tbody = table.getElementsByTagName('tbody')[0];\n");
        html.push_str("  var rows = Array.from(tbody.getElementsByTagName('tr'));\n");
        html.push_str(
            "  var asc = table.getAttribute('data-sort-col') != n || table.getAttribute('data-sort-dir') != 'asc';\n",
        );
        html.push_str("  rows.sort(function(a, b) {\n");
        html.push_str("    if (n === 4) {\n");
        html.push_str(
            "      var aBytes = parseInt(a.getElementsByTagName('td')[4].getAttribute('data-bytes') || 0);\n",
        );
        html.push_str(
            "      var bBytes = parseInt(b.getElementsByTagName('td')[4].getAttribute('data-bytes') || 0);\n",
        );
        html.push_str("      return asc ? aBytes - bBytes : bBytes - aBytes;\n");
        html.push_str("    }\n");
        html.push_str("    var aText = a.getElementsByTagName('td')[n].textContent.trim();\n");
        html.push_str("    var bText = b.getElementsByTagName('td')[n].textContent.trim();\n");
        html.push_str("    return asc ? aText.localeCompare(bText) : bText.localeCompare(aText);\n");
        html.push_str("  });\n");
        html.push_str("  table.setAttribute('data-sort-col', n);\n");
        html.push_str("  table.setAttribute('data-sort-dir', asc ? 'asc' : 'desc');\n");
        html.push_str("  rows.forEach(function(row) { tbody.appendChild(row); });\n");
        html.push_str("}\n");
        html.push_str("</script>\n</body>\n</html>\n");

        html
    }

    pub fn write_index_file(path: &Path, title: &str, rows: &[HtmlIndexRow]) -> io::Result<()> {
        let content = Self::generate_html(title, rows);
        let mut file = BufWriter::new(File::create(path)?);
        file.write_all(content.as_bytes())?;
        file.flush()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestRecord {
    pub message_id: String,
    pub date: String,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub size_bytes: usize,
    pub matched_keywords: String,
    pub source_container: String,
    pub sha256_hash: String,
    pub eml_filename: String,
}

pub struct ManifestWriter<W: Write> {
    writer: BufWriter<W>,
    count: usize,
}

impl ManifestWriter<File> {
    pub fn create_file(path: &Path) -> io::Result<Self> {
        let file = File::create(path)?;
        Self::new(file)
    }
}

impl<W: Write> ManifestWriter<W> {
    pub fn new(writer: W) -> io::Result<Self> {
        let mut buf = BufWriter::new(writer);
        writeln!(
            buf,
            "Message_ID,Date,From,To,Subject,Size_Bytes,Matched_Keywords,Source_Container,SHA256_Hash,EML_File"
        )?;
        Ok(Self { writer: buf, count: 0 })
    }

    pub fn write_record(&mut self, rec: &ManifestRecord) -> io::Result<()> {
        writeln!(
            self.writer,
            "{},{},{},{},{},{},{},{},{},{}",
            escape_csv(&rec.message_id),
            escape_csv(&rec.date),
            escape_csv(&rec.from),
            escape_csv(&rec.to),
            escape_csv(&rec.subject),
            rec.size_bytes,
            escape_csv(&rec.matched_keywords),
            escape_csv(&rec.source_container),
            escape_csv(&rec.sha256_hash),
            escape_csv(&rec.eml_filename),
        )?;
        self.count += 1;
        Ok(())
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_index_generation() {
        let rows = vec![HtmlIndexRow {
            filename: "000001_Test.eml".to_string(),
            subject: "Important discovery document".to_string(),
            from: "counsel@example.com".to_string(),
            date: "2026-11-08 12:00:00".to_string(),
            size_bytes: 45200,
            matched_keywords: "Corporate;ProjectTitan".to_string(),
        }];

        let html = HtmlIndexGenerator::generate_html("Case Index", &rows);
        assert!(html.contains("000001_Test.eml"));
        assert!(html.contains("Important discovery document"));
        assert!(html.contains("Corporate"));
        assert!(html.contains("sortTable"));
        assert!(html.contains("filterTable"));
    }

    #[test]
    fn test_manifest_writer_csv_escaping() {
        let mut buffer = Vec::new();
        {
            let mut writer = ManifestWriter::new(&mut buffer).unwrap();
            let rec = ManifestRecord {
                message_id: "<msg_1@example.com>".to_string(),
                date: "2026-11-08".to_string(),
                from: "\"Doe, Jane\" <jane@example.com>".to_string(),
                to: "team@example.com".to_string(),
                subject: "Re: \"Quoted\" & Commas, here".to_string(),
                size_bytes: 1024,
                matched_keywords: "RULE_1;RULE_2".to_string(),
                source_container: "INBOX.mbox".to_string(),
                sha256_hash: "abcdef123456".to_string(),
                eml_filename: "000001.eml".to_string(),
            };
            writer.write_record(&rec).unwrap();
            writer.flush().unwrap();
        }

        let csv_output = String::from_utf8(buffer).unwrap();
        assert!(csv_output.contains("Message_ID,Date,From,To,Subject"));
        assert!(csv_output.contains("\"\"\"Doe, Jane\"\" <jane@example.com>\""));
        assert!(csv_output.contains("\"Re: \"\"Quoted\"\" & Commas, here\""));
    }
}
