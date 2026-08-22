use kanban_backend_http::HttpBackend;
use kanban_domain::{DataStore, KanbanError};
use uuid::Uuid;

fn unreachable_backend() -> HttpBackend {
    HttpBackend::new("http://127.0.0.1:1").unwrap()
}

fn assert_declines_under_its_own_name<T>(result: kanban_domain::KanbanResult<T>, expected: &str) {
    match result {
        Err(KanbanError::Unsupported { operation }) => {
            assert_eq!(operation, expected, "declined under the wrong name");
        }
        other => panic!("expected Unsupported({expected:?}), got {other:?}"),
    }
}

#[test]
fn test_get_card_by_board_and_number_declines_under_its_own_name() {
    let backend = unreachable_backend();
    let result = backend.get_card_by_board_and_number(Uuid::new_v4(), 1);
    assert_declines_under_its_own_name(result, "get_card_by_board_and_number");
}

#[test]
fn test_list_cards_by_number_declines_under_its_own_name() {
    let backend = unreachable_backend();
    let result = backend.list_cards_by_number(1);
    assert_declines_under_its_own_name(result, "list_cards_by_number");
}

#[test]
fn test_list_cards_by_prefix_and_number_declines_under_its_own_name() {
    let backend = unreachable_backend();
    let result = backend.list_cards_by_prefix_and_number("kan", 1);
    assert_declines_under_its_own_name(result, "list_cards_by_prefix_and_number");
}

#[test]
fn test_list_all_cards_and_siblings_stay_unsupported_under_their_own_names() {
    let backend = unreachable_backend();
    assert_declines_under_its_own_name(backend.list_all_cards(), "list_all_cards");
    assert_declines_under_its_own_name(backend.list_all_columns(), "list_all_columns");
    assert_declines_under_its_own_name(backend.list_all_sprints(), "list_all_sprints");
}
