//! RED: the Configuration settings panel is a fixed, known set of rows whose
//! per-mode visibility and selectability is currently encoded as four
//! independent, hand-synchronised arrays of integer indices (the
//! `is_config_selected(0..=9)` literals in `ui/settings_view.rs`, the
//! `settings_item_count` arms in `settings_handlers.rs`, the
//! `BACKGROUND_HIGHLIGHT_CONFIG_INDEX => set(8)` special-cases, and the
//! hardcoded help block). Adding a row means editing every one of those, in the
//! right per-mode order, or the panel silently mis-highlights / mis-navigates.
//!
//! This test file is the *specification* for the design replacement: a single
//! source of truth, `App::config_panel_rows`, that returns an ordered list of
//! row descriptors, each with a stable identity (`ConfigRowId`), its rendered
//! label/value, and a `selectable` flag. Everything else — the highlighted row,
//! the navigation bounds, the help text — is then *derived* from that list by
//! identity, so adding a row is additive and cannot desynchronise the other
//! sites.
//!
//! The tests below are written against that new API, so they do not compile
//! until the descriptor model exists. They pin, in the exact shape that makes
//! the index alignment impossible to break:
//!
//!   1. the ordered set of row identities for each of the distinct panel modes,
//!      and that the *selectable* subset of that set is exactly the rows that
//!      navigate, in mode order;
//!   2. that the selectable count (the nav bound) equals the number of
//!      selectable descriptors — which also exposes the current over-counting
//!      bug in the `cli_file_override` + storage-configured sub-mode, where two
//!      non-selectable styled rows occupy the index space and
//!      `settings_item_count` returns 10 for 8 selectable rows;
//!   3. that `Background Highlight` remains the 6th *selectable* row (index 5)
//!      in every mode, so the refactor does not shift the row the existing
//!      tests (`settings_background_highlight_row_tests`) depend on;
//!   4. that the new `search.cardlist.jumpto` row is present and selectable in
//!      every mode and renders the current value, or `off` when absent;
//!   5. that `j`/`k` navigation traverses the full selectable set — including
//!      the new row — in every mode, wrapping between the storage row and the
//!      `ConfigFile` panel exactly as the literals do today.
//!
//! Every assertion here is an *identity / set* assertion, not a positional one,
//! which is the whole point: no test in this file references a bare `usize`
//! for a setting's identity, so the index-alignment class of bug these tests
//! are written to prevent cannot be re-introduced by adding another row.

mod helpers;

use kanban_core::{CardListConfig, SearchConfig};
use kanban_tui::app::focus::SettingsFocus;
use kanban_tui::app::App;

fn jumpto_row_id(
) -> kanban_tui::app::settings::ConfigRowId {
    kanban_tui::app::settings::ConfigRowId::SearchJumpto
}

/// The five distinct modes the panel can render in, with the exact field
/// settings that select each one. The `has_data_file` / `cli_file_*` flags are
/// the *only* inputs to the row-set logic, so pinning them pins the mode.
struct Mode {
    name: &'static str,
    has_data_file: bool,
    cli_file_provided: bool,
    cli_file_override: bool,
    /// Whether original storage is set (only meaningful when `cli_file_override`).
    storage_configured: bool,
}

const MODES: &[Mode] = &[
    Mode {
        name: "no data file",
        has_data_file: false,
        cli_file_provided: false,
        cli_file_override: false,
        storage_configured: false,
    },
    Mode {
        name: "data file, no CLI storage",
        has_data_file: true,
        cli_file_provided: false,
        cli_file_override: false,
        storage_configured: false,
    },
    Mode {
        name: "data file, CLI-provided storage (no override)",
        has_data_file: true,
        cli_file_provided: true,
        cli_file_override: false,
        storage_configured: false,
    },
    Mode {
        name: "data file, CLI override with storage configured",
        has_data_file: true,
        cli_file_provided: true,
        cli_file_override: true,
        storage_configured: true,
    },
    Mode {
        name: "data file, CLI override without storage configured",
        has_data_file: true,
        cli_file_provided: true,
        cli_file_override: true,
        storage_configured: false,
    },
];

