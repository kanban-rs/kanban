/// The URL scheme of `locator`, when it has one. `None` for every filesystem
/// path, including a Windows drive letter and a relative path whose first
/// component happens to contain `://`.
pub fn scheme_of(locator: &str) -> Option<&str> {
    let (scheme, _) = locator.split_once("://")?;
    if scheme.len() < 2 {
        return None;
    }
    let mut chars = scheme.chars();
    if !chars.next()?.is_ascii_alphabetic() {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
        return None;
    }
    Some(scheme)
}

/// True when the locator carries a URL scheme and must therefore never be
/// treated as a filesystem path.
pub fn is_remote_locator(locator: &str) -> bool {
    scheme_of(locator).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_an_http_locator_reports_its_scheme() {
        assert_eq!(scheme_of("http://127.0.0.1:3000"), Some("http"));
        assert_eq!(scheme_of("https://example.com/boards"), Some("https"));
        assert!(is_remote_locator("http://127.0.0.1:3000"));
        assert!(is_remote_locator("https://example.com/boards"));
    }

    #[test]
    fn test_a_non_http_scheme_is_still_remote() {
        assert!(is_remote_locator("notes://draft.json"));
        assert_eq!(scheme_of("s3+v4://bucket/key"), Some("s3+v4"));
    }

    #[test]
    fn test_a_path_segment_containing_the_scheme_marker_is_not_remote() {
        assert!(!is_remote_locator("data/a://b.json"));
        assert!(!is_remote_locator("/abs/dir/x://y.json"));
    }

    #[test]
    fn test_a_single_letter_scheme_is_not_remote() {
        assert!(!is_remote_locator("C://boards"));
        assert!(!is_remote_locator("c://boards"));
    }

    #[test]
    fn test_a_scheme_with_a_leading_digit_is_not_remote() {
        assert!(!is_remote_locator("2fast://x"));
        assert!(!is_remote_locator("ht tp://x"));
    }

    #[test]
    fn test_a_plain_relative_path_has_no_scheme() {
        assert!(scheme_of("kanban.json").is_none());
        assert!(scheme_of("/home/u/boards.sqlite").is_none());
        assert!(scheme_of("").is_none());
    }
}
