use kanban_core::{
    AppConfig, CoreResult, Editable, DEFAULT_JSON_FILENAME, DEFAULT_SQLITE_FILENAME,
    DEFAULT_STORAGE_BACKEND, DEFAULT_SERVER_ADDR,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub fn config_path() -> Option<PathBuf> {
    // Explicit override: full path to the config file. Honored on every platform
    // (dirs::config_dir() can't be redirected by env on Windows), which lets CI
    // and power users point at a custom location.
    if let Some(p) = std::env::var_os("KANBAN_CONFIG") {
        return Some(PathBuf::from(p));
    }
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|home| home.join(".config/kanban/config.toml"))
    }
    #[cfg(target_os = "linux")]
    {
        dirs::config_dir().map(|config| config.join("kanban/config.toml"))
    }
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir().map(|config| config.join("kanban\\config.toml"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

pub fn load() -> AppConfig {
    if let Some(path) = config_path() {
        load_from(&path)
    } else {
        AppConfig::default()
    }
}

pub fn load_from(path: &Path) -> AppConfig {
    if path.exists() {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                match ext {
                    "json" => match serde_json::from_str(&content) {
                        Ok(config) => return config,
                        Err(e) => {
                            tracing::warn!("Failed to parse config {}: {}", path.display(), e)
                        }
                    },
                    _ => match toml::from_str(&content) {
                        Ok(config) => return config,
                        Err(e) => {
                            tracing::warn!("Failed to parse config {}: {}", path.display(), e)
                        }
                    },
                }
            }
            Err(e) => {
                tracing::warn!("Failed to read config {}: {}", path.display(), e);
            }
        }
    }
    AppConfig::default()
}

pub fn save(config: &AppConfig) -> CoreResult<()> {
    let path_str = effective_configuration_location(config);
    if path_str.is_empty() {
        return Err(kanban_core::CoreError::Config(
            "No configuration location configured".to_string(),
        ));
    }
    let path = PathBuf::from(path_str);
    save_to(config, &path)
}

pub fn save_to(config: &AppConfig, path: &Path) -> CoreResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            kanban_core::CoreError::Config(format!("Failed to create config directory: {}", e))
        })?;
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let content = match ext {
        "json" => serde_json::to_string_pretty(config).map_err(|e| {
            kanban_core::CoreError::Config(format!("Failed to serialize config: {}", e))
        })?,
        _ => toml::to_string_pretty(config).map_err(|e| {
            kanban_core::CoreError::Config(format!("Failed to serialize config: {}", e))
        })?,
    };
    write_config_file(path, &content)
        .map_err(|e| kanban_core::CoreError::Config(format!("Failed to write config: {}", e)))?;
    Ok(())
}

pub fn move_config(old_path: &Path, new_path: &Path) -> CoreResult<()> {
    if old_path == new_path || !old_path.exists() {
        return Ok(());
    }
    if let Some(parent) = new_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            kanban_core::CoreError::Config(format!("Failed to create config directory: {}", e))
        })?;
    }
    if std::fs::rename(old_path, new_path).is_err() {
        std::fs::copy(old_path, new_path)
            .map_err(|e| kanban_core::CoreError::Config(format!("Failed to copy config: {}", e)))?;
        if let Err(e) = std::fs::remove_file(old_path) {
            let _ = std::fs::remove_file(new_path);
            return Err(kanban_core::CoreError::Config(format!(
                "Failed to remove old config '{}' after copy: {}",
                old_path.display(),
                e
            )));
        }
    }
    Ok(())
}

pub fn effective_configuration_location(config: &AppConfig) -> String {
    config.configuration_location.clone().unwrap_or_else(|| {
        config_path()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    })
}

/// Resolves `AppConfig::effective_storage_location` to an absolute path.
///
/// `kanban-core` is a pure data crate and returns relative paths as-is.
/// This function performs the cwd join for callers that need a filesystem path.
pub fn resolve_storage_location(config: &AppConfig) -> String {
    let raw = config.effective_storage_location();
    let path = Path::new(&raw);
    if path.is_absolute() {
        raw
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path).display().to_string())
            .unwrap_or(raw)
    }
}

/// The address kanban-server binds. Returns the configured value or the
/// compile-time default; unlike storage_location there is no cwd join.
pub fn resolve_server_addr(config: &AppConfig) -> String {
    config.effective_server_addr().to_string()
}

