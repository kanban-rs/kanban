use std::process::Command;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(has_git_commit)");
    println!("cargo::rustc-check-cfg=cfg(is_release_build)");
    println!("cargo::rerun-if-changed=../../.git/HEAD");
    println!("cargo::rerun-if-changed=../../.git/refs/heads/");
    println!("cargo::rerun-if-changed=../../.git/refs/tags/");
    println!("cargo::rerun-if-env-changed=GIT_COMMIT_HASH");

    let commit_hash = std::env::var("GIT_COMMIT_HASH")
        .ok()
        .filter(|s| !s.is_empty() && s != "unknown")
        .or_else(|| {
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
        })
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo::rustc-env=GIT_COMMIT_HASH={}", commit_hash);
    if commit_hash != "unknown" {
        println!("cargo::rustc-cfg=has_git_commit");
    }

    let is_release = Command::new("git")
        .args(["describe", "--tags", "--exact-match", "HEAD"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if is_release {
        println!("cargo::rustc-cfg=is_release_build");
    }
}
