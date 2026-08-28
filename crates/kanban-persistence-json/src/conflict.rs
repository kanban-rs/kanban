pub use kanban_persistence::conflict::FileMetadata;

#[cfg(test)]
mod tests {
    #[test]
    fn test_file_metadata_is_defined_exactly_once_in_workspace() {
        let needle = ["pub struct ", "FileMetadata"].concat();
        let this_src = include_str!("conflict.rs");
        let detector_src = include_str!("../../kanban-persistence/src/conflict/detector.rs");
        let occurrences = [this_src, detector_src]
            .iter()
            .filter(|src| src.contains(&needle))
            .count();
        assert_eq!(
            occurrences, 1,
            "FileMetadata must be defined exactly once, in kanban-persistence; \
             kanban-persistence-json must import it rather than redefining it"
        );
    }
}