pub fn validate(config: &AppConfig) -> CoreResult<()> {
    config.validate_values()?;
    if let Some(ref v) = config.storage_location {
        if std::path::Path::new(v)
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        {
            return Err(kanban_core::CoreError::Validation(
                "storage_location cannot contain '..'".to_string(),
            ));
        }
        let path = std::path::Path::new(v);
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        };
        if let Some(parent) = resolved.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                return Err(kanban_core::CoreError::Validation(format!(
                    "Invalid storage_location '{}': parent directory does not exist",
                    v
                )));
            }
        }
    }
    Ok(())
}

pub fn has_non_default_values(config: &AppConfig) -> bool {
    !is_all_defaults(config)
}

/// Returns `true` only when the config carries no information worth persisting.
///
/// # Storage field invariant
///
/// When *both* `storage_backend` and `storage_location` are `Some`, this function
/// always returns `false`, even if both hold default values (e.g. `"json"` /
/// `"boards.json"`).  The reasoning:
///
/// - For an *existing* file, the backend is detected from the file via
///   `sync_backend_with_file` and then stored as an explicit `Some` so that
///   the config accurately describes the on-disk reality.
/// - For a *new* file, the user's chosen `storage_backend` determines the
///   backend and must be persisted so the next startup uses the same backend.
///
/// In both cases the pair of explicit `Some` values is meaningful and must not
/// be discarded as if it were identical to the absence of config.
fn is_all_defaults(config: &AppConfig) -> bool {
    let all_none = config.default_card_prefix.is_none()
        && config.default_sprint_prefix.is_none()
        && config.storage_backend.is_none()
        && config.editing_format.is_none()
        && config.configuration_format.is_none()
        && config.configuration_location.is_none()
        && config.storage_location.is_none()
        && config.board_sort_field.is_none()
        && config.board_sort_order.is_none()
        && config.server_addr.is_none();

    if all_none {
        return true;
    }

    if config.storage_backend.is_some() && config.storage_location.is_some() {
        return false;
    }

    config
        .default_card_prefix
        .as_deref()
        .is_none_or(|v| v == "task")
        && config
            .default_sprint_prefix
            .as_deref()
            .is_none_or(|v| v == "sprint")
        && config
            .storage_backend
            .as_deref()
            .is_none_or(|v| v == DEFAULT_STORAGE_BACKEND)
        && config.editing_format.as_deref().is_none_or(|v| v == "json")
        && config
            .configuration_format
            .as_deref()
            .is_none_or(|v| v == "toml")
        && config.configuration_location.as_deref().is_none_or(|loc| {
            config_path().map(|p| p.display().to_string()).as_deref() == Some(loc)
        })
        && config.storage_location.as_deref().is_none_or(|loc| {
            let default = match config.effective_storage_backend() {
                "sqlite" => DEFAULT_SQLITE_FILENAME,
                _ => DEFAULT_JSON_FILENAME,
            };
            loc == default
        })
        && config.board_sort_field.is_none()
        && config.board_sort_order.is_none()
        && config.server_addr.as_deref().is_none_or(|v| v == DEFAULT_SERVER_ADDR)
}

