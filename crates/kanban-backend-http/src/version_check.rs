use kanban_domain::{KanbanError, KanbanResult};

fn minor_line(version: &str) -> Option<(u64, u64)> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

pub(crate) fn check_server_version(
    base_url: &str,
    server_version: Option<&str>,
    client_version: &str,
) -> KanbanResult<()> {
    let compatible = match (
        server_version.and_then(minor_line),
        minor_line(client_version),
    ) {
        (Some(server), Some(client)) => server >= client,
        _ => false,
    };
    if compatible {
        return Ok(());
    }
    Err(KanbanError::UnsupportedServerVersion {
        url: base_url.to_string(),
        server_version: server_version.map(str::to_string),
        client_version: client_version.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_minor_line_with_older_server_patch_is_compatible() -> KanbanResult<()> {
        check_server_version("http://host", Some("0.11.0"), "0.11.1")
    }

    #[test]
    fn test_newer_server_minor_is_compatible() -> KanbanResult<()> {
        check_server_version("http://host", Some("0.12.0"), "0.11.1")
    }

    #[test]
    fn test_newer_server_major_is_compatible() -> KanbanResult<()> {
        check_server_version("http://host", Some("1.0.0"), "0.11.1")
    }

    #[test]
    fn test_older_server_minor_is_refused_with_both_versions() {
        let err = match check_server_version("http://host:5177", Some("0.10.0"), "0.11.1") {
            Ok(()) => panic!("expected the older server to be refused"),
            Err(e) => e,
        };
        match err {
            KanbanError::UnsupportedServerVersion {
                server_version: Some(ref v),
                ref client_version,
                ref url,
            } if v == "0.10.0" && client_version == "0.11.1" && url == "http://host:5177" => {}
            other => panic!("expected UnsupportedServerVersion, got {other:?}"),
        }
    }

    #[test]
    fn test_absent_server_version_is_refused() {
        let err = match check_server_version("http://host", None, "0.11.1") {
            Ok(()) => panic!("expected the missing version to be refused"),
            Err(e) => e,
        };
        match err {
            KanbanError::UnsupportedServerVersion {
                server_version: None,
                ..
            } => {}
            other => panic!("expected UnsupportedServerVersion, got {other:?}"),
        }
    }

    #[test]
    fn test_unparseable_server_version_is_refused() {
        let result = check_server_version("http://host", Some("banana"), "0.11.1");
        assert!(result.is_err());
    }

    #[test]
    fn test_prerelease_suffix_compares_on_major_minor_only() -> KanbanResult<()> {
        check_server_version("http://host", Some("0.11.0-rc.1"), "0.11.0")
    }

    #[test]
    fn test_this_binarys_kanban_version_parses_as_a_minor_line() -> KanbanResult<()> {
        check_server_version(
            "http://host",
            Some(kanban_core::KANBAN_VERSION),
            kanban_core::KANBAN_VERSION,
        )
    }
}