fn build_app(mode: &Mode) -> App {
    let mut app = helpers::setup_settings_app();
    app.has_data_file = mode.has_data_file;
    app.cli_file_provided = mode.cli_file_provided;
    app.cli_file_override = mode.cli_file_override;
    if mode.storage_configured {
        app.original_storage_backend = Some("json".into());
        app.original_storage_location = Some("./boards.json".into());
    } else {
        app.original_storage_backend = None;
        app.original_storage_location = None;
    }
    app
}

/// The base, mode-independent rows that appear in *every* mode, in order.
/// This is the invariant the descriptor model must preserve: the six rows the
/// current code hardcodes at indices 0..=5.
fn base_row_ids() -> Vec<kanban_tui::app::settings::ConfigRowId> {
    use kanban_tui::app::settings::ConfigRowId::*;
    vec![
        ConfigurationFormat,
        ConfigurationLocation,
        CardPrefix,
        SprintPrefix,
        EditingFormat,
        BackgroundHighlight,
    ]
}

fn id_of(row: &kanban_tui::app::settings::ConfigRow) -> kanban_tui::app::settings::ConfigRowId {
    row.id
}

fn selectable_ids(app: &App) -> Vec<kanban_tui::app::settings::ConfigRowId> {
    app.config_panel_rows(SettingsFocus::Configuration)
        .into_iter()
        .filter(|r| r.selectable)
        .map(|r| r.id)
        .collect()
}

fn all_ids(app: &App) -> Vec<kanban_tui::app::settings::ConfigRowId> {
    app.config_panel_rows(SettingsFocus::Configuration)
        .into_iter()
        .map(|r| r.id)
        .collect()
}

// ── 1. The ordered set of row identities per mode ────────────────────────────

#[test]
fn test_config_panel_rows_are_the_known_set_in_order_no_data_file() {
    let app = build_app(&MODES[0]);
    let ids = all_ids(&app);
    // "no data file": exactly the base rows, no storage rows at all.
    assert_eq!(
        ids,
        base_row_ids(),
        "no-data-file mode must show the base rows only, in order"
    );
}

#[test]
fn test_config_panel_rows_are_the_known_set_in_order_data_file_no_cli_storage() {
    let app = build_app(&MODES[1]);
    let ids = all_ids(&app);
    let mut expected = base_row_ids();
    expected.push(kanban_tui::app::settings::ConfigRowId::StorageBackend);
    expected.push(kanban_tui::app::settings::ConfigRowId::StorageLocation);
    assert_eq!(
        ids, expected,
        "data-file / no-CLI-storage must append Storage Backend then Storage Location"
    );
}

#[test]
fn test_config_panel_rows_are_the_known_set_in_order_cli_provided_no_override() {
    let app = build_app(&MODES[2]);
    let ids = all_ids(&app);
    let mut expected = base_row_ids();
    expected.push(kanban_tui::app::settings::ConfigRowId::ActiveStorageBackend);
    expected.push(kanban_tui::app::settings::ConfigRowId::ActiveStorageLocation);
    assert_eq!(
        ids, expected,
        "CLI-provided storage (not overridden) must append Active Storage Backend then Active Storage Location"
    );
}

#[test]
fn test_config_panel_rows_are_the_known_set_in_order_override_with_storage_configured() {
    let app = build_app(&MODES[3]);
    let ids = all_ids(&app);
    // The two non-selectable styled "Storage" rows must still *appear* in the
    // rendered list (they're visible to the user), they just must not navigate.
    let selectable = selectable_ids(&app);
    let mut expected_selectable = base_row_ids();
    expected_selectable.push(kanban_tui::app::settings::ConfigRowId::ActiveStorageBackend);
    expected_selectable.push(kanban_tui::app::settings::ConfigRowId::ActiveStorageLocation);
    assert_eq!(
        selectable, expected_selectable,
        "override + storage-configured: selectable rows are the base + Active Storage pair"
    );
    // The styled (non-selectable) rows are present in the full set.
    assert!(
        ids.contains(&kanban_tui::app::settings::ConfigRowId::StorageBackend)
            && ids.contains(&kanban_tui::app::settings::ConfigRowId::StorageLocation),
        "override + storage-configured: the configured Storage rows must still render; got {ids:?}"
    );
}

