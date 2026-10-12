use serde::{de::DeserializeOwned, Serialize};
use std::fmt;

/// Converts JSONC (JSON with Comments) to plain JSON.
/// Strips `//` line comments and `/* */` block comments, preserving newlines
/// so that line numbers in parse errors remain accurate.
/// Correctly ignores `//` and `/*` that appear inside string literals.
fn strip_jsonc_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut in_string = false;

    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                // Consume the escaped character verbatim
                if let Some(escaped) = chars.next() {
                    out.push(escaped);
                }
            } else if c == '"' {
                in_string = false;
            }
        } else {
            match c {
                '"' => {
                    in_string = true;
                    out.push(c);
                }
                '/' if chars.peek() == Some(&'/') => {
                    chars.next(); // consume second '/'
                                  // Drop everything until end of line, but keep the newline
                    for nc in chars.by_ref() {
                        if nc == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                }
                '/' if chars.peek() == Some(&'*') => {
                    chars.next(); // consume '*'
                    let mut prev_star = false;
                    for nc in chars.by_ref() {
                        if nc == '\n' {
                            out.push('\n');
                        }
                        if prev_star && nc == '/' {
                            break;
                        }
                        prev_star = nc == '*';
                    }
                }
                _ => out.push(c),
            }
        }
    }
    out
}

fn fix_trailing_commas(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();
    let mut result: Vec<String> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_end();
        if trimmed.ends_with(',') {
            let next_significant = lines[i + 1..]
                .iter()
                .find(|l| !l.trim().is_empty())
                .map(|l| l.trim_start())
                .unwrap_or("");
            if next_significant.starts_with('}') || next_significant.starts_with(']') {
                let comma_pos = line.rfind(',').unwrap();
                result.push(format!("{}{}", &line[..comma_pos], &line[comma_pos + 1..]));
                continue;
            }
        }
        result.push(line.to_string());
    }
    result.join("\n")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditFormat {
    Json,
    Toml,
}

