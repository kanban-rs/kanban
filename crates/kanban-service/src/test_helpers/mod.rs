pub mod contract;
pub mod helpers;

pub type BackendFactory =
    Box<dyn Fn(&std::path::Path) -> std::sync::Arc<dyn crate::KanbanBackend> + Send + Sync>;

#[macro_export]
macro_rules! context_contract_tests {
    ($factory_fn:expr) => {
        // Board tests
        #[tokio::test(flavor = "multi_thread")]
        async fn test_board_basic_fields_roundtrip() {
            $crate::test_helpers::contract::board::test_board_basic_fields_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_board_update_all_optional_fields_roundtrip() {
            $crate::test_helpers::contract::board::test_board_update_all_optional_fields_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_board_sprint_names_roundtrip() {
            $crate::test_helpers::contract::board::test_board_sprint_names_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_board_card_counter_roundtrip() {
            $crate::test_helpers::contract::board::test_board_card_counter_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_board_next_sprint_number_roundtrip() {
            $crate::test_helpers::contract::board::test_board_next_sprint_number_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_update_card_status_done_lands_in_configured_column_not_last_column() {
            $crate::test_helpers::contract::board::test_update_card_status_done_lands_in_configured_column_not_last_column(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_move_card_into_configured_completion_column_keeps_status_done() {
            $crate::test_helpers::contract::board::test_move_card_into_configured_completion_column_keeps_status_done(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_delete_column_prunes_completion_configuration() {
            $crate::test_helpers::contract::board::test_delete_column_prunes_completion_configuration(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_undo_column_delete_restores_completion_membership() {
            $crate::test_helpers::contract::board::test_undo_column_delete_restores_completion_membership(&$factory_fn()).await;
        }
        // Column tests
        #[tokio::test(flavor = "multi_thread")]
        async fn test_column_all_fields_roundtrip() {
            $crate::test_helpers::contract::column::test_column_all_fields_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_column_without_wip_limit_roundtrip() {
            $crate::test_helpers::contract::column::test_column_without_wip_limit_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_multiple_columns_preserve_positions() {
            $crate::test_helpers::contract::column::test_multiple_columns_preserve_positions(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_column_default_status_roundtrip() {
            $crate::test_helpers::contract::column::test_column_default_status_roundtrip(&$factory_fn()).await;
        }

        // Sprint tests
        #[tokio::test(flavor = "multi_thread")]
        async fn test_sprint_planning_fields_roundtrip() {
            $crate::test_helpers::contract::sprint::test_sprint_planning_fields_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_sprint_active_fields_roundtrip() {
            $crate::test_helpers::contract::sprint::test_sprint_active_fields_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_sprint_completed_status_roundtrip() {
            $crate::test_helpers::contract::sprint::test_sprint_completed_status_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_sprint_cancelled_status_roundtrip() {
            $crate::test_helpers::contract::sprint::test_sprint_cancelled_status_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_sprint_with_card_prefix_override_roundtrip() {
            $crate::test_helpers::contract::sprint::test_sprint_with_card_prefix_override_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_sprint_no_prefix_uses_app_config_default() {
            $crate::test_helpers::contract::sprint::test_sprint_no_prefix_uses_app_config_default(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_sprint_board_prefix_overrides_app_config_default() {
            $crate::test_helpers::contract::sprint::test_sprint_board_prefix_overrides_app_config_default(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_sprint_explicit_prefix_overrides_all_defaults() {
            $crate::test_helpers::contract::sprint::test_sprint_explicit_prefix_overrides_all_defaults(&$factory_fn()).await;
        }

        // Card tests
        #[tokio::test(flavor = "multi_thread")]
        async fn test_card_all_fields_roundtrip() {
            $crate::test_helpers::contract::card::test_card_all_fields_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_card_minimal_fields_roundtrip() {
            $crate::test_helpers::contract::card::test_card_minimal_fields_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_card_all_priority_variants_roundtrip() {
            $crate::test_helpers::contract::card::test_card_all_priority_variants_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_card_all_status_variants_roundtrip() {
            $crate::test_helpers::contract::card::test_card_all_status_variants_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_card_completed_at_set_on_done_status() {
            $crate::test_helpers::contract::card::test_card_completed_at_set_on_done_status(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_column_filtered_reads_three_state() {
            $crate::test_helpers::contract::card::test_column_filtered_reads_three_state(&$factory_fn()).await;
        }

        // Sprint log tests
        #[tokio::test(flavor = "multi_thread")]
        async fn test_card_sprint_logs_roundtrip() {
            $crate::test_helpers::contract::sprint_log::test_card_sprint_logs_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_sprint_log_with_name_roundtrip() {
            $crate::test_helpers::contract::sprint_log::test_sprint_log_with_name_roundtrip(&$factory_fn()).await;
        }

        // Archive tests
        #[tokio::test(flavor = "multi_thread")]
        async fn test_archive_card_roundtrip() {
            $crate::test_helpers::contract::archive::test_archive_card_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_archive_card_with_sprint_logs_roundtrip() {
            $crate::test_helpers::contract::archive::test_archive_card_with_sprint_logs_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_restore_archived_card_roundtrip() {
            $crate::test_helpers::contract::archive::test_restore_archived_card_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_edit_archived_card_roundtrip() {
            $crate::test_helpers::contract::archive::test_edit_archived_card_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_list_cards_archived_selector_roundtrip() {
            $crate::test_helpers::contract::archive::test_list_cards_archived_selector_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_list_cards_archived_selector_board_scoped() {
            $crate::test_helpers::contract::archive::test_list_cards_archived_selector_board_scoped(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_clear_sprint_from_cards_leaves_archived_untouched() {
            $crate::test_helpers::contract::archive::test_clear_sprint_from_cards_leaves_archived_untouched(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_list_cards_archived_only_keeps_card_with_deleted_column() {
            $crate::test_helpers::contract::archive::test_list_cards_archived_only_keeps_card_with_deleted_column(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_list_cards_include_keeps_archived_card_with_deleted_column() {
            $crate::test_helpers::contract::archive::test_list_cards_include_keeps_archived_card_with_deleted_column(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_list_cards_archived_only_board_default_sort() {
            $crate::test_helpers::contract::archive::test_list_cards_archived_only_board_default_sort(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_list_cards_archived_only_explicit_override_wins() {
            $crate::test_helpers::contract::archive::test_list_cards_archived_only_explicit_override_wins(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_single_board_export_includes_archived_cards() {
            $crate::test_helpers::contract::archive::test_single_board_export_includes_archived_cards(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_single_board_export_roundtrips_archived_card() {
            $crate::test_helpers::contract::archive::test_single_board_export_roundtrips_archived_card(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_single_board_export_roundtrips_archived_board_marker() {
            $crate::test_helpers::contract::archive::test_single_board_export_roundtrips_archived_board_marker(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_delete_board_is_noop_on_archived_board() {
            $crate::test_helpers::contract::archive::test_delete_board_is_noop_on_archived_board(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_board_delete_undo_full_graph_roundtrip() {
            $crate::test_helpers::contract::archive::test_board_delete_undo_full_graph_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_board_archive_restore_full_graph_roundtrip() {
            $crate::test_helpers::contract::archive::test_board_archive_restore_full_graph_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_list_boards_archived_selector_roundtrip() {
            $crate::test_helpers::contract::archive::test_list_boards_archived_selector_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_list_boards_filtered_sorts_by_request_sort() {
            $crate::test_helpers::contract::archive::test_list_boards_filtered_sorts_by_request_sort(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_list_boards_filtered_order_desc_reverses() {
            $crate::test_helpers::contract::archive::test_list_boards_filtered_order_desc_reverses(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_list_boards_filtered_falls_back_to_config_default() {
            $crate::test_helpers::contract::archive::test_list_boards_filtered_falls_back_to_config_default(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_list_boards_no_config_no_request_is_position_order() {
            $crate::test_helpers::contract::archive::test_list_boards_no_config_no_request_is_position_order(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_list_boards_archived_only_default_is_recency() {
            $crate::test_helpers::contract::archive::test_list_boards_archived_only_default_is_recency(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_list_boards_live_default_is_position() {
            $crate::test_helpers::contract::archive::test_list_boards_live_default_is_position(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_list_boards_liveonly_does_not_fetch_archived_markers() {
            $crate::test_helpers::contract::archive::test_list_boards_liveonly_does_not_fetch_archived_markers(&$factory_fn()).await;
        }

        // LegacyEdge tests
        #[tokio::test(flavor = "multi_thread")]
        async fn test_blocks_edge_roundtrip() {
            $crate::test_helpers::contract::edge::test_blocks_edge_roundtrip(&$factory_fn()).await.unwrap();
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_relates_to_edge_roundtrip() {
            $crate::test_helpers::contract::edge::test_relates_to_edge_roundtrip(&$factory_fn()).await.unwrap();
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_parent_of_edge_roundtrip() {
            $crate::test_helpers::contract::edge::test_parent_of_edge_roundtrip(&$factory_fn()).await.unwrap();
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_archived_edge_roundtrip() {
            $crate::test_helpers::contract::edge::test_archived_edge_roundtrip(&$factory_fn()).await.unwrap();
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_multiple_edges_roundtrip() {
            $crate::test_helpers::contract::edge::test_multiple_edges_roundtrip(&$factory_fn()).await.unwrap();
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_empty_graph_roundtrip() {
            $crate::test_helpers::contract::edge::test_empty_graph_roundtrip(&$factory_fn()).await.unwrap();
        }

        // Movement tests
        #[tokio::test(flavor = "multi_thread")]
        async fn test_move_card_between_columns_roundtrip() {
            $crate::test_helpers::contract::movement::test_move_card_between_columns_roundtrip(&$factory_fn()).await;
        }

        // Lifecycle tests
        #[tokio::test(flavor = "multi_thread")]
        async fn test_multiple_boards_roundtrip() {
            $crate::test_helpers::contract::lifecycle::test_multiple_boards_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_incremental_save_preserves_prior_data() {
            $crate::test_helpers::contract::lifecycle::test_incremental_save_preserves_prior_data(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_delete_archived_card_roundtrip() {
            $crate::test_helpers::contract::lifecycle::test_delete_archived_card_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_delete_column_roundtrip() {
            $crate::test_helpers::contract::lifecycle::test_delete_column_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_delete_sprint_roundtrip() {
            $crate::test_helpers::contract::lifecycle::test_delete_sprint_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_full_populated_context_roundtrip() {
            $crate::test_helpers::contract::lifecycle::test_full_populated_context_roundtrip(&$factory_fn()).await.unwrap();
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_full_roundtrip_preserves_all_fields() {
            $crate::test_helpers::contract::lifecycle::test_full_roundtrip_preserves_all_fields(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_load_save_reload_roundtrip() {
            $crate::test_helpers::contract::lifecycle::test_load_save_reload_roundtrip(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_save_overwrites_correctly() {
            $crate::test_helpers::contract::lifecycle::test_save_overwrites_correctly(&$factory_fn()).await;
        }
        #[tokio::test(flavor = "multi_thread")]
        async fn test_reload_picks_up_external_changes() {
            $crate::test_helpers::contract::lifecycle::test_reload_picks_up_external_changes(&$factory_fn()).await;
        }
        // NOTE: `test_save_with_stale_metadata_returns_conflict` is intentionally
        // NOT in this shared macro. Optimistic-concurrency conflict detection is a
        // FILE-store feature (it versions on-disk metadata): the in-memory backend
        // has no persistence layer to conflict on, and the SQLite backend shares a
        // live DB connection rather than snapshot-versioning. It is invoked
        // directly for the JSON backend by the F4 registration test instead.
    };
}