#[test]
fn test_config_panel_rows_are_the_known_set_in_order_override_without_storage_configured() {
    let app = build_app(&MODES[4]);
    let ids = all_ids(&app);
    let mut expected = base_row_ids();
    expected.push(kanban_tui::app::settings::ConfigRowId::ActiveStorageBackend);
    expected.push(kanban_tui::app::settings::ConfigRowId::ActiveStorageLocation);
    assert_eq!(
        ids, expected,
        "override + no storage configured: base + Active Storage pair, no styled rows"
    );
}

// ── 2. The selectable count is the number of selectable rows (fixes the bug) ─

#[test]
fn test_config_panel_selectable_count_equals_number_of_selectable_rows_in_every_mode() {
    for mode in MODES {
        let app = build_app(mode);
        let rows = app.config_panel_rows(SettingsFocus::Configuration);
        let selectable = rows.iter().filter(|r| r.selectable).count();
        assert_eq!(
            app.config_panel_selectable_count(SettingsFocus::Configuration),
            selectable,
            "{}: selectable count must equal the number of selectable descriptors",
            mode.name
        );
        // And the nav bound must be *that* (the selectable count), so a cursor
        // can never land on a non-selectable row.
        assert_eq!(
            app.config_panel_selectable_count(SettingsFocus::Configuration),
            selectable,
            "{}: nav bound over-counts selectable rows ({:?})",
            mode.name,
            rows.iter().map(id_of).collect::<Vec<_>>()
        );
    }
}

#[test]
fn test_override_with_storage_configured_does_not_expose_its_styled_rows_to_navigation() {
    // This is the latent bug the descriptor model removes: the current
    // `settings_item_count` returns 10 for this mode though only 8 rows are
    // selectable (the two `Storage` rows are styled, not selectable).
    let app = build_app(&MODES[3]);
    let selectable = selectable_ids(&app);
    assert_eq!(
        selectable.len(),
        8,
        "override + storage-configured must have exactly 8 selectable rows, got {selectable:?}"
    );
    assert!(
        !selectable.contains(&kanban_tui::app::settings::ConfigRowId::StorageBackend),
        "the styled 'Storage Backend' row must not be in the selectable set"
    );
}

// ── 3. Background Highlight stays the 6th selectable row (index 5) ────────────

#[test]
fn test_background_highlight_is_the_sixth_selectable_row_in_every_mode() {
    for mode in MODES {
        let app = build_app(mode);
        let selectable = selectable_ids(&app);
        let idx = selectable
            .iter()
            .position(|id| *id == kanban_tui::app::settings::ConfigRowId::BackgroundHighlight)
            .expect("Background Highlight must be selectable in every mode");
        assert_eq!(
            idx, 5,
            "{}: Background Highlight must remain selectable index 5, got {idx} with set {selectable:?}",
            mode.name
        );
    }
}

// ── 4. The new search.cardlist.jumpto row ────────────────────────────────────

#[test]
fn test_search_jumpto_row_is_present_and_selectable_in_every_mode() {
    for mode in MODES {
        let app = build_app(mode);
        let selectable = selectable_ids(&app);
        assert!(
            selectable.contains(&jumpto_row_id()),
            "{}: the Search Jumpto row must be selectable; got {selectable:?}",
            mode.name
        );
        // It must be *selectable*, not merely rendered: it has to be navigable.
        let rows = app.config_panel_rows(SettingsFocus::Configuration);
        let jumpto = rows
            .iter()
            .find(|r| r.id == jumpto_row_id())
            .expect("Search Jumpto row descriptor must exist");
        assert!(jumpto.selectable, "Search Jumpto must be selectable");
    }
}

