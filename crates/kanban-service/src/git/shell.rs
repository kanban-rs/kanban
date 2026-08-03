use std::path::PathBuf;
use std::process::Command;

use chrono::{DateTime, Utc};
use kanban_domain::KanbanResult;

use super::provider::{CommitRef, GitProvider};

const FIELD_SEP: char = '\u{1f}';
const LOG_FORMAT: &str = "--format=%h%x1f%s%x1f%an%x1f%cI";

/// `GitProvider` that shells out to a local `git` binary against a checkout.
pub struct ShellGitProvider {
    repo_path: PathBuf,
}

impl ShellGitProvider {
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }
}

impl GitProvider for ShellGitProvider {
    fn commits_for_tag(&self, tag: &str) -> KanbanResult<Vec<CommitRef>> {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.repo_path)
            .args(["log", "--all", "--fixed-strings"])
            .arg(format!("--grep={tag}"))
            .arg(LOG_FORMAT)
            .output();

        let output = match output {
            Ok(output) if output.status.success() => output,
            // git missing (spawn error) or non-zero exit (not a repo / bad
            // invocation): the card-detail view degrades to "No linked commits".
            _ => return Ok(Vec::new()),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout
            .lines()
            .filter_map(parse_line)
            .filter(|c| subject_has_tag_token(&c.subject, tag))
            .collect())
    }
}

/// `--grep` is a substring match, so a coarse `git log --grep=KAN-5` also
/// returns KAN-50/KAN-512/etc. Re-check the match as a bounded token: neither
/// side of the matched substring may be alphanumeric, so `KAN-5` only matches
/// `KAN-5` itself, not a longer number sharing the same prefix.
fn subject_has_tag_token(subject: &str, tag: &str) -> bool {
    let mut search_from = 0;
    while let Some(offset) = subject[search_from..].find(tag) {
        let start = search_from + offset;
        let end = start + tag.len();
        let before_ok = subject[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_ascii_alphanumeric());
        let after_ok = subject[end..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_ascii_alphanumeric());
        if before_ok && after_ok {
            return true;
        }
        search_from = start + 1;
    }
    false
}

fn parse_line(line: &str) -> Option<CommitRef> {
    let mut fields = line.split(FIELD_SEP);
    let short_hash = fields.next()?;
    let subject = fields.next()?;
    let author = fields.next()?;
    let committed_at = fields.next()?;
    if fields.next().is_some() {
        return None;
    }
    let committed_at = DateTime::parse_from_rfc3339(committed_at)
        .ok()?
        .with_timezone(&Utc);
    Some(CommitRef {
        short_hash: short_hash.to_string(),
        subject: subject.to_string(),
        author: author.to_string(),
        committed_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::path::Path;
    use std::process::Command;
    use tempfile::TempDir;

    fn run_git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .expect("git must be on PATH for git-module tests");
        assert!(status.success(), "git {args:?} failed");
    }

    fn init_repo() -> TempDir {
        let dir = TempDir::new().expect("tempdir");
        run_git(dir.path(), &["init", "--quiet"]);
        run_git(dir.path(), &["config", "user.email", "test@example.com"]);
        run_git(dir.path(), &["config", "user.name", "Test User"]);
        dir
    }

    fn commit(dir: &Path, subject: &str) {
        run_git(
            dir,
            &[
                "commit",
                "--allow-empty",
                "--quiet",
                "--author=Test User <test@example.com>",
                "-m",
                subject,
            ],
        );
    }

    #[test]
    fn test_commits_for_tag_returns_matching_commit() {
        let repo = init_repo();
        commit(repo.path(), "KAN-5 do a thing");
        let provider = ShellGitProvider::new(repo.path().to_path_buf());

        let commits = provider.commits_for_tag("KAN-5").expect("ok");

        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].subject, "KAN-5 do a thing");
    }

    #[test]
    fn test_commits_for_tag_returns_empty_when_no_match() {
        let repo = init_repo();
        commit(repo.path(), "KAN-5 do a thing");
        let provider = ShellGitProvider::new(repo.path().to_path_buf());

        let commits = provider.commits_for_tag("KAN-999").expect("ok");

        assert!(commits.is_empty());
    }

    #[test]
    fn test_commits_for_tag_returns_empty_for_non_repo() {
        let dir = TempDir::new().expect("tempdir");
        let provider = ShellGitProvider::new(dir.path().to_path_buf());

        let commits = provider.commits_for_tag("KAN-5").expect("graceful");

        assert!(commits.is_empty());
    }

    #[test]
    fn test_parses_multiple_commits_newest_first() {
        let repo = init_repo();
        commit(repo.path(), "KAN-7 first");
        commit(repo.path(), "KAN-7 second");
        let provider = ShellGitProvider::new(repo.path().to_path_buf());

        let commits = provider.commits_for_tag("KAN-7").expect("ok");

        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].subject, "KAN-7 second");
        assert_eq!(commits[1].subject, "KAN-7 first");
    }

    #[test]
    fn test_commits_for_tag_excludes_numeric_prefix_collisions() {
        let repo = init_repo();
        commit(repo.path(), "KAN-5 the real card five");
        commit(repo.path(), "KAN-50 a different card");
        commit(repo.path(), "KAN-512 another one");
        let provider = ShellGitProvider::new(repo.path().to_path_buf());

        let commits = provider.commits_for_tag("KAN-5").expect("ok");

        assert_eq!(
            commits.len(),
            1,
            "KAN-5 must not match KAN-50/KAN-512 by substring: got {commits:?}"
        );
        assert_eq!(commits[0].subject, "KAN-5 the real card five");
    }

    #[test]
    fn test_commit_fields_parsed() {
        let repo = init_repo();
        commit(repo.path(), "KAN-8 all fields");
        let provider = ShellGitProvider::new(repo.path().to_path_buf());

        let commits = provider.commits_for_tag("KAN-8").expect("ok");
        let c = &commits[0];

        assert_eq!(c.short_hash.len(), 7);
        assert!(c.short_hash.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert_eq!(c.subject, "KAN-8 all fields");
        assert_eq!(c.author, "Test User");
        assert!(c.committed_at <= Utc::now());
    }
}
