use std::fmt;

const CONSEQUENCES: &str = "Writes it has no route for fail with an upgrade message, and \
     rules added in newer releases may not be enforced. Upgrade the server to clear this warning.";

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompatibilityNotice {
    OlderServer {
        url: String,
        server_version: String,
        client_version: String,
    },
    /// `reported` is `None` when `/health` has no `version` (every server
    /// before v0.11.0), `Some` when it has one this client cannot parse.
    UnknownServerVersion {
        url: String,
        reported: Option<String>,
        client_version: String,
    },
}

impl fmt::Display for CompatibilityNotice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OlderServer {
                url,
                server_version,
                client_version,
            } => write!(
                f,
                "kanban server at {url} is v{server_version}, older than this client \
                 (v{client_version}). {CONSEQUENCES}"
            ),
            Self::UnknownServerVersion {
                url,
                reported: None,
                client_version,
            } => write!(
                f,
                "kanban server at {url} reports no version, so it predates this client \
                 (v{client_version}). {CONSEQUENCES}"
            ),
            Self::UnknownServerVersion {
                url,
                reported: Some(v),
                client_version,
            } => write!(
                f,
                "kanban server at {url} reports version '{v}', which this client \
                 (v{client_version}) cannot compare. {CONSEQUENCES}"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_older_server_notice_names_url_both_versions_and_the_upgrade() {
        let notice = CompatibilityNotice::OlderServer {
            url: "http://host:5177".to_string(),
            server_version: "0.10.0".to_string(),
            client_version: "0.11.0".to_string(),
        };
        let msg = notice.to_string();
        assert!(msg.contains("http://host:5177"), "msg: {msg}");
        assert!(msg.contains("v0.10.0"), "msg: {msg}");
        assert!(msg.contains("v0.11.0"), "msg: {msg}");
        assert!(msg.contains("Upgrade the server"), "msg: {msg}");
    }

    #[test]
    fn test_unknown_version_notice_without_a_version_says_the_server_reports_none() {
        let notice = CompatibilityNotice::UnknownServerVersion {
            url: "http://host:5177".to_string(),
            reported: None,
            client_version: "0.11.0".to_string(),
        };
        let msg = notice.to_string();
        assert!(msg.contains("reports no version"), "msg: {msg}");
        assert!(msg.contains("v0.11.0"), "msg: {msg}");
    }

    #[test]
    fn test_unknown_version_notice_with_an_unparseable_version_quotes_it() {
        let notice = CompatibilityNotice::UnknownServerVersion {
            url: "http://host:5177".to_string(),
            reported: Some("banana".to_string()),
            client_version: "0.11.0".to_string(),
        };
        let msg = notice.to_string();
        assert!(msg.contains("'banana'"), "msg: {msg}");
    }
}
