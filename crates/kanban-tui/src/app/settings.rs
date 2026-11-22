//! The Configuration settings panel, modelled as an identity-addressed list of
//! row descriptors.
//!
//! ## Why this module exists
//!
//! Before this model the panel's row order, per-mode visibility and
//! selectability were encoded as four independent, hand-synchronised arrays of
//! integer indices: the `is_config_selected(0..=9)` literals in the render
//! (`ui/settings_view.rs`), the `settings_item_count` arms
//! (`handlers/settings_handlers.rs`), the `BACKGROUND_HIGHLIGHT_CONFIG_INDEX =>
//! set(8)` nav special-cases, and the hardcoded help block. Adding a row meant
//! editing every one of those, in the right per-mode order, or the panel would
//! silently mis-highlight or mis-navigate.
//!
//! The replacement is a single source of truth: an ordered list of
//! [`ConfigRow`] descriptors, each carrying a stable identity ([`ConfigRowId`]),
//! its displayed label and value, its non-selectable styling (when any), and a
//! `selectable` flag. Everything else -- the highlighted row, the navigation
//! bounds, and the added `search.cardlist.jumpto` row -- is *derived* from that
//! list by identity, so adding a row is additive and cannot desynchronise the
//! other call sites.
//!
//! The authoritative specification for this model is
//! `tests/settings_config_panel_descriptor_tests.rs`.

use crate::app::App;
use ratatui::style::Color;

/// The stable identity of a row in the Configuration settings panel.
///
/// `Hashing` by identity (rather than by a positional usize) is what makes the
/// index-alignment class of bug impossible: the render, the nav and the help
/// block all ask "which row is `BackgroundHighlight`?" and derive, rather than
/// each hardcode, an answer that can silently drift.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConfigRowId {
    /// `Configuration Format` -- the TOML/JSON wire format of the config file.
    ConfigurationFormat,
    /// `Configuration Location` -- the resolved path of the config file.
    ConfigurationLocation,
    /// `Default Card Prefix` -- e.g. `kan-`.
    CardPrefix,
    /// `Default Sprint Prefix` -- e.g. `sprint-`.
    SprintPrefix,
    /// `Editing Format` -- the external `$EDITOR` the config editor opens in.
    EditingFormat,
    /// `Background Highlight` -- the swatch row; the one row with a non-`label: value`
    /// layout (a preview swatch is appended) which is why the render special-cases its
    /// [`ConfigRow`] into [`crate::ui::settings_view::render_settings_configuration`].
    BackgroundHighlight,
    /// `Storage Backend` -- the configured (editable) backend. Selectable.
    StorageBackend,
    /// `Storage Location` -- the configured (editable) location. Selectable.
    StorageLocation,
    /// `Active Storage Backend` -- the backend in effect this session. Selectable.
    ActiveStorageBackend,
    /// `Active Storage Location` -- the location in effect this session. Selectable.
    ActiveStorageLocation,
    /// `Search Jumpto` -- the `search.cardlist.jumpto` preference (KAN-1806).
    /// Rendered as the value, or `off` when the key is absent. Selectable.
    SearchJumpto,
}

/// One row of the Configuration settings panel.
///
/// `selectable` is the single source of truth for whether a row participates
/// in `j`/`k` navigation; `settings_item_count` / `config_panel_selectable_count`
/// is derived by counting the `selectable` rows, so the nav bound can never
/// over- or under-count a row that merely renders.
#[derive(Clone, Debug)]
pub struct ConfigRow {
    /// Stable identity of the row.
    pub id: ConfigRowId,
    /// The rendered label, e.g. `Configuration Format`.
    pub label: String,
    /// The rendered value, e.g. `toml`, `off`, `current`.
    pub value: String,
    /// Whether the row participates in navigation and gets the selected
    /// highlight. The override-mode `Storage` rows render (their value is
    /// visible) but are not selectable, so they appear in the list with
    /// `selectable = false`.
    pub selectable: bool,
    /// The non-selectable row's value style, when the render needs one. For
    /// selectable rows this is ignored (the selection highlight determines the
    /// style). The override-mode `Storage` rows use `DarkGray` text.
    pub non_selectable_value_color: Option<Color>,
    /// The special `label: value [swatch]` layout used by `BackgroundHighlight`
    /// only. When `false` the row renders as a plain `label: value` line.
    pub background_highlight_swatch: bool,
}

