pub const CAPABILITY_MANIFEST: &[&str] = &[
    "create_board",
    "list_boards",
    "get_board",
    "update_board",
    "delete_board",
    "create_card",
    "list_cards",
    "get_card",
    "update_card",
    "delete_card",
    "list_columns",
    "get_column",
    "update_column",
    "delete_column",
    "reorder_column",
    "create_sprint",
    "list_sprints",
    "get_sprint",
    "update_sprint",
    "delete_sprint",
    "undo",
    "redo",
];

pub fn strip_test_modules(src: &str) -> String {
    let mut kept = Vec::new();
    let mut skipping = false;
    for line in src.lines() {
        if skipping {
            if line == "}" {
                skipping = false;
            }
            continue;
        }
        if line.trim() == "#[cfg(test)]" {
            skipping = true;
            continue;
        }
        kept.push(line);
    }
    kept.join("\n")
}

fn contains_capability(src: &str, name: &str) -> bool {
    let bytes = src.as_bytes();
    for pattern in [format!("{name}("), format!("{name}_impl(")] {
        let plen = pattern.len();
        if plen > bytes.len() {
            continue;
        }
        for start in 0..=bytes.len() - plen {
            if &bytes[start..start + plen] == pattern.as_bytes() {
                let boundary_ok = start == 0 || !(bytes[start - 1] as char).is_ascii_alphanumeric();
                if boundary_ok {
                    return true;
                }
            }
        }
    }
    false
}

pub fn capability_violations(
    manifest: &[&str],
    wiring_sources: &[&str],
    declined: &[(&str, &str)],
) -> Vec<String> {
    let stripped: Vec<String> = wiring_sources
        .iter()
        .map(|s| strip_test_modules(s))
        .collect();
    let mut violations = Vec::new();
    for &name in manifest {
        let wired = stripped.iter().any(|s| contains_capability(s, name));
        let decline = declined.iter().find(|(n, _)| *n == name);
        match (wired, decline) {
            (true, Some(_)) => {
                violations.push(format!(
                    "{name}: declined but wired; remove the stale decline"
                ));
            }
            (false, Some((_, reason))) if reason.trim().is_empty() => {
                violations.push(format!("{name}: declined with an empty reason; state why"));
            }
            (false, None) => {
                violations.push(format!("{name}: not wired in any source and not declined"));
            }
            _ => {}
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_production_wiring_after_a_mid_file_cfg_test_module_is_still_seen() {
        let src = "fn a() {}\n\n#[cfg(test)]\nmod t {\n    #[test]\n    fn x() { create_board(); }\n}\n\nfn wired() { archive_board(); }\n";
        let violations = capability_violations(&["archive_board", "create_board"], &[src], &[]);
        assert!(violations.iter().any(|v| v.contains("create_board")));
        assert!(!violations.iter().any(|v| v.contains("archive_board")));
    }

    #[test]
    fn test_wiring_inside_trailing_cfg_test_module_is_ignored() {
        let src = "fn r() {}\n\n#[cfg(test)]\nmod t {\n    fn x() { delete_card(); }\n}\n";
        let violations = capability_violations(&["delete_card"], &[src], &[]);
        assert!(violations.iter().any(|v| v.contains("delete_card")));
    }

    #[test]
    fn test_unwired_manifest_entry_is_reported() {
        let violations = capability_violations(&["archive_board"], &["fn nothing() {}"], &[]);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("archive_board"));
    }

    #[test]
    fn test_wired_entry_with_bare_call_passes() {
        let violations = capability_violations(&["update_board"], &["c.update_board(id, u)"], &[]);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_wired_entry_with_impl_suffix_passes() {
        let violations = capability_violations(
            &["update_board"],
            &["mutate(|c| c.update_board_impl(id, u))"],
            &[],
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn test_wired_entry_behind_an_underscore_prefix_passes() {
        let violations = capability_violations(&["undo"], &["pub async fn tool_undo(&self)"], &[]);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_entry_name_does_not_match_pluralized_sibling() {
        let violations = capability_violations(&["update_card"], &["c.update_cards(v)"], &[]);
        assert!(violations.iter().any(|v| v.contains("update_card")));
    }

    #[test]
    fn test_entry_name_does_not_match_longer_identifier_prefix() {
        let violations = capability_violations(&["block"], &["c.unblock(a, b)"], &[]);
        assert!(violations.iter().any(|v| v.contains("block")));
    }

    #[test]
    fn test_declined_entry_with_reason_passes() {
        let violations = capability_violations(
            &["undo"],
            &["fn nothing() {}"],
            &[("undo", "sessions are shared across clients")],
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn test_declined_entry_with_empty_reason_is_reported() {
        let violations = capability_violations(&["undo"], &["fn nothing() {}"], &[("undo", "")]);
        assert!(violations.iter().any(|v| v.contains("undo")));
    }

    #[test]
    fn test_declined_but_wired_entry_is_reported() {
        let violations =
            capability_violations(&["undo"], &["c.undo()"], &[("undo", "sessions are shared")]);
        assert!(violations.iter().any(|v| v.contains("undo")));
    }
}
