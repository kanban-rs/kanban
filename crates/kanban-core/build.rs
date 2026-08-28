use std::process::Command;

include!("build_support.rs");

fn main() {
    println!("cargo::rustc-check-cfg=cfg(has_git_commit)");
    println!("cargo::rustc-check-cfg=cfg(is_unstable_build)");
    println!("cargo::rerun-if-changed=../../.git/HEAD");
    println!("cargo::rerun-if-changed=../../.git/refs/heads/");
    println!("cargo::rerun-if-changed=../../.git/refs/tags/");
    println!("cargo::rerun-if-env-changed=GIT_COMMIT_HASH");

    let env_hash = std::env::var("GIT_COMMIT_HASH")
        .ok()
        .filter(|s| !s.is_empty() && s != "unknown");

    let (commit_hash, hash_source) = match env_hash {
        Some(hash) => (hash, HashSource::Env),
        None => match git_rev_parse_head() {
            Some(hash) => (hash, HashSource::GitRevParse),
            None => ("unknown".to_string(), HashSource::Unknown),
        },
    };

    println!("cargo::rustc-env=GIT_COMMIT_HASH={}", commit_hash);
    if commit_hash != "unknown" {
        println!("cargo::rustc-cfg=has_git_commit");
    }

    let is_release = git_describe_exact_tag();

    if is_unstable_build(hash_source, is_release) {
        println!("cargo::rustc-cfg=is_unstable_build");
    }
}

fn git_rev_parse_head() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
}

fn git_describe_exact_tag() -> bool {
    Command::new("git")
        .args(["describe", "--tags", "--exact-match", "HEAD"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