impl EditFormat {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "toml" => Self::Toml,
            _ => Self::Json,
        }
    }

    pub fn file_extension(&self) -> &'static str {
        match self {
            Self::Json => "jsonc",
            Self::Toml => "toml",
        }
    }

    pub fn serialize<T: Serialize>(&self, value: &T) -> Result<String, String> {
        match self {
            Self::Json => {
                serde_json::to_string_pretty(value).map_err(|e| format!("JSON serialize: {}", e))
            }
            Self::Toml => {
                toml::to_string_pretty(value).map_err(|e| format!("TOML serialize: {}", e))
            }
        }
    }

    pub fn deserialize<T: DeserializeOwned>(&self, s: &str) -> Result<T, String> {
        match self {
            Self::Json => {
                let stripped = strip_jsonc_comments(s);
                let cleaned = fix_trailing_commas(&stripped);
                serde_json::from_str(&cleaned).map_err(|e| format!("JSON deserialize: {}", e))
            }
            Self::Toml => toml::from_str(s).map_err(|e| format!("TOML deserialize: {}", e)),
        }
    }

    /// Inserts a single comment line before the first storage field so that the
    /// fields remain active (editable) while making their read-only status clear.
    pub fn annotate_storage_fields(&self, content: &str) -> String {
        let comment = match self {
            Self::Json => {
                "  // Storage is controlled by the CLI file argument — changes are ignored:"
            }
            Self::Toml => "# Storage is controlled by the CLI file argument — changes are ignored:",
        };
        let mut result: Vec<String> = Vec::new();
        let mut inserted = false;
        for line in content.lines() {
            if !inserted && Self::is_storage_line(line) {
                result.push(comment.to_string());
                inserted = true;
            }
            result.push(line.to_string());
        }
        result.join("\n")
    }

    /// Prefixes each storage field line with the format's comment marker so that
    /// it is visible but clearly marked as overridden by a CLI argument.
    pub fn comment_storage_fields(&self, content: &str) -> String {
        let prefix = match self {
            Self::Json => "// ",
            Self::Toml => "# ",
        };
        content
            .lines()
            .map(|line| {
                if Self::is_storage_line(line) {
                    format!("{}{}", prefix, line)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn is_storage_line(line: &str) -> bool {
        let t = line.trim_start();
        t.starts_with("\"storage_backend\"")
            || t.starts_with("\"storage_location\"")
            || t.starts_with("storage_backend")
            || t.starts_with("storage_location")
    }
}

impl fmt::Display for EditFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json => write!(f, "json"),
            Self::Toml => write!(f, "toml"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_service::AppConfigDto;

    #[test]
    fn test_edit_format_from_str_json() {
        assert_eq!(EditFormat::parse("json"), EditFormat::Json);
    }

    #[test]
    fn test_edit_format_from_str_toml() {
        assert_eq!(EditFormat::parse("toml"), EditFormat::Toml);
    }

    #[test]
    fn test_edit_format_from_str_unknown_defaults_json() {
        assert_eq!(EditFormat::parse("yaml"), EditFormat::Json);
        assert_eq!(EditFormat::parse(""), EditFormat::Json);
    }

    #[test]
    fn test_edit_format_file_extension_json() {
        assert_eq!(EditFormat::Json.file_extension(), "jsonc");
    }

    #[test]
    fn test_edit_format_file_extension_toml() {
        assert_eq!(EditFormat::Toml.file_extension(), "toml");
    }

    #[test]
    fn test_annotate_storage_fields_json_keeps_lines_active_with_comment_before() {
        let input = "{\n  \"default_card_prefix\": \"feat\",\n  \"storage_backend\": \"json\",\n  \"storage_location\": \"/tmp/b.json\"\n}";
        let result = EditFormat::Json.annotate_storage_fields(input);
        let lines: Vec<&str> = result.lines().collect();
        let backend_idx = lines
            .iter()
            .position(|l| l.trim_start().starts_with("\"storage_backend\""))
            .expect("storage_backend must be present as an active line");
        let location_idx = lines
            .iter()
            .position(|l| l.trim_start().starts_with("\"storage_location\""))
            .expect("storage_location must be present as an active line");
        assert!(
            lines[..backend_idx]
                .iter()
                .any(|l| l.trim_start().starts_with("//")),
            "a // comment must appear before storage_backend"
        );
        assert!(
            backend_idx < location_idx,
            "storage_backend must appear before storage_location"
        );
    }

    #[test]
    fn test_annotate_storage_fields_toml_keeps_lines_active_with_comment_before() {
        let input = "default_card_prefix = \"feat\"\nstorage_backend = \"json\"\nstorage_location = \"/tmp/b.json\"\n";
        let result = EditFormat::Toml.annotate_storage_fields(input);
        let lines: Vec<&str> = result.lines().collect();
        let backend_idx = lines
            .iter()
            .position(|l| l.trim_start().starts_with("storage_backend"))
            .expect("storage_backend must be present as an active line");
        assert!(
            lines[..backend_idx]
                .iter()
                .any(|l| l.trim_start().starts_with('#')),
            "a # comment must appear before storage_backend"
        );
        let location_present = lines
            .iter()
            .any(|l| l.trim_start().starts_with("storage_location"));
        assert!(
            location_present,
            "storage_location must be present as an active line"
        );
    }

    #[test]
    fn test_comment_storage_fields_json() {
        let input =
            "{\n  \"default_card_prefix\": \"feat\",\n  \"storage_backend\": \"sqlite\",\n  \"storage_location\": \"/tmp/b.db\"\n}";
        let result = EditFormat::Json.comment_storage_fields(input);
        assert!(
            result
                .lines()
                .any(|l| l.starts_with("// ") && l.contains("\"storage_backend\"")),
            "storage_backend line should be commented out"
        );
        assert!(
            result
                .lines()
                .any(|l| l.starts_with("// ") && l.contains("\"storage_location\"")),
            "storage_location line should be commented out"
        );
        assert!(result.contains("\"default_card_prefix\""));
        assert!(
            !result
                .lines()
                .any(|l| l.starts_with("// ") && l.contains("\"default_card_prefix\"")),
            "default_card_prefix should not be commented out"
        );
    }

    #[test]
    fn test_comment_storage_fields_toml() {
        let input =
            "default_card_prefix = \"feat\"\nstorage_backend = \"sqlite\"\nstorage_location = \"/tmp/b.db\"\n";
        let result = EditFormat::Toml.comment_storage_fields(input);
        assert!(
            result
                .lines()
                .any(|l| l.starts_with("# ") && l.contains("storage_backend")),
            "storage_backend line should be commented out"
        );
        assert!(
            result
                .lines()
                .any(|l| l.starts_with("# ") && l.contains("storage_location")),
            "storage_location line should be commented out"
        );
        assert!(
            !result
                .lines()
                .any(|l| l.starts_with("# ") && l.contains("default_card_prefix")),
            "default_card_prefix should not be commented out"
        );
    }

    #[test]
    fn test_json_deserialize_strips_line_comments() {
        let input = "{\n  \"default_card_prefix\": \"feat\",\n//   \"storage_backend\": \"sqlite\",\n  \"editing_format\": \"json\"\n}";
        let dto: AppConfigDto = EditFormat::Json.deserialize(input).unwrap();
        assert_eq!(dto.default_card_prefix.as_deref(), Some("feat"));
        assert!(dto.storage_backend.is_none());
    }

    #[test]
    fn test_json_deserialize_strips_inline_comments() {
        let input = "{\n  \"default_card_prefix\": \"feat\", // the prefix\n  \"editing_format\": \"json\"\n}";
        let dto: AppConfigDto = EditFormat::Json.deserialize(input).unwrap();
        assert_eq!(dto.default_card_prefix.as_deref(), Some("feat"));
        assert_eq!(dto.editing_format.as_deref(), Some("json"));
    }

    #[test]
    fn test_json_deserialize_ignores_comment_markers_inside_strings() {
        let input =
            "{\n  \"default_card_prefix\": \"feat//nope\",\n  \"editing_format\": \"json\"\n}";
        let dto: AppConfigDto = EditFormat::Json.deserialize(input).unwrap();
        assert_eq!(dto.default_card_prefix.as_deref(), Some("feat//nope"));
    }

    #[test]
    fn test_json_deserialize_fixes_trailing_comma_when_last_fields_commented() {
        let input = "{\n  \"default_card_prefix\": \"feat\",\n  \"editing_format\": \"json\",\n//   \"storage_backend\": \"sqlite\",\n//   \"storage_location\": \"/tmp/b.db\"\n}";
        let dto: AppConfigDto = EditFormat::Json.deserialize(input).unwrap();
        assert_eq!(dto.default_card_prefix.as_deref(), Some("feat"));
        assert!(dto.storage_backend.is_none());
        assert!(dto.storage_location.is_none());
    }

    #[test]
    fn test_edit_format_json_serialize_round_trip() {
        let dto = AppConfigDto {
            background_highlight: Some("#2E3440".into()),
            default_card_prefix: Some("feat".into()),
            default_sprint_prefix: Some("sprint".into()),
            storage_backend: Some("sqlite".into()),
            editing_format: Some("json".into()),
            configuration_format: Some("toml".into()),
            configuration_location: Some("/tmp/test.toml".into()),
            storage_location: None,
        };
        let serialized = EditFormat::Json.serialize(&dto).unwrap();
        let deserialized: AppConfigDto = EditFormat::Json.deserialize(&serialized).unwrap();
        assert_eq!(deserialized.default_card_prefix.as_deref(), Some("feat"));
        assert_eq!(deserialized.storage_backend.as_deref(), Some("sqlite"));
        assert_eq!(deserialized.editing_format.as_deref(), Some("json"));
        assert_eq!(
            deserialized.background_highlight.as_deref(),
            Some("#2E3440")
        );
    }

    #[test]
    fn test_edit_format_toml_serialize_round_trip() {
        let dto = AppConfigDto {
            background_highlight: Some("#2E3440".into()),
            default_card_prefix: Some("feat".into()),
            default_sprint_prefix: Some("sprint".into()),
            storage_backend: Some("sqlite".into()),
            editing_format: Some("toml".into()),
            configuration_format: Some("toml".into()),
            configuration_location: Some("/tmp/test.toml".into()),
            storage_location: None,
        };
        let serialized = EditFormat::Toml.serialize(&dto).unwrap();
        let deserialized: AppConfigDto = EditFormat::Toml.deserialize(&serialized).unwrap();
        assert_eq!(deserialized.default_card_prefix.as_deref(), Some("feat"));
        assert_eq!(deserialized.storage_backend.as_deref(), Some("sqlite"));
        assert_eq!(deserialized.editing_format.as_deref(), Some("toml"));
        assert_eq!(
            deserialized.background_highlight.as_deref(),
            Some("#2E3440")
        );
    }
}