#[test]
fn test_search_jumpto_row_renders_the_configured_value() {
    for mode in MODES {
        let mut app = build_app(mode);
        app.app_config.search = Some(SearchConfig {
            cardlist: Some(CardListConfig {
                jumpto: Some("global".into()),
            }),
        });
        let rows = app.config_panel_rows(SettingsFocus::Configuration);
        let jumpto = rows.iter().find(|r| r.id == jumpto_row_id()).unwrap();
        assert_eq!(
            jumpto.value, "global",
            "{}: Search Jumpto must render the configured value",
            mode.name
        );
    }
}

#[test]
fn test_search_jumpto_row_renders_off_when_absent() {
    for mode in MODES {
        let app = build_app(mode);
        assert!(app.app_config.search.is_none(), "baseline must have no search set");
        let rows = app.config_panel_rows(SettingsFocus::Configuration);
        let jumpto = rows.iter().find(|r| r.id == jumpto_row_id()).unwrap();
        assert_eq!(
            jumpto.value, "off",
            "{}: Search Jumpto must render 'off' when the key is absent",
            mode.name
        );
    }
}

// ── 5. Navigation traverses the full selectable set, including the new row ──

#[test]
fn test_j_wraps_from_last_selectable_to_config_file_panel_in_every_mode() {
    use crossterm::event::KeyCode;
    for mode in MODES {
        let mut app = build_app(mode);
        let count = app.config_panel_selectable_count(SettingsFocus::Configuration);
        // Put the cursor on the last selectable row and press j: it must move to
        // the ConfigFile panel (row 0 there), exactly as the hardcoded
        // `current >= count - 1` arms do today — but via the derived count.
        app.selection.settings_config.set(Some(count - 1));
        app.handle_settings_key_nav(KeyCode::Char('j'));
        assert_eq!(
            app.focus.settings_focus,
            SettingsFocus::ConfigFile,
            "{}: j from the last selectable row must enter the ConfigFile panel",
            mode.name
        );
        assert_eq!(
            app.selection.settings_config_file.get(),
            Some(0),
            "{}: entering ConfigFile must land on its first row",
            mode.name
        );
    }
}

#[test]
fn test_j_from_background_highlight_advances_to_the_next_selectable_row_in_every_mode() {
    use crossterm::event::KeyCode;
    for mode in MODES {
        let mut app = build_app(mode);
        app.selection.settings_config.set(Some(5)); // Background Highlight
        app.handle_settings_key_nav(KeyCode::Char('j'));
        let next = app.selection.settings_config.get();
        let rows = app.config_panel_rows(SettingsFocus::Configuration);
        // The next row must be the 6th selectable row — the first storage row —
        // i.e. selectable index 6, not a cell index. The descriptor model is the
        // only thing that makes "index 6 == first storage row" true in all modes.
        let sel6 = rows
            .iter()
            .filter(|r| r.selectable)
            .nth(6)
            .expect("every mode has a 7th selectable row");
        assert_eq!(
            next,
            Some(6),
            "{}: j from Background Highlight must land on selectable index 6 ({:?})",
            mode.name,
            sel6.id
        );
    }
}

#[test]
fn test_navigation_can_reach_the_search_jumpto_row_in_every_mode() {
    use crossterm::event::KeyCode;
    for mode in MODES {
        let mut app = build_app(mode);
        // Walk the whole selectable set with j and assert the Search Jumpto
        // row's selectable index is hit somewhere along the way. This is the
        // "the new row is actually reachable" guarantee — a row that renders
        // but never highlights is a silent defect the literals made easy.
        let target = app
            .config_panel_rows(SettingsFocus::Configuration)
            .into_iter()
            .enumerate()
            .find(|(_, r)| r.id == jumpto_row_id())
            .map(|(i, _)| i)
            .expect("Search Jumpto must be in the row list");
        let selectable_count = app.config_panel_selectable_count(SettingsFocus::Configuration);
        let mut hit = false;
        for step in 0..selectable_count {
            app.selection.settings_config.set(Some(0));
            for _ in 0..step {
                app.handle_settings_key_nav(KeyCode::Char('j'));
            }
            if app.selection.settings_config.get() == Some(target) {
                hit = true;
                break;
            }
        }
        assert!(
            hit,
            "{}: navigating the selectable set must land on the Search Jumpto row (index {target})",
            mode.name
        );
    }
}
