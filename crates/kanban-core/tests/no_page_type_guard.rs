use std::path::Path;

#[test]
fn test_no_type_named_page_exists_in_kanban_core() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();

    for entry in walk_rs_files(&src_dir) {
        let contents = std::fs::read_to_string(&entry).expect("readable source file");
        for (line_no, line) in contents.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("struct Page")
                || trimmed.starts_with("pub struct Page")
                || trimmed.starts_with("enum Page")
                || trimmed.starts_with("pub enum Page")
                || trimmed.starts_with("type Page")
                || trimmed.starts_with("pub type Page")
            {
                offenders.push(format!("{}:{}: {}", entry.display(), line_no + 1, trimmed));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "found type(s) named `Page` in kanban-core, which is reserved for kanban-api's wire pagination envelope:\n{}",
        offenders.join("\n")
    );
}

fn walk_rs_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current).expect("readable src directory") {
            let entry = entry.expect("readable dir entry");
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    files
}
