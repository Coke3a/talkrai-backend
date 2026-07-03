use crate::usecases::UsecaseError;

/// Canonical legal content, embedded at compile time so the running binary carries no runtime
/// filesystem dependency (and the LIFF app + landing page can converge on this one source later).
const TERMS_MD: &str = include_str!("../../../content/legal/terms.th.md");
const PRIVACY_MD: &str = include_str!("../../../content/legal/privacy.th.md");

pub struct GetLegalDocInput {
    pub doc: String,
}

pub struct LegalSection {
    pub title: String,
    pub body: String,
}

pub struct GetLegalDocOutput {
    pub doc: String,
    pub title: String,
    pub version: i32,
    pub updated: String,
    pub sections: Vec<LegalSection>,
}

#[derive(Default)]
pub struct GetLegalDocUseCase;

impl GetLegalDocUseCase {
    pub fn new() -> Self {
        Self
    }

    pub fn execute(&self, input: GetLegalDocInput) -> Result<GetLegalDocOutput, UsecaseError> {
        let raw = match input.doc.as_str() {
            "terms" => TERMS_MD,
            "privacy" => PRIVACY_MD,
            other => {
                return Err(UsecaseError::NotFound(format!(
                    "Unknown legal document: {other}"
                )))
            }
        };

        let parsed = parse_legal_markdown(raw);

        Ok(GetLegalDocOutput {
            doc: input.doc,
            title: parsed.title,
            version: parsed.version,
            updated: parsed.updated,
            sections: parsed.sections,
        })
    }
}

struct ParsedLegal {
    title: String,
    version: i32,
    updated: String,
    sections: Vec<LegalSection>,
}

/// Minimal front-matter + `## ` section parser for the legal documents. Kept dependency-free on
/// purpose: the content is authored in-repo and embedded at compile time, so a full Markdown engine
/// would be overkill. Everything before the first `## ` heading (after front matter) is ignored.
fn parse_legal_markdown(md: &str) -> ParsedLegal {
    let mut title = String::new();
    let mut version = 1;
    let mut updated = String::new();
    let mut body = md;

    // Front matter: a leading `---` … `---` block of `key: value` lines.
    if let Some(rest) = md.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            for line in rest[..end].lines() {
                if let Some((key, value)) = line.split_once(':') {
                    let value = value.trim();
                    match key.trim() {
                        "title" => title = value.to_string(),
                        "version" => version = value.parse().unwrap_or(1),
                        "updated" => updated = value.to_string(),
                        _ => {}
                    }
                }
            }
            body = &rest[end + "\n---\n".len()..];
        }
    }

    let mut sections: Vec<LegalSection> = Vec::new();
    let mut current: Option<(String, String)> = None;

    for line in body.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            if let Some((t, b)) = current.take() {
                sections.push(LegalSection {
                    title: t,
                    body: b.trim().to_string(),
                });
            }
            current = Some((heading.trim().to_string(), String::new()));
        } else if let Some((_, b)) = current.as_mut() {
            b.push_str(line);
            b.push('\n');
        }
    }
    if let Some((t, b)) = current.take() {
        sections.push(LegalSection {
            title: t,
            body: b.trim().to_string(),
        });
    }

    ParsedLegal {
        title,
        version,
        updated,
        sections,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_front_matter_and_sections() {
        let md = "---\ntitle: หัวข้อ\nversion: 2\nupdated: 1 ม.ค. 2026\n---\n\n## 1. หนึ่ง\nเนื้อหาหนึ่ง\nบรรทัดสอง\n\n## 2. สอง\nเนื้อหาสอง\n";
        let parsed = parse_legal_markdown(md);

        assert_eq!(parsed.title, "หัวข้อ");
        assert_eq!(parsed.version, 2);
        assert_eq!(parsed.updated, "1 ม.ค. 2026");
        assert_eq!(parsed.sections.len(), 2);
        assert_eq!(parsed.sections[0].title, "1. หนึ่ง");
        assert_eq!(parsed.sections[0].body, "เนื้อหาหนึ่ง\nบรรทัดสอง");
        assert_eq!(parsed.sections[1].title, "2. สอง");
        assert_eq!(parsed.sections[1].body, "เนื้อหาสอง");
    }

    #[test]
    fn embedded_terms_resolve_with_all_sections() {
        let out = GetLegalDocUseCase::new()
            .execute(GetLegalDocInput {
                doc: "terms".to_string(),
            })
            .expect("terms should resolve");

        assert_eq!(out.doc, "terms");
        assert_eq!(out.title, "ข้อกำหนดการใช้งาน");
        assert!(!out.updated.is_empty());
        assert_eq!(out.sections.len(), 12);
        assert!(out.sections.iter().all(|s| !s.body.is_empty()));
    }

    #[test]
    fn embedded_privacy_resolves() {
        let out = GetLegalDocUseCase::new()
            .execute(GetLegalDocInput {
                doc: "privacy".to_string(),
            })
            .expect("privacy should resolve");

        assert_eq!(out.title, "นโยบายความเป็นส่วนตัว");
        assert!(out.sections.iter().any(|s| s.title.contains("PDPA")));
    }

    #[test]
    fn unknown_doc_is_not_found() {
        let result = GetLegalDocUseCase::new().execute(GetLegalDocInput {
            doc: "cookies".to_string(),
        });
        assert!(matches!(result, Err(UsecaseError::NotFound(_))));
    }
}
