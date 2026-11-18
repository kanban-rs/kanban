use super::InMemoryStore;
use kanban_domain::{KanbanResult, Label};

impl InMemoryStore {
    pub(crate) fn get_label_impl(&self, name: &str) -> KanbanResult<Option<Label>> {
        Ok(self
            .read_state()?
            .labels
            .iter()
            .find(|l| l.name == name)
            .cloned())
    }

    pub(crate) fn list_labels_impl(&self) -> KanbanResult<Vec<Label>> {
        let mut labels = self.read_state()?.labels.clone();
        labels.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(labels)
    }

    pub(crate) fn upsert_label_impl(&self, label: Label) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        match state.labels.iter_mut().find(|l| l.name == label.name) {
            Some(existing) => *existing = label,
            None => state.labels.push(label),
        }
        Ok(())
    }

    pub(crate) fn delete_label_impl(&self, name: &str) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        state.labels.retain(|l| l.name != name);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::data_store::DataStore;

    #[test]
    fn test_label_roundtrip() {
        let store = InMemoryStore::new();
        let label = Label::new("bug", Some("#ff0000".to_string()), None::<String>);
        store.upsert_label(label.clone()).unwrap();

        assert_eq!(store.get_label("bug").unwrap(), Some(label));
        assert_eq!(store.list_labels().unwrap().len(), 1);
    }

    #[test]
    fn test_label_upsert_replaces() {
        let store = InMemoryStore::new();
        store.upsert_label(Label::new("bug", None, None)).unwrap();
        store
            .upsert_label(Label::new("bug", Some("#ff0000".to_string()), None))
            .unwrap();

        let label = store.get_label("bug").unwrap().unwrap();
        assert_eq!(label.color.as_deref(), Some("#ff0000"));
        assert_eq!(store.list_labels().unwrap().len(), 1);
    }

    #[test]
    fn test_label_delete() {
        let store = InMemoryStore::new();
        store.upsert_label(Label::named("bug")).unwrap();
        store.delete_label("bug").unwrap();
        assert_eq!(store.get_label("bug").unwrap(), None);
    }

    #[test]
    fn test_list_labels_sorted() {
        let store = InMemoryStore::new();
        store.upsert_label(Label::named("zeta")).unwrap();
        store.upsert_label(Label::named("alpha")).unwrap();
        store.upsert_label(Label::named("mid")).unwrap();

        let labels = store.list_labels().unwrap();
        let names: Vec<&str> = labels.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "mid", "zeta"]);
    }
}
