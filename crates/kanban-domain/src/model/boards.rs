use super::*;

impl Model {
    pub fn boards_state(&self) -> &LoadState<Vec<Board>> {
        todo!()
    }

    pub fn live_boards(&self) -> impl Iterator<Item = &Board> {
        std::iter::empty()
    }

    pub fn archived_boards(&self) -> &[ArchivedBoard] {
        todo!()
    }

    pub fn archived_board_ids(&self) -> &HashSet<Uuid> {
        todo!()
    }

    pub fn board_by_id_state(&self, _id: Uuid) -> LoadState<&Board> {
        todo!()
    }
}
