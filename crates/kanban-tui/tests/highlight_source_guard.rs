use std::path::Path;

const FORBIDDEN: &[&str] = &["SELECTED_BG", "Color::Blue"];

fn drop_cfg_test_mod_declarations(src: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let mut kept = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let is_declaration = lines[i].trim() == "#[cfg(test)]"
            && lines.get(i + 1).is_some_and(|next| {
                let next = next.trim();
                next.ends_with(';') && (next.starts_with("mod ") || next.contains(" mod "))
            });
        if is_declaration {
            i += 2;
            continue;
        }
        kept.push(lines[i]);
        i += 1;
    }
    kept.join("\n")
}

fn violations_in(name: &str, src: &str) -> Vec<String> {
    let code =
        kanban_domain::capabilities::strip_test_modules(&drop_cfg_test_mod_declarations(src));
    FORBIDDEN
        .iter()
        .filter(|pattern| code.contains(*pattern))
        .map(|pattern| format!("{name}: {pattern}"))
        .collect()
}

fn collect_render_sources(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "theme") {
                continue;
            }
            collect_render_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let name = path.file_name().unwrap().to_string_lossy();
            if name.ends_with("_tests.rs") || name == "test_helpers.rs" {
                continue;
            }
            out.push((
                path.display().to_string(),
                std::fs::read_to_string(&path).unwrap(),
            ));
        }
    }
}

#[test]
fn test_no_render_source_hardcodes_the_selection_background() {
    let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src"));
    let mut sources = Vec::new();
    collect_render_sources(root, &mut sources);

    let mut violations = Vec::new();
    for (name, src) in &sources {
        violations.extend(violations_in(name, src));
    }

    assert!(
        violations.is_empty(),
        "non-theme render sources must not hard-code the selection background: {violations:#?}"
    );
}

#[test]
fn test_guard_flags_a_hardcoded_selection_background() {
    assert!(!violations_in("probe.rs", "fn f() { s.bg(Color::Blue); }").is_empty());
    assert!(!violations_in("probe.rs", "fn f() { s.bg(SELECTED_BG); }").is_empty());
    assert!(!violations_in("probe.rs", "fn f() { f(Color::Blue); }").is_empty());
}

#[test]
fn test_guard_ignores_test_modules() {
    let src = "fn f() {}\n#[cfg(test)]\nmod tests {\n    fn t() { s.bg(Color::Blue); }\n}\n";
    assert!(violations_in("probe.rs", src).is_empty());
}

#[test]
fn test_guard_scans_code_after_a_cfg_test_mod_declaration() {
    let src = "#[cfg(test)]\nmod dialog_tests;\nfn f() { s.bg(Color::Blue); }\n";
    assert!(!violations_in("probe.rs", src).is_empty());

    let src = "#[cfg(test)]\npub(crate) mod test_helpers;\nfn f() { s.bg(Color::Blue); }\n";
    assert!(!violations_in("probe.rs", src).is_empty());
}
