#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_rev_parse_hash_without_release_tag_is_unstable() {
        assert!(is_unstable_build(HashSource::GitRevParse, false));
    }

    #[test]
    fn test_git_rev_parse_hash_with_release_tag_is_stable() {
        assert!(!is_unstable_build(HashSource::GitRevParse, true));
    }

    #[test]
    fn test_env_injected_hash_is_never_unstable() {
        assert!(!is_unstable_build(HashSource::Env, false));
        assert!(!is_unstable_build(HashSource::Env, true));
    }

    #[test]
    fn test_unknown_hash_is_never_unstable() {
        assert!(!is_unstable_build(HashSource::Unknown, false));
    }

    #[test]
    fn test_injected_commit_hash_without_git_does_not_report_unstable() {
        assert!(!is_unstable_build(HashSource::Env, false));
    }
}
