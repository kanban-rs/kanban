use super::*;

impl Model {
    pub fn cards_state(&self) -> &LoadState<Vec<Card>> {
        todo!()
    }

    pub fn card_by_id_state(&self, _id: Uuid) -> LoadState<&Card> {
        todo!()
    }

    pub fn archived_card_markers(&self) -> &[ArchivedCard] {
        todo!()
    }

    pub fn archived_card_ids(&self) -> &std::collections::HashSet<Uuid> {
        todo!()
    }
}
