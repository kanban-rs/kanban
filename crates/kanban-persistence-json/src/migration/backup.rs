//! Shared backup-path policy used by both the async `Migrator::migrate`
//! orchestrator and the sync `migrate_to_latest_sync` chain.
//!
//! The destructive V→MAX chain (per-step migrations plus `split_graph`,
//! `v6_to_v7_rename`, and `v7_to_v8_archived_cards`) runs against a
//! freshly-written file via the atomic temp+rename pattern. A pre-chain
//! `.v{N}.backup` is the user's rollback artifact if any step fails
//! mid-chain. The backup is taken before the first per-step migration
//! runs and removed only on full V→MAX success, so it covers the entire
//! chain from V1/V2/V3/V4/V5/V6/V7 all the way to the latest version.

use kanban_persistence::FormatVersion;
use std::path::{Path, PathBuf};

/// Return `Some(path.vN.backup)` for source versions that need a
/// pre-latest-chain backup; `None` for the current MAX version (no
/// migration needed).
pub(crate) fn pre_latest_backup_path_for(from: FormatVersion, path: &Path) -> Option<PathBuf> {
    match from {
        FormatVersion::V1 => Some(path.with_extension("v1.backup")),
        FormatVersion::V2 => Some(path.with_extension("v2.backup")),
        FormatVersion::V3 => Some(path.with_extension("v3.backup")),
        FormatVersion::V4 => Some(path.with_extension("v4.backup")),
        FormatVersion::V5 => Some(path.with_extension("v5.backup")),
        FormatVersion::V6 => Some(path.with_extension("v6.backup")),
        FormatVersion::V7 => Some(path.with_extension("v7.backup")),
        FormatVersion::V8 => Some(path.with_extension("v8.backup")),
        FormatVersion::V9 => Some(path.with_extension("v9.backup")),
        FormatVersion::V10 => Some(path.with_extension("v10.backup")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn p() -> PathBuf {
        PathBuf::from("/tmp/board.json")
    }

    #[test]
    fn returns_some_for_v1() {
        assert_eq!(
            pre_latest_backup_path_for(FormatVersion::V1, &p()),
            Some(PathBuf::from("/tmp/board.v1.backup"))
        );
    }

    #[test]
    fn returns_some_for_v2() {
        assert_eq!(
            pre_latest_backup_path_for(FormatVersion::V2, &p()),
            Some(PathBuf::from("/tmp/board.v2.backup"))
        );
    }

    #[test]
    fn returns_some_for_v3() {
        assert_eq!(
            pre_latest_backup_path_for(FormatVersion::V3, &p()),
            Some(PathBuf::from("/tmp/board.v3.backup"))
        );
    }

    #[test]
    fn returns_some_for_v4() {
        assert_eq!(
            pre_latest_backup_path_for(FormatVersion::V4, &p()),
            Some(PathBuf::from("/tmp/board.v4.backup"))
        );
    }

    #[test]
    fn returns_some_for_v5() {
        assert_eq!(
            pre_latest_backup_path_for(FormatVersion::V5, &p()),
            Some(PathBuf::from("/tmp/board.v5.backup"))
        );
    }

    #[test]
    fn returns_some_for_v6() {
        assert_eq!(
            pre_latest_backup_path_for(FormatVersion::V6, &p()),
            Some(PathBuf::from("/tmp/board.v6.backup"))
        );
    }

    #[test]
    fn returns_some_for_v7() {
        // V7 is now a migratable source (V7→V8 archived-cards backfill).
        assert_eq!(
            pre_latest_backup_path_for(FormatVersion::V7, &p()),
            Some(PathBuf::from("/tmp/board.v7.backup"))
        );
    }

    #[test]
    fn returns_some_for_v8() {
        // V8 is now a migratable source (V8→V9→V10).
        assert_eq!(
            pre_latest_backup_path_for(FormatVersion::V8, &p()),
            Some(PathBuf::from("/tmp/board.v8.backup"))
        );
    }

    #[test]
    fn returns_some_for_v9() {
        // V9 is now a migratable source (V9→V10 archival reference collapse).
        assert_eq!(
            pre_latest_backup_path_for(FormatVersion::V9, &p()),
            Some(PathBuf::from("/tmp/board.v9.backup"))
        );
    }

    #[test]
    fn returns_some_for_v10() {
        // V10 is now a migratable source (V10→V11 cards.board_id backfill).
        assert_eq!(
            pre_latest_backup_path_for(FormatVersion::V10, &p()),
            Some(PathBuf::from("/tmp/board.v10.backup"))
        );
    }

    #[test]
    fn returns_none_for_v11() {
        // V11→V11 is a no-op upstream; should never reach the chain.
        assert_eq!(pre_latest_backup_path_for(FormatVersion::V11, &p()), None);
    }
}