/// Removes fields whose values are equal to the compile-time defaults so that
/// the persisted config file stays minimal.
///
/// # Storage field invariant
///
/// `storage_location` is only stripped when `storage_backend` was *not*
/// explicitly set (i.e. `had_explicit_backend == false`).  When the backend
/// is explicit, both fields were set intentionally (see [`is_all_defaults`])
/// and must be preserved together even if their values match the defaults.
pub fn strip_defaults(config: &mut AppConfig) {
    let had_explicit_backend = config.storage_backend.is_some();
    if config.default_card_prefix.as_deref() == Some("task") {
        config.default_card_prefix = None;
    }
    if config.default_sprint_prefix.as_deref() == Some("sprint") {
        config.default_sprint_prefix = None;
    }
    if config.storage_backend.as_deref() == Some(DEFAULT_STORAGE_BACKEND) {
        config.storage_backend = None;
    }
    if config.editing_format.as_deref() == Some("json") {
        config.editing_format = None;
    }
    if config.configuration_format.as_deref() == Some("toml") {
        config.configuration_format = None;
    }
    if let Some(ref loc) = config.configuration_location {
        if config_path().map(|p| p.display().to_string()).as_deref() == Some(loc.as_str()) {
            config.configuration_location = None;
        }
    }
    if !had_explicit_backend {
        if let Some(ref loc) = config.storage_location {
            let default = match config.effective_storage_backend() {
                "sqlite" => DEFAULT_SQLITE_FILENAME,
                _ => DEFAULT_JSON_FILENAME,
            };
            if loc == default {
                config.storage_location = None;
            }
        }
    }
    if config.server_addr.as_deref() == Some(DEFAULT_SERVER_ADDR) {
        config.server_addr = None;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfigDto {
    pub configuration_format: Option<String>,
    pub configuration_location: Option<String>,
    pub default_card_prefix: Option<String>,
    pub default_sprint_prefix: Option<String>,
    pub editing_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_backend: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_location: Option<String>,
}

impl AppConfigDto {
    pub fn from_config(entity: &AppConfig, has_data_file: bool) -> Self {
        let (storage_backend, storage_location) = if has_data_file {
            (
                Some(entity.effective_storage_backend().to_string()),
                Some(resolve_storage_location(entity)),
            )
        } else {
            (None, None)
        };
        Self {
            default_card_prefix: Some(entity.effective_default_card_prefix().to_string()),
            default_sprint_prefix: Some(entity.effective_default_sprint_prefix().to_string()),
            storage_backend,
            editing_format: Some(entity.effective_editing_format().to_string()),
            configuration_format: Some(entity.effective_configuration_format().to_string()),
            configuration_location: Some(effective_configuration_location(entity)),
            storage_location,
        }
    }

    pub fn validate_and_apply(self, entity: &mut AppConfig) -> CoreResult<()> {
        self.apply_to(entity);
        validate(entity)?;
        Ok(())
    }
}

impl Editable<AppConfig> for AppConfigDto {
    /// Returns a DTO with storage fields hidden (suitable for display without a
    /// known data file). Use [`AppConfigDto::from_config`] with `has_data_file: true`
    /// when storage fields must be shown.
    fn from_entity(entity: &AppConfig) -> Self {
        Self::from_config(entity, false)
    }

    /// Applies DTO values to the config and strips default values to keep the
    /// persisted config minimal. Callers that need un-stripped values (e.g. for
    /// diffing) should copy the config before calling this method.
    fn apply_to(self, entity: &mut AppConfig) {
        let old_format = entity.effective_configuration_format().to_string();
        entity.default_card_prefix = self.default_card_prefix;
        entity.default_sprint_prefix = self.default_sprint_prefix;
        if let Some(backend) = self.storage_backend {
            entity.storage_backend = Some(backend);
        }
        entity.editing_format = self.editing_format;
        entity.configuration_format = self.configuration_format;
        entity.configuration_location = self.configuration_location;
        if let Some(location) = self.storage_location {
            entity.storage_location = Some(location);
        }

        let new_format = entity.effective_configuration_format();
        if old_format != new_format {
            let new_ext = new_format.to_string();
            let location = effective_configuration_location(entity);
            if let Some((stem, _)) = location.rsplit_once('.') {
                entity.configuration_location = Some(format!("{}.{}", stem, new_ext));
            }
        }

        strip_defaults(entity);
    }
}

#[cfg(unix)]
fn write_config_file(path: &Path, content: &str) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    std::io::Write::write_all(&mut tmp, content.as_bytes())?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_config_file(path: &Path, content: &str) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    std::io::Write::write_all(&mut tmp, content.as_bytes())?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

pub fn try_load_from(path: &Path) -> kanban_core::CoreResult<AppConfig> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| kanban_core::CoreError::Config(format!("Failed to read config: {}", e)))?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "json" => serde_json::from_str(&content).map_err(|e| {
            kanban_core::CoreError::Config(format!("Failed to parse JSON config: {}", e))
        }),
        _ => toml::from_str(&content).map_err(|e| {
            kanban_core::CoreError::Config(format!("Failed to parse TOML config: {}", e))
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    // --- Fix 1: file permissions (Red phase — fails before write_config_file is wired in) ---

    #[cfg(unix)]
    #[test]
    fn test_save_to_creates_file_with_restricted_permissions() {
        use std::os::unix::fs::MetadataExt;

        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let config = AppConfig::default();
        save_to(&config, &path).unwrap();

        let mode = std::fs::metadata(&path).unwrap().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "config file should be owner-read/write only (0o600)"
        );
    }

    // --- Fix 2: try_load_from error surfacing (Red phase — fails before try_load_from exists) ---

    #[test]
    fn test_try_load_from_missing_file_returns_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.toml");
        let result = try_load_from(&path);
        assert!(result.is_err(), "missing file should return Err");
    }

    #[test]
    fn test_try_load_from_malformed_toml_returns_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "this is not valid toml {{{").unwrap();
        let result = try_load_from(&path);
        assert!(result.is_err(), "malformed TOML should return Err");
        assert!(
            result.unwrap_err().to_string().contains("parse"),
            "error should mention parsing"
        );
    }

    #[test]
    fn test_try_load_from_malformed_json_returns_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, "{not valid json").unwrap();
        let result = try_load_from(&path);
        assert!(result.is_err(), "malformed JSON should return Err");
        assert!(
            result.unwrap_err().to_string().contains("parse"),
            "error should mention parsing"
        );
    }

    #[test]
    fn test_try_load_from_valid_toml_returns_config() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "default_card_prefix = \"feat\"").unwrap();
        let config = try_load_from(&path).unwrap();
        assert_eq!(config.default_card_prefix.as_deref(), Some("feat"));
    }

    #[test]
    fn test_try_load_from_valid_json_returns_config() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"default_card_prefix":"feat"}"#).unwrap();
        let config = try_load_from(&path).unwrap();
        assert_eq!(config.default_card_prefix.as_deref(), Some("feat"));
    }

    #[test]
    fn test_load_missing_new_fields_defaults() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "default_card_prefix = \"feat\"").unwrap();

        let config = load_from(&path);
        assert_eq!(config.default_card_prefix.as_deref(), Some("feat"));
        assert!(config.storage_backend.is_none());
        assert!(config.editing_format.is_none());
        assert!(config.default_sprint_prefix.is_none());
        assert!(config.configuration_format.is_none());
        assert!(config.configuration_location.is_none());
    }

    #[test]
    fn test_load_legacy_field_aliases() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "default_branch_prefix = \"feat\"").unwrap();
        writeln!(f, "default_db_mode = \"sqlite\"").unwrap();
        writeln!(f, "default_format = \"toml\"").unwrap();

        let config = load_from(&path);
        assert_eq!(config.default_card_prefix.as_deref(), Some("feat"));
        assert_eq!(config.storage_backend.as_deref(), Some("sqlite"));
        assert_eq!(config.editing_format.as_deref(), Some("toml"));
    }

    #[test]
    fn test_effective_configuration_location_defaults_to_config_path() {
        let config = AppConfig::default();
        let expected = config_path()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        assert_eq!(effective_configuration_location(&config), expected);
    }

    #[test]
    fn test_effective_configuration_location_custom() {
        let config = AppConfig {
            configuration_location: Some("/tmp/my_config.toml".into()),
            ..Default::default()
        };
        assert_eq!(
            effective_configuration_location(&config),
            "/tmp/my_config.toml"
        );
    }

    #[test]
    fn test_save_and_load_round_trip_toml() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");

        let config = AppConfig {
            default_card_prefix: Some("feature".into()),
            default_sprint_prefix: Some("iteration".into()),
            storage_backend: Some("sqlite".into()),
            editing_format: Some("json".into()),
            configuration_format: Some("toml".into()),
            configuration_location: Some("/tmp/test.toml".into()),
            ..Default::default()
        };
        save_to(&config, &path).unwrap();

        let loaded = load_from(&path);
        assert_eq!(loaded.default_card_prefix.as_deref(), Some("feature"));
        assert_eq!(loaded.default_sprint_prefix.as_deref(), Some("iteration"));
        assert_eq!(loaded.storage_backend.as_deref(), Some("sqlite"));
        assert_eq!(loaded.editing_format.as_deref(), Some("json"));
        assert_eq!(loaded.configuration_format.as_deref(), Some("toml"));
        assert_eq!(
            loaded.configuration_location.as_deref(),
            Some("/tmp/test.toml")
        );
    }

    #[test]
    fn test_save_and_load_round_trip_json() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");

        let config = AppConfig {
            default_card_prefix: Some("feature".into()),
            default_sprint_prefix: Some("iteration".into()),
            storage_backend: Some("sqlite".into()),
            editing_format: Some("json".into()),
            configuration_format: Some("json".into()),
            configuration_location: Some("/tmp/test.json".into()),
            ..Default::default()
        };
        save_to(&config, &path).unwrap();

        let loaded = load_from(&path);
        assert_eq!(loaded.default_card_prefix.as_deref(), Some("feature"));
        assert_eq!(loaded.default_sprint_prefix.as_deref(), Some("iteration"));
        assert_eq!(loaded.storage_backend.as_deref(), Some("sqlite"));
        assert_eq!(loaded.editing_format.as_deref(), Some("json"));
        assert_eq!(loaded.configuration_format.as_deref(), Some("json"));
        assert_eq!(
            loaded.configuration_location.as_deref(),
            Some("/tmp/test.json")
        );
    }

    #[test]
    fn test_save_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested").join("dirs").join("config.toml");

        let config = AppConfig::default();
        save_to(&config, &path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_app_config_dto_round_trip() {
        let config = AppConfig {
            default_card_prefix: Some("sprint".into()),
            default_sprint_prefix: Some("iter".into()),
            storage_backend: Some("sqlite".into()),
            editing_format: Some("json".into()),
            configuration_format: Some("toml".into()),
            configuration_location: Some("/tmp/test.toml".into()),
            ..Default::default()
        };

        let dto = AppConfigDto::from_config(&config, true);
        let mut target = AppConfig::default();
        dto.apply_to(&mut target);

        assert_eq!(target.default_card_prefix.as_deref(), Some("sprint"));
        assert_eq!(target.default_sprint_prefix.as_deref(), Some("iter"));
        assert_eq!(target.storage_backend.as_deref(), Some("sqlite"));
        assert_eq!(
            target.configuration_location.as_deref(),
            Some("/tmp/test.toml")
        );
        assert!(target.editing_format.is_none());
        assert!(target.configuration_format.is_none());
        assert_eq!(target.effective_editing_format(), "json");
        assert_eq!(target.effective_configuration_format(), "toml");
    }

    #[test]
    fn test_app_config_dto_serialization_has_expected_keys() {
        let dto = AppConfigDto {
            default_card_prefix: Some("sprint".into()),
            default_sprint_prefix: Some("iter".into()),
            storage_backend: Some("json".into()),
            editing_format: Some("json".into()),
            configuration_format: Some("toml".into()),
            configuration_location: Some("/tmp/test.toml".into()),
            storage_location: None,
        };
        let serialized = toml::to_string(&dto).unwrap();
        assert!(serialized.contains("default_card_prefix"));
        assert!(serialized.contains("default_sprint_prefix"));
        assert!(serialized.contains("storage_backend"));
        assert!(serialized.contains("editing_format"));
        assert!(serialized.contains("configuration_format"));
        assert!(serialized.contains("configuration_location"));
    }

    #[test]
    fn test_auto_sync_extension_on_format_change() {
        let mut config = AppConfig {
            configuration_format: Some("toml".into()),
            configuration_location: Some("/home/user/.config/kanban/config.toml".into()),
            ..Default::default()
        };

        let dto = AppConfigDto {
            default_card_prefix: None,
            default_sprint_prefix: None,
            storage_backend: None,
            editing_format: None,
            configuration_format: Some("json".into()),
            configuration_location: Some("/home/user/.config/kanban/config.toml".into()),
            storage_location: None,
        };
        dto.apply_to(&mut config);

        assert_eq!(
            config.configuration_location.as_deref(),
            Some("/home/user/.config/kanban/config.json")
        );
    }

    #[test]
    fn test_dto_from_default_config_hides_storage_fields() {
        let config = AppConfig::default();
        let dto = AppConfigDto::from_entity(&config);
        assert_eq!(dto.default_card_prefix.as_deref(), Some("task"));
        assert_eq!(dto.default_sprint_prefix.as_deref(), Some("sprint"));
        assert!(dto.storage_backend.is_none());
        assert!(dto.storage_location.is_none());
        assert_eq!(dto.editing_format.as_deref(), Some("json"));
        assert_eq!(dto.configuration_format.as_deref(), Some("toml"));
        assert!(dto.configuration_location.is_some());
    }

    #[test]
    fn test_dto_from_config_with_data_file_shows_storage_fields() {
        let config = AppConfig::default();
        let dto = AppConfigDto::from_config(&config, true);
        assert_eq!(dto.storage_backend.as_deref(), Some("json"));
        assert!(
            Path::new(dto.storage_location.as_deref().unwrap()).ends_with("boards.json"),
            "got: {:?}",
            dto.storage_location
        );
    }

    #[test]
    fn test_dto_from_explicit_config_preserves_values() {
        let config = AppConfig {
            default_card_prefix: Some("feat".into()),
            default_sprint_prefix: Some("iter".into()),
            storage_backend: Some("sqlite".into()),
            editing_format: Some("toml".into()),
            configuration_format: Some("json".into()),
            configuration_location: Some("/custom/path.json".into()),
            ..Default::default()
        };
        let dto = AppConfigDto::from_config(&config, true);
        assert_eq!(dto.default_card_prefix.as_deref(), Some("feat"));
        assert_eq!(dto.default_sprint_prefix.as_deref(), Some("iter"));
        assert_eq!(dto.storage_backend.as_deref(), Some("sqlite"));
        assert!(
            Path::new(dto.storage_location.as_deref().unwrap()).ends_with("boards.sqlite"),
            "got: {:?}",
            dto.storage_location
        );
        assert_eq!(dto.editing_format.as_deref(), Some("toml"));
        assert_eq!(dto.configuration_format.as_deref(), Some("json"));
        assert_eq!(
            dto.configuration_location.as_deref(),
            Some("/custom/path.json")
        );
    }

    #[test]
    fn test_move_config_renames_file() {
        let dir = TempDir::new().unwrap();
        let old = dir.path().join("old.toml");
        let new = dir.path().join("new.toml");
        std::fs::write(&old, "test = true").unwrap();

        move_config(&old, &new).unwrap();
        assert!(!old.exists());
        assert!(new.exists());
        assert_eq!(std::fs::read_to_string(&new).unwrap(), "test = true");
    }

    #[test]
    fn test_move_config_noop_when_same_path() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "test = true").unwrap();

        move_config(&path, &path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_move_config_noop_when_old_missing() {
        let dir = TempDir::new().unwrap();
        let old = dir.path().join("missing.toml");
        let new = dir.path().join("new.toml");

        move_config(&old, &new).unwrap();
        assert!(!new.exists());
    }

    #[test]
    fn test_no_extension_sync_when_format_unchanged() {
        let mut config = AppConfig {
            configuration_format: Some("toml".into()),
            configuration_location: Some("/home/user/.config/kanban/config.toml".into()),
            ..Default::default()
        };

        let dto = AppConfigDto {
            default_card_prefix: Some("feat".into()),
            default_sprint_prefix: None,
            storage_backend: None,
            editing_format: None,
            configuration_format: Some("toml".into()),
            configuration_location: Some("/home/user/.config/kanban/config.toml".into()),
            storage_location: None,
        };
        dto.apply_to(&mut config);

        assert_eq!(
            config.configuration_location.as_deref(),
            Some("/home/user/.config/kanban/config.toml")
        );
    }

    #[test]
    fn test_validate_storage_location_any_extension_accepted() {
        let dir = TempDir::new().unwrap();
        for name in &[
            "board.json",
            "board.sqlite",
            "board.txt",
            "board.dat",
            "mydata",
        ] {
            let location = dir.path().join(name);
            let config = AppConfig {
                storage_location: Some(location.to_string_lossy().into_owned()),
                ..Default::default()
            };
            validate(&config).unwrap();
        }
    }

    #[test]
    fn test_validate_storage_location_parent_dir_missing() {
        let config = AppConfig {
            storage_location: Some("/nonexistent_dir_xyz/board.json".into()),
            ..Default::default()
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("parent directory"));
    }

    #[test]
    fn test_validate_storage_location_none_is_valid() {
        let config = AppConfig {
            storage_location: None,
            ..Default::default()
        };
        validate(&config).unwrap();
    }

    #[test]
    fn test_has_non_default_values_all_none_returns_false() {
        let config = AppConfig::default();
        assert!(!has_non_default_values(&config));
    }

    #[test]
    fn test_has_non_default_values_with_explicit_defaults_returns_true_when_both_storage_set() {
        let config = AppConfig {
            default_card_prefix: Some("task".into()),
            default_sprint_prefix: Some("sprint".into()),
            storage_backend: Some("json".into()),
            editing_format: Some("json".into()),
            configuration_format: Some("toml".into()),
            configuration_location: config_path().map(|p| p.display().to_string()),
            storage_location: Some("boards.json".into()),
            ..Default::default()
        };
        assert!(has_non_default_values(&config));
    }

    #[test]
    fn test_has_non_default_values_with_explicit_defaults_no_storage_location_returns_false() {
        let config = AppConfig {
            default_card_prefix: Some("task".into()),
            default_sprint_prefix: Some("sprint".into()),
            storage_backend: Some("json".into()),
            editing_format: Some("json".into()),
            configuration_format: Some("toml".into()),
            configuration_location: config_path().map(|p| p.display().to_string()),
            ..Default::default()
        };
        assert!(!has_non_default_values(&config));
    }

    #[test]
    fn test_has_non_default_values_with_card_prefix_returns_true() {
        let config = AppConfig {
            default_card_prefix: Some("feat".into()),
            ..Default::default()
        };
        assert!(has_non_default_values(&config));
    }

    #[test]
    fn test_has_non_default_values_with_storage_backend_returns_true() {
        let config = AppConfig {
            storage_backend: Some("sqlite".into()),
            ..Default::default()
        };
        assert!(has_non_default_values(&config));
    }

    #[test]
    fn test_has_non_default_values_true_for_board_sort_field_only() {
        let config = AppConfig {
            board_sort_field: Some("name".into()),
            ..Default::default()
        };
        assert!(
            has_non_default_values(&config),
            "a config whose only non-default value is board_sort_field must be non-default"
        );
    }

    #[test]
    fn test_has_non_default_values_true_for_board_sort_order_only() {
        let config = AppConfig {
            board_sort_order: Some("Descending".into()),
            ..Default::default()
        };
        assert!(
            has_non_default_values(&config),
            "a config whose only non-default value is board_sort_order must be non-default"
        );
    }

    #[test]
    fn test_apply_to_strips_default_values() {
        let dto = AppConfigDto {
            default_card_prefix: Some("task".into()),
            default_sprint_prefix: Some("sprint".into()),
            storage_backend: Some("sqlite".into()),
            editing_format: Some("json".into()),
            configuration_format: Some("toml".into()),
            configuration_location: config_path().map(|p| p.display().to_string()),
            storage_location: None,
        };
        let mut config = AppConfig::default();
        dto.apply_to(&mut config);

        assert!(config.default_card_prefix.is_none());
        assert!(config.default_sprint_prefix.is_none());
        assert_eq!(config.storage_backend.as_deref(), Some("sqlite"));
        assert!(config.editing_format.is_none());
        assert!(config.configuration_format.is_none());
        assert!(config.configuration_location.is_none());
    }

    #[test]
    fn test_save_only_contains_non_default_fields() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");

        let config = AppConfig {
            storage_backend: Some("sqlite".into()),
            ..Default::default()
        };
        save_to(&config, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("storage_backend"));
        assert!(!content.contains("default_card_prefix"));
        assert!(!content.contains("editing_format"));
        assert!(!content.contains("configuration_format"));
    }

    #[test]
    fn test_apply_to_rejects_invalid_storage_backend() {
        let dto = AppConfigDto {
            default_card_prefix: Some("task".into()),
            default_sprint_prefix: Some("sprint".into()),
            storage_backend: Some("yaml".into()),
            editing_format: Some("json".into()),
            configuration_format: Some("toml".into()),
            configuration_location: Some("/tmp/test.toml".into()),
            storage_location: None,
        };
        let mut config = AppConfig::default();
        let err = dto.validate_and_apply(&mut config).unwrap_err();
        assert!(err.to_string().contains("storage_backend"));
    }

    #[test]
    fn test_app_config_dto_serializes_keys_in_alphabetical_order() {
        let dto = AppConfigDto {
            configuration_format: Some("toml".into()),
            configuration_location: Some("/tmp/test.toml".into()),
            default_card_prefix: Some("feat".into()),
            default_sprint_prefix: Some("sprint".into()),
            editing_format: Some("json".into()),
            storage_backend: Some("json".into()),
            storage_location: Some("boards.json".into()),
        };
        let serialized = serde_json::to_string_pretty(&dto).unwrap();
        let keys: Vec<&str> = serialized
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with('"') {
                    trimmed.split('"').nth(1)
                } else {
                    None
                }
            })
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(
            keys, sorted,
            "DTO JSON keys should be in alphabetical order"
        );
    }

    #[test]
    fn test_app_config_serializes_keys_in_alphabetical_order() {
        let config = AppConfig {
            storage_backend: Some("sqlite".into()),
            default_card_prefix: Some("feat".into()),
            ..Default::default()
        };
        let serialized = toml::to_string(&config).unwrap();
        let pos_card = serialized.find("default_card_prefix").unwrap();
        let pos_backend = serialized.find("storage_backend").unwrap();
        assert!(
            pos_card < pos_backend,
            "default_card_prefix should appear before storage_backend"
        );
    }

    #[test]
    fn test_load_from_malformed_toml_returns_defaults() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "this is not valid toml {{{").unwrap();

        let config = load_from(&path);
        assert!(config.default_card_prefix.is_none());
        assert!(config.storage_backend.is_none());
    }

    #[test]
    fn test_load_from_malformed_json_returns_defaults() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, "{not valid json").unwrap();

        let config = load_from(&path);
        assert!(config.default_card_prefix.is_none());
        assert!(config.storage_backend.is_none());
    }

    #[test]
    fn test_load_from_empty_file_returns_defaults() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "").unwrap();

        let config = load_from(&path);
        assert!(config.default_card_prefix.is_none());
        assert!(config.storage_backend.is_none());
    }

    #[test]
    fn test_strip_defaults_strips_default_storage_location() {
        let mut config = AppConfig {
            storage_location: Some("boards.json".into()),
            ..Default::default()
        };
        strip_defaults(&mut config);
        assert!(config.storage_location.is_none());
    }

    #[test]
    fn test_strip_defaults_keeps_custom_storage_location() {
        let mut config = AppConfig {
            storage_location: Some("my_data.json".into()),
            ..Default::default()
        };
        strip_defaults(&mut config);
        assert_eq!(config.storage_location.as_deref(), Some("my_data.json"));
    }

    #[test]
    fn test_strip_defaults_preserves_location_when_backend_explicit() {
        let mut config = AppConfig {
            storage_backend: Some("json".into()),
            storage_location: Some("boards.json".into()),
            ..Default::default()
        };
        strip_defaults(&mut config);
        assert_eq!(config.storage_location.as_deref(), Some("boards.json"));
    }

    #[test]
    fn test_strip_defaults_strips_location_when_backend_not_set() {
        let mut config = AppConfig {
            storage_location: Some("boards.json".into()),
            ..Default::default()
        };
        strip_defaults(&mut config);
        assert!(config.storage_location.is_none());
    }

    #[test]
    fn test_save_returns_error_when_config_location_empty() {
        let config = AppConfig {
            configuration_location: Some(String::new()),
            ..Default::default()
        };
        if effective_configuration_location(&config).is_empty() {
            let err = save(&config).unwrap_err();
            assert!(err.to_string().contains("No configuration location"));
        }
    }

    #[test]
    fn test_write_config_file_no_corruption_on_overwrite() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "default_card_prefix = \"fix\"\ndefault_sprint_prefix = \"mysprint\"\n",
        )
        .unwrap();
        let config = AppConfig {
            default_card_prefix: Some("feat".into()),
            default_sprint_prefix: Some("mysprint".into()),
            ..Default::default()
        };
        save_to(&config, &path).unwrap();
        let result = std::fs::read_to_string(&path).unwrap();
        assert!(
            !result.contains("ix\""),
            "should not contain fragment from old 'fix' value"
        );
        assert!(result.contains("feat"), "should contain new value");
    }

    #[test]
    fn test_appconfig_roundtrips_board_sort_fields() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");

        let config = AppConfig {
            board_sort_field: Some("Name".into()),
            board_sort_order: Some("Descending".into()),
            ..Default::default()
        };
        save_to(&config, &path).unwrap();

        let loaded = load_from(&path);
        assert_eq!(loaded.board_sort_field.as_deref(), Some("Name"));
        assert_eq!(loaded.board_sort_order.as_deref(), Some("Descending"));
    }

    #[test]
    fn test_appconfig_omits_board_sort_when_none() {
        let config = AppConfig::default();
        let serialized = toml::to_string(&config).unwrap();
        assert!(
            !serialized.contains("board_sort_field"),
            "default config should not serialize board_sort_field, got: {serialized:?}"
        );
        assert!(
            !serialized.contains("board_sort_order"),
            "default config should not serialize board_sort_order, got: {serialized:?}"
        );
    }

    #[test]
    fn test_appconfig_legacy_without_board_sort_loads_as_none() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "default_card_prefix = \"feat\"\n").unwrap();

        let config = load_from(&path);
        assert_eq!(config.default_card_prefix.as_deref(), Some("feat"));
        assert!(config.board_sort_field.is_none());
        assert!(config.board_sort_order.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn test_write_config_file_no_temp_file_remains() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let config = AppConfig::default();
        save_to(&config, &path).unwrap();
        let all_files: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy() != "config.toml")
            .collect();
        assert!(
            all_files.is_empty(),
            "no leftover files should remain after successful write, found: {:?}",
            all_files.iter().map(|e| e.file_name()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_resolve_server_addr_falls_back_to_default() {
        let config = AppConfig::default();
        assert_eq!(resolve_server_addr(&config), "127.0.0.1:0");
    }

    #[test]
    fn test_resolve_server_addr_returns_configured() {
        let config = AppConfig {
            server_addr: Some("0.0.0.0:5175".into()),
            ..Default::default()
        };
        assert_eq!(resolve_server_addr(&config), "0.0.0.0:5175");
    }

    #[test]
    fn test_is_all_defaults_false_when_only_server_addr_set() {
        let config = AppConfig {
            server_addr: Some("0.0.0.0:5175".into()),
            ..Default::default()
        };
        assert!(
            !is_all_defaults(&config),
            "a config with only server_addr set should not be all defaults"
        );
    }

    #[test]
    fn test_is_all_defaults_true_when_server_addr_is_default() {
        let config = AppConfig {
            server_addr: Some("127.0.0.1:0".into()),
            ..Default::default()
        };
        assert!(
            is_all_defaults(&config),
            "a config with server_addr set to the default value should be all defaults"
        );
    }

    #[test]
    fn test_strip_defaults_strips_default_server_addr() {
        let mut config = AppConfig {
            server_addr: Some("127.0.0.1:0".into()),
            ..Default::default()
        };
        strip_defaults(&mut config);
        assert!(
            config.server_addr.is_none(),
            "default server_addr should be stripped"
        );
    }

    #[test]
    fn test_strip_defaults_preserves_non_default_server_addr() {
        let mut config = AppConfig {
            server_addr: Some("0.0.0.0:5175".into()),
            ..Default::default()
        };
        strip_defaults(&mut config);
        assert_eq!(
            config.server_addr.as_deref(),
            Some("0.0.0.0:5175"),
            "non-default server_addr should be preserved"
        );
    }
}
