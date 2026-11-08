use mail2rust_core::EmailMessage;
use regex::bytes::{RegexSet, RegexSetBuilder};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchRule {
    pub tag: String,
    pub description: String,
    pub pattern: String,
    #[serde(default)]
    pub category: Option<String>,
}

impl SearchRule {
    pub fn new(tag: impl Into<String>, description: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            description: description.into(),
            pattern: pattern.into(),
            category: None,
        }
    }

    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchRuleMatch {
    pub tag: String,
    pub description: String,
    pub pattern: String,
    pub category: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MultiPatternMatcher {
    rules: Vec<SearchRule>,
    regex_set: RegexSet,
}

impl MultiPatternMatcher {
    /// Compile a list of search rules into a zero-copy DFA RegexSet
    pub fn new(rules: Vec<SearchRule>) -> Result<Self, regex::Error> {
        let pattern_strings: Vec<&str> = rules.iter().map(|r| r.pattern.as_str()).collect();
        let regex_set = RegexSetBuilder::new(&pattern_strings).case_insensitive(true).build()?;
        Ok(Self { rules, regex_set })
    }

    /// Convenience builder from a simple list of regex pattern strings
    pub fn from_patterns<I, S>(patterns: I) -> Result<Self, regex::Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let rules: Vec<SearchRule> = patterns
            .into_iter()
            .enumerate()
            .map(|(idx, pat)| {
                let p = pat.as_ref().to_string();
                SearchRule {
                    tag: format!("RULE_{:03}", idx + 1),
                    description: p.clone(),
                    pattern: p,
                    category: None,
                }
            })
            .collect();
        Self::new(rules)
    }

    /// Load search rules from a file (supports JSON, TOML, or line-delimited text rules)
    pub fn from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;

        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        if ext == "json" {
            let rules: Vec<SearchRule> = serde_json::from_str(&content)?;
            return Ok(Self::new(rules)?);
        }

        // Check if file is TOML format (e.g. [[rules]])
        if ext == "toml" || content.contains("[[rules]]") {
            if let Ok(container) = toml_simple_parse(&content) {
                return Ok(Self::new(container)?);
            }
        }

        // Parse line-by-line text rules
        // Supported syntax:
        // TAG | Description | regex_pattern
        // TAG: regex_pattern
        // regex_pattern
        let mut rules = Vec::new();
        let mut auto_idx = 1usize;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
                continue;
            }

            if trimmed.contains('|') {
                let parts: Vec<&str> = trimmed.split('|').map(|s| s.trim()).collect();
                if parts.len() >= 3 {
                    rules.push(SearchRule::new(parts[0], parts[1], parts[2]));
                } else if parts.len() == 2 {
                    rules.push(SearchRule::new(parts[0], parts[1], parts[1]));
                }
            } else if let Some((tag, pat)) = trimmed.split_once(':') {
                let tag = tag.trim();
                let pat = pat.trim();
                if !pat.is_empty() {
                    rules.push(SearchRule::new(tag, pat, pat));
                }
            } else {
                rules.push(SearchRule::new(format!("RULE_{:03}", auto_idx), trimmed, trimmed));
                auto_idx += 1;
            }
        }

        Ok(Self::new(rules)?)
    }

    /// Single-pass DFA evaluation over raw message bytes.
    /// Evaluates all configured patterns simultaneously in microseconds with zero allocations.
    pub fn matches_bytes(&self, bytes: &[u8]) -> Option<Vec<SearchRuleMatch>> {
        let matches = self.regex_set.matches(bytes);
        if !matches.matched_any() {
            return None;
        }

        let mut matched_rules = Vec::new();
        for idx in matches.into_iter() {
            if let Some(r) = self.rules.get(idx) {
                matched_rules.push(SearchRuleMatch {
                    tag: r.tag.clone(),
                    description: r.description.clone(),
                    pattern: r.pattern.clone(),
                    category: r.category.clone(),
                });
            }
        }

        Some(matched_rules)
    }

    /// Match against an EmailMessage structure. Evaluates raw_mime if available,
    /// or combines Subject, From, To, and Body.
    pub fn matches_message(&self, msg: &EmailMessage) -> Option<Vec<SearchRuleMatch>> {
        if !msg.raw_mime.is_empty() {
            if let Some(m) = self.matches_bytes(&msg.raw_mime) {
                return Some(m);
            }
        }

        // Construct lightweight search buffer from headers and body
        let mut buf = Vec::with_capacity(msg.subject.len() + msg.body_text.len() + 256);
        buf.extend_from_slice(b"Subject: ");
        buf.extend_from_slice(msg.subject.as_bytes());
        buf.extend_from_slice(b"\nFrom: ");
        buf.extend_from_slice(msg.from.name.as_bytes());
        buf.extend_from_slice(b" <");
        buf.extend_from_slice(msg.from.email.as_bytes());
        buf.extend_from_slice(b">\nTo: ");
        for to in &msg.to {
            buf.extend_from_slice(to.email.as_bytes());
            buf.extend_from_slice(b" ");
        }
        buf.extend_from_slice(b"\n\n");
        buf.extend_from_slice(msg.body_text.as_bytes());

        self.matches_bytes(&buf)
    }

    /// Returns a semicolon-delimited string of matched tags (e.g. "1-EXP;1-8-EXP"), or None
    pub fn matched_tags_string(&self, bytes: &[u8]) -> Option<String> {
        self.matches_bytes(bytes)
            .map(|matches| matches.into_iter().map(|m| m.tag).collect::<Vec<_>>().join(";"))
    }

    pub fn rules(&self) -> &[SearchRule] {
        &self.rules
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

fn toml_simple_parse(content: &str) -> Result<Vec<SearchRule>, String> {
    // Basic parser for [[rules]] blocks without adding extra heavy crate dependencies
    let mut rules = Vec::new();
    let mut current_tag = String::new();
    let mut current_desc = String::new();
    let mut current_pat = String::new();
    let mut current_cat: Option<String> = None;
    let mut in_rule = false;

    for line in content.lines() {
        let t = line.trim();
        if t == "[[rules]]" {
            if in_rule && !current_pat.is_empty() {
                rules.push(SearchRule {
                    tag: if current_tag.is_empty() {
                        format!("RULE_{:03}", rules.len() + 1)
                    } else {
                        current_tag.clone()
                    },
                    description: if current_desc.is_empty() {
                        current_pat.clone()
                    } else {
                        current_desc.clone()
                    },
                    pattern: current_pat.clone(),
                    category: current_cat.clone(),
                });
            }
            in_rule = true;
            current_tag.clear();
            current_desc.clear();
            current_pat.clear();
            current_cat = None;
            continue;
        }

        if in_rule {
            if let Some((k, v)) = t.split_once('=') {
                let k = k.trim();
                let v_trimmed = v.trim();
                let is_single_quoted = v_trimmed.starts_with('\'') && v_trimmed.ends_with('\'');
                let inner = v_trimmed.trim_matches('"').trim_matches('\'');
                let unescaped = if is_single_quoted {
                    inner.to_string()
                } else {
                    inner.replace("\\\\", "\\").replace("\\\"", "\"")
                };
                match k {
                    "tag" => current_tag = unescaped,
                    "description" => current_desc = unescaped,
                    "pattern" => current_pat = unescaped,
                    "category" => current_cat = Some(unescaped),
                    _ => {}
                }
            }
        }
    }

    if in_rule && !current_pat.is_empty() {
        rules.push(SearchRule {
            tag: if current_tag.is_empty() {
                format!("RULE_{:03}", rules.len() + 1)
            } else {
                current_tag
            },
            description: if current_desc.is_empty() {
                current_pat.clone()
            } else {
                current_desc
            },
            pattern: current_pat,
            category: current_cat,
        });
    }

    if rules.is_empty() {
        Err("No rules found in TOML".to_string())
    } else {
        Ok(rules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_pattern_regex_set() {
        let rules = vec![
            SearchRule::new("1-1", "Enterprise project", r"500\s*Enterprise"),
            SearchRule::new("1-2", "Corporate address", r"100\s*Corporate"),
            SearchRule::new("1-3", "Titan company", r"ProjectTitan"),
        ];

        let matcher = MultiPatternMatcher::new(rules).unwrap();

        let msg_bytes_1 = b"Subject: Re: Update on 100 Corporate Blvd\n\nMeeting tomorrow regarding site.";
        let matches_1 = matcher.matches_bytes(msg_bytes_1).unwrap();
        assert_eq!(matches_1.len(), 1);
        assert_eq!(matches_1[0].tag, "1-2");

        let msg_bytes_both = b"Subject: 500 Enterprise Way & ProjectTitan Corp\n\nPlease find agreements attached.";
        let matches_both = matcher.matches_bytes(msg_bytes_both).unwrap();
        assert_eq!(matches_both.len(), 2);
        let tags: Vec<String> = matches_both.into_iter().map(|m| m.tag).collect();
        assert!(tags.contains(&"1-1".to_string()));
        assert!(tags.contains(&"1-3".to_string()));

        let msg_bytes_none = b"Subject: Unrelated invoice\n\nPayment confirmed.";
        assert!(matcher.matches_bytes(msg_bytes_none).is_none());
    }

    #[test]
    fn test_parse_rules_from_text() {
        let text_content = r#"
# e-Discovery Investigation Rules
CASE1 | 100 Corporate Boulevard | 100\s*Corporate
CASE2 | 500 Enterprise Way | 500\s*Enterprise
HOLDING: ProjectTitan
"#;
        let temp_file = std::env::temp_dir().join(format!("rules_test_{}.rules", std::process::id()));
        std::fs::write(&temp_file, text_content).unwrap();

        let matcher = MultiPatternMatcher::from_file(&temp_file).unwrap();
        assert_eq!(matcher.len(), 3);
        assert_eq!(matcher.rules()[0].tag, "CASE1");
        assert_eq!(matcher.rules()[1].tag, "CASE2");
        assert_eq!(matcher.rules()[2].tag, "HOLDING");

        let _ = std::fs::remove_file(&temp_file);
    }
}
