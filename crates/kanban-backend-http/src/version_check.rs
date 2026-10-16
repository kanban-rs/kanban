fn minor_line(version: &str) -> Option<(u64, u64)> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

use kanban_backend::CompatibilityNotice;

pub(crate) fn compatibility_notice(
    base_url: &str,
    server_version: Option<&str>,
    client_version: &str,
) -> Option<CompatibilityNotice> {
    let client = minor_line(client_version)?;
    match server_version.map(|v| (v, minor_line(v))) {
        Some((_, Some(server))) if server >= client => None,
        Some((v, Some(_))) => Some(CompatibilityNotice::OlderServer {
            url: base_url.to_string(),
            server_version: v.to_string(),
            client_version: client_version.to_string(),
        }),
        reported => Some(CompatibilityNotice::UnknownServerVersion {
            url: base_url.to_string(),
            reported: reported.map(|(v, _)| v.to_string()),
            client_version: client_version.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_backend::CompatibilityNotice;

    #[test]
    fn test_same_minor_line_with_older_server_patch_has_no_notice() {
        assert_eq!(
            compatibility_notice("http://host", Some("0.11.0"), "0.11.1"),
            None
        );
    }

    #[test]
    fn test_newer_server_minor_has_no_notice() {
        assert_eq!(
            compatibility_notice("http://host", Some("0.12.0"), "0.11.1"),
            None
        );
    }

    #[test]
    fn test_newer_server_major_has_no_notice() {
        assert_eq!(
            compatibility_notice("http://host", Some("1.0.0"), "0.11.1"),
            None
        );
    }

    #[test]
    fn test_prerelease_suffix_compares_on_major_minor_only() {
        assert_eq!(
            compatibility_notice("http://host", Some("0.11.0-rc.1"), "0.11.0"),
            None
        );
    }

    #[test]
    fn test_older_server_minor_yields_an_older_server_notice_with_both_versions() {
        assert_eq!(
            compatibility_notice("http://host:5177", Some("0.10.0"), "0.11.1"),
            Some(CompatibilityNotice::OlderServer {
                url: "http://host:5177".to_string(),
                server_version: "0.10.0".to_string(),
                client_version: "0.11.1".to_string(),
            })
        );
    }

    #[test]
    fn test_absent_server_version_yields_an_unknown_version_notice() {
        assert_eq!(
            compatibility_notice("http://host", None, "0.11.1"),
            Some(CompatibilityNotice::UnknownServerVersion {
                url: "http://host".to_string(),
                reported: None,
                client_version: "0.11.1".to_string(),
            })
        );
    }

    #[test]
    fn test_unparseable_server_version_yields_an_unknown_version_notice_carrying_the_raw_string() {
        assert_eq!(
            compatibility_notice("http://host", Some("banana"), "0.11.1"),
            Some(CompatibilityNotice::UnknownServerVersion {
                url: "http://host".to_string(),
                reported: Some("banana".to_string()),
                client_version: "0.11.1".to_string(),
            })
        );
    }

    #[test]
    fn test_this_binarys_kanban_version_parses_as_a_minor_line() {
        assert!(minor_line(kanban_core::KANBAN_VERSION).is_some());
    }
}