impl App {
    /// The ordered row descriptors for a settings panel, for the current
    /// `has_data_file` / `cli_file_*` / `original_storage_*` state.
    ///
    /// This is **the** source of truth for the panel's composition. The render
    /// and the navigation both read it; `settings_item_count` for
    /// [`crate::app::focus::SettingsFocus::Configuration`] and
    /// [`App::config_panel_selectable_count`] are both just
    /// `config_panel_rows(...).iter().filter(selectable).count()`.
    pub fn config_panel_rows(&self, panel: crate::app::focus::SettingsFocus) -> Vec<ConfigRow> {
        if panel != crate::app::focus::SettingsFocus::Configuration {
            return Vec::new();
        }

        let rows = config_rows_base(self);

        // The base rows (0..=5) are present in *every* mode, in a fixed order.
        let mut out = Vec::with_capacity(rows.len() + 4);
        out.extend(rows);

        // The storage rows are appended after the base rows, and which pair (or
        // pair plus styled non-selectable rows) appear depends on the mode --
        // exactly the matrix the old if/else chain in render_settings_configuration
        // encoded. Reusing the same predicate here keeps them in lockstep.
        if self.has_data_file {
            if self.cli_file_override {
                let storage_configured = self.original_storage_backend.is_some()
                    || self.original_storage_location.is_some();
                if storage_configured {
                    // The configured Storage rows render (so the user can *see* the
                    // CLI-supplied values) but are NOT selectable: the effective
                    // storage this session is the Active pair. This is the row that
                    // historically made `settings_item_count` over-count.
                    out.push(
                        ConfigRowId::StorageBackend
                            .styled_row(&self.config_storage_backend, Some(Color::DarkGray)),
                    );
                    out.push(
                        ConfigRowId::StorageLocation
                            .styled_row(&self.config_storage_location, Some(Color::DarkGray)),
                    );
                }
                out.push(
                    ConfigRowId::ActiveStorageBackend
                        .selectable_row(self.app_config.effective_storage_backend().to_string()),
                );
                out.push(ConfigRowId::ActiveStorageLocation.selectable_row(
                    kanban_service::config::resolve_storage_location(&self.app_config),
                ));
            } else if self.cli_file_provided {
                out.push(
                    ConfigRowId::ActiveStorageBackend
                        .selectable_row(self.app_config.effective_storage_backend().to_string()),
                );
                out.push(ConfigRowId::ActiveStorageLocation.selectable_row(
                    kanban_service::config::resolve_storage_location(&self.app_config),
                ));
            } else {
                out.push(
                    ConfigRowId::StorageBackend
                        .selectable_row(self.app_config.effective_storage_backend().to_string()),
                );
                out.push(ConfigRowId::StorageLocation.selectable_row(
                    kanban_service::config::resolve_storage_location(&self.app_config),
                ));
            }
        }

        // The new search.cardlist.jumpto row (KAN-1806) is present in every mode,
        // appended after the storage rows so it never reorders a mode's existing
        // indices -- and its value is derived from AppConfig, not a literal.
        out.push(self.search_jumpto_row());
        out
    }

    /// The `search.cardlist.jumpto` descriptor, present in every mode.
    fn search_jumpto_row(&self) -> ConfigRow {
        let value = self
            .app_config
            .search_cardlist_jumpto()
            .map(str::to_string)
            .unwrap_or_else(|| "off".to_string());
        ConfigRowId::SearchJumpto.selectable_row(value)
    }

    /// The Configuration panel's *selectable* row count -- the navigation
    /// bound. Derived from the descriptor list, so it is the number of rows
    /// the cursor can actually land on (never a row that merely renders).
    ///
    /// `App::settings_item_count` for the Configuration panel is this value;
    /// the `ConfigFile` and `Storage` panels keep their independent counts.
    pub fn config_panel_selectable_count(&self, panel: crate::app::focus::SettingsFocus) -> usize {
        if panel != crate::app::focus::SettingsFocus::Configuration {
            return 0;
        }
        self.config_panel_rows(panel)
            .iter()
            .filter(|r| r.selectable)
            .count()
    }
}

impl ConfigRowId {
    const fn label(&self) -> &'static str {
        match self {
            Self::ConfigurationFormat => "Configuration Format",
            Self::ConfigurationLocation => "Configuration Location",
            Self::CardPrefix => "Default Card Prefix",
            Self::SprintPrefix => "Default Sprint Prefix",
            Self::EditingFormat => "Editing Format",
            Self::BackgroundHighlight => "Background Highlight",
            Self::StorageBackend => "Storage Backend",
            Self::StorageLocation => "Storage Location",
            Self::ActiveStorageBackend => "Active Storage Backend",
            Self::ActiveStorageLocation => "Active Storage Location",
            Self::SearchJumpto => "Search Jumpto",
        }
    }
}

impl ConfigRow {
    fn for_id(id: ConfigRowId, value: String) -> Self {
        Self {
            id,
            label: id.label().to_string(),
            value,
            selectable: true,
            non_selectable_value_color: None,
            background_highlight_swatch: id == ConfigRowId::BackgroundHighlight,
        }
    }
}

impl ConfigRowId {
    fn selectable_row(&self, value: String) -> ConfigRow {
        ConfigRow::for_id(*self, value)
    }

    fn styled_row(&self, value: &str, color: Option<Color>) -> ConfigRow {
        let mut row = ConfigRow::for_id(*self, value.to_string());
        row.selectable = false;
        row.non_selectable_value_color = color;
        row
    }
}

/// The base, mode-independent rows (the six currently hardcoded at indices
/// 0..=5), in their fixed order. Extracted as a free function so the render
/// and the nav both build from the *same* list rather than each re-deriving the
/// six rows independently.
fn config_rows_base(app: &App) -> Vec<ConfigRow> {
    let cfg = &app.app_config;
    let location = kanban_service::config::effective_configuration_location(cfg);
    let mut rows = vec![
        ConfigRowId::ConfigurationFormat
            .selectable_row(cfg.effective_configuration_format().to_string()),
        ConfigRowId::ConfigurationLocation.selectable_row(location),
        ConfigRowId::CardPrefix.selectable_row(cfg.effective_default_card_prefix().to_string()),
        ConfigRowId::SprintPrefix.selectable_row(cfg.effective_default_sprint_prefix().to_string()),
        ConfigRowId::EditingFormat.selectable_row(cfg.effective_editing_format().to_string()),
    ];
    // Background Highlight is always selectable and always 6th (index 5); its
    // value/swatch is resolved at render time by the special-case in
    // render_settings_configuration, so here the value is a placeholder that the
    // render overrides with the swatch line.
    let highlight_value = cfg
        .background_highlight_color()
        .map(|c| c.to_string())
        .unwrap_or_else(|| format!("default ({})", crate::theme::DEFAULT_HIGHLIGHT_HEX));
    rows.push(ConfigRowId::BackgroundHighlight.selectable_row(highlight_value));
    rows
}
